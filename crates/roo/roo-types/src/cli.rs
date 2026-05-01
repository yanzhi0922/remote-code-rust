//! CLI type definitions.
//!
//! Derived from `packages/types/src/cli.ts`.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// CLI command names
// ---------------------------------------------------------------------------

/// CLI stdin command names.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RooCliCommandName {
    Start,
    Message,
    Cancel,
    Ping,
    Shutdown,
}

/// Base fields shared by all CLI commands.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RooCliCommandBase {
    pub command: RooCliCommandName,
    pub request_id: String,
}

// ---------------------------------------------------------------------------
// Individual command types
// ---------------------------------------------------------------------------

/// `start` command payload.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RooCliStartCommand {
    pub command: RooCliCommandName,
    pub request_id: String,
    pub prompt: String,
    pub task_id: Option<String>,
    pub images: Option<Vec<String>>,
    pub configuration: Option<serde_json::Value>,
}

/// `message` command payload.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RooCliMessageCommand {
    pub command: RooCliCommandName,
    pub request_id: String,
    pub prompt: String,
    pub images: Option<Vec<String>>,
}

/// `cancel` command payload.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RooCliCancelCommand {
    pub command: RooCliCommandName,
    pub request_id: String,
}

/// `ping` command payload.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RooCliPingCommand {
    pub command: RooCliCommandName,
    pub request_id: String,
}

/// `shutdown` command payload.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RooCliShutdownCommand {
    pub command: RooCliCommandName,
    pub request_id: String,
}

/// Tagged union of all CLI input commands.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "lowercase")]
pub enum RooCliInputCommand {
    Start(RooCliStartCommand),
    Message(RooCliMessageCommand),
    Cancel(RooCliCancelCommand),
    Ping(RooCliPingCommand),
    Shutdown(RooCliShutdownCommand),
}

// ---------------------------------------------------------------------------
// Output format
// ---------------------------------------------------------------------------

/// CLI output format.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RooCliOutputFormat {
    Text,
    Json,
    #[serde(rename = "stream-json")]
    StreamJson,
}

// ---------------------------------------------------------------------------
// Stream event types
// ---------------------------------------------------------------------------

/// CLI stream event type discriminator.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RooCliEventType {
    System,
    Control,
    Queue,
    Assistant,
    User,
    ToolUse,
    ToolResult,
    Thinking,
    Error,
    Result,
}

/// Control event subtype.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RooCliControlSubtype {
    Ack,
    Done,
    Error,
}

/// Queue item in a queue event.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RooCliQueueItem {
    pub id: String,
    pub text: Option<String>,
    pub image_count: Option<u32>,
    pub timestamp: Option<f64>,
}

/// Tool use information in a tool_use event.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RooCliToolUse {
    pub name: String,
    pub input: Option<serde_json::Value>,
}

/// Tool result information in a tool_result event.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RooCliToolResult {
    pub name: String,
    pub output: Option<String>,
    pub error: Option<String>,
    pub exit_code: Option<i32>,
}

/// Cost breakdown in a CLI event.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RooCliCost {
    pub total_cost: Option<f64>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_writes: Option<u64>,
    pub cache_reads: Option<u64>,
}

/// A single event in the CLI stream-json output.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RooCliStreamEvent {
    #[serde(rename = "type")]
    pub event_type: Option<RooCliEventType>,
    pub subtype: Option<String>,
    pub request_id: Option<String>,
    pub command: Option<RooCliCommandName>,
    pub task_id: Option<String>,
    pub code: Option<String>,
    pub content: Option<String>,
    pub success: Option<bool>,
    pub id: Option<u64>,
    pub done: Option<bool>,
    pub queue_depth: Option<u32>,
    pub queue: Option<Vec<RooCliQueueItem>>,
    pub schema_version: Option<u32>,
    pub protocol: Option<String>,
    pub capabilities: Option<Vec<String>>,
    pub tool_use: Option<RooCliToolUse>,
    pub tool_result: Option<RooCliToolResult>,
    pub cost: Option<RooCliCost>,
}

/// Control event (ack / done / error).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RooCliControlEvent {
    #[serde(rename = "type")]
    pub event_type: RooCliEventType,
    pub subtype: RooCliControlSubtype,
    pub request_id: String,
}

/// Final output of a CLI task.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RooCliFinalOutput {
    #[serde(rename = "type")]
    pub event_type: RooCliEventType,
    pub success: bool,
    pub content: Option<String>,
    pub cost: Option<RooCliCost>,
    pub events: Vec<RooCliStreamEvent>,
}
