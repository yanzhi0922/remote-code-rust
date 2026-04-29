//! # Roo Provider: AWS Bedrock
//!
//! AWS Bedrock provider for Roo Code Rust.
//! Uses the Bedrock Converse API with SigV4 signing.
//! Supports cross-region inference and custom model IDs.

mod bedrock_events;
mod handler;
mod models;
mod signing;
mod types;

pub use handler::AwsBedrockHandler;
pub use models::{default_model_id, models};
pub use types::AwsBedrockConfig;
