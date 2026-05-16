#![allow(dead_code)]
#![allow(clippy::all)]
#![deny(clippy::dbg_macro, clippy::todo)]
pub mod service;
pub mod types;

pub use service::MdmService;
pub use types::{ComplianceResult, MdmConfig, MdmError, MdmPlatform};
