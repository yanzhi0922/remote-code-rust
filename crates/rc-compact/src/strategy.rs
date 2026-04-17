//! Compact strategy trait and core types.
//!
//! Defines the [`CompactStrategy`] trait that all compaction strategies implement,
//! along with shared configuration ([`CompactOptions`]), progress events
//! ([`CompactProgressEvent`]), and the result type ([`CompactionResult`]).

use std::fmt;

use rc_core::Message;

// ---------------------------------------------------------------------------
// Strategy type enum
// ---------------------------------------------------------------------------

/// Identifies which compaction strategy was used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompactStrategyType {
    /// Full conversation compaction — summarize everything, keep recent tail.
    Full,
    /// Partial compaction around a pivot message.
    Partial,
    /// Automatic compaction triggered when token usage exceeds a threshold.
    Auto,
    /// Micro compaction via cache editing (clear old tool results).
    Micro,
    /// Snip compaction — trim oversized tool outputs.
    Snip,
    /// Reactive compaction — triggered by API prompt-too-long errors.
    Reactive,
    /// Session-memory compaction — preserve key facts, compress the rest.
    SessionMemory,
}

impl fmt::Display for CompactStrategyType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Full => write!(f, "full"),
            Self::Partial => write!(f, "partial"),
            Self::Auto => write!(f, "auto"),
            Self::Micro => write!(f, "micro"),
            Self::Snip => write!(f, "snip"),
            Self::Reactive => write!(f, "reactive"),
            Self::SessionMemory => write!(f, "session_memory"),
        }
    }
}

// ---------------------------------------------------------------------------
// Preserved segment metadata
// ---------------------------------------------------------------------------

/// Describes a range of messages that survived compaction.
#[derive(Debug, Clone)]
pub struct PreservedSegment {
    /// UUID of the first preserved message.
    pub head_uuid: uuid::Uuid,
    /// UUID of the message immediately preceding the preserved range.
    pub anchor_uuid: uuid::Uuid,
    /// UUID of the last preserved message.
    pub tail_uuid: uuid::Uuid,
}

// ---------------------------------------------------------------------------
// Compaction result
// ---------------------------------------------------------------------------

/// Result returned by every compaction strategy.
#[derive(Debug, Clone)]
pub struct CompactionResult {
    /// The LLM-generated summary text (after formatting).
    pub summary: String,
    /// How many messages were removed / replaced by the summary.
    pub messages_removed: usize,
    /// Approximate number of tokens saved by the compaction.
    pub tokens_saved: u64,
    /// Which strategy produced this result.
    pub strategy_used: CompactStrategyType,
    /// Segments of the original conversation that were preserved verbatim.
    pub preserved_segments: Vec<PreservedSegment>,
    /// Token count before compaction.
    pub pre_compact_token_count: Option<u64>,
    /// Token count after compaction (estimated from the result payload).
    pub post_compact_token_count: Option<u64>,
    /// Messages to keep verbatim (for partial compaction).
    pub messages_to_keep: Vec<Message>,
    /// Attachment messages to re-inject after compaction.
    pub attachments: Vec<Message>,
    /// Hook result messages produced during compaction.
    pub hook_results: Vec<Message>,
    /// Optional user-facing display message (from hooks).
    pub user_display_message: Option<String>,
}

// ---------------------------------------------------------------------------
// Compact options
// ---------------------------------------------------------------------------

/// Configuration controlling compaction behaviour.
#[derive(Debug, Clone)]
pub struct CompactOptions {
    /// Maximum context-window size in tokens.
    pub max_tokens: u64,
    /// Target token count after compaction.
    pub target_tokens: u64,
    /// Number of recent messages to always preserve.
    pub preserve_recent_messages: usize,
    /// Whether to keep the system prompt intact.
    pub preserve_system_prompt: bool,
    /// Whether to preserve file attachments across compaction.
    pub preserve_attachments: bool,
    /// Optional custom instructions appended to the compact prompt.
    pub custom_instructions: Option<String>,
    /// Whether this is an auto-compact (vs manual /compact).
    pub is_auto_compact: bool,
}

