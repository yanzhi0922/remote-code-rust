//! Error diagnostics generation.
//!
//! Derived from `src/core/webview/diagnosticsHandler.ts`.
//!
//! Generates error diagnostics files containing error metadata and API
//! conversation history for support purposes.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::info;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Error diagnostics metadata values.
///
/// Source: `src/core/webview/diagnosticsHandler.ts` — `ErrorDiagnosticsValues`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorDiagnosticsValues {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

/// Parameters for generating error diagnostics.
///
/// Source: `src/core/webview/diagnosticsHandler.ts` — `GenerateDiagnosticsParams`
pub struct GenerateDiagnosticsParams {
    pub task_id: String,
    pub global_storage_path: PathBuf,
    pub values: Option<ErrorDiagnosticsValues>,
    pub log: Box<dyn Fn(&str) + Send + Sync>,
}

/// Result of generating error diagnostics.
///
/// Source: `src/core/webview/diagnosticsHandler.ts` — `GenerateDiagnosticsResult`
#[derive(Debug, Clone)]
pub struct GenerateDiagnosticsResult {
    pub success: bool,
    pub file_path: Option<PathBuf>,
    pub error: Option<String>,
}

/// The diagnostics file content structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DiagnosticsContent {
    error: DiagnosticsError,
    history: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DiagnosticsError {
    timestamp: String,
    version: String,
    provider: String,
    model: String,
    details: String,
}

// ---------------------------------------------------------------------------
// Diagnostics generation
// ---------------------------------------------------------------------------

/// Generates an error diagnostics file containing error metadata and API
/// conversation history.
///
/// Source: `src/core/webview/diagnosticsHandler.ts` — `generateErrorDiagnostics`
///
/// The file is created in the system temp directory with a descriptive name
/// and includes human-readable guidance comments before the JSON payload.
///
/// # Arguments
/// * `params` - Parameters for diagnostics generation
///
/// # Returns
/// A `GenerateDiagnosticsResult` indicating success or failure.
pub async fn generate_error_diagnostics(
    params: GenerateDiagnosticsParams,
) -> GenerateDiagnosticsResult {
    let GenerateDiagnosticsParams {
        task_id,
        global_storage_path,
        values,
        log,
    } = params;

    let log = log;

    match generate_diagnostics_inner(&task_id, &global_storage_path, &values).await {
        Ok(path) => GenerateDiagnosticsResult {
            success: true,
            file_path: Some(path),
            error: None,
        },
        Err(e) => {
            let error_msg = e.to_string();
            log(&format!("Error generating diagnostics: {error_msg}"));
            GenerateDiagnosticsResult {
                success: false,
                file_path: None,
                error: Some(error_msg),
            }
        }
    }
}

async fn generate_diagnostics_inner(
    task_id: &str,
    global_storage_path: &Path,
    values: &Option<ErrorDiagnosticsValues>,
) -> std::io::Result<PathBuf> {
    // Construct task directory path
    let task_dir = global_storage_path.join("tasks").join(task_id);

    // Load API conversation history
    let api_history_path = task_dir.join("api_conversation_history.json");
    let history: Value = if api_history_path.exists() {
        let content = tokio::fs::read_to_string(&api_history_path).await?;
        serde_json::from_str(&content).unwrap_or(Value::Array(vec![]))
    } else {
        Value::Array(vec![])
    };

    let diagnostics = DiagnosticsContent {
        error: DiagnosticsError {
            timestamp: values
                .as_ref()
                .and_then(|v| v.timestamp.clone())
                .unwrap_or_else(|| chrono::Utc::now().to_rfc3339()),
            version: values
                .as_ref()
                .and_then(|v| v.version.clone())
                .unwrap_or_default(),
            provider: values
                .as_ref()
                .and_then(|v| v.provider.clone())
                .unwrap_or_default(),
            model: values
                .as_ref()
                .and_then(|v| v.model.clone())
                .unwrap_or_default(),
            details: values
                .as_ref()
                .and_then(|v| v.details.clone())
                .unwrap_or_default(),
        },
        history,
    };

    // Create the full content with header comments
    let header_comment = "// Please share this file with Roo Code Support (support@roocode.com) to diagnose the issue faster\n\
                          // Just make sure you're OK sharing the contents of the conversation below.\n\n";
    let json_content = serde_json::to_string_pretty(&diagnostics)?;
    let full_content = format!("{header_comment}{json_content}");

    // Create a temporary diagnostics file
    let tmp_dir = std::env::temp_dir();
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let task_prefix = if task_id.len() >= 8 {
        &task_id[..8]
    } else {
        task_id
    };
    let temp_file_name = format!("roo-diagnostics-{task_prefix}-{timestamp}.json");
    let temp_file_path = tmp_dir.join(&temp_file_name);

    tokio::fs::write(&temp_file_path, full_content).await?;

    info!("Generated diagnostics file at: {}", temp_file_path.display());
    Ok(temp_file_path)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_diagnostics_values_serialization() {
        let values = ErrorDiagnosticsValues {
            timestamp: Some("2024-01-01T00:00:00Z".to_string()),
            version: Some("1.0.0".to_string()),
            provider: Some("openai".to_string()),
            model: Some("gpt-4".to_string()),
            details: Some("test error".to_string()),
        };
        let json = serde_json::to_string(&values).unwrap();
        assert!(json.contains("openai"));
        assert!(json.contains("gpt-4"));
    }

    #[test]
    fn test_error_diagnostics_values_optional_fields() {
        let values = ErrorDiagnosticsValues {
            timestamp: None,
            version: None,
            provider: None,
            model: None,
            details: None,
        };
        let json = serde_json::to_string(&values).unwrap();
        // Optional fields should be skipped
        assert!(!json.contains("timestamp"));
    }

    #[tokio::test]
    async fn test_generate_diagnostics_nonexistent_task() {
        let result = generate_error_diagnostics(GenerateDiagnosticsParams {
            task_id: "nonexistent-task".to_string(),
            global_storage_path: PathBuf::from("/tmp/test-storage"),
            values: None,
            log: Box::new(|_| {}),
        }).await;
        // Should succeed even with nonexistent task (creates empty history)
        assert!(result.success);
        assert!(result.file_path.is_some());
        // Clean up temp file
        if let Some(path) = result.file_path {
            let _ = tokio::fs::remove_file(path).await;
        }
    }
}
