//! # rc-agent-protocol
//!
//! Unified Agent protocol layer for the multi-agent architecture.
//!
//! This crate defines the common types, events, and traits that all Agent
//! adapters must implement, enabling seamless integration of:
//! - **Remote Claude** (in-process)
//! - **Remote Roo** (in-process)
//! - **Remote Codex** (in-process)
//!
//! ## Core modules (always available)
//!
//! - [`adapter`] / [`AgentAdapter`] — trait every adapter implements
//! - [`events`] / [`UnifiedAgentEvent`] — unified event type
//! - [`bridge`] — UnifiedEvent → RuntimeEventDetail conversion
//! - [`types`] — AgentConfig, AgentInfo, AgentStatus, etc.
//! - [`permission`] — PermissionRequest / PermissionDecision
//! - [`error`] — error types
//! - [`util`] — shared helpers (standard_capabilities, panic_to_error_event)
//!
//! ## Test-only modules (gated behind `test-helpers` feature)
//!
//! These are used by integration tests and are not included in production builds:
//! - [`adapters`] — InProcessAdapter and type aliases for test doubles
//! - [`health`] — HealthChecker for periodic agent health probes
//! - [`restart`] — RestartTracker for restart policy
//! - [`router`] — AgentRouter for session-to-adapter routing
//! - [`from_engine`] — direct EngineEvent → UnifiedAgentEvent conversion

// ── Core modules (always compiled) ──────────────────────────────────────
pub mod adapter;
pub mod bridge;
pub mod error;
pub mod events;
pub mod permission;
pub mod types;
pub mod util;

// ── Test-only modules ───────────────────────────────────────────────────
#[cfg(feature = "test-helpers")]
pub mod adapters;
#[cfg(feature = "test-helpers")]
pub mod from_engine;
#[cfg(feature = "test-helpers")]
pub mod health;
#[cfg(feature = "test-helpers")]
pub mod restart;
#[cfg(feature = "test-helpers")]
pub mod router;

// ── Re-exports ──────────────────────────────────────────────────────────
pub use adapter::AgentAdapter;
pub use bridge::unified_event_to_runtime_detail;
pub use error::{AdapterError, AgentProtocolError};
pub use events::{AgentResult, ToolCallInfo, UnifiedAgentEvent, UsageInfo};
pub use permission::{PermissionDecision, PermissionRequest};
pub use types::{AgentCapability, AgentConfig, AgentInfo, AgentStatus, AgentType};

#[cfg(feature = "test-helpers")]
pub use adapters::{InProcessAdapter, RemoteClaudeAdapter, RemoteCodexAdapter, RemoteRooAdapter};
#[cfg(feature = "test-helpers")]
pub use from_engine::engine_event_to_unified;
#[cfg(feature = "test-helpers")]
pub use health::HealthChecker;
#[cfg(feature = "test-helpers")]
pub use restart::RestartTracker;
#[cfg(feature = "test-helpers")]
pub use router::AgentRouter;
