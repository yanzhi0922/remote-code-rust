mod mobile;

// Shared modules always compiled.
mod quic_bridge;

// Desktop-only code (codex, deno, v8, claude-core, claude-provider, etc.).
#[cfg(feature = "desktop")]
mod desktop;
#[cfg(feature = "desktop")]
pub(crate) mod dto;
#[cfg(feature = "desktop")]
mod query_engine_gui;
#[cfg(feature = "desktop")]
pub mod remote_runner;
#[cfg(feature = "desktop")]
pub(crate) mod state;

#[cfg(feature = "desktop")]
pub use desktop::run;
// Re-export desktop items at crate root so sibling modules can find them.

// Mobile entry point — minimal dependencies, no heavy crates.
#[cfg(feature = "mobile")]
mod mobile_entry;
#[cfg(feature = "mobile")]
pub use mobile_entry::run;
