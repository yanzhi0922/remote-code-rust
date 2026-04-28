//! Stub service module — provides type definitions without V8 runtime.
//!
//! When V8 is available, replace this with the original source from
//! `agents/codex/codex-rs/code-mode/src/service.rs`.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value as JsonValue;
use tokio_util::sync::CancellationToken;

use crate::runtime::CodeModeNestedToolCall;

#[async_trait]
pub trait CodeModeTurnHost: Send + Sync {
    async fn invoke_tool(
        &self,
        invocation: CodeModeNestedToolCall,
        cancellation_token: CancellationToken,
    ) -> Result<JsonValue, String>;

    async fn notify(&self, call_id: String, cell_id: String, text: String) -> Result<(), String>;
}

/// Stub — code mode service without V8 runtime.
///
/// All methods return errors indicating V8 is not available.
pub struct CodeModeService;

impl CodeModeService {
    pub fn new() -> Self {
        Self
    }

    pub async fn execute(
        &self,
        _request: crate::runtime::ExecuteRequest,
    ) -> Result<crate::runtime::RuntimeResponse, String> {
        Err("codex-code-mode: V8 runtime not available (stub)".to_string())
    }

    pub async fn wait(
        &self,
        _request: crate::runtime::WaitRequest,
    ) -> Result<crate::runtime::WaitOutcome, String> {
        Err("codex-code-mode: V8 runtime not available (stub)".to_string())
    }

    pub fn start_turn_worker(
        &self,
        _host: Arc<dyn CodeModeTurnHost>,
    ) -> CodeModeTurnWorker {
        CodeModeTurnWorker
    }

    pub fn allocate_cell_id(&self) -> String {
        "stub".to_string()
    }

    pub async fn stored_values(&self) -> std::collections::HashMap<String, JsonValue> {
        std::collections::HashMap::new()
    }

    pub async fn replace_stored_values(
        &self,
        _values: std::collections::HashMap<String, JsonValue>,
    ) {
    }
}

/// Stub turn worker — no-op without V8.
pub struct CodeModeTurnWorker;
