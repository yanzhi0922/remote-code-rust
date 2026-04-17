//! IDE configuration and detection.
//!
//! Detects the current IDE from environment variables and provides
//! configuration types for the bridge connection.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Supported IDE types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IdeType {
    /// Visual Studio Code.
    VsCode,
    /// JetBrains IDEs (IntelliJ, WebStorm, etc.).
    JetBrains,
    /// Neovim.
    Neovim,
    /// Unknown / unsupported IDE.
    Unknown,
}

impl std::fmt::Display for IdeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IdeType::VsCode => write!(f, "vscode"),
            IdeType::JetBrains => write!(f, "jetbrains"),
            IdeType::Neovim => write!(f, "neovim"),
            IdeType::Unknown => write!(f, "unknown"),
        }
    }
}

/// Connection mode between remote-code and the IDE.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConnectionMode {
    /// Standard I/O (stdin/stdout) communication.
    Stdio,
    /// HTTP-based communication.
    Http,
    /// WebSocket-based communication.
    WebSocket,
}

impl std::fmt::Display for ConnectionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectionMode::Stdio => write!(f, "stdio"),
            ConnectionMode::Http => write!(f, "http"),
            ConnectionMode::WebSocket => write!(f, "websocket"),
        }
    }
}

/// IDE bridge configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdeConfig {
    /// The type of IDE to connect to.
    pub ide_type: IdeType,
    /// How to communicate with the IDE.
    pub connection_mode: ConnectionMode,
    /// Optional endpoint URL (for HTTP/WebSocket modes).
    pub endpoint: Option<String>,
}

impl IdeConfig {
    /// Create a new configuration.
    pub fn new(ide_type: IdeType, connection_mode: ConnectionMode) -> Self {
        Self {
            ide_type,
            connection_mode,
            endpoint: None,
        }
    }

    /// Create a configuration with an explicit endpoint.
    pub fn with_endpoint(ide_type: IdeType, connection_mode: ConnectionMode, endpoint: String) -> Self {
        Self {
            ide_type,
            connection_mode,
            endpoint: Some(endpoint),
        }
    }

    /// Auto-detect the IDE and create a default configuration.
    pub fn auto_detect() -> Self {
        let ide_type = detect_ide();
        let mode = default_connection_mode(ide_type);
        Self::new(ide_type, mode)
    }
}

// ---------------------------------------------------------------------------
// Detection
// ---------------------------------------------------------------------------

/// Detect the current IDE from environment variables.
///
/// Checks the following env vars in order:
/// - `VSCODE_IPC_HOOK` → VS Code
/// - `TERM_PROGRAM` containing "vscode" → VS Code
/// - `JETBRAINS_IDE` or `IDEA_INITIAL_DIRECTORY` → JetBrains
/// - `NVIM` or `NVIM_LISTEN_ADDRESS` → Neovim
pub fn detect_ide() -> IdeType {
    detect_ide_from_env(|key| std::env::var(key).ok())
}

/// Testable detection that accepts an env-var reader.
///
/// This avoids the need for `unsafe` `set_var`/`remove_var` in tests.
pub fn detect_ide_from_env(get_env: impl Fn(&str) -> Option<String>) -> IdeType {
    // VS Code detection.
    if get_env("VSCODE_IPC_HOOK").is_some() {
        return IdeType::VsCode;
    }
    if let Some(term_program) = get_env("TERM_PROGRAM")
        && term_program.to_lowercase().contains("vscode")
    {
        return IdeType::VsCode;
    }

    // JetBrains detection.
    if get_env("JETBRAINS_IDE").is_some() || get_env("IDEA_INITIAL_DIRECTORY").is_some() {
        return IdeType::JetBrains;
    }

    // Neovim detection.
    if get_env("NVIM").is_some() || get_env("NVIM_LISTEN_ADDRESS").is_some() {
        return IdeType::Neovim;
    }

    IdeType::Unknown
}

