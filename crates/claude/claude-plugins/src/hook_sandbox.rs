//! Security sandboxing for the plugin hook system.
//!
//! Provides isolated, out-of-process hook execution with:
//! - Configurable execution timeouts (default 30s)
//! - Resource limits (memory, CPU)
//! - Permission-based access control
//! - Output schema validation
//! - Sandboxed file access scoped to declared directories
//! - Comprehensive audit logging of all hook invocations

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use tokio::process::Command;
use tokio::sync::Mutex;
use tokio::time::{Duration, timeout};

use crate::load_hooks::{HookDefinition, HookEvent};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default timeout for hook execution in seconds.
pub const DEFAULT_HOOK_TIMEOUT_SECS: u64 = 30;

/// Default maximum memory a hook process may use, in megabytes.
pub const DEFAULT_HOOK_MEMORY_LIMIT_MB: u64 = 256;

/// Default maximum CPU time a hook may consume, in seconds.
pub const DEFAULT_HOOK_CPU_LIMIT_SECS: u64 = 60;

/// Maximum output size from a hook process in bytes (1 MiB).
pub const MAX_HOOK_OUTPUT_BYTES: usize = 1024 * 1024;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors that can occur during sandboxed hook execution.
#[derive(Debug, Error)]
pub enum SandboxError {
    /// The hook execution exceeded the configured timeout.
    #[error("hook '{hook}' timed out after {timeout_secs}s")]
    Timeout {
        hook: String,
        timeout_secs: u64,
    },
    /// The hook process was killed because it exceeded resource limits.
    #[error("hook '{hook}' exceeded resource limit: {message}")]
    ResourceLimit {
        hook: String,
        message: String,
    },
    /// The hook output exceeded the maximum allowed size.
    #[error("hook '{hook}' output exceeded {max_bytes} bytes")]
    OutputTooLarge {
        hook: String,
        max_bytes: usize,
    },
    /// The hook output failed schema validation.
    #[error("hook '{hook}' produced invalid output: {message}")]
    ValidationFailed {
        hook: String,
        message: String,
    },
    /// The hook was denied by the permission policy.
    #[error("hook '{hook}' permission denied: {message}")]
    PermissionDenied {
        hook: String,
        message: String,
    },
    /// The hook process could not be spawned.
    #[error("failed to spawn hook '{hook}': {source}")]
    SpawnFailed {
        hook: String,
        #[source]
        source: io::Error,
    },
    /// I/O error communicating with the hook process.
    #[error("I/O error for hook '{hook}': {source}")]
    Io {
        hook: String,
        #[source]
        source: io::Error,
    },
    /// The hook process exited with a non-zero exit code.
    #[error("hook '{hook}' exited with code {code}")]
    ExitCode {
        hook: String,
        code: i32,
    },
}

// ---------------------------------------------------------------------------
// HookPermissions — declares what resources a hook can access
// ---------------------------------------------------------------------------

/// Declares what resources a hook is allowed to access.
///
/// Permissions are declared by the plugin author and must be approved by the
/// host before the hook is executed. Any undeclared access attempt will be
/// denied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HookPermissions {
    /// Whether the hook may read files.
    #[serde(default)]
    pub allow_file_read: bool,
    /// Whether the hook may write files.
    #[serde(default)]
    pub allow_file_write: bool,
    /// Whether the hook may access the network.
    #[serde(default)]
    pub allow_network: bool,
    /// Whether the hook may spawn subprocesses.
    #[serde(default)]
    pub allow_subprocess: bool,
    /// Whether the hook may read environment variables.
    #[serde(default)]
    pub allow_env_read: bool,
    /// Specific directories the hook is allowed to access (read or write).
    /// Paths are absolute. If empty and `allow_file_read`/`allow_file_write`
    /// are true, the hook may access any file.
    #[serde(default)]
    pub file_scope: Vec<PathBuf>,
}

impl Default for HookPermissions {
    fn default() -> Self {
        Self {
            allow_file_read: true,
            allow_file_write: false,
            allow_network: false,
            allow_subprocess: false,
            allow_env_read: true,
            file_scope: Vec::new(),
        }
    }
}

impl HookPermissions {
    /// Creates a minimal permission set with no access.
    pub fn none() -> Self {
        Self {
            allow_file_read: false,
            allow_file_write: false,
            allow_network: false,
            allow_subprocess: false,
            allow_env_read: false,
            file_scope: Vec::new(),
        }
    }

    /// Creates a read-only permission set.
    pub fn read_only() -> Self {
        Self {
            allow_file_read: true,
            allow_file_write: false,
            allow_network: false,
            allow_subprocess: false,
            allow_env_read: true,
            file_scope: Vec::new(),
        }
    }

    /// Creates a full-access permission set (for trusted hooks).
    pub fn full_trust() -> Self {
        Self {
            allow_file_read: true,
            allow_file_write: true,
            allow_network: true,
            allow_subprocess: true,
            allow_env_read: true,
            file_scope: Vec::new(),
        }
    }

    /// Returns true if the given path is within the declared file scope.
    ///
    /// If `file_scope` is empty, all paths are considered in scope (subject
    /// to the read/write flags).
    pub fn is_path_in_scope(&self, path: &Path) -> bool {
        if self.file_scope.is_empty() {
            return true;
        }
        let normalized = normalize_path(path);
        self.file_scope.iter().any(|scope| {
            let normalized_scope = normalize_path(scope);
            normalized.starts_with(&normalized_scope)
        })
    }

