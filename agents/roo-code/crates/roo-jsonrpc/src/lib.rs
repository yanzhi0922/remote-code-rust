//! # roo-jsonrpc
//!
//! A JSON-RPC 2.0 protocol library for Roo Code.
//!
//! This crate provides types and utilities for working with the JSON-RPC 2.0 protocol,
//! as specified in <https://www.jsonrpc.org/specification>.
//!
//! ## Overview
//!
//! - **Message types**: [`Message`] for unified request/response/notification/error
//! - **Error types**: [`JsonRpcError`] with standard error codes
//! - **Codec**: Serialization/deserialization with Content-Length framing
//! - **ID generation**: Thread-safe [`IdGenerator`]
//! - **Batch support**: Encode/decode batch requests and responses
//! - **Validation**: Message validation against the JSON-RPC 2.0 spec

pub mod batch;
pub mod codec;
pub mod id;
pub mod types;
pub mod validator;

// Re-export primary types
pub use batch::{decode_batch, encode_batch};
pub use codec::{decode_message, encode_message, encode_with_content_length, parse_content_length_header};
pub use id::IdGenerator;
pub use types::{error_codes, Error as JsonRpcError, Id, Message};
pub use validator::validate;
