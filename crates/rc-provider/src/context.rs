//! Context-window management for conversation compaction and token estimation.
//!
//! The [`ContextWindowManager`] tracks an approximate token budget for a
//! conversation and can compact it when the budget is exceeded. Compaction
//! preserves the system prompt, keeps the most recent turns, and replaces
//! older turns with a short summary.
//!
//! # Advanced compaction strategies
//!
//! Three compaction strategies are available, matching upstream Claude Code:
//!
//! - **`compact`** — Standard compaction: preserve system prompt + recent N turns,
//!   summarize the rest.
//! - **`reactive_compact`** — Triggered when the provider returns a
//!   `prompt-too-long` error. More aggressive: keeps only the last 2 turns.
//! - **`context_collapse`** — Replaces the entire conversation (except system
//!   prompt) with a single summary entry. Used as a last resort.
//! - **`microcompact`** — Removes only tool outputs and truncates verbose entries
//!   without changing conversation structure.

use rc_core::{ConversationEntry, ConversationRole};

// ---------------------------------------------------------------------------
// TokenEstimator
// ---------------------------------------------------------------------------

/// Rough token estimator based on character count.
///
/// The estimation is deliberately conservative: it uses a lower
/// chars-per-token ratio so that the budget is consumed faster, reducing the
/// risk of hitting the real model limit.
#[derive(Debug, Clone)]
pub struct TokenEstimator {
    /// Average characters per token (English ≈ 4.0, Chinese ≈ 2.0).
    /// We default to 2.5 which is a safe middle ground.
    chars_per_token: f64,
}

impl TokenEstimator {
    /// Create a new estimator with the default chars-per-token ratio.
    #[must_use]
    pub fn new() -> Self {
        Self {
            chars_per_token: 2.5,
        }
    }

    /// Roughly estimate the number of tokens in `text`.
    ///
    /// The heuristic treats every character (including whitespace) as a
    /// potential token fraction. For mixed English / Chinese text this gives a
    /// reasonable upper bound.
    #[must_use]
    pub fn estimate(&self, text: &str) -> u64 {
        if text.is_empty() {
            return 0;
        }
        // Count characters (not bytes) so CJK characters are counted individually.
        let char_count = text.chars().count() as f64;
        (char_count / self.chars_per_token).ceil() as u64
    }

    /// Estimate the token count of a single [`ConversationEntry`].
    ///
    /// The estimate includes the main `text` field and, if present, the
    /// `history_text` field (whichever is longer). Tool-call payloads are
    /// approximated from their JSON representation.
    #[must_use]
    pub fn estimate_entry(&self, entry: &ConversationEntry) -> u64 {
        let text_tokens = self.estimate(&entry.text);
        let history_tokens = entry.history_text.as_ref().map_or(0, |h| self.estimate(h));
        let tool_tokens: u64 = entry
            .tool_calls
            .iter()
            .map(|tc| self.estimate(&tc.input.to_string()) + self.estimate(&tc.name))
            .sum();
        text_tokens.max(history_tokens) + tool_tokens
    }

    /// Estimate the total token count of an entire conversation.
    #[must_use]
    pub fn estimate_conversation(&self, conversation: &[ConversationEntry]) -> u64 {
        conversation.iter().map(|e| self.estimate_entry(e)).sum()
    }
}

impl Default for TokenEstimator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ContextWindowManager
// ---------------------------------------------------------------------------

/// Default maximum context window in tokens.
const DEFAULT_MAX_TOKENS: u64 = 128_000;

/// Default number of tokens reserved for model output.
const DEFAULT_OUTPUT_RESERVE: u64 = 4_096;

/// Default compaction threshold (80 %).
const DEFAULT_COMPACTION_THRESHOLD: f64 = 0.80;

/// Default number of recent *turns* (user/assistant pairs) to keep.
const DEFAULT_RECENT_TURNS: usize = 4;

/// Default maximum length (in characters) for a single tool output before
/// truncation.
const DEFAULT_TOOL_OUTPUT_MAX_CHARS: usize = 10_000;

/// Context-window manager that tracks an approximate token budget and
/// compacts conversations when the budget is exceeded.
#[derive(Debug, Clone)]
pub struct ContextWindowManager {
    /// Maximum context tokens (input + output).
    max_tokens: u64,
    /// Tokens reserved for the model output.
    output_reserve: u64,
    /// Compaction is triggered when usage exceeds `max_tokens - output_reserve`
    /// multiplied by this ratio.
    compaction_threshold: f64,
    /// Number of recent turns to preserve during compaction.
    recent_turns: usize,
    /// Maximum characters for a single tool output before truncation.
    tool_output_max_chars: usize,
    /// Token estimator.
    estimator: TokenEstimator,
}