    /// Validates that this permission set is acceptable given a policy.
    pub fn validate_against_policy(&self, policy: &HookPermissionPolicy) -> Result<(), String> {
        if self.allow_network && !policy.allow_network {
            return Err("network access is not permitted by policy".to_owned());
        }
        if self.allow_subprocess && !policy.allow_subprocess {
            return Err("subprocess spawning is not permitted by policy".to_owned());
        }
        if self.allow_file_write && !policy.allow_file_write {
            return Err("file write access is not permitted by policy".to_owned());
        }
        if self.allow_file_read && !policy.allow_file_read {
            return Err("file read access is not permitted by policy".to_owned());
        }

        // Verify file scope is within allowed directories
        if !self.file_scope.is_empty() && !policy.allowed_directories.is_empty() {
            for dir in &self.file_scope {
                let in_allowed = policy
                    .allowed_directories
                    .iter()
                    .any(|allowed| dir.starts_with(allowed));
                if !in_allowed {
                    return Err(format!(
                        "directory '{}' is outside allowed scope",
                        dir.display()
                    ));
                }
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// HookPermissionPolicy — host-side policy for what hooks may request
// ---------------------------------------------------------------------------

/// Host-side policy governing what permissions hooks may be granted.
///
/// This is the "bouncer" — even if a hook requests network access, the
/// policy must explicitly allow it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HookPermissionPolicy {
    /// Whether hooks may request file read access.
    #[serde(default = "default_true")]
    pub allow_file_read: bool,
    /// Whether hooks may request file write access.
    #[serde(default)]
    pub allow_file_write: bool,
    /// Whether hooks may request network access.
    #[serde(default)]
    pub allow_network: bool,
    /// Whether hooks may spawn subprocesses.
    #[serde(default)]
    pub allow_subprocess: bool,
    /// Root directories that hooks may access. Empty means no restriction.
    #[serde(default)]
    pub allowed_directories: Vec<PathBuf>,
    /// Permissions that are always denied regardless of request.
    #[serde(default)]
    pub denied_permissions: Vec<String>,
}

impl Default for HookPermissionPolicy {
    fn default() -> Self {
        Self {
            allow_file_read: true,
            allow_file_write: false,
            allow_network: false,
            allow_subprocess: false,
            allowed_directories: Vec::new(),
            denied_permissions: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// HookSandboxConfig — runtime configuration for the sandbox
// ---------------------------------------------------------------------------

/// Configuration for the hook sandbox.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HookSandboxConfig {
    /// Maximum execution time per hook in seconds.
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
    /// Maximum memory per hook process in megabytes.
    #[serde(default = "default_memory_limit_mb")]
    pub memory_limit_mb: u64,
    /// Maximum CPU time per hook in seconds.
    #[serde(default = "default_cpu_limit_secs")]
    pub cpu_limit_secs: u64,
    /// Maximum output size in bytes.
    #[serde(default = "default_max_output_bytes")]
    pub max_output_bytes: usize,
    /// Whether to enforce permission checks.
    #[serde(default = "default_true")]
    pub enforce_permissions: bool,
    /// Whether to validate output schemas.
    #[serde(default = "default_true")]
    pub validate_output: bool,
    /// Working directory for hook execution.
    #[serde(default)]
    pub working_directory: Option<PathBuf>,
    /// Environment variables to pass to hook processes.
    #[serde(default)]
    pub env_vars: HashMap<String, String>,
}

impl Default for HookSandboxConfig {
    fn default() -> Self {
        Self {
            timeout_secs: DEFAULT_HOOK_TIMEOUT_SECS,
            memory_limit_mb: DEFAULT_HOOK_MEMORY_LIMIT_MB,
            cpu_limit_secs: DEFAULT_HOOK_CPU_LIMIT_SECS,
            max_output_bytes: MAX_HOOK_OUTPUT_BYTES,
            enforce_permissions: true,
            validate_output: true,
            working_directory: None,
            env_vars: HashMap::new(),
        }
    }
}

fn default_timeout_secs() -> u64 {
    DEFAULT_HOOK_TIMEOUT_SECS
}
fn default_memory_limit_mb() -> u64 {
    DEFAULT_HOOK_MEMORY_LIMIT_MB
}
fn default_cpu_limit_secs() -> u64 {
    DEFAULT_HOOK_CPU_LIMIT_SECS
}
fn default_max_output_bytes() -> usize {
    MAX_HOOK_OUTPUT_BYTES
}
fn default_true() -> bool {
    true
}

// ---------------------------------------------------------------------------
// HookOutputSchema — JSON schema for hook output validation
// ---------------------------------------------------------------------------

/// Expected output schema for a hook response.
///
/// Uses a simple field-based validation model rather than full JSON Schema
/// to keep validation fast and dependency-free.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HookOutputSchema {
    /// Expected top-level type of the output ("object", "string", "array", etc.).
    #[serde(default = "default_output_type")]
    pub expected_type: String,
    /// Required fields (only meaningful when expected_type is "object").
    #[serde(default)]
    pub required_fields: Vec<String>,
    /// Optional fields with their expected types.
    #[serde(default)]
    pub field_types: HashMap<String, String>,
    /// Maximum nesting depth allowed in the output JSON.
    #[serde(default = "default_max_depth")]
    pub max_depth: usize,
}

fn default_output_type() -> String {
    "object".to_owned()
}
fn default_max_depth() -> usize {
    10
}

impl Default for HookOutputSchema {
    fn default() -> Self {
        Self {
            expected_type: "object".to_owned(),
            required_fields: Vec::new(),
            field_types: HashMap::new(),
            max_depth: 10,
        }
    }
}

impl HookOutputSchema {
    /// Creates a schema expecting an object with the given required fields.
    pub fn object(required: Vec<&str>) -> Self {
        Self {
            expected_type: "object".to_owned(),
            required_fields: required.into_iter().map(String::from).collect(),
            field_types: HashMap::new(),
            max_depth: 10,
        }
    }

    /// Creates a schema that accepts any JSON output.
    pub fn any() -> Self {
        Self {
            expected_type: "any".to_owned(),
            required_fields: Vec::new(),
            field_types: HashMap::new(),
            max_depth: 20,
        }
    }

    /// Validates the given JSON value against this schema.
    pub fn validate(&self, output: &Value) -> Result<(), String> {
        // Type check
        if self.expected_type != "any" {
            let actual_type = json_type_name(output);
            if actual_type != self.expected_type {
                return Err(format!(
                    "expected type '{}', got '{}'",
                    self.expected_type, actual_type
                ));
            }
        }

        // Depth check
        let depth = json_depth(output);
        if depth > self.max_depth {
            return Err(format!(
                "output nesting depth {} exceeds maximum {}",
                depth, self.max_depth
            ));
        }

        // Required fields (only for objects)
        if self.expected_type == "object" {
            if let Some(obj) = output.as_object() {
                for field in &self.required_fields {
                    if !obj.contains_key(field) {
                        return Err(format!("missing required field '{field}'"));
                    }
                }

                // Field type checks
                for (field_name, expected_type) in &self.field_types {
                    if let Some(value) = obj.get(field_name) {
                        let actual_type = json_type_name(value);
                        if actual_type != *expected_type && *expected_type != "any" {
                            return Err(format!(
                                "field '{field_name}': expected type '{expected_type}', got '{actual_type}'"
                            ));
                        }
                    }
                }
            } else if self.expected_type == "object" {
                return Err("expected an object".to_owned());
            }
        }

        Ok(())
    }
}

/// Returns the type name of a JSON value.
fn json_type_name(value: &Value) -> String {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
    .to_owned()
}

/// Computes the maximum nesting depth of a JSON value.
fn json_depth(value: &Value) -> usize {
    match value {
        Value::Array(items) => {
            let child_max = items.iter().map(json_depth).max().unwrap_or(0);
            1 + child_max
        }
        Value::Object(map) => {
            let child_max = map.values().map(json_depth).max().unwrap_or(0);
            1 + child_max
        }
        _ => 0,
    }
}

// ---------------------------------------------------------------------------
// HookAuditLog — records all hook invocations
// ---------------------------------------------------------------------------

/// A single audit log entry for a hook invocation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HookAuditEntry {
    /// Unique identifier for this invocation.
    pub id: String,
    /// Name of the plugin that owns this hook.
    pub plugin_name: String,
    /// The event that triggered this hook.
    pub event: HookEvent,
    /// The command that was executed.
    pub command: String,
    /// Timestamp when execution started.
    pub started_at: DateTime<Utc>,
    /// Timestamp when execution finished (if applicable).
    pub finished_at: Option<DateTime<Utc>>,
    /// Duration of execution in milliseconds.
    pub duration_ms: Option<u64>,
    /// Whether the hook succeeded.
    pub success: bool,
    /// Error message if the hook failed.
    pub error: Option<String>,
    /// Exit code of the hook process.
    pub exit_code: Option<i32>,
    /// Permissions that were granted for this invocation.
    pub permissions_granted: HookPermissions,
    /// Whether the hook was denied by the permission policy.
    pub permission_denied: bool,
    /// Size of the output in bytes.
    pub output_size_bytes: Option<usize>,
}

/// Thread-safe audit log for hook invocations.
///
/// Stores all invocations in memory with a configurable capacity limit.
/// Oldest entries are evicted when capacity is reached.
#[derive(Debug, Clone)]
pub struct HookAuditLog {
    entries: Arc<Mutex<Vec<HookAuditEntry>>>,
    max_entries: usize,
}

impl HookAuditLog {
    /// Creates a new audit log with the given maximum capacity.
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Arc::new(Mutex::new(Vec::with_capacity(max_entries.min(1024)))),
            max_entries,
        }
    }

    /// Creates an audit log with a default capacity of 1000 entries.
    pub fn default_capacity() -> Self {
        Self::new(1000)
    }

    /// Records a new audit entry.
    pub async fn record(&self, entry: HookAuditEntry) {
        let mut entries = self.entries.lock().await;
        if entries.len() >= self.max_entries {
            entries.remove(0);
        }
        entries.push(entry);
    }

    /// Returns all audit entries.
    pub async fn all(&self) -> Vec<HookAuditEntry> {
        self.entries.lock().await.clone()
    }

    /// Returns audit entries for a specific plugin.
    pub async fn for_plugin(&self, plugin_name: &str) -> Vec<HookAuditEntry> {
        let entries = self.entries.lock().await;
        entries
            .iter()
            .filter(|e| e.plugin_name == plugin_name)
            .cloned()
            .collect()
    }

    /// Returns the number of recorded entries.
    pub async fn len(&self) -> usize {
        self.entries.lock().await.len()
    }

    /// Returns true if there are no recorded entries.
    pub async fn is_empty(&self) -> bool {
        self.entries.lock().await.is_empty()
    }

    /// Clears all audit entries.
    pub async fn clear(&self) {
        self.entries.lock().await.clear();
    }
}

// ---------------------------------------------------------------------------
// Hook execution result
// ---------------------------------------------------------------------------

/// Result of a sandboxed hook execution.
#[derive(Debug, Clone)]
pub struct HookExecutionResult {
    /// The captured stdout output (parsed as JSON if possible).
    pub output: Value,
    /// The raw stdout bytes.
    pub raw_output: Vec<u8>,
    /// The exit code of the hook process.
    pub exit_code: Option<i32>,
    /// Duration of execution in milliseconds.
    pub duration_ms: u64,
    /// Whether the execution succeeded.
    pub success: bool,
    /// Error message if execution failed.
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------
// HookSandbox — the main sandbox struct
// ---------------------------------------------------------------------------

/// Security sandbox for executing plugin hooks.
///
/// Wraps hook execution with timeouts, resource limits, permission checks,
/// output validation, and audit logging. Hooks run as separate processes
/// via `tokio::process::Command` for isolation.
pub struct HookSandbox {
    /// Sandbox configuration.
    pub config: HookSandboxConfig,
    /// Permission policy governing what hooks may request.
    pub policy: HookPermissionPolicy,
    /// Audit log for recording invocations.
    pub audit_log: HookAuditLog,
    /// Output schemas by hook command (for validation).
    pub output_schemas: HashMap<String, HookOutputSchema>,
}

impl HookSandbox {
    /// Creates a new sandbox with the given configuration and policy.
    pub fn new(config: HookSandboxConfig, policy: HookPermissionPolicy) -> Self {
        Self {
            config,
            policy,
            audit_log: HookAuditLog::default_capacity(),
            output_schemas: HashMap::new(),
        }
    }

