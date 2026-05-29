use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{NaiveDate, Utc};
use serde_json::{Value, json};
use tracing_subscriber::EnvFilter;

use crate::dto::DiagnosticBundleResultDto;
use crate::paths::RuntimePathLayout;

pub(crate) const GUI_LOG_FILE_PREFIX: &str = "remote-code-gui.log";
pub(crate) const DEFAULT_LOG_RETENTION_DAYS: i64 = 14;
pub(crate) const MAX_LOG_FIELD_CHARS: usize = 4096;
const MAX_DIAGNOSTIC_LOG_FILES: usize = 20;
const MAX_DIAGNOSTIC_SINGLE_LOG_BYTES: u64 = 10 * 1024 * 1024;
const MAX_DIAGNOSTIC_TOTAL_LOG_BYTES: u64 = 50 * 1024 * 1024;
const DEFAULT_GUI_LOG_FILTER: &str = concat!(
    "remote_code_gui=info,",
    "rc_remote_transport=info,",
    "rc_control_plane=info,",
    "rc_runner=info,",
    "rc_codex_adapter=info,",
    "rc_roo_adapter=info,",
    "rc_claude_adapter=info,",
    "claude_config=info,",
    "claude_core=info,",
    "claude_mcp=info,",
    "claude_provider=info,",
    "claude_session=info,",
    "claude_tools=info,",
    "roo_cloud=info,",
    "roo_index=info,",
    "roo_telemetry=info,",
    "codex_app_server=info,",
    "codex_core=info,",
    "codex_mcp=info"
);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DiagnosticBundleOptions {
    pub(crate) include_logs: bool,
    pub(crate) include_settings: bool,
}

pub(crate) fn gui_env_filter_from_env() -> EnvFilter {
    EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_GUI_LOG_FILTER))
}

pub(crate) fn sanitize_log_field(value: &str) -> String {
    let mut sanitized = String::new();
    for (index, line) in value.lines().enumerate() {
        if index > 0 {
            sanitized.push('\n');
        }
        if is_sensitive_line(line) {
            sanitized.push_str("[redacted]");
        } else {
            sanitized.push_str(line);
        }
        if sanitized.chars().count() > MAX_LOG_FIELD_CHARS {
            break;
        }
    }

    truncate_chars(&sanitized, MAX_LOG_FIELD_CHARS)
}

pub(crate) fn prune_old_gui_logs(log_dir: &Path, retention_days: i64) -> std::io::Result<usize> {
    prune_old_gui_logs_for_date(log_dir, Utc::now().date_naive(), retention_days)
}

pub(crate) fn prune_old_gui_logs_for_date(
    log_dir: &Path,
    today: NaiveDate,
    retention_days: i64,
) -> std::io::Result<usize> {
    if !log_dir.exists() {
        return Ok(0);
    }

    let cutoff = today - chrono::Duration::days(retention_days.max(1));
    let mut removed = 0usize;

    for entry in fs::read_dir(log_dir)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        if !metadata.is_file() {
            continue;
        }

        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        if file_name.as_ref() == GUI_LOG_FILE_PREFIX {
            continue;
        }

        if let Some(date) = parse_rotated_gui_log_date(&file_name)
            && date < cutoff
        {
            fs::remove_file(entry.path())?;
            removed += 1;
        }
    }

    Ok(removed)
}

pub(crate) fn create_diagnostic_bundle(
    layout: &RuntimePathLayout,
    options: DiagnosticBundleOptions,
) -> Result<DiagnosticBundleResultDto> {
    let diagnostics_root = layout.cache_dir.join("diagnostics");
    fs::create_dir_all(&diagnostics_root)
        .with_context(|| format!("failed to create {}", diagnostics_root.display()))?;

    let bundle_dir = unique_bundle_dir(&diagnostics_root);
    fs::create_dir_all(&bundle_dir)
        .with_context(|| format!("failed to create {}", bundle_dir.display()))?;

    let mut files = 0usize;
    let mut bytes = 0u64;

    let manifest = json!({
        "createdAt": Utc::now().to_rfc3339(),
        "app": "remote-code-gui",
        "version": env!("CARGO_PKG_VERSION"),
        "profileDir": layout.profile_dir,
        "logsDir": layout.logs_dir,
        "sessionsDir": layout.sessions_dir,
        "artifactsDir": layout.artifacts_dir,
        "includeLogs": options.include_logs,
        "includeSettings": options.include_settings,
        "limits": {
            "maxLogFiles": MAX_DIAGNOSTIC_LOG_FILES,
            "maxSingleLogBytes": MAX_DIAGNOSTIC_SINGLE_LOG_BYTES,
            "maxTotalLogBytes": MAX_DIAGNOSTIC_TOTAL_LOG_BYTES
        }
    });
    let manifest_path = bundle_dir.join("manifest.json");
    let manifest_bytes = write_json_file(&manifest_path, &manifest)?;
    files += 1;
    bytes += manifest_bytes;

    if options.include_logs {
        let (log_files, log_bytes) = copy_diagnostic_logs(&layout.logs_dir, &bundle_dir)?;
        files += log_files;
        bytes += log_bytes;
    }

    if options.include_settings {
        let (config_files, config_bytes) = copy_diagnostic_config(layout, &bundle_dir)?;
        files += config_files;
        bytes += config_bytes;
    }

    Ok(DiagnosticBundleResultDto {
        path: bundle_dir.display().to_string(),
        files,
        bytes,
    })
}

