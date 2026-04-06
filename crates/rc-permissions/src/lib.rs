use async_trait::async_trait;
use rc_core::PermissionMode;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionClass {
    Read,
    Edit,
    Command,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRequest {
    pub tool_name: String,
    pub tool_use_id: String,
    pub title: String,
    pub description: String,
    pub input: Value,
    pub blocked_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionDecision {
    pub allowed: bool,
    pub message: Option<String>,
}

impl PermissionDecision {
    pub fn allow() -> Self {
        Self {
            allowed: true,
            message: None,
        }
    }

    pub fn deny(message: impl Into<String>) -> Self {
        Self {
            allowed: false,
            message: Some(message.into()),
        }
    }
}

#[async_trait]
pub trait PermissionBroker: Send + Sync {
    fn mode(&self) -> PermissionMode;

    async fn decide(&self, request: PermissionRequest) -> PermissionDecision;
}

#[derive(Debug, Clone)]
pub struct StaticPermissionBroker {
    mode: PermissionMode,
}

impl StaticPermissionBroker {
    pub fn new(mode: PermissionMode) -> Self {
        Self { mode }
    }
}

#[async_trait]
impl PermissionBroker for StaticPermissionBroker {
    fn mode(&self) -> PermissionMode {
        self.mode
    }

    async fn decide(&self, request: PermissionRequest) -> PermissionDecision {
        if auto_allows(self.mode, classify_tool(&request.tool_name)) {
            PermissionDecision::allow()
        } else {
            PermissionDecision::deny(format!(
                "Permission mode {} denied {}.",
                self.mode.as_legacy_str(),
                request.tool_name
            ))
        }
    }
}

pub fn classify_tool(name: &str) -> PermissionClass {
    match name {
        "list_directory" | "read_file" | "search_text" => PermissionClass::Read,
        "write_file" | "replace_in_file" | "edit_file" => PermissionClass::Edit,
        _ => PermissionClass::Command,
    }
}

pub fn auto_allows(mode: PermissionMode, class: PermissionClass) -> bool {
    match mode {
        PermissionMode::BypassPermissions => true,
        PermissionMode::AcceptEdits => !matches!(class, PermissionClass::Command),
        PermissionMode::Default => matches!(class, PermissionClass::Read),
        PermissionMode::DontAsk | PermissionMode::Plan => matches!(class, PermissionClass::Read),
    }
}