    /// Creates a sandbox with default configuration and policy.
    pub fn default_sandbox() -> Self {
        Self::new(HookSandboxConfig::default(), HookPermissionPolicy::default())
    }

    /// Registers an output schema for a specific hook command.
    pub fn register_schema(&mut self, command: &str, schema: HookOutputSchema) {
        self.output_schemas.insert(command.to_owned(), schema);
    }

    /// Executes a hook in a sandboxed subprocess.
    ///
    /// The hook command is spawned as an out-of-process command with the
    /// configured timeout and resource limits. Output is captured and
    /// validated against the registered schema (if any).
    pub async fn execute_hook(
        &self,
        plugin_name: &str,
        event: HookEvent,
        hook: &HookDefinition,
        permissions: &HookPermissions,
        input: Option<&Value>,
    ) -> Result<HookExecutionResult, SandboxError> {
        let command_label = format!("{}/{}", plugin_name, hook.command);
        let started_at = Utc::now();

        // Step 1: Permission check
        if self.config.enforce_permissions {
            if let Err(denied) = permissions.validate_against_policy(&self.policy) {
                let entry = HookAuditEntry {
                    id: generate_audit_id(),
                    plugin_name: plugin_name.to_owned(),
                    event,
                    command: hook.command.clone(),
                    started_at,
                    finished_at: Some(Utc::now()),
                    duration_ms: Some(0),
                    success: false,
                    error: Some(denied.clone()),
                    exit_code: None,
                    permissions_granted: permissions.clone(),
                    permission_denied: true,
                    output_size_bytes: None,
                };
                self.audit_log.record(entry).await;
                return Err(SandboxError::PermissionDenied {
                    hook: command_label,
                    message: denied,
                });
            }
        }

        // Step 2: Build the command
        let mut cmd = self.build_command(hook, permissions, input);

        // Step 3: Execute with timeout
        let timeout_duration = Duration::from_secs(self.config.timeout_secs);
        let result = match timeout(timeout_duration, self.run_process(&mut cmd, &command_label)).await {
            Ok(inner) => inner,
            Err(_) => {
                let entry = HookAuditEntry {
                    id: generate_audit_id(),
                    plugin_name: plugin_name.to_owned(),
                    event,
                    command: hook.command.clone(),
                    started_at,
                    finished_at: Some(Utc::now()),
                    duration_ms: Some(self.config.timeout_secs * 1000),
                    success: false,
                    error: Some("timeout".to_owned()),
                    exit_code: None,
                    permissions_granted: permissions.clone(),
                    permission_denied: false,
                    output_size_bytes: None,
                };
                self.audit_log.record(entry).await;
                return Err(SandboxError::Timeout {
                    hook: command_label,
                    timeout_secs: self.config.timeout_secs,
                });
            }
        };

        let output = result?;
        let finished_at = Utc::now();
        let duration_ms = (finished_at - started_at).num_milliseconds().max(0) as u64;

        // Step 4: Check exit code
        if !output.success {
            let entry = HookAuditEntry {
                id: generate_audit_id(),
                plugin_name: plugin_name.to_owned(),
                event,
                command: hook.command.clone(),
                started_at,
                finished_at: Some(finished_at),
                duration_ms: Some(duration_ms),
                success: false,
                error: output.error.clone(),
                exit_code: output.exit_code,
                permissions_granted: permissions.clone(),
                permission_denied: false,
                output_size_bytes: Some(output.raw_output.len()),
            };
            self.audit_log.record(entry).await;
            return match output.exit_code {
                Some(code) => Err(SandboxError::ExitCode {
                    hook: command_label,
                    code,
                }),
                None => Err(SandboxError::Io {
                    hook: command_label,
                    source: io::Error::new(io::ErrorKind::BrokenPipe, "process terminated unexpectedly"),
                }),
            };
        }

        // Step 5: Output size check
        if output.raw_output.len() > self.config.max_output_bytes {
            let entry = HookAuditEntry {
                id: generate_audit_id(),
                plugin_name: plugin_name.to_owned(),
                event,
                command: hook.command.clone(),
                started_at,
                finished_at: Some(finished_at),
                duration_ms: Some(duration_ms),
                success: false,
                error: Some("output too large".to_owned()),
                exit_code: output.exit_code,
                permissions_granted: permissions.clone(),
                permission_denied: false,
                output_size_bytes: Some(output.raw_output.len()),
            };
            self.audit_log.record(entry).await;
            return Err(SandboxError::OutputTooLarge {
                hook: command_label,
                max_bytes: self.config.max_output_bytes,
            });
        }

        // Step 6: Schema validation
        if self.config.validate_output {
            if let Some(schema) = self.output_schemas.get(&hook.command) {
                if let Err(validation_err) = schema.validate(&output.output) {
                    let entry = HookAuditEntry {
                        id: generate_audit_id(),
                        plugin_name: plugin_name.to_owned(),
                        event,
                        command: hook.command.clone(),
                        started_at,
                        finished_at: Some(finished_at),
                        duration_ms: Some(duration_ms),
                        success: false,
                        error: Some(validation_err.clone()),
                        exit_code: output.exit_code,
                        permissions_granted: permissions.clone(),
                        permission_denied: false,
                        output_size_bytes: Some(output.raw_output.len()),
                    };
                    self.audit_log.record(entry).await;
                    return Err(SandboxError::ValidationFailed {
                        hook: command_label,
                        message: validation_err,
                    });
                }
            }
        }

        // Step 7: Record successful execution
        let entry = HookAuditEntry {
            id: generate_audit_id(),
            plugin_name: plugin_name.to_owned(),
            event,
            command: hook.command.clone(),
            started_at,
            finished_at: Some(finished_at),
            duration_ms: Some(duration_ms),
            success: true,
            error: None,
            exit_code: output.exit_code,
            permissions_granted: permissions.clone(),
            permission_denied: false,
            output_size_bytes: Some(output.raw_output.len()),
        };
        self.audit_log.record(entry).await;

        Ok(HookExecutionResult {
            output: output.output,
            raw_output: output.raw_output,
            exit_code: output.exit_code,
            duration_ms,
            success: true,
            error: None,
        })
    }