fn parse_rotated_gui_log_date(file_name: &str) -> Option<NaiveDate> {
    let suffix = file_name.strip_prefix(&format!("{GUI_LOG_FILE_PREFIX}."))?;
    let date = suffix.get(0..10)?;
    NaiveDate::parse_from_str(date, "%Y-%m-%d").ok()
}

fn unique_bundle_dir(root: &Path) -> PathBuf {
    let base = format!(
        "remote-code-diagnostics-{}",
        Utc::now().format("%Y%m%d-%H%M%S")
    );
    let first = root.join(&base);
    if !first.exists() {
        return first;
    }

    for suffix in 1..1000 {
        let candidate = root.join(format!("{base}-{suffix}"));
        if !candidate.exists() {
            return candidate;
        }
    }

    root.join(format!("{base}-{}", Utc::now().timestamp_millis()))
}

fn copy_diagnostic_logs(log_dir: &Path, bundle_dir: &Path) -> Result<(usize, u64)> {
    if !log_dir.exists() {
        return Ok((0, 0));
    }

    let target_dir = bundle_dir.join("logs");
    fs::create_dir_all(&target_dir)
        .with_context(|| format!("failed to create {}", target_dir.display()))?;

    let mut candidates = Vec::new();
    for entry in
        fs::read_dir(log_dir).with_context(|| format!("failed to read {}", log_dir.display()))?
    {
        let entry = entry?;
        let metadata = entry.metadata()?;
        if !metadata.is_file() {
            continue;
        }
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        if !file_name.starts_with(GUI_LOG_FILE_PREFIX) {
            continue;
        }
        let modified = metadata.modified().ok();
        candidates.push((
            entry.path(),
            file_name.into_owned(),
            metadata.len(),
            modified,
        ));
    }

    candidates.sort_by(|left, right| right.3.cmp(&left.3).then_with(|| right.1.cmp(&left.1)));

    let mut files = 0usize;
    let mut bytes = 0u64;
    for (path, file_name, size, _) in candidates.into_iter().take(MAX_DIAGNOSTIC_LOG_FILES) {
        if bytes >= MAX_DIAGNOSTIC_TOTAL_LOG_BYTES {
            break;
        }
        let remaining = MAX_DIAGNOSTIC_TOTAL_LOG_BYTES - bytes;
        let limit = MAX_DIAGNOSTIC_SINGLE_LOG_BYTES.min(remaining);
        if limit == 0 {
            break;
        }
        let copied = copy_tail_bounded(&path, &target_dir.join(file_name), size, limit)
            .with_context(|| format!("failed to copy {}", path.display()))?;
        files += 1;
        bytes += copied;
    }

    Ok((files, bytes))
}

fn copy_diagnostic_config(layout: &RuntimePathLayout, bundle_dir: &Path) -> Result<(usize, u64)> {
    let target_dir = bundle_dir.join("config");
    fs::create_dir_all(&target_dir)
        .with_context(|| format!("failed to create {}", target_dir.display()))?;

    let mut files = 0usize;
    let mut bytes = 0u64;
    for path in [
        &layout.gui_settings_file,
        &layout.gui_projects_file,
        &layout.gui_providers_file,
        &layout.remote_control_file,
    ] {
        if !path.exists() {
            continue;
        }
        let Some(file_name) = path.file_name() else {
            continue;
        };
        let written = copy_redacted_json(path, &target_dir.join(file_name))
            .with_context(|| format!("failed to copy {}", path.display()))?;
        files += 1;
        bytes += written;
    }

    Ok((files, bytes))
}

fn copy_tail_bounded(src: &Path, dst: &Path, size: u64, limit: u64) -> Result<u64> {
    let mut input = fs::File::open(src)?;
    let mut output = fs::File::create(dst)?;

    if size > limit {
        input.seek(SeekFrom::Start(size - limit))?;
        output.write_all(b"[remote-code diagnostic export truncated older log bytes]\n")?;
    }

    let mut limited = input.take(limit);
    let copied = std::io::copy(&mut limited, &mut output)?;
    output.flush()?;
    Ok(copied)
}

fn copy_redacted_json(src: &Path, dst: &Path) -> Result<u64> {
    let contents = fs::read_to_string(src)?;
    let mut value: Value = serde_json::from_str(&contents).unwrap_or_else(|_| {
        json!({
            "unparsedFile": src.display().to_string(),
            "contents": sanitize_log_field(&contents)
        })
    });
    redact_json_secrets(&mut value);
    write_json_file(dst, &value)
}

fn write_json_file(path: &Path, value: &Value) -> Result<u64> {
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(path, &bytes)?;
    Ok(u64::try_from(bytes.len()).unwrap_or(u64::MAX))
}

fn redact_json_secrets(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (key, nested) in map.iter_mut() {
                if is_sensitive_key(key) {
                    *nested = Value::String("[redacted]".to_owned());
                } else {
                    redact_json_secrets(nested);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                redact_json_secrets(item);
            }
        }
        _ => {}
    }
}

fn is_sensitive_line(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    [
        "api_key",
        "apikey",
        "authorization",
        "password",
        "secret",
        "token",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn is_sensitive_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    [
        "api_key",
        "apikey",
        "authorization",
        "password",
        "secret",
        "token",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_owned();
    }

    let mut truncated = value.chars().take(max_chars).collect::<String>();
    truncated.push_str("\n[truncated]");
    truncated
}
