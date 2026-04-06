use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Stdio,
};

use rc_mcp::McpConfig;
use rc_skills::{SkillDocument, SkillError};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::DeserializeOwned};
use serde_json::Value;
use thiserror::Error;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines},
    process::{Child, ChildStdin, ChildStdout, Command},
    time::{Duration, timeout},
};
use walkdir::WalkDir;

pub const PLUGIN_MANIFEST_FILE: &str = "plugin.json";
pub const PLUGIN_MANIFEST_DIR: &str = ".codex-plugin";
pub const DEFAULT_PLUGIN_RUNTIME_PROTOCOL_VERSION: &str = "2026-04-07";
pub const DEFAULT_PLUGIN_HANDSHAKE_TIMEOUT_SECS: u64 = 10;
pub const DEFAULT_PLUGIN_REQUEST_TIMEOUT_SECS: u64 = 15;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub author: Option<PluginAuthor>,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub repository: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub skills: Option<String>,
    #[serde(default)]
    pub hooks: Option<String>,
    #[serde(default)]
    pub apps: Option<String>,
    #[serde(default, alias = "mcpServers")]
    pub mcp: Option<String>,
    #[serde(default)]
    pub interface: Option<PluginInterface>,
    #[serde(default)]
    pub runtime: Option<PluginRuntimeConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginAuthor {
    pub name: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginInterface {
    #[serde(rename = "displayName")]
    pub display_name: String,
    #[serde(rename = "shortDescription")]
    pub short_description: String,
    #[serde(rename = "longDescription")]
    pub long_description: Option<String>,
    #[serde(rename = "developerName")]
    pub developer_name: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub capabilities: Vec<PluginCapability>,
    #[serde(rename = "websiteURL")]
    pub website_url: Option<String>,
    #[serde(rename = "privacyPolicyURL")]
    pub privacy_policy_url: Option<String>,
    #[serde(rename = "termsOfServiceURL")]
    pub terms_of_service_url: Option<String>,
    #[serde(rename = "defaultPrompt", default)]
    pub default_prompt: Vec<String>,
    #[serde(rename = "composerIcon")]
    pub composer_icon: Option<String>,
    #[serde(default)]
    pub logo: Option<String>,
    #[serde(default)]
    pub screenshots: Vec<String>,
    #[serde(rename = "brandColor")]
    pub brand_color: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginRuntimeConfig {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub cwd: Option<PathBuf>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub handshake_timeout_secs: Option<u64>,
    #[serde(default)]
    pub request_timeout_secs: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedPluginRuntimeConfig {
    pub command: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: BTreeMap<String, String>,
    pub handshake_timeout_secs: u64,
    pub request_timeout_secs: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginCapability {
    Read,
    Write,
    Interactive,
    Background,
    Network,
    Unknown(String),
}

impl Serialize for PluginCapability {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(match self {
            Self::Read => "Read",
            Self::Write => "Write",
            Self::Interactive => "Interactive",
            Self::Background => "Background",
            Self::Network => "Network",
            Self::Unknown(value) => value,
        })
    }
}

impl<'de> Deserialize<'de> for PluginCapability {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(match value.as_str() {
            "Read" => Self::Read,
            "Write" => Self::Write,
            "Interactive" => Self::Interactive,
            "Background" => Self::Background,
            "Network" => Self::Network,
            _ => Self::Unknown(value),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginBundle {
    pub manifest: PluginManifest,
    pub manifest_path: PathBuf,
    pub root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginHostInfo {
    pub name: String,
    pub version: String,
}

impl PluginHostInfo {
    #[must_use]
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
        }
    }
}

impl Default for PluginHostInfo {
    fn default() -> Self {
        Self::new("remote-code-rust", env!("CARGO_PKG_VERSION"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginPeerInfo {
    pub name: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginRuntimeActionDescriptor {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub input_schema: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginRuntimeInspection {
    pub plugin_name: String,
    pub protocol_version: String,
    #[serde(default)]
    pub plugin_info: Option<PluginPeerInfo>,
    #[serde(default)]
    pub actions: Vec<PluginRuntimeActionDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInvokeResult {
    #[serde(default)]
    pub output: Value,
    #[serde(default)]
    pub is_error: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInvokeResponse {
    pub plugin_name: String,
    pub action: String,
    pub protocol_version: String,
    #[serde(default)]
    pub plugin_info: Option<PluginPeerInfo>,
    pub result: PluginInvokeResult,
}

#[derive(Debug, Error)]
pub enum PluginError {
    #[error("failed to read plugin manifest `{path}`")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse plugin manifest `{path}`")]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
}

#[derive(Debug, Error)]
pub enum PluginRuntimeError {
    #[error("plugin `{plugin}` does not define a runtime adapter configuration")]
    MissingRuntimeConfig { plugin: String },
    #[error("failed to spawn plugin runtime for `{plugin}` using `{command}`")]
    Spawn {
        plugin: String,
        command: String,
        #[source]
        source: std::io::Error,
    },
    #[error("plugin runtime for `{plugin}` did not expose {pipe}")]
    MissingPipe { plugin: String, pipe: &'static str },
    #[error("failed to serialize JSON-RPC payload for plugin `{plugin}` during {phase}")]
    Serialize {
        plugin: String,
        phase: &'static str,
        #[source]
        source: serde_json::Error,
    },
    #[error("failed to write to plugin `{plugin}` during {phase}")]
    Write {
        plugin: String,
        phase: &'static str,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to read from plugin `{plugin}` during {phase}")]
    Read {
        plugin: String,
        phase: &'static str,
        #[source]
        source: std::io::Error,
    },
    #[error("timed out waiting for plugin `{plugin}` during {phase} after {timeout_secs}s")]
    Timeout {
        plugin: String,
        phase: &'static str,
        timeout_secs: u64,
    },
    #[error("plugin `{plugin}` closed stdout while waiting for {phase}")]
    Closed { plugin: String, phase: &'static str },
    #[error("failed to decode JSON from plugin `{plugin}` during {phase}")]
    Decode {
        plugin: String,
        phase: &'static str,
        #[source]
        source: serde_json::Error,
    },
    #[error("plugin `{plugin}` returned an invalid response during {phase}: {message}")]
    Protocol {
        plugin: String,
        phase: &'static str,
        message: String,
    },
    #[error("plugin `{plugin}` returned JSON-RPC error {code}: {message}")]
    Rpc {
        plugin: String,
        code: i64,
        message: String,
    },
}

#[derive(Debug, Serialize)]
struct JsonRpcRequest<T> {
    jsonrpc: &'static str,
    id: u64,
    method: &'static str,
    params: T,
}

#[derive(Debug, Serialize)]
struct JsonRpcNotification<T> {
    jsonrpc: &'static str,
    method: &'static str,
    params: T,
}

#[derive(Debug, Deserialize)]
struct JsonRpcEnvelope {
    #[serde(default)]
    id: Option<Value>,
    #[serde(default)]
    result: Option<Value>,
    #[serde(default)]
    error: Option<JsonRpcErrorPayload>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcErrorPayload {
    code: i64,
    message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PluginInitializeParams<'a> {
    protocol_version: &'a str,
    host_info: &'a PluginHostInfo,
    plugin: PluginIdentity<'a>,
}

#[derive(Debug, Serialize)]
struct PluginIdentity<'a> {
    name: &'a str,
    version: &'a str,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PluginInitializeResult {
    #[serde(default)]
    protocol_version: String,
    #[serde(default)]
    plugin_info: Option<PluginPeerInfo>,
    #[serde(default)]
    actions: Vec<PluginRuntimeActionDescriptor>,
}

#[derive(Debug, Serialize)]
struct PluginInvokeParams<'a> {
    action: &'a str,
    input: Value,
}

struct PluginRuntimeSession {
    plugin_name: String,
    child: Child,
    stdin: ChildStdin,
    lines: Lines<BufReader<ChildStdout>>,
    initialized: PluginInitializeResult,
    request_timeout_secs: u64,
}

pub fn discover_plugins(root: &Path) -> Result<Vec<PluginBundle>, PluginError> {
    let mut plugins = WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| entry.file_name() == PLUGIN_MANIFEST_FILE)
        .map(|entry| load_plugin(entry.path()))
        .collect::<Result<Vec<_>, _>>()?;

    plugins.sort_by(|left, right| left.manifest.name.cmp(&right.manifest.name));
    Ok(plugins)
}

pub fn load_plugin(path: impl AsRef<Path>) -> Result<PluginBundle, PluginError> {
    let path = path.as_ref();
    let content = fs::read_to_string(path).map_err(|source| PluginError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let manifest = serde_json::from_str(&content).map_err(|source| PluginError::Parse {
        path: path.to_path_buf(),
        source,
    })?;

    Ok(PluginBundle {
        manifest,
        manifest_path: path.to_path_buf(),
        root: resolve_plugin_root(path),
    })
}

pub async fn inspect_runtime(
    plugin: &PluginBundle,
    host_info: &PluginHostInfo,
) -> Result<PluginRuntimeInspection, PluginRuntimeError> {
    let runtime =
        plugin
            .runtime_config()
            .ok_or_else(|| PluginRuntimeError::MissingRuntimeConfig {
                plugin: plugin.manifest.name.clone(),
            })?;
    let mut session = PluginRuntimeSession::connect(plugin, &runtime, host_info).await?;
    let inspection = session.inspect();
    session.shutdown().await;
    Ok(inspection)
}

pub async fn invoke_runtime(
    plugin: &PluginBundle,
    host_info: &PluginHostInfo,
    action: &str,
    input: Value,
) -> Result<PluginInvokeResponse, PluginRuntimeError> {
    let runtime =
        plugin
            .runtime_config()
            .ok_or_else(|| PluginRuntimeError::MissingRuntimeConfig {
                plugin: plugin.manifest.name.clone(),
            })?;
    let mut session = PluginRuntimeSession::connect(plugin, &runtime, host_info).await?;
    let response = session.invoke(action, input).await;
    session.shutdown().await;
    response
}

pub async fn inspect_plugin_runtime(
    plugin: &PluginBundle,
    host_info: &PluginHostInfo,
) -> Result<PluginRuntimeInspection, PluginRuntimeError> {
    inspect_runtime(plugin, host_info).await
}

pub async fn invoke_plugin_action(
    plugin: &PluginBundle,
    host_info: &PluginHostInfo,
    action: &str,
    input: Value,
) -> Result<PluginInvokeResponse, PluginRuntimeError> {
    invoke_runtime(plugin, host_info, action, input).await
}

impl PluginBundle {
    #[must_use]
    pub fn skills_root(&self) -> Option<PathBuf> {
        self.manifest
            .skills
            .as_deref()
            .map(|relative| self.resolve_relative(relative))
    }

    #[must_use]
    pub fn app_manifest_path(&self) -> Option<PathBuf> {
        self.manifest
            .apps
            .as_deref()
            .map(|relative| self.resolve_relative(relative))
    }

    #[must_use]
    pub fn mcp_config_path(&self) -> Option<PathBuf> {
        self.manifest
            .mcp
            .as_deref()
            .map(|relative| self.resolve_relative(relative))
    }

    #[must_use]
    pub fn runtime_config(&self) -> Option<ResolvedPluginRuntimeConfig> {
        self.manifest.runtime.as_ref().map(|runtime| {
            let cwd = runtime
                .cwd
                .as_ref()
                .map(|cwd| {
                    if cwd.is_absolute() {
                        cwd.clone()
                    } else {
                        self.root.join(cwd)
                    }
                })
                .unwrap_or_else(|| self.root.clone());
            ResolvedPluginRuntimeConfig {
                command: runtime.command.clone(),
                args: runtime.args.clone(),
                cwd,
                env: runtime.env.clone(),
                handshake_timeout_secs: runtime
                    .handshake_timeout_secs
                    .unwrap_or(DEFAULT_PLUGIN_HANDSHAKE_TIMEOUT_SECS)
                    .max(1),
                request_timeout_secs: runtime
                    .request_timeout_secs
                    .unwrap_or(DEFAULT_PLUGIN_REQUEST_TIMEOUT_SECS)
                    .max(1),
            }
        })
    }

    pub fn discover_bundled_skills(&self) -> Result<Vec<SkillDocument>, SkillError> {
        match self.skills_root() {
            Some(root) => rc_skills::discover_skills(&root),
            None => Ok(Vec::new()),
        }
    }

    pub fn load_mcp_config(&self) -> Result<Option<McpConfig>, rc_mcp::McpConfigError> {
        match self.mcp_config_path() {
            Some(path) => McpConfig::load(path).map(Some),
            None => Ok(None),
        }
    }

    fn resolve_relative(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }
}

impl PluginRuntimeSession {
    async fn connect(
        plugin: &PluginBundle,
        runtime: &ResolvedPluginRuntimeConfig,
        host_info: &PluginHostInfo,
    ) -> Result<Self, PluginRuntimeError> {
        let mut process = Command::new(&runtime.command);
        process
            .args(&runtime.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .current_dir(&runtime.cwd)
            .kill_on_drop(true);
        if !runtime.env.is_empty() {
            process.envs(&runtime.env);
        }

        let mut child = process
            .spawn()
            .map_err(|source| PluginRuntimeError::Spawn {
                plugin: plugin.manifest.name.clone(),
                command: runtime.command.clone(),
                source,
            })?;
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| PluginRuntimeError::MissingPipe {
                plugin: plugin.manifest.name.clone(),
                pipe: "stdin",
            })?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| PluginRuntimeError::MissingPipe {
                plugin: plugin.manifest.name.clone(),
                pipe: "stdout",
            })?;
        let mut lines = BufReader::new(stdout).lines();

        let initialize = JsonRpcRequest {
            jsonrpc: "2.0",
            id: 1,
            method: "initialize",
            params: PluginInitializeParams {
                protocol_version: DEFAULT_PLUGIN_RUNTIME_PROTOCOL_VERSION,
                host_info,
                plugin: PluginIdentity {
                    name: &plugin.manifest.name,
                    version: &plugin.manifest.version,
                },
            },
        };
        write_message(
            &mut stdin,
            &plugin.manifest.name,
            "initialize request",
            &initialize,
        )
        .await?;
        let initialized: PluginInitializeResult = wait_for_response(
            &mut lines,
            &plugin.manifest.name,
            1,
            "initialize response",
            runtime.handshake_timeout_secs,
        )
        .await?;
        if initialized.protocol_version.trim().is_empty() {
            return Err(PluginRuntimeError::Protocol {
                plugin: plugin.manifest.name.clone(),
                phase: "initialize response",
                message: "protocolVersion was empty".to_owned(),
            });
        }

        let ready = JsonRpcNotification {
            jsonrpc: "2.0",
            method: "notifications/initialized",
            params: serde_json::json!({}),
        };
        write_message(
            &mut stdin,
            &plugin.manifest.name,
            "initialized notification",
            &ready,
        )
        .await?;

        Ok(Self {
            plugin_name: plugin.manifest.name.clone(),
            child,
            stdin,
            lines,
            initialized,
            request_timeout_secs: runtime.request_timeout_secs,
        })
    }

    fn inspect(&self) -> PluginRuntimeInspection {
        PluginRuntimeInspection {
            plugin_name: self.plugin_name.clone(),
            protocol_version: self.initialized.protocol_version.clone(),
            plugin_info: self.initialized.plugin_info.clone(),
            actions: self.initialized.actions.clone(),
        }
    }

    async fn invoke(
        &mut self,
        action: &str,
        input: Value,
    ) -> Result<PluginInvokeResponse, PluginRuntimeError> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id: 2,
            method: "plugin/invoke",
            params: PluginInvokeParams { action, input },
        };
        write_message(
            &mut self.stdin,
            &self.plugin_name,
            "plugin/invoke request",
            &request,
        )
        .await?;
        let result: PluginInvokeResult = wait_for_response(
            &mut self.lines,
            &self.plugin_name,
            2,
            "plugin/invoke response",
            self.request_timeout_secs,
        )
        .await?;

        Ok(PluginInvokeResponse {
            plugin_name: self.plugin_name.clone(),
            action: action.to_owned(),
            protocol_version: self.initialized.protocol_version.clone(),
            plugin_info: self.initialized.plugin_info.clone(),
            result,
        })
    }

    async fn shutdown(&mut self) {
        let _ = self.child.start_kill();
        let _ = self.child.wait().await;
    }
}

async fn write_message<T: Serialize>(
    stdin: &mut ChildStdin,
    plugin: &str,
    phase: &'static str,
    payload: &T,
) -> Result<(), PluginRuntimeError> {
    let mut body = serde_json::to_vec(payload).map_err(|source| PluginRuntimeError::Serialize {
        plugin: plugin.to_owned(),
        phase,
        source,
    })?;
    body.push(b'\n');
    stdin
        .write_all(&body)
        .await
        .map_err(|source| PluginRuntimeError::Write {
            plugin: plugin.to_owned(),
            phase,
            source,
        })?;
    stdin
        .flush()
        .await
        .map_err(|source| PluginRuntimeError::Write {
            plugin: plugin.to_owned(),
            phase,
            source,
        })
}

async fn wait_for_response<T: DeserializeOwned>(
    lines: &mut Lines<BufReader<ChildStdout>>,
    plugin: &str,
    request_id: u64,
    phase: &'static str,
    timeout_secs: u64,
) -> Result<T, PluginRuntimeError> {
    timeout(Duration::from_secs(timeout_secs), async {
        loop {
            let line = lines
                .next_line()
                .await
                .map_err(|source| PluginRuntimeError::Read {
                    plugin: plugin.to_owned(),
                    phase,
                    source,
                })?;
            let Some(line) = line else {
                return Err(PluginRuntimeError::Closed {
                    plugin: plugin.to_owned(),
                    phase,
                });
            };
            if line.trim().is_empty() {
                continue;
            }
            let envelope: JsonRpcEnvelope =
                serde_json::from_str(&line).map_err(|source| PluginRuntimeError::Decode {
                    plugin: plugin.to_owned(),
                    phase,
                    source,
                })?;
            let Some(id) = envelope.id.as_ref() else {
                continue;
            };
            if !rpc_id_matches(id, request_id) {
                continue;
            }
            if let Some(error) = envelope.error {
                return Err(PluginRuntimeError::Rpc {
                    plugin: plugin.to_owned(),
                    code: error.code,
                    message: error.message,
                });
            }
            let result = envelope
                .result
                .ok_or_else(|| PluginRuntimeError::Protocol {
                    plugin: plugin.to_owned(),
                    phase,
                    message: "response did not include a result payload".to_owned(),
                })?;
            return serde_json::from_value(result).map_err(|source| PluginRuntimeError::Decode {
                plugin: plugin.to_owned(),
                phase,
                source,
            });
        }
    })
    .await
    .map_err(|_| PluginRuntimeError::Timeout {
        plugin: plugin.to_owned(),
        phase,
        timeout_secs,
    })?
}

fn rpc_id_matches(id: &Value, request_id: u64) -> bool {
    id.as_u64() == Some(request_id)
        || id.as_i64() == Some(request_id as i64)
        || id
            .as_str()
            .is_some_and(|value| value == request_id.to_string())
}

fn resolve_plugin_root(manifest_path: &Path) -> PathBuf {
    let Some(parent) = manifest_path.parent() else {
        return PathBuf::from(".");
    };
    if parent
        .file_name()
        .is_some_and(|name| name == PLUGIN_MANIFEST_DIR)
    {
        return parent
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| parent.to_path_buf());
    }
    parent.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command as ProcessCommand;
    use tempfile::tempdir;

    fn ok<T, E: std::fmt::Display>(result: Result<T, E>) -> T {
        match result {
            Ok(value) => value,
            Err(error) => panic!("unexpected error: {error}"),
        }
    }

    #[test]
    fn loads_plugin_manifest_and_resolves_paths() {
        let temp = ok(tempdir());
        let root = temp.path().join("github-plugin");
        ok(fs::create_dir_all(root.join(PLUGIN_MANIFEST_DIR)));
        ok(fs::write(
            root.join(PLUGIN_MANIFEST_DIR).join(PLUGIN_MANIFEST_FILE),
            r#"{
                "name": "github",
                "version": "0.1.0",
                "skills": "./skills",
                "hooks": "./hooks.json",
                "apps": "./.app.json",
                "mcpServers": "./mcp.toml",
                "runtime": {
                    "command": "python",
                    "args": ["adapter.py"],
                    "cwd": "./adapter"
                },
                "interface": {
                    "displayName": "GitHub",
                    "shortDescription": "Triage GitHub work",
                    "capabilities": ["Interactive", "Write", "ExperimentalCapability"],
                    "defaultPrompt": ["Help with GitHub"]
                }
            }"#,
        ));

        let plugin = ok(load_plugin(
            root.join(PLUGIN_MANIFEST_DIR).join(PLUGIN_MANIFEST_FILE),
        ));

        assert_eq!(plugin.root, root);
        assert_eq!(plugin.manifest.name, "github");
        assert_eq!(plugin.skills_root(), Some(plugin.root.join("./skills")));
        assert_eq!(plugin.manifest.hooks.as_deref(), Some("./hooks.json"));
        assert_eq!(
            plugin.app_manifest_path(),
            Some(plugin.root.join("./.app.json"))
        );
        assert_eq!(
            plugin.mcp_config_path(),
            Some(plugin.root.join("./mcp.toml"))
        );
        let runtime = plugin
            .runtime_config()
            .unwrap_or_else(|| panic!("missing runtime config"));
        assert_eq!(runtime.command, "python");
        assert_eq!(runtime.args, vec!["adapter.py"]);
        assert_eq!(runtime.cwd, plugin.root.join("./adapter"));
        let interface = match plugin.manifest.interface {
            Some(interface) => interface,
            None => panic!("missing interface"),
        };
        assert_eq!(
            interface.capabilities,
            vec![
                PluginCapability::Interactive,
                PluginCapability::Write,
                PluginCapability::Unknown("ExperimentalCapability".to_owned())
            ]
        );
    }

    #[test]
    fn discovers_plugins_sorted_by_name() {
        let temp = ok(tempdir());
        let alpha = temp.path().join("alpha");
        let zeta = temp.path().join("zeta");
        ok(fs::create_dir_all(alpha.join(PLUGIN_MANIFEST_DIR)));
        ok(fs::create_dir_all(zeta.join(PLUGIN_MANIFEST_DIR)));
        ok(fs::write(
            alpha.join(PLUGIN_MANIFEST_DIR).join(PLUGIN_MANIFEST_FILE),
            r#"{"name":"alpha","version":"0.1.0"}"#,
        ));
        ok(fs::write(
            zeta.join(PLUGIN_MANIFEST_DIR).join(PLUGIN_MANIFEST_FILE),
            r#"{"name":"zeta","version":"0.1.0"}"#,
        ));

        let plugins = ok(discover_plugins(temp.path()));

        assert_eq!(plugins.len(), 2);
        assert_eq!(plugins[0].manifest.name, "alpha");
        assert_eq!(plugins[1].manifest.name, "zeta");
    }

    #[test]
    fn discovers_bundled_skills() {
        let temp = ok(tempdir());
        let root = temp.path().join("bundle");
        ok(fs::create_dir_all(root.join(PLUGIN_MANIFEST_DIR)));
        ok(fs::create_dir_all(root.join("skills").join("demo")));
        ok(fs::write(
            root.join(PLUGIN_MANIFEST_DIR).join(PLUGIN_MANIFEST_FILE),
            r#"{"name":"bundle","version":"0.1.0","skills":"./skills"}"#,
        ));
        ok(fs::write(
            root.join("skills").join("demo").join("SKILL.md"),
            "# Demo\n\nDemo summary.\n",
        ));

        let plugin = ok(load_plugin(
            root.join(PLUGIN_MANIFEST_DIR).join(PLUGIN_MANIFEST_FILE),
        ));
        let skills = ok(plugin.discover_bundled_skills());

        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].metadata.slug, "demo");
    }

    #[test]
    fn loads_plugin_mcp_config() {
        let temp = ok(tempdir());
        let root = temp.path().join("mcp-plugin");
        ok(fs::create_dir_all(root.join(PLUGIN_MANIFEST_DIR)));
        ok(fs::write(
            root.join(PLUGIN_MANIFEST_DIR).join(PLUGIN_MANIFEST_FILE),
            r#"{"name":"mcp-plugin","version":"0.1.0","mcp":"./mcp.toml"}"#,
        ));
        ok(fs::write(
            root.join("mcp.toml"),
            "[mcp_servers.demo]\ncommand = \"uvx\"\n",
        ));

        let plugin = ok(load_plugin(
            root.join(PLUGIN_MANIFEST_DIR).join(PLUGIN_MANIFEST_FILE),
        ));
        let config = ok(plugin.load_mcp_config());
        let config = match config {
            Some(config) => config,
            None => panic!("missing MCP config"),
        };

        assert!(config.servers.contains_key("demo"));
    }

    #[test]
    fn runtime_config_is_optional() {
        let temp = ok(tempdir());
        let root = temp.path().join("plain-plugin");
        ok(fs::create_dir_all(root.join(PLUGIN_MANIFEST_DIR)));
        ok(fs::write(
            root.join(PLUGIN_MANIFEST_DIR).join(PLUGIN_MANIFEST_FILE),
            r#"{"name":"plain","version":"0.1.0"}"#,
        ));

        let plugin = ok(load_plugin(
            root.join(PLUGIN_MANIFEST_DIR).join(PLUGIN_MANIFEST_FILE),
        ));
        assert!(plugin.runtime_config().is_none());
    }

    #[tokio::test]
    async fn inspects_runtime_and_invokes_action() {
        let Some((python, mut prefix_args)) = python_command() else {
            eprintln!("Skipping plugin runtime test because Python is unavailable.");
            return;
        };

        let temp = ok(tempdir());
        let root = temp.path().join("plugin");
        ok(fs::create_dir_all(root.join(PLUGIN_MANIFEST_DIR)));
        let script = root.join("adapter.py");
        ok(fs::write(&script, mock_plugin_runtime_script()));
        prefix_args.push("adapter.py".to_owned());
        prefix_args.push("success".to_owned());

        write_runtime_manifest(&root, &python, &prefix_args);

        let plugin = ok(load_plugin(
            root.join(PLUGIN_MANIFEST_DIR).join(PLUGIN_MANIFEST_FILE),
        ));
        let host = PluginHostInfo::new("remote-code-rust", "test");

        let inspection = inspect_runtime(&plugin, &host)
            .await
            .unwrap_or_else(|error| panic!("inspection failed: {error}"));
        assert_eq!(inspection.plugin_name, "demo-plugin");
        assert_eq!(
            inspection.protocol_version,
            DEFAULT_PLUGIN_RUNTIME_PROTOCOL_VERSION
        );
        assert_eq!(inspection.actions.len(), 1);
        assert_eq!(inspection.actions[0].name, "echo");

        let response = invoke_runtime(&plugin, &host, "echo", serde_json::json!({"text": "hello"}))
            .await
            .unwrap_or_else(|error| panic!("invoke failed: {error}"));
        assert_eq!(response.action, "echo");
        assert_eq!(response.plugin_name, "demo-plugin");
        assert!(!response.result.is_error);
        assert_eq!(
            response.result.output,
            serde_json::json!({"echoed": "hello"})
        );
    }

    #[tokio::test]
    async fn surfaces_runtime_protocol_errors() {
        let Some((python, mut prefix_args)) = python_command() else {
            eprintln!("Skipping plugin runtime protocol test because Python is unavailable.");
            return;
        };

        let temp = ok(tempdir());
        let root = temp.path().join("plugin");
        ok(fs::create_dir_all(root.join(PLUGIN_MANIFEST_DIR)));
        let script = root.join("adapter.py");
        ok(fs::write(&script, mock_plugin_runtime_script()));
        prefix_args.push("adapter.py".to_owned());
        prefix_args.push("protocol_error".to_owned());

        write_runtime_manifest(&root, &python, &prefix_args);

        let plugin = ok(load_plugin(
            root.join(PLUGIN_MANIFEST_DIR).join(PLUGIN_MANIFEST_FILE),
        ));
        let error = inspect_runtime(&plugin, &PluginHostInfo::default())
            .await
            .expect_err("protocol error should surface");
        assert!(matches!(
            error,
            PluginRuntimeError::Protocol { phase, .. } if phase == "initialize response"
        ));
    }

    #[tokio::test]
    async fn surfaces_runtime_rpc_errors() {
        let Some((python, mut prefix_args)) = python_command() else {
            eprintln!("Skipping plugin runtime RPC test because Python is unavailable.");
            return;
        };

        let temp = ok(tempdir());
        let root = temp.path().join("plugin");
        ok(fs::create_dir_all(root.join(PLUGIN_MANIFEST_DIR)));
        let script = root.join("adapter.py");
        ok(fs::write(&script, mock_plugin_runtime_script()));
        prefix_args.push("adapter.py".to_owned());
        prefix_args.push("rpc_error".to_owned());

        write_runtime_manifest(&root, &python, &prefix_args);

        let plugin = ok(load_plugin(
            root.join(PLUGIN_MANIFEST_DIR).join(PLUGIN_MANIFEST_FILE),
        ));
        let error = invoke_runtime(
            &plugin,
            &PluginHostInfo::default(),
            "echo",
            serde_json::json!({"text": "boom"}),
        )
        .await
        .expect_err("RPC error should surface");
        assert!(matches!(
            error,
            PluginRuntimeError::Rpc {
                code: -32001,
                ref message,
                ..
            } if message == "invoke failed"
        ));
    }

    fn write_runtime_manifest(root: &Path, command: &str, args: &[String]) {
        ok(fs::write(
            root.join(PLUGIN_MANIFEST_DIR).join(PLUGIN_MANIFEST_FILE),
            format!(
                r#"{{
                    "name": "demo-plugin",
                    "version": "0.1.0",
                    "runtime": {{
                        "command": "{command}",
                        "args": [{args}],
                        "cwd": "."
                    }}
                }}"#,
                command = command,
                args = args
                    .iter()
                    .map(|arg| format!(r#""{}""#, arg.replace('\\', "\\\\").replace('"', "\\\"")))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        ));
    }

    fn python_command() -> Option<(String, Vec<String>)> {
        if let Ok(path) = std::env::var("PYTHON")
            && ProcessCommand::new(&path)
                .arg("--version")
                .output()
                .is_ok_and(|output| output.status.success())
        {
            return Some((path, Vec::new()));
        }

        for candidate in ["python", "python3"] {
            if ProcessCommand::new(candidate)
                .arg("--version")
                .output()
                .is_ok_and(|output| output.status.success())
            {
                return Some((candidate.to_owned(), Vec::new()));
            }
        }

        if cfg!(windows)
            && ProcessCommand::new("py")
                .args(["-3", "--version"])
                .output()
                .is_ok_and(|output| output.status.success())
        {
            return Some(("py".to_owned(), vec!["-3".to_owned()]));
        }

        None
    }

    fn mock_plugin_runtime_script() -> &'static str {
        r#"
import json
import sys

mode = sys.argv[1] if len(sys.argv) > 1 else "success"

for raw in sys.stdin:
    raw = raw.strip()
    if not raw:
        continue
    message = json.loads(raw)
    method = message.get("method")
    message_id = message.get("id")

    if method == "initialize":
        if mode == "protocol_error":
            print(json.dumps({
                "jsonrpc": "2.0",
                "id": message_id,
                "result": {
                    "pluginInfo": {"name": "demo-adapter", "version": "0.1.0"},
                    "actions": [{"name": "echo"}]
                }
            }), flush=True)
        else:
            print(json.dumps({
                "jsonrpc": "2.0",
                "id": message_id,
                "result": {
                    "protocolVersion": "2026-04-07",
                    "pluginInfo": {
                        "name": "demo-adapter",
                        "title": "Demo Adapter",
                        "version": "0.1.0"
                    },
                    "actions": [{
                        "name": "echo",
                        "description": "Echo a text payload",
                        "inputSchema": {"type": "object"}
                    }]
                }
            }), flush=True)
    elif method == "notifications/initialized":
        continue
    elif method == "plugin/invoke":
        if mode == "rpc_error":
            print(json.dumps({
                "jsonrpc": "2.0",
                "id": message_id,
                "error": {"code": -32001, "message": "invoke failed"}
            }), flush=True)
        else:
            text = message["params"]["input"]["text"]
            print(json.dumps({
                "jsonrpc": "2.0",
                "id": message_id,
                "result": {
                    "output": {"echoed": text},
                    "isError": False
                }
            }), flush=True)
        break
"#
    }
}