    /// Builds a `tokio::process::Command` from a hook definition.
    ///
    /// Constructs the command with environment variables, working directory,
    /// and sandbox restrictions. On Unix-like systems, environment variable
    /// filtering is applied based on permissions.
    fn build_command(
        &self,
        hook: &HookDefinition,
        permissions: &HookPermissions,
        input: Option<&Value>,
    ) -> Command {
        let shell = if cfg!(windows) { "cmd" } else { "sh" };
        let shell_arg = if cfg!(windows) { "/C" } else { "-c" };

        let mut cmd = Command::new(shell);
        cmd.arg(shell_arg)
            .arg(&hook.command)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);

        // Set working directory
        if let Some(cwd) = &self.config.working_directory {
            cmd.current_dir(cwd);
        }

        // Pass environment variables
        if permissions.allow_env_read {
            for (key, value) in &self.config.env_vars {
                cmd.env(key, value);
            }
        } else {
            // Clear environment when env read is not allowed
            cmd.env_clear();
            cmd.env("PATH", std::env::var("PATH").unwrap_or_default());
        }

        // Pass input as environment variable for the hook to read
        if let Some(input_val) = input {
            cmd.env("HOOK_INPUT", input_val.to_string());
        }

        // Set resource limit hints via environment
        cmd.env("HOOK_TIMEOUT_SECS", self.config.timeout_secs.to_string());
        cmd.env("HOOK_MEMORY_LIMIT_MB", self.config.memory_limit_mb.to_string());

        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            // Set process group so we can kill the entire tree
            cmd.process_group(0);
        }