impl ContextWindowManager {
    /// Create a new manager with the given token budget.
    #[must_use]
    pub fn new(max_tokens: u64, output_reserve: u64) -> Self {
        Self {
            max_tokens,
            output_reserve,
            compaction_threshold: DEFAULT_COMPACTION_THRESHOLD,
            recent_turns: DEFAULT_RECENT_TURNS,
            tool_output_max_chars: DEFAULT_TOOL_OUTPUT_MAX_CHARS,
            estimator: TokenEstimator::new(),
        }
    }

    /// Create a manager whose token budget is derived from the model name.
    ///
    /// Uses [`crate::model_info::get_model_info`] to look up the maximum
    /// context window and output reserve for the given model, then constructs
    /// a [`ContextWindowManager`] with those values.
    #[must_use]
    pub fn for_model(model: &str) -> Self {
        let info = crate::model_info::get_model_info(model);
        Self {
            max_tokens: info.max_context,
            output_reserve: info.max_output,
            compaction_threshold: DEFAULT_COMPACTION_THRESHOLD,
            recent_turns: DEFAULT_RECENT_TURNS,
            tool_output_max_chars: DEFAULT_TOOL_OUTPUT_MAX_CHARS,
            estimator: TokenEstimator::new(),
        }
    }

    /// Return the available budget for input tokens.
    #[must_use]
    pub fn available_budget(&self) -> u64 {
        self.max_tokens.saturating_sub(self.output_reserve)
    }

    /// Check whether the conversation exceeds the compaction threshold.
    #[must_use]
    pub fn needs_compaction(&self, conversation: &[ConversationEntry]) -> bool {
        self.usage_ratio(conversation) >= self.compaction_threshold
    }

    /// Return the current usage ratio (0.0 – 1.0) of the input budget.
    #[must_use]
    pub fn usage_ratio(&self, conversation: &[ConversationEntry]) -> f64 {
        let budget = self.available_budget();
        if budget == 0 {
            return 1.0;
        }
        let used = self.estimator.estimate_conversation(conversation);
        (used as f64) / (budget as f64)
    }

    /// Compact a conversation that has grown too large.
    ///
    /// # Strategy
    ///
    /// 1. Always preserve the first system message.
    /// 2. Preserve the most recent `recent_turns` user/assistant exchanges
    ///    (and any tool calls/results between them).
    /// 3. Replace older turns with a short summary entry.
    ///
    /// If the conversation is short enough that no compaction is needed, the
    /// original slice is returned unchanged (as a new `Vec`).
    pub fn compact(&self, conversation: &[ConversationEntry]) -> Vec<ConversationEntry> {
        if conversation.is_empty() {
            return Vec::new();
        }

        // Step 1: Extract the system prompt (first system entry).
        let system_entry = conversation
            .iter()
            .find(|e| matches!(e.role, ConversationRole::System))
            .cloned();

        // Step 2: Find the boundary index for "recent" turns.
        // We scan backwards to find `recent_turns` user messages.
        let non_system: Vec<(usize, &ConversationEntry)> = conversation
            .iter()
            .enumerate()
            .filter(|(_, e)| !matches!(e.role, ConversationRole::System))
            .collect();

        let cutoff_idx = if non_system.len() <= self.recent_turns * 2 {
            // Conversation is short — nothing to compact.
            return conversation.to_vec();
        } else {
            // Walk backwards through non-system entries, counting user turns.
            let mut user_count = 0usize;
            let mut split_point = non_system.len();
            for (i, entry) in non_system.iter().rev() {
                if matches!(entry.role, ConversationRole::User) {
                    user_count += 1;
                    if user_count >= self.recent_turns {
                        split_point = *i;
                        break;
                    }
                }
            }
            split_point
        };

        // Step 3: Build the compacted conversation.
        let mut result = Vec::new();

        // Preserve system prompt.
        if let Some(sys) = system_entry {
            result.push(sys);
        }

        // Generate a summary for older entries.
        let older: Vec<&ConversationEntry> = non_system
            .iter()
            .take(cutoff_idx)
            .map(|(_, e)| *e)
            .collect();

        if !older.is_empty() {
            let summary = build_summary(&older);
            result.push(ConversationEntry::system(summary));
        }

        // Append recent entries.
        for (_, entry) in non_system.iter().skip(cutoff_idx) {
            result.push((*entry).clone());
        }

        result
    }

