//! `rc-analytics` — Analytics system for event logging and export.
//!
//! This crate provides an analytics pipeline matching Claude Code's
//! `services/analytics/` module, including event logging, feature flags,
//! event sinks, and exporters.
//!
//! # Overview
//!
//! - **[`config`]** — Analytics configuration with sensible defaults
//! - **[`metadata`]** — Event metadata types and builder
//! - **[`sink`]** — Event sink trait and implementations (queued, composite, null)
//! - **[`event_logger`]** — Main analytics service
//! - **[`growthbook`]** — Feature flag system
//! - **[`exporter`]** — Event exporter backends (Datadog, 1P, File)
//!
//! # Example
//!
//! ```rust,ignore
//! use rc_analytics::{AnalyticsService, AnalyticsConfig};
//!
//! let config = AnalyticsConfig::default();
//! let service = AnalyticsService::new(config);
//! service.log_tool_use("read_file", 150, true);
//! ```

pub mod config;
pub mod event_logger;
pub mod exporter;
pub mod growthbook;
pub mod metadata;
pub mod sink;

pub use config::AnalyticsConfig;
pub use event_logger::AnalyticsService;
pub use metadata::AnalyticsEvent;