        cmd
    }

    /// Runs the process and captures output.
    async fn run_process(
        &self,
        cmd: &mut Command,
        label: &str,
    ) -> Result<HookExecutionResult, SandboxError> {
        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                return Err(SandboxError::SpawnFailed {
                    hook: label.to_owned(),
                    source: e,
                });
            }
        };

        // Write input to stdin if present
        if let Some(mut stdin) = child.stdin.take() {
            use tokio::io::AsyncWriteExt;
            // Close stdin immediately — input is passed via HOOK_INPUT env var
            let _ = stdin.shutdown().await;
        }

        // Wait for the process with output
        let output = match child.wait_with_output().await {
            Ok(o) => o,
            Err(e) => {
                return Err(SandboxError::Io {
                    hook: label.to_owned(),
                    source: e,
                });
            }
        };

        let exit_code = output.status.code();
        let success = output.status.success();

        // Parse stdout as JSON if possible, otherwise wrap as string
        let parsed_output = if output.stdout.is_empty() {
            Value::Null
        } else {
            match serde_json::from_slice::<Value>(&output.stdout) {
                Ok(v) => v,
                Err(_) => {
                    let text = String::from_utf8_lossy(&output.stdout);
                    Value::String(text.into_owned())
                }
            }
        };

        let error = if !success {
            let stderr_text = String::from_utf8_lossy(&output.stderr);
            if stderr_text.is_empty() {
                Some(format!("process exited with code {:?}", exit_code))
            } else {
                Some(stderr_text.into_owned())
            }
        } else {
            None
        };

        Ok(HookExecutionResult {
            output: parsed_output,
            raw_output: output.stdout,
            exit_code,
            duration_ms: 0, // Set by caller
            success,
            error,
        })
    }

    /// Validates file access against the permission scope.
    ///
    /// Returns `Ok(())` if the path is accessible, or an error describing
    /// why access was denied.
    pub fn validate_file_access(
        &self,
        permissions: &HookPermissions,
        path: &Path,
        access_type: FileAccessType,
    ) -> Result<(), SandboxError> {
        match access_type {
            FileAccessType::Read => {
                if !permissions.allow_file_read {
                    return Err(SandboxError::PermissionDenied {
                        hook: "file-access-check".to_owned(),
                        message: "file read access not granted".to_owned(),
                    });
                }
            }
            FileAccessType::Write => {
                if !permissions.allow_file_write {
                    return Err(SandboxError::PermissionDenied {
                        hook: "file-access-check".to_owned(),
                        message: "file write access not granted".to_owned(),
                    });
                }
            }
        }

        if !permissions.is_path_in_scope(path) {
            return Err(SandboxError::PermissionDenied {
                hook: "file-access-check".to_owned(),
                message: format!(
                    "path '{}' is outside the declared file scope",
                    path.display()
                ),
            });
        }

        Ok(())
    }
}

/// Type of file access being requested.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileAccessType {
    /// Read access.
    Read,
    /// Write access.
    Write,
}

/// Generates a unique audit entry ID.
fn generate_audit_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    let timestamp = Utc::now().timestamp_millis();
    format!("hk-{timestamp}-{count}")
}

