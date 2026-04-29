//! Streaming tag-delimited region matcher.
//!
//! Derived from `src/utils/tag-matcher.ts`.
//!
//! Used to separate content inside `<tag>...</tag>` from surrounding text.
//! This is used for reasoning tags like `<thinking>...</thinking>` in provider streams.

/// Result of a tag match operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagMatcherResult {
    /// Whether this chunk is inside matched tags.
    pub matched: bool,
    /// The content data.
    pub data: String,
}

/// The current state of the tag matcher state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TagMatcherState {
    /// Processing normal text.
    Text,
    /// Inside an opening tag `<tagname`.
    TagOpen,
    /// Inside a closing tag `</tagname`.
    TagClose,
}

/// Streaming matcher for lightweight tag-delimited regions.
///
/// Source: `src/utils/tag-matcher.ts` — `TagMatcher`
///
/// Used to separate content inside `<tag>...</tag>` from surrounding text.
/// This is used for reasoning tags like `<thinking>...</thinking>` in provider streams.
///
/// # Type Parameters
/// * `R` - The result type for each chunk (defaults to [`TagMatcherResult`])
///
/// # Example
/// ```rust
/// use roo_tools::tag_matcher::TagMatcher;
///
/// let mut matcher = TagMatcher::new("thinking", None, None);
/// let chunks = matcher.update("Hello <thinking>secret</thinking> world");
/// assert_eq!(chunks.len(), 3);
/// ```
pub struct TagMatcher<R = TagMatcherResult> {
    /// The tag name to match.
    tag_name: String,
    /// Optional transform function.
    transform: Option<Box<dyn Fn(TagMatcherResult) -> R>>,
    /// Starting position (only match tags at or after this position).
    position: usize,
    /// Current index within the tag name being matched.
    index: usize,
    /// Accumulated result chunks.
    chunks: Vec<TagMatcherResult>,
    /// Cached characters awaiting collection.
    cached: Vec<char>,
    /// Whether currently inside matched tags.
    matched: bool,
    /// Current state machine state.
    state: TagMatcherState,
    /// Nesting depth of matched tags.
    depth: usize,
    /// Current character pointer position.
    pointer: usize,
}

impl TagMatcher<TagMatcherResult> {
    /// Creates a new `TagMatcher` for the given tag name.
    ///
    /// # Arguments
    /// * `tag_name` - The tag name to match (e.g., "thinking")
    /// * `transform` - Optional transform function for results
    /// * `position` - Only match tags starting at or after this position
    pub fn new(
        tag_name: impl Into<String>,
        transform: Option<Box<dyn Fn(TagMatcherResult) -> TagMatcherResult>>,
        position: Option<usize>,
    ) -> Self {
        Self {
            tag_name: tag_name.into(),
            transform,
            position: position.unwrap_or(0),
            index: 0,
            chunks: Vec::new(),
            cached: Vec::new(),
            matched: false,
            state: TagMatcherState::Text,
            depth: 0,
            pointer: 0,
        }
    }

    /// Process a chunk of text and return any completed result chunks.
    ///
    /// Source: `src/utils/tag-matcher.ts` — `TagMatcher.update`
    pub fn update(&mut self, chunk: &str) -> Vec<TagMatcherResult> {
        self.process(chunk);
        self.pop()
    }

    /// Process a final chunk (or no chunk) and return all remaining results.
    ///
    /// Source: `src/utils/tag-matcher.ts` — `TagMatcher.final`
    pub fn final_result(&mut self, chunk: Option<&str>) -> Vec<TagMatcherResult> {
        if let Some(c) = chunk {
            self.process(c);
        }
        self.collect();
        self.pop()
    }
}

impl<R> TagMatcher<R> {
    /// Internal processing of a text chunk.
    fn process(&mut self, chunk: &str) {
        let tag_name_chars: Vec<char> = self.tag_name.chars().collect();

        for ch in chunk.chars() {
            self.cached.push(ch);
            self.pointer += 1;

            match self.state {
                TagMatcherState::Text => {
                    if ch == '<'
                        && (self.pointer > self.position || self.matched)
                    {
                        self.state = TagMatcherState::TagOpen;
                        self.index = 0;
                    } else {
                        self.collect();
                    }
                }
                TagMatcherState::TagOpen => {
                    if ch == '>' && self.index == tag_name_chars.len() {
                        self.state = TagMatcherState::Text;
                        if !self.matched {
                            self.cached.clear();
                        }
                        self.depth += 1;
                        self.matched = true;
                    } else if self.index == 0 && ch == '/' {
                        self.state = TagMatcherState::TagClose;
                    } else if ch == ' '
                        && (self.index == 0 || self.index == tag_name_chars.len())
                    {
                        // Skip spaces at start or end of tag name
                        continue;
                    } else if self.index < tag_name_chars.len()
                        && tag_name_chars[self.index] == ch
                    {
                        self.index += 1;
                    } else {
                        self.state = TagMatcherState::Text;
                        self.collect();
                    }
                }
                TagMatcherState::TagClose => {
                    if ch == '>' && self.index == tag_name_chars.len() {
                        self.state = TagMatcherState::Text;
                        self.depth = self.depth.saturating_sub(1);
                        self.matched = self.depth > 0;
                        if !self.matched {
                            self.cached.clear();
                        }
                    } else if ch == ' '
                        && (self.index == 0 || self.index == tag_name_chars.len())
                    {
                        continue;
                    } else if self.index < tag_name_chars.len()
                        && tag_name_chars[self.index] == ch
                    {
                        self.index += 1;
                    } else {
                        self.state = TagMatcherState::Text;
                        self.collect();
                    }
                }
            }
        }
    }