/// Return the default connection mode for an IDE type.
pub fn default_connection_mode(ide_type: IdeType) -> ConnectionMode {
    match ide_type {
        IdeType::VsCode => ConnectionMode::Stdio,
        IdeType::JetBrains => ConnectionMode::Http,
        IdeType::Neovim => ConnectionMode::Stdio,
        IdeType::Unknown => ConnectionMode::Stdio,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: env reader that returns `None` for everything.
    fn no_env(_: &str) -> Option<String> {
        None
    }

    /// Helper: env reader with a single key-value pair.
    fn env_with(key: &'static str, val: &'static str) -> impl Fn(&str) -> Option<String> {
        move |k| {
            if k == key {
                Some(val.to_string())
            } else {
                None
            }
        }
    }

    #[test]
    fn ide_type_display() {
        assert_eq!(IdeType::VsCode.to_string(), "vscode");
        assert_eq!(IdeType::JetBrains.to_string(), "jetbrains");
        assert_eq!(IdeType::Neovim.to_string(), "neovim");
        assert_eq!(IdeType::Unknown.to_string(), "unknown");
    }

    #[test]
    fn connection_mode_display() {
        assert_eq!(ConnectionMode::Stdio.to_string(), "stdio");
        assert_eq!(ConnectionMode::Http.to_string(), "http");
        assert_eq!(ConnectionMode::WebSocket.to_string(), "websocket");
    }

    #[test]
    fn ide_type_serde_roundtrip() {
        let ide = IdeType::VsCode;
        let json = serde_json::to_string(&ide).expect("serialize");
        assert_eq!(json, "\"vscode\"");
        let back: IdeType = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, IdeType::VsCode);
    }

    #[test]
    fn connection_mode_serde_roundtrip() {
        let mode = ConnectionMode::WebSocket;
        let json = serde_json::to_string(&mode).expect("serialize");
        assert_eq!(json, "\"websocket\"");
        let back: ConnectionMode = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, ConnectionMode::WebSocket);
    }

    #[test]
    fn config_new() {
        let config = IdeConfig::new(IdeType::Neovim, ConnectionMode::Stdio);
        assert_eq!(config.ide_type, IdeType::Neovim);
        assert_eq!(config.connection_mode, ConnectionMode::Stdio);
        assert!(config.endpoint.is_none());
    }

    #[test]
    fn config_with_endpoint() {
        let config = IdeConfig::with_endpoint(
            IdeType::JetBrains,
            ConnectionMode::Http,
            "http://localhost:8080".to_string(),
        );
        assert_eq!(config.endpoint.as_deref(), Some("http://localhost:8080"));
    }

    #[test]
    fn detect_unknown_when_no_env() {
        assert_eq!(detect_ide_from_env(no_env), IdeType::Unknown);
    }

    #[test]
    fn detect_vscode_from_ipc_hook() {
        assert_eq!(
            detect_ide_from_env(env_with("VSCODE_IPC_HOOK", "/tmp/vscode-ipc")),
            IdeType::VsCode
        );
    }

    #[test]
    fn detect_vscode_from_term_program() {
        let env = |k: &str| -> Option<String> {
            if k == "TERM_PROGRAM" {
                Some("vscode".to_string())
            } else {
                None
            }
        };
        assert_eq!(detect_ide_from_env(env), IdeType::VsCode);
    }

    #[test]
    fn detect_jetbrains() {
        assert_eq!(
            detect_ide_from_env(env_with("JETBRAINS_IDE", "idea")),
            IdeType::JetBrains
        );
    }

    #[test]
    fn detect_jetbrains_from_idea_dir() {
        assert_eq!(
            detect_ide_from_env(env_with("IDEA_INITIAL_DIRECTORY", "/home/user")),
            IdeType::JetBrains
        );
    }

    #[test]
    fn detect_neovim() {
        assert_eq!(
            detect_ide_from_env(env_with("NVIM", "1")),
            IdeType::Neovim
        );
    }

    #[test]
    fn detect_neovim_from_listen_addr() {
        assert_eq!(
            detect_ide_from_env(env_with("NVIM_LISTEN_ADDRESS", "/tmp/nvim")),
            IdeType::Neovim
        );
    }

    #[test]
    fn default_connection_modes() {
        assert_eq!(default_connection_mode(IdeType::VsCode), ConnectionMode::Stdio);
        assert_eq!(default_connection_mode(IdeType::JetBrains), ConnectionMode::Http);
        assert_eq!(default_connection_mode(IdeType::Neovim), ConnectionMode::Stdio);
        assert_eq!(default_connection_mode(IdeType::Unknown), ConnectionMode::Stdio);
    }
}
