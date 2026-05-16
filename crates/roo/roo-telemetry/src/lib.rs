#![allow(dead_code)]
#![allow(clippy::all)]
#![deny(clippy::dbg_macro, clippy::todo)]
//! Roo-telemetry: Telemetry service for Roo Code.

pub mod client;
pub mod posthog_client;
pub mod service;
pub mod types;

pub use client::{BaseTelemetryClient, TelemetryClient, TelemetryClientError};
pub use posthog_client::PostHogTelemetryClient;
pub use service::TelemetryService;
pub use types::{
    SubscriptionType, TelemetryEvent, TelemetryEventName, TelemetryEventSubscription,
    TelemetrySetting,
};
