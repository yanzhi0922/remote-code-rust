//! `rc-managed-settings` — Remote managed settings system.
//!
//! Provides a settings management framework with sync caching, security
//! validation, conflict resolution, and MDM (Mobile Device Management)
//! enterprise profile support.
//!
//! # Overview
//!
//! - **[`types`]** — Core types (`ManagedSetting`, `SettingSource`, `SettingsPolicy`)
//! - **[`sync_cache`]** — TTL-based settings cache
//! - **[`security_check`]** — Security validation for setting changes
//! - **[`sync_engine`]** — Settings synchronization with conflict resolution
//! - **[`mdm`]** — MDM enterprise profile loading and enforcement
//!
//! # Example
//!
//! ```rust,ignore
//! use rc_managed_settings::ManagedSettingsService;
//! use rc_managed_settings::types::{ManagedSetting, SettingSource};
//!
//! let svc = ManagedSettingsService::new();
//! let setting = ManagedSetting::new("editor.tab_size", json!(4), SettingSource::User);
//! svc.apply_settings(&[setting]).expect("apply");
//! ```

pub mod mdm;
pub mod security_check;
pub mod sync_cache;
pub mod sync_engine;
pub mod types;

pub use security_check::{RiskLevel, SecurityCheckResult, SecurityChecker};
pub use sync_cache::{SyncCache, SyncCacheState};
pub use sync_engine::ManagedSettingsService;