impl Default for CompactOptions {
    fn default() -> Self {
        Self {
            max_tokens: 200_000,
            target_tokens: 50_000,
            preserve_recent_messages: 5,
            preserve_system_prompt: true,
            preserve_attachments: true,
            custom_instructions: None,
            is_auto_compact: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Progress event
// ---------------------------------------------------------------------------

/// Events emitted during a compaction run, used for UI progress reporting.
#[derive(Debug, Clone)]
pub enum CompactProgressEvent {
    /// Compaction has started with the given strategy.
    Started { strategy: CompactStrategyType },
    /// Progress update: N messages have been processed so far.
    Summarizing { messages_processed: usize },
    /// Compaction completed successfully.
    Completed(CompactionResult),
    /// Compaction failed with an error message.
    Failed(String),
}

/// Type-erased progress callback that is `Send + Sync`.
pub type ProgressCallback = dyn Fn(CompactProgressEvent) + Send + Sync;

// ---------------------------------------------------------------------------
// Summary request callback
// ---------------------------------------------------------------------------

/// Callback trait for generating a summary from a list of messages.
///
/// The compact engine does **not** depend on any specific LLM provider.
/// Instead, callers supply an implementation of [`SummaryProvider`] that
/// knows how to call the model and return the summary text.
#[async_trait::async_trait]
pub trait SummaryProvider: Send + Sync {
    /// Given `messages` to summarize and an optional `system_prompt`,
    /// return the raw summary text produced by the LLM.
    async fn generate_summary(
        &self,
        messages: &[Message],
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<String, anyhow::Error>;
}

/// A simple `Fn`-based [`SummaryProvider`] for convenience.
///
/// Uses `Pin<Box<dyn Future>>` to avoid complex lifetime bounds.
pub struct FnSummaryProvider<F>
where
    F: Fn(
            Vec<Message>,
            String,
            String,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<String, anyhow::Error>> + Send>,
        > + Send
        + Sync,
{
    f: F,
}

impl<F> FnSummaryProvider<F>
where
    F: Fn(
            Vec<Message>,
            String,
            String,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<String, anyhow::Error>> + Send>,
        > + Send
        + Sync,
{
    /// Create a new callback-based summary provider.
    pub fn new(f: F) -> Self {
        Self { f }
    }
}

#[async_trait::async_trait]
impl<F> SummaryProvider for FnSummaryProvider<F>
where
    F: Fn(
            Vec<Message>,
            String,
            String,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<String, anyhow::Error>> + Send>,
        > + Send
        + Sync,
{
    async fn generate_summary(
        &self,
        messages: &[Message],
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<String, anyhow::Error> {
        (self.f)(
            messages.to_vec(),
            system_prompt.to_string(),
            user_prompt.to_string(),
        )
        .await
    }
}

// ---------------------------------------------------------------------------
// Strategy trait
// ---------------------------------------------------------------------------

/// A compaction strategy reduces the token footprint of a conversation.
///
/// Each strategy decides *which* messages to keep, *how* to summarise the
/// rest, and returns a [`CompactionResult`] describing what happened.
#[async_trait::async_trait]
pub trait CompactStrategy: Send + Sync {
    /// Return the type identifier for this strategy.
    fn strategy_type(&self) -> CompactStrategyType;

    /// Execute the compaction.
    ///
    /// - `messages` — the full conversation so far.
    /// - `options`  — configuration controlling behaviour.
    /// - `provider` — callback used to generate the LLM summary.
    /// - `progress` — optional sink for progress events.
    async fn compact(
        &self,
        messages: &[Message],
        options: &CompactOptions,
        provider: &dyn SummaryProvider,
        progress: Option<&ProgressCallback>,
    ) -> Result<CompactionResult, anyhow::Error>;
}