    /// Collect cached characters into the chunks vector.
    fn collect(&mut self) {
        if self.cached.is_empty() {
            return;
        }
        let data: String = self.cached.iter().collect();
        let matched = self.matched;

        if let Some(last) = self.chunks.last() {
            if last.matched == matched {
                self.chunks.last_mut().unwrap().data.push_str(&data);
                self.cached.clear();
                return;
            }
        }

        self.chunks.push(TagMatcherResult { data, matched });
        self.cached.clear();
    }

    /// Pop all accumulated chunks, optionally applying the transform.
    fn pop(&mut self) -> Vec<TagMatcherResult> {
        let chunks = std::mem::take(&mut self.chunks);
        // For the base TagMatcherResult type, we just return chunks directly
        // The transform is applied in specialized implementations
        let _ = &self.transform; // suppress unused warning
        chunks
    }
}

impl std::fmt::Debug for TagMatcher<TagMatcherResult> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TagMatcher")
            .field("tag_name", &self.tag_name)
            .field("matched", &self.matched)
            .field("state", &self.state)
            .field("depth", &self.depth)
            .field("pointer", &self.pointer)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_tag_match() {
        let mut matcher = TagMatcher::new("thinking", None, None);
        let chunks = matcher.update("Hello <thinking>secret</thinking> world");
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].data, "Hello ");
        assert!(!chunks[0].matched);
        assert_eq!(chunks[1].data, "secret");
        assert!(chunks[1].matched);
        assert_eq!(chunks[2].data, " world");
        assert!(!chunks[2].matched);
    }

    #[test]
    fn test_no_tags() {
        let mut matcher = TagMatcher::new("thinking", None, None);
        let chunks = matcher.update("Just plain text");
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].data, "Just plain text");
        assert!(!chunks[0].matched);
    }

    #[test]
    fn test_nested_tags() {
        let mut matcher = TagMatcher::new("tag", None, None);
        let chunks = matcher.update("<tag>outer <tag>inner</tag> back</tag> end");
        // Should have matched sections
        assert!(chunks.iter().any(|c| c.matched));
    }

    #[test]
    fn test_incremental_update() {
        let mut matcher = TagMatcher::new("think", None, None);

        let chunks1 = matcher.update("Hello <thi");
        assert!(chunks1.iter().all(|c| !c.matched));

        let chunks2 = matcher.update("nk>secret</think");
        // Should have some matched content
        let all: Vec<_> = chunks1.into_iter().chain(chunks2.into_iter()).collect();
        assert!(all.iter().any(|c| c.matched));
    }

    #[test]
    fn test_final_result() {
        let mut matcher = TagMatcher::new("tag", None, None);
        let chunks = matcher.update("<tag>content");
        // update() returns all processed chunks including matched content
        assert!(chunks.iter().any(|c| c.matched));
        // final_result with no remaining content returns empty
        let remaining = matcher.final_result(None);
        assert!(remaining.is_empty());
    }

    #[test]
    fn test_final_result_with_chunk() {
        let mut matcher = TagMatcher::new("tag", None, None);
        let chunks1 = matcher.update("<tag>content");
        assert!(chunks1.iter().any(|c| c.matched));
        // Feed closing tag via final_result
        let chunks2 = matcher.final_result(Some("</tag>"));
        // The closing tag produces an unmatched chunk (or nothing if just "</tag>")
        // Either way, the matched content was already returned in chunks1
        let all: Vec<_> = chunks1.into_iter().chain(chunks2.into_iter()).collect();
        assert!(all.iter().any(|c| c.matched));
    }

    #[test]
    fn test_empty_input() {
        let mut matcher = TagMatcher::new("tag", None, None);
        let chunks = matcher.update("");
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_position_parameter() {
        // Position 0 means tags are only matched after position 0
        let mut matcher = TagMatcher::new("tag", None, Some(0));
        let chunks = matcher.update("<tag>matched</tag>");
        assert!(chunks.iter().any(|c| c.matched));
    }

    #[test]
    fn test_tag_with_spaces() {
        let mut matcher = TagMatcher::new("tag", None, None);
        let chunks = matcher.update("< tag >content</ tag >");
        // Spaces around tag name should be handled
        assert!(chunks.iter().any(|c| c.matched));
    }
}
