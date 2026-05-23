//! # claude-services
//!
//! Service layer for the Claude Code Rust runtime. Provides:
//!
//! - **Rate limiting**: Token-bucket based rate limit tracking with automatic
//!   backoff using provider-computed `Retry-After` headers.
//! - **Prompt suggestions**: Context-aware suggestion scoring using conversation
//!   history and session state.
//! - **Context collapse monitoring**: Integration with `claude-compact` for
//!   proactive context window management.

mod rate_limiter;
pub use rate_limiter::{RateLimitState, RateLimiter, RateLimiterConfig};

mod prompt_suggestions;
pub use prompt_suggestions::{PromptSuggestion, PromptSuggestionService, SuggestionWeight};

mod context_monitor;
pub use context_monitor::ContextMonitor;