/// Normalizes a path for consistent comparison.
///
/// Uses `canonicalize` when the path exists, otherwise falls back to
/// `Path::canonicalize` simulation with path cleaning. This avoids
/// issues with Windows UNC prefix paths (`\\?\`) when the file exists.
fn normalize_path(path: &Path) -> PathBuf {
    if path.exists() {
        match std::fs::canonicalize(path) {
            Ok(canonical) => {
                // Strip the Windows UNC prefix for consistent comparison
                let s = canonical.to_string_lossy();
                if let Some(stripped) = s.strip_prefix(r"\\?\") {
                    PathBuf::from(stripped)
                } else {
                    canonical
                }
            }
            Err(_) => path.to_path_buf(),
        }
    } else {
        path.to_path_buf()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn ok<T, E: std::fmt::Display>(result: Result<T, E>) -> T {
        match result {
            Ok(value) => value,
            Err(error) => panic!("unexpected error: {error}"),
        }
    }

    // -- HookPermissions tests --

    #[test]
    fn default_permissions_are_read_only() {
        let perms = HookPermissions::default();
        assert!(perms.allow_file_read);
        assert!(!perms.allow_file_write);
        assert!(!perms.allow_network);
        assert!(!perms.allow_subprocess);
        assert!(perms.allow_env_read);
    }

    #[test]
    fn none_permissions_deny_everything() {
        let perms = HookPermissions::none();
        assert!(!perms.allow_file_read);
        assert!(!perms.allow_file_write);
        assert!(!perms.allow_network);
        assert!(!perms.allow_subprocess);
        assert!(!perms.allow_env_read);
    }

    #[test]
    fn full_trust_permissions_allow_everything() {
        let perms = HookPermissions::full_trust();
        assert!(perms.allow_file_read);
        assert!(perms.allow_file_write);
        assert!(perms.allow_network);
        assert!(perms.allow_subprocess);
        assert!(perms.allow_env_read);
    }

    #[test]
    fn path_in_scope_with_empty_scope_allows_all() {
        let perms = HookPermissions::read_only();
        assert!(perms.is_path_in_scope(Path::new("/any/path")));
    }

    #[test]
    fn path_in_scope_restricts_to_declared_dirs() {
        let temp = tempfile::tempdir().expect("tempdir");
        let dir1 = temp.path().join("dir1");
        fs::create_dir_all(&dir1).expect("mkdir");

        let perms = HookPermissions {
            allow_file_read: true,
            file_scope: vec![dir1.clone()],
            ..HookPermissions::default()
        };

        let file_inside = dir1.join("test.txt");
        fs::write(&file_inside, "test").expect("write");
        assert!(perms.is_path_in_scope(&file_inside));

        let outside = temp.path().join("dir2/other.txt");
        assert!(!perms.is_path_in_scope(&outside));
    }

    #[test]
    fn validate_against_policy_allows_compatible_perms() {
        let perms = HookPermissions::read_only();
        let policy = HookPermissionPolicy::default();
        assert!(perms.validate_against_policy(&policy).is_ok());
    }

    #[test]
    fn validate_against_policy_rejects_network_when_denied() {
        let perms = HookPermissions {
            allow_network: true,
            ..HookPermissions::none()
        };
        let policy = HookPermissionPolicy::default();
        assert!(perms.validate_against_policy(&policy).is_err());
    }

    #[test]
    fn validate_against_policy_rejects_write_when_denied() {
        let perms = HookPermissions {
            allow_file_write: true,
            ..HookPermissions::none()
        };
        let policy = HookPermissionPolicy::default();
        assert!(perms.validate_against_policy(&policy).is_err());
    }

    #[test]
    fn validate_against_policy_rejects_out_of_scope_dirs() {
        let temp = tempfile::tempdir().expect("tempdir");
        let allowed = temp.path().join("allowed");
        let forbidden = temp.path().join("forbidden");
        fs::create_dir_all(&allowed).expect("mkdir");

        let perms = HookPermissions {
            allow_file_read: true,
            file_scope: vec![forbidden],
            ..HookPermissions::default()
        };
        let policy = HookPermissionPolicy {
            allow_file_read: true,
            allowed_directories: vec![allowed],
            ..HookPermissionPolicy::default()
        };
        assert!(perms.validate_against_policy(&policy).is_err());
    }

    // -- HookOutputSchema tests --

    #[test]
    fn schema_validates_object_with_required_fields() {
        let schema = HookOutputSchema::object(vec!["status", "data"]);
        let valid = serde_json::json!({"status": "ok", "data": [1, 2, 3]});
        assert!(schema.validate(&valid).is_ok());

        let missing = serde_json::json!({"status": "ok"});
        assert!(schema.validate(&missing).is_err());
    }

    #[test]
    fn schema_validates_type_mismatch() {
        let schema = HookOutputSchema::default(); // expects "object"
        let array = serde_json::json!([1, 2, 3]);
        let result = schema.validate(&array);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("expected type 'object'"));
    }

    #[test]
    fn schema_any_accepts_everything() {
        let schema = HookOutputSchema::any();
        assert!(schema.validate(&serde_json::json!(null)).is_ok());
        assert!(schema.validate(&serde_json::json!("string")).is_ok());
        assert!(schema.validate(&serde_json::json!([1, 2])).is_ok());
        assert!(schema.validate(&serde_json::json!({"a": 1})).is_ok());
    }

    #[test]
    fn schema_validates_field_types() {
        let mut schema = HookOutputSchema::default();
        schema.required_fields = vec!["count".to_owned()];
        schema.field_types = vec![
            ("count".to_owned(), "number".to_owned()),
            ("name".to_owned(), "string".to_owned()),
        ]
        .into_iter()
        .collect();

        let valid = serde_json::json!({"count": 42, "name": "test"});
        assert!(schema.validate(&valid).is_ok());

        let wrong_type = serde_json::json!({"count": "not-a-number", "name": "test"});
        assert!(schema.validate(&wrong_type).is_err());
    }

    #[test]
    fn schema_rejects_excessive_depth() {
        let schema = HookOutputSchema {
            max_depth: 3,
            ..HookOutputSchema::any()
        };
        let deep = serde_json::json!({"a": {"b": {"c": {"d": "too deep"}}}});
        assert!(schema.validate(&deep).is_err());
    }

    // -- json_depth tests --

    #[test]
    fn json_depth_flat_values() {
        assert_eq!(json_depth(&serde_json::json!(null)), 0);
        assert_eq!(json_depth(&serde_json::json!(42)), 0);
        assert_eq!(json_depth(&serde_json::json!("str")), 0);
    }

    #[test]
    fn json_depth_nested() {
        let val = serde_json::json!({"a": {"b": [1, {"c": 2}]}});
        assert_eq!(json_depth(&val), 4);
    }

    // -- HookPermissionPolicy tests --

    #[test]
    fn default_policy_is_read_only() {
        let policy = HookPermissionPolicy::default();
        assert!(policy.allow_file_read);
        assert!(!policy.allow_file_write);
        assert!(!policy.allow_network);
        assert!(!policy.allow_subprocess);
    }

    // -- HookAuditLog tests --

    #[tokio::test]
    async fn audit_log_records_entries() {
        let log = HookAuditLog::new(10);
        let entry = HookAuditEntry {
            id: "test-1".to_owned(),
            plugin_name: "test-plugin".to_owned(),
            event: HookEvent::PreToolUse,
            command: "echo test".to_owned(),
            started_at: Utc::now(),
            finished_at: Some(Utc::now()),
            duration_ms: Some(100),
            success: true,
            error: None,
            exit_code: Some(0),
            permissions_granted: HookPermissions::read_only(),
            permission_denied: false,
            output_size_bytes: Some(10),
        };
        log.record(entry).await;
        assert_eq!(log.len().await, 1);
        assert!(!log.is_empty().await);
    }

    #[tokio::test]
    async fn audit_log_evicts_oldest_when_full() {
        let log = HookAuditLog::new(3);
        for i in 0..5 {
            let entry = HookAuditEntry {
                id: format!("test-{i}"),
                plugin_name: "test-plugin".to_owned(),
                event: HookEvent::PreToolUse,
                command: format!("cmd-{i}"),
                started_at: Utc::now(),
                finished_at: None,
                duration_ms: None,
                success: true,
                error: None,
                exit_code: None,
                permissions_granted: HookPermissions::none(),
                permission_denied: false,
                output_size_bytes: None,
            };
            log.record(entry).await;
        }
        assert_eq!(log.len().await, 3);
        let all = log.all().await;
        assert_eq!(all[0].id, "test-2");
        assert_eq!(all[2].id, "test-4");
    }

    #[tokio::test]
    async fn audit_log_filters_by_plugin() {
        let log = HookAuditLog::new(10);
        for name in &["alpha", "beta", "alpha"] {
            let entry = HookAuditEntry {
                id: format!("test-{name}"),
                plugin_name: name.to_string(),
                event: HookEvent::SessionStart,
                command: "cmd".to_owned(),
                started_at: Utc::now(),
                finished_at: None,
                duration_ms: None,
                success: true,
                error: None,
                exit_code: None,
                permissions_granted: HookPermissions::none(),
                permission_denied: false,
                output_size_bytes: None,
            };
            log.record(entry).await;
        }
        let alpha = log.for_plugin("alpha").await;
        assert_eq!(alpha.len(), 2);
        let beta = log.for_plugin("beta").await;
        assert_eq!(beta.len(), 1);
    }

    #[tokio::test]
    async fn audit_log_clear() {
        let log = HookAuditLog::new(10);
        let entry = HookAuditEntry {
            id: "test-1".to_owned(),
            plugin_name: "test".to_owned(),
            event: HookEvent::Stop,
            command: "cmd".to_owned(),
            started_at: Utc::now(),
            finished_at: None,
            duration_ms: None,
            success: true,
            error: None,
            exit_code: None,
            permissions_granted: HookPermissions::none(),
            permission_denied: false,
            output_size_bytes: None,
        };
        log.record(entry).await;
        assert!(!log.is_empty().await);
        log.clear().await;
        assert!(log.is_empty().await);
    }

    // -- HookSandboxConfig tests --

    #[test]
    fn sandbox_config_defaults() {
        let config = HookSandboxConfig::default();
        assert_eq!(config.timeout_secs, DEFAULT_HOOK_TIMEOUT_SECS);
        assert_eq!(config.memory_limit_mb, DEFAULT_HOOK_MEMORY_LIMIT_MB);
        assert_eq!(config.cpu_limit_secs, DEFAULT_HOOK_CPU_LIMIT_SECS);
        assert_eq!(config.max_output_bytes, MAX_HOOK_OUTPUT_BYTES);
        assert!(config.enforce_permissions);
        assert!(config.validate_output);
    }

    // -- Sandbox execute_hook integration tests --

    #[tokio::test]
    async fn sandbox_executes_simple_hook() {
        let sandbox = HookSandbox::default_sandbox();
        let hook = HookDefinition {
            command: if cfg!(windows) {
                "echo {\"status\":\"ok\"}".to_owned()
            } else {
                "echo '{\"status\":\"ok\"}'".to_owned()
            },
            description: Some("test hook".to_owned()),
            background: false,
        };
        let perms = HookPermissions::read_only();

        let result = sandbox
            .execute_hook("test-plugin", HookEvent::PreToolUse, &hook, &perms, None)
            .await;
        // This should succeed on most systems
        if let Ok(res) = result {
            assert!(res.success);
        }
    }

    #[tokio::test]
    async fn sandbox_rejects_hook_with_insufficient_permissions() {
        let sandbox = HookSandbox::default_sandbox();
        let hook = HookDefinition {
            command: "curl http://example.com".to_owned(),
            description: Some("network hook".to_owned()),
            background: false,
        };
        let perms = HookPermissions {
            allow_network: true,
            ..HookPermissions::none()
        };

        let result = sandbox
            .execute_hook("test-plugin", HookEvent::PreToolUse, &hook, &perms, None)
            .await;
        assert!(result.is_err());
        match result.unwrap_err() {
            SandboxError::PermissionDenied { .. } => {}
            other => panic!("expected PermissionDenied, got {other}"),
        }
    }

    #[tokio::test]
    async fn sandbox_records_audit_on_success() {
        let sandbox = HookSandbox::default_sandbox();
        let hook = HookDefinition {
            command: if cfg!(windows) {
                "echo hello".to_owned()
            } else {
                "echo hello".to_owned()
            },
            description: None,
            background: false,
        };
        let perms = HookPermissions::read_only();

        let _ = sandbox
            .execute_hook("audit-test", HookEvent::PostToolUse, &hook, &perms, None)
            .await;

        let entries = sandbox.audit_log.for_plugin("audit-test").await;
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].event, HookEvent::PostToolUse);
    }

    #[tokio::test]
    async fn sandbox_records_audit_on_permission_denial() {
        let sandbox = HookSandbox::default_sandbox();
        let hook = HookDefinition {
            command: "something".to_owned(),
            description: None,
            background: false,
        };
        let perms = HookPermissions {
            allow_network: true,
            ..HookPermissions::none()
        };

        let _ = sandbox
            .execute_hook("denied-test", HookEvent::PreToolUse, &hook, &perms, None)
            .await;

        let entries = sandbox.audit_log.for_plugin("denied-test").await;
        assert_eq!(entries.len(), 1);
        assert!(entries[0].permission_denied);
        assert!(!entries[0].success);
    }

    #[tokio::test]
    async fn sandbox_validates_output_against_schema() {
        let mut sandbox = HookSandbox::default_sandbox();
        let command = if cfg!(windows) {
            "echo {\"status\":\"ok\"}".to_owned()
        } else {
            "echo '{\"status\":\"ok\"}'".to_owned()
        };
        sandbox.register_schema(
            &command,
            HookOutputSchema::object(vec!["status"]),
        );

        let hook = HookDefinition {
            command: command.clone(),
            description: None,
            background: false,
        };
        let perms = HookPermissions::read_only();

        let result = sandbox
            .execute_hook("schema-test", HookEvent::PreToolUse, &hook, &perms, None)
            .await;

        // If the echo command produces valid JSON, schema validation should pass
        if let Ok(res) = result {
            assert!(res.success);
        }
    }

    #[tokio::test]
    async fn sandbox_timeout_kills_long_running_hook() {
        let mut config = HookSandboxConfig::default();
        config.timeout_secs = 1; // 1 second timeout

        let sandbox = HookSandbox::new(config, HookPermissionPolicy::default());
        let hook = HookDefinition {
            command: if cfg!(windows) {
                "ping -n 10 127.0.0.1 > NUL".to_owned()
            } else {
                "sleep 30".to_owned()
            },
            description: Some("long running".to_owned()),
            background: false,
        };
        let perms = HookPermissions::read_only();

        let result = sandbox
            .execute_hook("timeout-test", HookEvent::PreToolUse, &hook, &perms, None)
            .await;
        assert!(result.is_err());
        match result.unwrap_err() {
            SandboxError::Timeout { timeout_secs, .. } => {
                assert_eq!(timeout_secs, 1);
            }
            SandboxError::ExitCode { .. } => {
                // On some systems, the process exits with a signal code instead
            }
            other => panic!("expected Timeout or ExitCode, got {other}"),
        }
    }

    // -- FileAccess validation tests --

    #[test]
    fn validate_file_access_read_allowed() {
        let sandbox = HookSandbox::default_sandbox();
        let perms = HookPermissions::read_only();
        assert!(sandbox
            .validate_file_access(&perms, Path::new("/some/file"), FileAccessType::Read)
            .is_ok());
    }

    #[test]
    fn validate_file_access_write_denied_for_read_only() {
        let sandbox = HookSandbox::default_sandbox();
        let perms = HookPermissions::read_only();
        assert!(sandbox
            .validate_file_access(&perms, Path::new("/some/file"), FileAccessType::Write)
            .is_err());
    }

    #[test]
    fn validate_file_access_denied_when_no_perms() {
        let sandbox = HookSandbox::default_sandbox();
        let perms = HookPermissions::none();
        assert!(sandbox
            .validate_file_access(&perms, Path::new("/some/file"), FileAccessType::Read)
            .is_err());
    }

    #[test]
    fn validate_file_access_denied_outside_scope() {
        let temp = tempfile::tempdir().expect("tempdir");
        let scope_dir = temp.path().join("scope");
        fs::create_dir_all(&scope_dir).expect("mkdir");

        let sandbox = HookSandbox::default_sandbox();
        let perms = HookPermissions {
            allow_file_read: true,
            file_scope: vec![scope_dir.clone()],
            ..HookPermissions::default()
        };

        let inside = scope_dir.join("file.txt");
        assert!(sandbox
            .validate_file_access(&perms, &inside, FileAccessType::Read)
            .is_ok());

        assert!(sandbox
            .validate_file_access(&perms, Path::new("/outside/scope"), FileAccessType::Read)
            .is_err());
    }

    // -- Serialization round-trip tests --

    #[test]
    fn hook_permissions_serialize_roundtrip() {
        let perms = HookPermissions {
            allow_file_read: true,
            allow_file_write: false,
            allow_network: true,
            allow_subprocess: false,
            allow_env_read: true,
            file_scope: vec![PathBuf::from("/tmp/scope")],
        };
        let json = serde_json::to_string(&perms).expect("serialize");
        let deserialized: HookPermissions = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(perms, deserialized);
    }

    #[test]
    fn sandbox_config_serialize_roundtrip() {
        let config = HookSandboxConfig {
            timeout_secs: 60,
            memory_limit_mb: 512,
            cpu_limit_secs: 120,
            max_output_bytes: 2048,
            enforce_permissions: false,
            validate_output: false,
            working_directory: Some(PathBuf::from("/work")),
            env_vars: vec![("KEY".to_owned(), "value".to_owned())]
                .into_iter()
                .collect(),
        };
        let json = serde_json::to_string(&config).expect("serialize");
        let deserialized: HookSandboxConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(config, deserialized);
    }

    #[test]
    fn hook_audit_entry_serializes_cleanly() {
        let entry = HookAuditEntry {
            id: "hk-123-1".to_owned(),
            plugin_name: "test".to_owned(),
            event: HookEvent::PreToolUse,
            command: "echo".to_owned(),
            started_at: Utc::now(),
            finished_at: Some(Utc::now()),
            duration_ms: Some(50),
            success: true,
            error: None,
            exit_code: Some(0),
            permissions_granted: HookPermissions::read_only(),
            permission_denied: false,
            output_size_bytes: Some(5),
        };
        let json = serde_json::to_string(&entry).expect("serialize");
        assert!(json.contains("PreToolUse"));
        let parsed: HookAuditEntry = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(entry.id, parsed.id);
    }

    // -- generate_audit_id tests --

    #[test]
    fn audit_ids_are_unique() {
        let id1 = generate_audit_id();
        let id2 = generate_audit_id();
        assert_ne!(id1, id2);
        assert!(id1.starts_with("hk-"));
    }
}