    /// Truncate a tool output string if it exceeds `max_chars`.
    ///
    /// A truncation marker is appended when the output is shortened.
    #[must_use]
    pub fn truncate_tool_output(&self, output: &str, max_chars: usize) -> String {
        if output.chars().count() <= max_chars {
            return output.to_owned();
        }
        let truncated: String = output.chars().take(max_chars).collect();
        format!(
            "{truncated}\n\n... [truncated: original was {} chars, showing first {max_chars} chars]",
            output.chars().count(),
        )
    }

    /// Convenience wrapper that uses the default `tool_output_max_chars`.
    #[must_use]
    pub fn truncate_tool_output_default(&self, output: &str) -> String {
        self.truncate_tool_output(output, self.tool_output_max_chars)
    }

    // ── Advanced compaction strategies ──────────────────────────────────

    /// Reactive compaction: a more aggressive compaction triggered when the
    /// provider returns a `prompt-too-long` error.
    ///
    /// Compared to [`compact`](Self::compact), this keeps only the last 2
    /// user/assistant turns (instead of `recent_turns`) and aggressively
    /// truncates tool outputs.
    pub fn reactive_compact(&self, conversation: &[ConversationEntry]) -> Vec<ConversationEntry> {
        if conversation.is_empty() {
            return Vec::new();
        }

        let system_entry = conversation
            .iter()
            .find(|e| matches!(e.role, ConversationRole::System))
            .cloned();

        let non_system: Vec<(usize, &ConversationEntry)> = conversation
            .iter()
            .enumerate()
            .filter(|(_, e)| !matches!(e.role, ConversationRole::System))
            .collect();

        // Keep only the last 2 turns (much more aggressive than standard).
        let reactive_turns = 2;
        let cutoff_idx = if non_system.len() <= reactive_turns * 2 {
            return conversation.to_vec();
        } else {
            let mut user_count = 0usize;
            let mut split_point = non_system.len();
            for (i, entry) in non_system.iter().rev() {
                if matches!(entry.role, ConversationRole::User) {
                    user_count += 1;
                    if user_count >= reactive_turns {
                        split_point = *i;
                        break;
                    }
                }
            }
            split_point
        };

        let mut result = Vec::new();
        if let Some(sys) = system_entry {
            result.push(sys);
        }

        // Generate summary for older entries.
        let older: Vec<&ConversationEntry> = non_system
            .iter()
            .take(cutoff_idx)
            .map(|(_, e)| *e)
            .collect();
        if !older.is_empty() {
            let summary = build_summary(&older);
            result.push(ConversationEntry::system(format!(
                "[reactive-compaction] {summary}"
            )));
        }

        // Append recent entries with aggressive tool output truncation.
        for (_, entry) in non_system.iter().skip(cutoff_idx) {
            let mut truncated = (*entry).clone();
            if matches!(entry.role, ConversationRole::Tool) {
                let truncated_text = self.truncate_tool_output(&entry.text, 2000);
                truncated.text = truncated_text;
            }
            result.push(truncated);
        }

        result
    }

    /// Context collapse: replaces the entire conversation (except system prompt)
    /// with a single summary entry. This is the last-resort compaction strategy.
    ///
    /// Use when even `reactive_compact` doesn't free enough tokens.
    pub fn context_collapse(&self, conversation: &[ConversationEntry]) -> Vec<ConversationEntry> {
        if conversation.is_empty() {
            return Vec::new();
        }

        let system_entry = conversation
            .iter()
            .find(|e| matches!(e.role, ConversationRole::System))
            .cloned();

        // Summarize everything except the system prompt.
        let non_system: Vec<&ConversationEntry> = conversation
            .iter()
            .filter(|e| !matches!(e.role, ConversationRole::System))
            .collect();

        let mut result = Vec::new();
        if let Some(sys) = system_entry {
            result.push(sys);
        }

        if !non_system.is_empty() {
            let summary = build_summary(&non_system);
            result.push(ConversationEntry::system(format!(
                "[context-collapse] Full conversation summary:\n{summary}"
            )));
            // Keep only the very last user message if available.
            if let Some(last_user) = non_system
                .iter()
                .rev()
                .find(|e| matches!(e.role, ConversationRole::User))
            {
                result.push((*last_user).clone());
            }
        }

        result
    }

