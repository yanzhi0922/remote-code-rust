//! # Roo Provider: AWS Bedrock
//!
//! AWS Bedrock provider for Roo Code Rust.
//! Uses the Bedrock Converse API with SigV4 signing.
//! Supports cross-region inference and custom model IDs.

mod bedrock_events;
mod handler;
mod models;
mod signing;
pub mod tool_schema;
mod types;

pub use handler::{ArnInfo, AwsBedrockHandler, BedrockErrorType, parse_arn};
pub use models::{default_model_id, models};
pub use tool_schema::normalize_tool_schema;
pub use types::{AwsBedrockConfig, apply_service_tier_pricing};
