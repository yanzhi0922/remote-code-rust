//! `rc-team-memory` — Team memory sync system.
//!
//! Provides session-scoped and team-scoped memory storage, synchronization,
//! secret scanning for memory safety, and file watching for memory persistence.
//!
//! # Overview
//!
//! - **[`types`]** — Core memory types (`MemoryEntry`, `MemoryType`, `SyncStatus`)
//! - **[`session_memory`]** — In-memory session store with memory extraction
//! - **[`team_sync`]** — Team memory synchronization with conflict resolution
//! - **[`secret_scanner`]** — Regex-based secret detection for safe memory storage
//! - **[`watcher`]** — File change watcher for memory persistence
//!
//! # Example
//!
//! ```rust,ignore
//! use rc_team_memory::SessionMemoryStore;
//! use rc_team_memory::TeamMemoryService;
//! use rc_team_memory::SecretScanner;
//!
//! let store = SessionMemoryStore::new();
//! store.store("session-1", "User prefers dark mode").expect("store");
//!
//! let scanner = SecretScanner::new();
//! assert!(scanner.is_safe("User prefers dark mode"));
//! ```

pub mod secret_scanner;
pub mod session_memory;
pub mod team_sync;
pub mod types;
pub mod watcher;

pub use secret_scanner::{SecretScanResult, SecretScanner};
pub use session_memory::SessionMemoryStore;
pub use team_sync::TeamMemoryService;