    /// Microcompact: removes tool outputs and truncates verbose entries
    /// without changing the conversation structure.
    ///
    /// This is the least disruptive strategy — it keeps all turns but shrinks
    /// large tool outputs and truncates long assistant messages.
    pub fn microcompact(&self, conversation: &[ConversationEntry]) -> Vec<ConversationEntry> {
        conversation
            .iter()
            .map(|entry| {
                let mut compacted = entry.clone();
                match entry.role {
                    ConversationRole::Tool => {
                        // Truncate tool outputs to a shorter limit.
                        if entry.text.chars().count() > 2000 {
                            compacted.text = self.truncate_tool_output(&entry.text, 2000);
                        }
                    }
                    ConversationRole::Assistant => {
                        // Truncate very long assistant messages.
                        if entry.text.chars().count() > 5000 {
                            let truncated: String = entry.text.chars().take(5000).collect();
                            compacted.text = format!(
                                "{truncated}\n\n[... microcompact: truncated from {} chars]",
                                entry.text.chars().count()
                            );
                        }
                    }
                    _ => {}
                }
                compacted
            })
            .collect()
    }

    /// Select the best compaction strategy based on current usage.
    ///
    /// Returns the compacted conversation using the least disruptive strategy
    /// that will bring usage below the threshold.
    pub fn auto_compact(&self, conversation: &[ConversationEntry]) -> Vec<ConversationEntry> {
        let ratio = self.usage_ratio(conversation);

        if ratio < self.compaction_threshold {
            // No compaction needed.
            return conversation.to_vec();
        }

        // Try strategies from least to most disruptive.
        let micro = self.microcompact(conversation);
        if self.usage_ratio(&micro) < self.compaction_threshold {
            return micro;
        }

        let standard = self.compact(conversation);
        if self.usage_ratio(&standard) < self.compaction_threshold {
            return standard;
        }

        let reactive = self.reactive_compact(conversation);
        if self.usage_ratio(&reactive) < self.compaction_threshold {
            return reactive;
        }

        // Last resort: full context collapse.
        self.context_collapse(conversation)
    }

    /// Compact the conversation in response to an API error (e.g. context_length_exceeded).
    ///
    /// This is the "reactiveCompact" pattern from upstream: when the API rejects a request
    /// because the context is too long, this method aggressively compacts and returns a
    /// shorter conversation that should fit within the model's context window.
    ///
    /// Returns `Some(compacted)` if compaction was performed, `None` if the conversation
    /// is already too short to compact further.
    pub fn compact_on_error(
        &self,
        conversation: &[ConversationEntry],
    ) -> Option<Vec<ConversationEntry>> {
        if conversation.len() <= 3 {
            // Can't compact further — system + 1 user + 1 assistant is the minimum.
            return None;
        }

        // Try progressive compaction strategies, most aggressive first since we already
        // know the context is too long (the API told us).
        let reactive = self.reactive_compact(conversation);
        if reactive.len() < conversation.len() {
            return Some(reactive);
        }

        let collapsed = self.context_collapse(conversation);
        if collapsed.len() < conversation.len() {
            return Some(collapsed);
        }

        None
    }
    // ── L3-L5 Advanced compaction strategies ──────────────────────────

    /// L3: Sliding window compaction with progressive summarization.
    ///
    /// Instead of a fixed cutoff, this strategy keeps entries within a dynamic
    /// token budget. Older entries are progressively summarized in groups,
    /// allowing more fine-grained control than the standard `compact()`.
    ///
    /// Each "window" of N entries is summarized into a single entry, reducing
    /// token usage while preserving chronological context.
    pub fn sliding_window_compact(
        &self,
        conversation: &[ConversationEntry],
    ) -> Vec<ConversationEntry> {
        if conversation.is_empty() {
            return Vec::new();
        }

        let system_entry = conversation
            .iter()
            .find(|e| matches!(e.role, ConversationRole::System))
            .cloned();

        let non_system: Vec<&ConversationEntry> = conversation
            .iter()
            .filter(|e| !matches!(e.role, ConversationRole::System))
            .collect();

        if non_system.is_empty() {
            return conversation.to_vec();
        }

        // Determine how many entries to keep verbatim at the end.
        let keep_recent = self.recent_turns * 2; // user/assistant pairs
        if non_system.len() <= keep_recent {
            return conversation.to_vec();
        }

        let older_slice = &non_system[..non_system.len() - keep_recent];
        let recent_slice = &non_system[non_system.len() - keep_recent..];

        // Chunk older entries into groups of ~6 and summarize each chunk.
        let chunk_size = 6;
        let mut summaries = Vec::new();
        for chunk in older_slice.chunks(chunk_size) {
            let summary = build_summary(chunk);
            summaries.push(summary);
        }

        let mut result = Vec::new();
        if let Some(sys) = system_entry {
            result.push(sys);
        }

        // Merge all chunk summaries into a single system entry.
        if !summaries.is_empty() {
            let combined = summaries.join("\n\n---\n\n");
            result.push(ConversationEntry::system(format!(
                "[sliding-window-compaction] {combined}"
            )));
        }

        // Append the recent entries verbatim.
        for entry in recent_slice {
            result.push((*entry).clone());
        }

        result
    }

