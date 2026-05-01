//! `rc-ide` — IDE integration bridge for VS Code, JetBrains, Neovim and other editors.
//!
//! This crate provides the bridge layer between remote-code and IDEs, handling
//! configuration detection, messaging protocols, path conversion, and connection
//! lifecycle management.
//!
//! # Overview
//!
//! - **[`config`]** — IDE type detection and configuration
//! - **[`bridge`]** — Main bridge between remote-code and the IDE
//! - **[`messaging`]** — Bridge messaging protocol with JSON serialization
//! - **[`path_conversion`]** — Path conversion between IDE and remote-code formats
//! - **[`connection`]** — Connection management with stdio/HTTP transports
//!
//! # Example
//!
//! ```rust
//! use claude_ide::config::{IdeConfig, IdeType, ConnectionMode, detect_ide};
//!
//! let ide_type = detect_ide();
//! let config = IdeConfig::new(ide_type, ConnectionMode::Stdio);
//! println!("Detected IDE: {:?}", config.ide_type);
//! ```

pub mod bridge;
pub mod bridge_config;
pub mod bridge_messaging;
pub mod bridge_status;
pub mod config;
pub mod connection;
pub mod messaging;
pub mod path_conversion;

pub use bridge::{IdeAction, IdeBridge, IdeNotification, IdeResponse};
pub use bridge_config::{BridgeConfig, BridgeTransport, default_bridge_config, load_bridge_config};
pub use bridge_messaging::{
    BridgeMessage, BridgeMessageType, deserialize_bridge_message, serialize_bridge_message,
};
pub use bridge_status::{BridgeConnectionInfo, BridgeStatus, format_bridge_status};
pub use config::{ConnectionMode, IdeConfig, IdeType, detect_ide};
pub use connection::{HttpConnection, IdeConnection, IdeStatus, StdioConnection};
