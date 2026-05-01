//! # Roo Server — JSON-RPC Server for Roo Code
//!
//! A JSON-RPC 2.0 server that provides IPC communication for Roo Code.
//!
//! ## Architecture
//!
//! The server wraps the [`roo_app::App`] application layer and exposes its
//! functionality through JSON-RPC 2.0 methods. This enables communication
//! between a client (CLI, TUI, or WebView) and the Roo Code backend.
//!
//! ## Source Mapping
//!
//! - TypeScript: `src/core/webview/webviewMessageHandler.ts` — WebviewMessage handler
//! - TypeScript: `packages/types/src/ipc.ts` — IPC message types
//! - TypeScript: `packages/types/src/vscode-extension-host.ts` — ExtensionMessage/WebviewMessage
//!
//! ## JSON-RPC Method Mapping
//!
//! | JSON-RPC Method           | TypeScript Equivalent                    |
//! |---------------------------|------------------------------------------|
//! | `initialize`              | `webviewDidLaunch`                       |
//! | `shutdown`                | `dispose`                                |
//! | `task/start`              | `newTask` / `StartNewTask`              |
//! | `task/cancel`             | `cancelTask` / `CancelTask`             |
//! | `task/close`              | `clearTask` / `CloseTask`               |
//! | `task/resume`             | `ResumeTask`                             |
//! | `task/sendMessage`        | `SendMessage`                            |
//! | `task/getCommands`        | `GetCommands`                            |
//! | `task/getModes`           | `GetModes`                               |
//! | `task/getModels`          | `GetModels`                              |
//! | `state/get`               | `getState`                               |
//! | `state/setMode`           | `mode`                                   |
//! | `systemPrompt/build`      | `getSystemPrompt`                        |
//!
//! ## Usage
//!
//! ```rust,ignore
//! use roo_server::Server;
//! use roo_app::{App, AppConfig};
//!
//! let app = App::new(AppConfig::default());
//! let server = Server::new(app);
//! server.serve_stdio().await?;
//! ```

pub mod aggregate_task_costs;
pub mod checkpoint_restore_handler;
pub mod diagnostics_handler;
pub mod error;
pub mod generate_system_prompt;
pub mod handler;
pub mod message_enhancer;
pub mod router;
pub mod server;
pub mod skills_message_handler;
pub mod transport;
pub mod webview_message_handler;

pub use error::ServerResult;
pub use router::Router;
pub use server::Server;
pub use transport::Transport;

use roo_provider::{Provider, register_provider};
use roo_types::api::ProviderName;

/// Register all built-in provider factories.
///
/// Must be called once during application startup, before any
/// [`roo_provider::build_api_handler`] calls.
///
/// Each provider crate's `Handler::from_settings()` is wrapped in a
/// factory closure and registered under its [`ProviderName`] variant.
pub fn register_providers() {
    // Anthropic
    register_provider(ProviderName::Anthropic, |settings| {
        roo_provider_anthropic::AnthropicHandler::from_settings(settings)
            .map(|h| Box::new(h) as Box<dyn Provider>)
    });

    // OpenAI
    register_provider(ProviderName::Openai, |settings| {
        roo_provider_openai::OpenAiHandler::from_settings(settings)
            .map(|h| Box::new(h) as Box<dyn Provider>)
    });

    // MiniMax (uses Anthropic protocol)
    register_provider(ProviderName::MiniMax, |settings| {
        roo_provider_minimax::MiniMaxHandler::from_settings(settings)
            .map(|h| Box::new(h) as Box<dyn Provider>)
    });

    // DeepSeek
    register_provider(ProviderName::DeepSeek, |settings| {
        roo_provider_deepseek::DeepSeekHandler::from_settings(settings)
            .map(|h| Box::new(h) as Box<dyn Provider>)
    });

    // OpenRouter
    register_provider(ProviderName::OpenRouter, |settings| {
        roo_provider_openrouter::OpenRouterHandler::from_settings(settings)
            .map(|h| Box::new(h) as Box<dyn Provider>)
    });

    // Google Gemini
    register_provider(ProviderName::Gemini, |settings| {
        roo_provider_google::GoogleHandler::from_settings(settings)
            .map(|h| Box::new(h) as Box<dyn Provider>)
    });

    // Ollama
    register_provider(ProviderName::Ollama, |settings| {
        roo_provider_ollama::OllamaHandler::from_settings(settings)
            .map(|h| Box::new(h) as Box<dyn Provider>)
    });
}