    /// L4: Priority-based compaction.
    ///
    /// Assigns a priority score to each entry based on its content:
    /// - Entries with tool calls: high priority (they contain actionable state)
    /// - Error entries: high priority (they contain debugging context)
    /// - System entries: always preserved
    /// - Recent entries: boosted priority
    /// - Older plain text: low priority (candidates for summarization)
    ///
    /// Low-priority entries are summarized, high-priority ones are kept intact.
    pub fn priority_compact(&self, conversation: &[ConversationEntry]) -> Vec<ConversationEntry> {
        if conversation.is_empty() {
            return Vec::new();
        }

        let total = conversation.len();

        // Score each entry: higher = more important to keep.
        let scored: Vec<(usize, u32, &ConversationEntry)> = conversation
            .iter()
            .enumerate()
            .map(|(i, entry)| {
                let mut score: u32 = 0;

                // System entries are always kept.
                if matches!(entry.role, ConversationRole::System) {
                    score += 1000;
                }

                // Entries with tool calls are important.
                if !entry.tool_calls.is_empty() {
                    score += 50;
                }

                // Error entries contain debugging context.
                if entry.is_error {
                    score += 40;
                }

                // Assistant entries with substantial content.
                if matches!(entry.role, ConversationRole::Assistant)
                    && entry.text.chars().count() > 100
                {
                    score += 20;
                }

                // Recency boost: more recent entries get higher scores.
                let recency = (total - i) as u32;
                score += recency.min(30);

                (i, score, entry)
            })
            .collect();

        // Determine a score threshold: keep entries above the median score.
        let mut scores: Vec<u32> = scored.iter().map(|(_, s, _)| *s).collect();
        scores.sort();
        let median = scores.get(scores.len() / 2).copied().unwrap_or(0);

        let mut kept = Vec::new();
        let mut summarized_indices = Vec::new();

        for (i, score, entry) in &scored {
            if *score >= median || matches!(entry.role, ConversationRole::System) {
                kept.push((*entry).clone());
            } else {
                summarized_indices.push(*i);
            }
        }

        // If nothing was summarized, return original.
        if summarized_indices.is_empty() {
            return conversation.to_vec();
        }

        // Build summary for the removed entries.
        let summarized_entries: Vec<&ConversationEntry> = summarized_indices
            .iter()
            .map(|&i| &conversation[i])
            .collect();
        let summary = build_summary(&summarized_entries);

        // Reconstruct: system + summary + kept entries (in order).
        let mut result = Vec::new();
        let mut summary_inserted = false;

        for entry in &kept {
            if matches!(entry.role, ConversationRole::System) && !summary_inserted {
                result.push(entry.clone());
                // Insert summary right after system prompt.
                result.push(ConversationEntry::system(format!(
                    "[priority-compaction] {summary}"
                )));
                summary_inserted = true;
            } else if !summary_inserted {
                // Shouldn't happen, but handle gracefully.
                result.push(entry.clone());
            } else {
                result.push(entry.clone());
            }
        }

        if !summary_inserted {
            result.push(ConversationEntry::system(format!(
                "[priority-compaction] {summary}"
            )));
        }

        result
    }

    /// L5: Semantic chunk compaction.
    ///
    /// Groups entries into semantic "turns" (user query + assistant response +
    /// any tool calls/results) and progressively summarizes from oldest to
    /// newest until the token budget is met.
    ///
    /// This preserves the logical structure of the conversation better than
    /// simple windowing.
    pub fn semantic_chunk_compact(
        &self,
        conversation: &[ConversationEntry],
    ) -> Vec<ConversationEntry> {
        if conversation.is_empty() {
            return Vec::new();
        }

        let system_entry = conversation
            .iter()
            .find(|e| matches!(e.role, ConversationRole::System))
            .cloned();

        // Group non-system entries into semantic chunks.
        // A chunk starts with each User entry and includes subsequent
        // Assistant + Tool entries until the next User entry.
        let non_system: Vec<&ConversationEntry> = conversation
            .iter()
            .filter(|e| !matches!(e.role, ConversationRole::System))
            .collect();

        let mut chunks: Vec<Vec<&ConversationEntry>> = Vec::new();
        let mut current_chunk = Vec::new();

        for entry in &non_system {
            if matches!(entry.role, ConversationRole::User) && !current_chunk.is_empty() {
                chunks.push(std::mem::take(&mut current_chunk));
            }
            current_chunk.push(*entry);
        }
        if !current_chunk.is_empty() {
            chunks.push(current_chunk);
        }

        if chunks.len() <= 2 {
            return conversation.to_vec();
        }

        // Calculate token cost for each chunk.
        let chunk_tokens: Vec<u64> = chunks
            .iter()
            .map(|chunk| chunk.iter().map(|e| self.estimator.estimate_entry(e)).sum())
            .collect();

        let budget = self.available_budget();
        let total_tokens: u64 = chunk_tokens.iter().sum();

        // If within budget, no compaction needed.
        if total_tokens <= budget {
            return conversation.to_vec();
        }

        // Progressive summarization: summarize oldest chunks first.
        // Keep the last 2 chunks intact, summarize the rest.
        let keep_chunks = 2;
        let mut result = Vec::new();

        if let Some(sys) = system_entry {
            result.push(sys);
        }

        // Summarize older chunks.
        let older_chunks = &chunks[..chunks.len() - keep_chunks];
        if !older_chunks.is_empty() {
            let older_entries: Vec<&ConversationEntry> = older_chunks
                .iter()
                .flat_map(|c| c.iter().copied())
                .collect();
            let summary = build_summary(&older_entries);
            result.push(ConversationEntry::system(format!(
                "[semantic-chunk-compaction] Summarized {} earlier conversation turns:\n{summary}",
                older_chunks.len(),
            )));
        }

        // Keep recent chunks intact.
        for chunk in &chunks[chunks.len() - keep_chunks..] {
            for entry in chunk {
                result.push((*entry).clone());
            }
        }

        result
    }

    /// Enhanced auto-compaction with L3-L5 strategies.
    ///
    /// Tries strategies from least to most disruptive:
    /// 1. No compaction (if within budget)
    /// 2. L4: Microcompact (truncate tool outputs)
    /// 3. L3: Sliding window (progressive chunk summarization)
    /// 4. L5: Semantic chunk (topic-based summarization)
    /// 5. L4: Priority-based (keep important entries)
    /// 6. Standard compact (fixed turn preservation)
    /// 7. Reactive compact (aggressive turn reduction)
    /// 8. Context collapse (last resort)
    pub fn auto_compact_v2(&self, conversation: &[ConversationEntry]) -> Vec<ConversationEntry> {
        let ratio = self.usage_ratio(conversation);

        if ratio < self.compaction_threshold {
            return conversation.to_vec();
        }

        // Strategy cascade: least → most disruptive.
        #[allow(clippy::type_complexity)]
        let strategies: &[(
            &str,
            fn(&ContextWindowManager, &[ConversationEntry]) -> Vec<ConversationEntry>,
        )] = &[
            ("microcompact", |mgr, conv| mgr.microcompact(conv)),
            ("sliding_window", |mgr, conv| {
                mgr.sliding_window_compact(conv)
            }),
            ("semantic_chunk", |mgr, conv| {
                mgr.semantic_chunk_compact(conv)
            }),
            ("priority", |mgr, conv| mgr.priority_compact(conv)),
            ("standard", |mgr, conv| mgr.compact(conv)),
            ("reactive", |mgr, conv| mgr.reactive_compact(conv)),
            ("collapse", |mgr, conv| mgr.context_collapse(conv)),
        ];

        for (_name, strategy) in strategies {
            let result = strategy(self, conversation);
            if self.usage_ratio(&result) < self.compaction_threshold {
                return result;
            }
        }

        // Absolute last resort.
        self.context_collapse(conversation)
    }
}

impl Default for ContextWindowManager {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_TOKENS, DEFAULT_OUTPUT_RESERVE)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a short text summary from a slice of older conversation entries.
fn build_summary(entries: &[&ConversationEntry]) -> String {
    let mut parts = Vec::new();
    for entry in entries {
        let label = match entry.role {
            ConversationRole::System => "system",
            ConversationRole::User => "user",
            ConversationRole::Assistant => "assistant",
            ConversationRole::Tool => "tool",
        };
        let text_preview = truncate_str(&entry.history_text(), 120);
        parts.push(format!("[{label}]: {text_preview}"));
    }
    let body = parts.join("\n");
    format!(
        "[Context Summary — {count} earlier messages compacted]\n{body}",
        count = entries.len(),
    )
}

/// Truncate a string to at most `max_chars` characters, appending "…" when
/// truncated.
fn truncate_str(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_owned();
    }
    let s: String = text.chars().take(max_chars).collect();
    format!("{s}…")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rc_core::{ConversationEntry, ConversationRole};

    // -- TokenEstimator tests ------------------------------------------------

    #[test]
    fn token_estimator_approximates_english_correctly() {
        let est = TokenEstimator::new();
        // "Hello world" = 11 chars / 2.5 ≈ 4.4 → ceil = 5 tokens
        let tokens = est.estimate("Hello world");
        assert!(tokens > 0, "should estimate some tokens for English text");
        assert!(
            (tokens as f64 - 11.0 / 2.5).abs() < 2.0,
            "estimate should be close to chars/2.5"
        );
    }

    #[test]
    fn token_estimator_approximates_chinese_correctly() {
        let est = TokenEstimator::new();
        // "你好世界测试" = 6 chars / 2.5 = 2.4 → ceil = 3 tokens
        let tokens = est.estimate("你好世界测试");
        assert!(tokens > 0, "should estimate some tokens for Chinese text");
        assert!(
            tokens >= 2,
            "Chinese text should have at least 2 tokens estimated"
        );
    }

    #[test]
    fn token_estimator_handles_empty_string() {
        let est = TokenEstimator::new();
        assert_eq!(est.estimate(""), 0);
    }

    #[test]
    fn token_estimator_entry_includes_tool_calls() {
        let est = TokenEstimator::new();
        let entry = ConversationEntry::tool("id1", "bash", "some output", false);
        let tokens = est.estimate_entry(&entry);
        assert!(tokens > 0, "tool entry should have non-zero token estimate");
    }

    #[test]
    fn token_estimator_conversation_sums_entries() {
        let est = TokenEstimator::new();
        let conv = vec![
            ConversationEntry::system("You are helpful."),
            ConversationEntry::user("Hello"),
        ];
        let total = est.estimate_conversation(&conv);
        let sum_parts = est.estimate_entry(&conv[0]) + est.estimate_entry(&conv[1]);
        assert_eq!(total, sum_parts);
    }

    // -- ContextWindowManager tests ------------------------------------------

    #[test]
    fn context_window_detects_overflow() {
        let mgr = ContextWindowManager::new(100, 20);
        // Budget = 80 tokens. We need >80% of 80 = 64 tokens to trigger.
        // Build a conversation with enough text to exceed that.
        let long_text = "a".repeat(200); // 200 chars / 2.5 = 80 tokens
        let conv = vec![
            ConversationEntry::system("short"),
            ConversationEntry::user(long_text),
        ];
        assert!(
            mgr.needs_compaction(&conv),
            "conversation exceeding 80% of budget should trigger compaction"
        );
    }

    #[test]
    fn context_window_within_budget() {
        let mgr = ContextWindowManager::new(100_000, 4_096);
        let conv = vec![
            ConversationEntry::system("You are helpful."),
            ConversationEntry::user("Hi there"),
        ];
        assert!(
            !mgr.needs_compaction(&conv),
            "short conversation should not trigger compaction"
        );
    }

    #[test]
    fn compaction_preserves_system_prompt() {
        let mgr = ContextWindowManager::new(100, 20);
        let conv = vec![
            ConversationEntry::system("IMPORTANT SYSTEM PROMPT"),
            ConversationEntry::user("msg 1"),
            ConversationEntry::assistant("reply 1"),
            ConversationEntry::user("msg 2"),
            ConversationEntry::assistant("reply 2"),
            ConversationEntry::user("msg 3"),
            ConversationEntry::assistant("reply 3"),
            ConversationEntry::user("msg 4"),
            ConversationEntry::assistant("reply 4"),
            ConversationEntry::user("msg 5"),
            ConversationEntry::assistant("reply 5"),
        ];

        let compacted = mgr.compact(&conv);

        // System prompt should be preserved.
        let system_entries: Vec<_> = compacted
            .iter()
            .filter(|e| matches!(e.role, ConversationRole::System))
            .collect();
        assert!(
            system_entries
                .iter()
                .any(|e| e.text.contains("IMPORTANT SYSTEM PROMPT")),
            "original system prompt must be preserved"
        );
    }

    #[test]
    fn compaction_preserves_recent_turns() {
        let mgr = ContextWindowManager::new(100, 20);
        let conv = vec![
            ConversationEntry::system("sys"),
            ConversationEntry::user("old msg 1"),
            ConversationEntry::assistant("old reply 1"),
            ConversationEntry::user("old msg 2"),
            ConversationEntry::assistant("old reply 2"),
            ConversationEntry::user("old msg 3"),
            ConversationEntry::assistant("old reply 3"),
            ConversationEntry::user("recent msg 4"),
            ConversationEntry::assistant("recent reply 4"),
            ConversationEntry::user("recent msg 5"),
            ConversationEntry::assistant("recent reply 5"),
        ];

        let compacted = mgr.compact(&conv);

        // Recent messages should be preserved verbatim.
        assert!(
            compacted.iter().any(|e| e.text.contains("recent msg 5")),
            "most recent user message must be preserved"
        );
        assert!(
            compacted.iter().any(|e| e.text.contains("recent reply 5")),
            "most recent assistant message must be preserved"
        );
    }

    #[test]
    fn compaction_short_conversation_unchanged() {
        let mgr = ContextWindowManager::new(100_000, 4_096);
        let conv = vec![
            ConversationEntry::system("sys"),
            ConversationEntry::user("hi"),
            ConversationEntry::assistant("hello"),
        ];

        let compacted = mgr.compact(&conv);
        assert_eq!(
            compacted.len(),
            conv.len(),
            "short conversation should not be compacted"
        );
    }

    #[test]
    fn compaction_empty_conversation() {
        let mgr = ContextWindowManager::default();
        let compacted = mgr.compact(&[]);
        assert!(compacted.is_empty());
    }

    #[test]
    fn tool_output_truncation_works() {
        let mgr = ContextWindowManager::default();
        let long_output = "x".repeat(15_000);
        let truncated = mgr.truncate_tool_output(&long_output, 10_000);

        assert!(
            truncated.contains("[truncated"),
            "truncated output should contain truncation marker"
        );
        assert!(
            truncated.len() < long_output.len(),
            "truncated output should be shorter"
        );
    }

    #[test]
    fn tool_output_short_enough_not_truncated() {
        let mgr = ContextWindowManager::default();
        let short_output = "hello world".to_owned();
        let result = mgr.truncate_tool_output(&short_output, 10_000);
        assert_eq!(result, short_output);
    }

    #[test]
    fn usage_ratio_is_zero_for_empty_conversation() {
        let mgr = ContextWindowManager::default();
        assert_eq!(mgr.usage_ratio(&[]), 0.0);
    }

    #[test]
    fn usage_ratio_increases_with_more_text() {
        let mgr = ContextWindowManager::new(1_000, 100);
        let short = vec![ConversationEntry::user("hi")];
        let long = vec![ConversationEntry::user("a".repeat(800))];

        let short_ratio = mgr.usage_ratio(&short);
        let long_ratio = mgr.usage_ratio(&long);

        assert!(
            long_ratio > short_ratio,
            "longer conversation should have higher usage ratio"
        );
    }

    #[test]
    fn compaction_includes_summary_for_older_messages() {
        let mgr = ContextWindowManager::new(100, 20);
        let conv = vec![
            ConversationEntry::system("sys"),
            ConversationEntry::user("old msg 1"),
            ConversationEntry::assistant("old reply 1"),
            ConversationEntry::user("old msg 2"),
            ConversationEntry::assistant("old reply 2"),
            ConversationEntry::user("msg 3"),
            ConversationEntry::assistant("reply 3"),
            ConversationEntry::user("msg 4"),
            ConversationEntry::assistant("reply 4"),
            ConversationEntry::user("msg 5"),
            ConversationEntry::assistant("reply 5"),
        ];

        let compacted = mgr.compact(&conv);

        // Should have a summary entry.
        assert!(
            compacted
                .iter()
                .any(|e| e.text.contains("[Context Summary")),
            "compacted conversation should contain a summary of older messages"
        );
    }

    // -- for_model() tests --------------------------------------------------

    #[test]
    fn for_model_creates_correct_manager() {
        // GLM-4-Plus → 200 K / 4 K (updated per 2026 specs)
        let mgr = ContextWindowManager::for_model("glm-4-plus");
        assert_eq!(mgr.available_budget(), 200_000 - 4_096);

        // GLM-4-Long → 1 M / 4 K
        let mgr = ContextWindowManager::for_model("glm-4-long");
        assert_eq!(mgr.available_budget(), 1_000_000 - 4_096);

        // GPT-4o → 200 K / 16 K (updated per 2026 specs)
        let mgr = ContextWindowManager::for_model("gpt-4o");
        assert_eq!(mgr.available_budget(), 200_000 - 16_384);

        // Claude 3.5 Sonnet → 200 K / 8 K
        let mgr = ContextWindowManager::for_model("claude-3.5-sonnet");
        assert_eq!(mgr.available_budget(), 200_000 - 8_192);

        // o1 → 200 K / 100 K
        let mgr = ContextWindowManager::for_model("o1");
        assert_eq!(mgr.available_budget(), 200_000 - 100_000);

        // Unknown → 128 K / 4 K (default)
        let mgr = ContextWindowManager::for_model("some-random-model");
        assert_eq!(mgr.available_budget(), 128_000 - 4_096);
    }
}
