//! Core LSP types aligned with the Language Server Protocol specification.
//!
//! Provides serialisable representations of common LSP data structures.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Position / Range / Location
// ---------------------------------------------------------------------------

/// A position in a text document (zero-based).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    /// Line number (0-based).
    pub line: u32,
    /// Character offset within the line (0-based, UTF-16 code units).
    pub character: u32,
}

impl Position {
    /// Create a new position.
    #[must_use]
    pub fn new(line: u32, character: u32) -> Self {
        Self { line, character }
    }

    /// The zero-origin position (0, 0).
    #[must_use]
    pub fn zero() -> Self {
        Self { line: 0, character: 0 }
    }

    /// Whether this position is at the start of a document.
    #[must_use]
    pub fn is_start(&self) -> bool {
        self.line == 0 && self.character == 0
    }
}

impl PartialOrd for Position {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Position {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.line
            .cmp(&other.line)
            .then_with(|| self.character.cmp(&other.character))
    }
}

/// A range in a text document (half-open: `[start, end)`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Range {
    /// Start position (inclusive).
    pub start: Position,
    /// End position (exclusive).
    pub end: Position,
}

impl Range {
    /// Create a new range.
    #[must_use]
    pub fn new(start: Position, end: Position) -> Self {
        Self { start, end }
    }

    /// A range covering a single line.
    #[must_use]
    pub fn single_line(line: u32, start_char: u32, end_char: u32) -> Self {
        Self {
            start: Position::new(line, start_char),
            end: Position::new(line, end_char),
        }
    }

    /// Whether the range is empty (start == end).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Whether `pos` falls within this range.
    #[must_use]
    pub fn contains(&self, pos: Position) -> bool {
        pos >= self.start && pos < self.end
    }
}

/// A location in a specific document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Location {
    /// Document URI.
    pub uri: String,
    /// Range within the document.
    pub range: Range,
}

impl Location {
    /// Create a new location.
    #[must_use]
    pub fn new(uri: impl Into<String>, range: Range) -> Self {
        Self {
            uri: uri.into(),
            range,
        }
    }
}

// ---------------------------------------------------------------------------
// Diagnostic
// ---------------------------------------------------------------------------

/// Diagnostic severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosticSeverity {
    /// Reports an error.
    Error,
    /// Reports a warning.
    Warning,
    /// Reports an information.
    Information,
    /// Reports a hint.
    Hint,
}

impl DiagnosticSeverity {
    /// Returns the numeric LSP severity value.
    #[must_use]
    pub fn as_u32(&self) -> u32 {
        match self {
            Self::Error => 1,
            Self::Warning => 2,
            Self::Information => 3,
            Self::Hint => 4,
        }
    }
}

/// A diagnostic item representing a problem in a document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    /// The range at which the message applies.
    pub range: Range,
    /// The diagnostic's severity.
    pub severity: Option<DiagnosticSeverity>,
    /// The diagnostic's code.
    pub code: Option<String>,
    /// A human-readable message.
    pub message: String,
    /// The source of the diagnostic (e.g. "rustc").
    pub source: Option<String>,
    /// Additional metadata.
    #[serde(default)]
    pub data: Option<serde_json::Value>,
}

impl Diagnostic {
    /// Create a new diagnostic with a range and message.
    #[must_use]
    pub fn new(range: Range, message: impl Into<String>) -> Self {
        Self {
            range,
            severity: None,
            code: None,
            message: message.into(),
            source: None,
            data: None,
        }
    }

    /// Set the severity.
    #[must_use]
    pub fn with_severity(mut self, severity: DiagnosticSeverity) -> Self {
        self.severity = Some(severity);
        self
    }

    /// Set the source.
    #[must_use]
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Set the code.
    #[must_use]
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    /// Whether this diagnostic represents an error.
    #[must_use]
    pub fn is_error(&self) -> bool {
        matches!(self.severity, Some(DiagnosticSeverity::Error))
    }

    /// Whether this diagnostic represents a warning.
    #[must_use]
    pub fn is_warning(&self) -> bool {
        matches!(self.severity, Some(DiagnosticSeverity::Warning))
    }
}

// ---------------------------------------------------------------------------
// CompletionItem
// ---------------------------------------------------------------------------

/// A completion item presented to the user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionItem {
    /// The label of this completion item.
    pub label: String,
    /// A human-readable string with additional information.
    #[serde(default)]
    pub detail: Option<String>,
    /// A documentation string.
    #[serde(default)]
    pub documentation: Option<String>,
    /// The kind of completion item.
    #[serde(default)]
    pub kind: Option<u32>,
    /// Pre-select this item when showing the completion list.
    #[serde(default)]
    pub preselect: bool,
    /// Additional text edits when selecting this completion.
    #[serde(default)]
    pub additional_text_edits: Vec<TextEdit>,
}

impl CompletionItem {
    /// Create a new completion item with a label.
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            detail: None,
            documentation: None,
            kind: None,
            preselect: false,
            additional_text_edits: Vec::new(),
        }
    }

    /// Set the detail text.
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

/// A text edit operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextEdit {
    /// The range to replace.
    pub range: Range,
    /// The new text.
    pub new_text: String,
}

// ---------------------------------------------------------------------------
// Hover
// ---------------------------------------------------------------------------

/// Hover result containing documentation for a symbol.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hover {
    /// The hover's content as Markdown.
    pub contents: String,
    /// An optional range that the hover applies to.
    pub range: Option<Range>,
}

impl Hover {
    /// Create a new hover with Markdown content.
    #[must_use]
    pub fn new(contents: impl Into<String>) -> Self {
        Self {
            contents: contents.into(),
            range: None,
        }
    }

    /// Set the range.
    #[must_use]
    pub fn with_range(mut self, range: Range) -> Self {
        self.range = Some(range);
        self
    }
}

// ---------------------------------------------------------------------------
// DocumentSymbol
// ---------------------------------------------------------------------------

/// A symbol in a document (function, class, variable, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSymbol {
    /// Name of the symbol.
    pub name: String,
    /// More detail for this symbol.
    #[serde(default)]
    pub detail: Option<String>,
    /// Kind of symbol (e.g. Function = 12, Class = 5).
    pub kind: u32,
    /// Range enclosing this symbol (including leading/trailing whitespace).
    pub range: Range,
    /// Range of the symbol's name identifier.
    pub selection_range: Range,
    /// Children of this symbol.
    #[serde(default)]
    pub children: Vec<DocumentSymbol>,
}

impl DocumentSymbol {
    /// Create a new document symbol.
    #[must_use]
    pub fn new(name: impl Into<String>, kind: u32, range: Range, selection_range: Range) -> Self {
        Self {
            name: name.into(),
            detail: None,
            kind,
            range,
            selection_range,
            children: Vec::new(),
        }
    }

    /// Add a child symbol.
    pub fn add_child(&mut self, child: DocumentSymbol) {
        self.children.push(child);
    }
}

// ---------------------------------------------------------------------------
// LspMessage / LspResponse
// ---------------------------------------------------------------------------

/// An LSP request or notification message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspMessage {
    /// JSON-RPC version (always "2.0").
    pub jsonrpc: String,
    /// Request ID (absent for notifications).
    #[serde(default)]
    pub id: Option<u64>,
    /// Method name.
    pub method: String,
    /// Method parameters.
    #[serde(default)]
    pub params: Option<serde_json::Value>,
}

impl LspMessage {
    /// Create a request message.
    #[must_use]
    pub fn request(id: u64, method: impl Into<String>, params: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: Some(id),
            method: method.into(),
            params,
        }
    }

    /// Create a notification message (no ID).
    #[must_use]
    pub fn notification(method: impl Into<String>, params: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: None,
            method: method.into(),
            params,
        }
    }

    /// Whether this is a request (has an ID).
    #[must_use]
    pub fn is_request(&self) -> bool {
        self.id.is_some()
    }

    /// Whether this is a notification (no ID).
    #[must_use]
    pub fn is_notification(&self) -> bool {
        self.id.is_none()
    }

    /// Serialize to a JSON string.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails.
    pub fn to_json(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string(self)?)
    }

    /// Deserialize from a JSON string.
    ///
    /// # Errors
    ///
    /// Returns an error if deserialization fails.
    pub fn from_json(json: &str) -> anyhow::Result<Self> {
        Ok(serde_json::from_str(json)?)
    }
}

/// An LSP response message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspResponse {
    /// JSON-RPC version (always "2.0").
    pub jsonrpc: String,
    /// Request ID this response corresponds to.
    pub id: u64,
    /// Result (present on success).
    #[serde(default)]
    pub result: Option<serde_json::Value>,
    /// Error (present on failure).
    #[serde(default)]
    pub error: Option<LspError>,
}

impl LspResponse {
    /// Create a successful response.
    #[must_use]
    pub fn success(id: u64, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Create an error response.
    #[must_use]
    pub fn error(id: u64, code: i64, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(LspError {
                code,
                message: message.into(),
                data: None,
            }),
        }
    }

    /// Whether this response indicates success.
    #[must_use]
    pub fn is_success(&self) -> bool {
        self.error.is_none()
    }

    /// Serialize to a JSON string.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails.
    pub fn to_json(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string(self)?)
    }

    /// Deserialize from a JSON string.
    ///
    /// # Errors
    ///
    /// Returns an error if deserialization fails.
    pub fn from_json(json: &str) -> anyhow::Result<Self> {
        Ok(serde_json::from_str(json)?)
    }
}

/// An LSP error object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspError {
    /// Error code.
    pub code: i64,
    /// Error message.
    pub message: String,
    /// Additional error data.
    #[serde(default)]
    pub data: Option<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Position -------------------------------------------------------------

    #[test]
    fn position_new() {
        let p = Position::new(5, 10);
        assert_eq!(p.line, 5);
        assert_eq!(p.character, 10);
    }

    #[test]
    fn position_zero() {
        let p = Position::zero();
        assert!(p.is_start());
        assert_eq!(p, Position::new(0, 0));
    }

    #[test]
    fn position_ordering() {
        assert!(Position::new(0, 0) < Position::new(0, 1));
        assert!(Position::new(1, 0) > Position::new(0, 10));
        assert!(Position::new(5, 5) == Position::new(5, 5));
    }

    #[test]
    fn position_serialization() {
        let p = Position::new(3, 7);
        let json = serde_json::to_string(&p).expect("serialize");
        let back: Position = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(p, back);
    }

    // -- Range ----------------------------------------------------------------

    #[test]
    fn range_new() {
        let r = Range::new(Position::new(1, 0), Position::new(1, 5));
        assert_eq!(r.start, Position::new(1, 0));
        assert_eq!(r.end, Position::new(1, 5));
    }

    #[test]
    fn range_single_line() {
        let r = Range::single_line(3, 5, 10);
        assert_eq!(r.start.line, 3);
        assert_eq!(r.end.line, 3);
    }

    #[test]
    fn range_is_empty() {
        let r = Range::new(Position::new(1, 2), Position::new(1, 2));
        assert!(r.is_empty());
    }

    #[test]
    fn range_not_empty() {
        let r = Range::new(Position::new(1, 2), Position::new(1, 3));
        assert!(!r.is_empty());
    }

    #[test]
    fn range_contains() {
        let r = Range::new(Position::new(1, 5), Position::new(3, 0));
        assert!(r.contains(Position::new(2, 0)));
        assert!(r.contains(Position::new(1, 5)));
        assert!(!r.contains(Position::new(3, 0)));
        assert!(!r.contains(Position::new(0, 0)));
    }

    #[test]
    fn range_serialization() {
        let r = Range::single_line(1, 2, 3);
        let json = serde_json::to_string(&r).expect("serialize");
        let back: Range = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(r, back);
    }

    // -- Location -------------------------------------------------------------

    #[test]
    fn location_new() {
        let loc = Location::new("file:///test.rs", Range::single_line(1, 0, 5));
        assert_eq!(loc.uri, "file:///test.rs");
    }

    #[test]
    fn location_serialization() {
        let loc = Location::new("file:///a.rs", Range::single_line(0, 0, 1));
        let json = serde_json::to_string(&loc).expect("serialize");
        let back: Location = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(loc, back);
    }

    // -- DiagnosticSeverity ---------------------------------------------------

    #[test]
    fn severity_values() {
        assert_eq!(DiagnosticSeverity::Error.as_u32(), 1);
        assert_eq!(DiagnosticSeverity::Warning.as_u32(), 2);
        assert_eq!(DiagnosticSeverity::Information.as_u32(), 3);
        assert_eq!(DiagnosticSeverity::Hint.as_u32(), 4);
    }

    // -- Diagnostic -----------------------------------------------------------

    #[test]
    fn diagnostic_new() {
        let d = Diagnostic::new(Range::single_line(1, 0, 5), "test error");
        assert!(d.severity.is_none());
        assert!(d.source.is_none());
        assert_eq!(d.message, "test error");
    }

    #[test]
    fn diagnostic_builder() {
        let d = Diagnostic::new(Range::single_line(1, 0, 5), "err")
            .with_severity(DiagnosticSeverity::Error)
            .with_source("rustc")
            .with_code("E0001");
        assert!(d.is_error());
        assert!(!d.is_warning());
        assert_eq!(d.source.as_deref(), Some("rustc"));
        assert_eq!(d.code.as_deref(), Some("E0001"));
    }

    #[test]
    fn diagnostic_warning() {
        let d = Diagnostic::new(Range::single_line(1, 0, 5), "warn")
            .with_severity(DiagnosticSeverity::Warning);
        assert!(d.is_warning());
        assert!(!d.is_error());
    }

    #[test]
    fn diagnostic_serialization() {
        let d = Diagnostic::new(Range::single_line(1, 0, 5), "test")
            .with_severity(DiagnosticSeverity::Error);
        let json = serde_json::to_string(&d).expect("serialize");
        let back: Diagnostic = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.message, "test");
        assert!(back.is_error());
    }

    // -- CompletionItem -------------------------------------------------------

    #[test]
    fn completion_item_new() {
        let item = CompletionItem::new("foo");
        assert_eq!(item.label, "foo");
        assert!(item.detail.is_none());
    }

    #[test]
    fn completion_item_with_detail() {
        let item = CompletionItem::new("foo").with_detail("fn foo()");
        assert_eq!(item.detail.as_deref(), Some("fn foo()"));
    }

    #[test]
    fn completion_item_serialization() {
        let item = CompletionItem::new("bar");
        let json = serde_json::to_string(&item).expect("serialize");
        let back: CompletionItem = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(item.label, back.label);
    }

    // -- Hover ----------------------------------------------------------------

    #[test]
    fn hover_new() {
        let h = Hover::new("# Title");
        assert_eq!(h.contents, "# Title");
        assert!(h.range.is_none());
    }

    #[test]
    fn hover_with_range() {
        let h = Hover::new("docs").with_range(Range::single_line(1, 0, 5));
        assert!(h.range.is_some());
    }

    #[test]
    fn hover_serialization() {
        let h = Hover::new("docs").with_range(Range::single_line(1, 0, 5));
        let json = serde_json::to_string(&h).expect("serialize");
        let back: Hover = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(h.contents, back.contents);
    }

    // -- DocumentSymbol -------------------------------------------------------

    #[test]
    fn document_symbol_new() {
        let sym = DocumentSymbol::new(
            "main",
            12, // Function
            Range::single_line(1, 0, 20),
            Range::single_line(1, 3, 7),
        );
        assert_eq!(sym.name, "main");
        assert!(sym.children.is_empty());
    }

    #[test]
    fn document_symbol_add_child() {
        let mut parent = DocumentSymbol::new(
            "MyClass",
            5, // Class
            Range::single_line(1, 0, 30),
            Range::single_line(1, 6, 13),
        );
        let child = DocumentSymbol::new(
            "method",
            6, // Method
            Range::single_line(2, 4, 15),
            Range::single_line(2, 7, 13),
        );
        parent.add_child(child);
        assert_eq!(parent.children.len(), 1);
        assert_eq!(parent.children[0].name, "method");
    }

    #[test]
    fn document_symbol_serialization() {
        let sym = DocumentSymbol::new("x", 13, Range::single_line(1, 0, 5), Range::single_line(1, 0, 1));
        let json = serde_json::to_string(&sym).expect("serialize");
        let back: DocumentSymbol = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(sym.name, back.name);
    }

    // -- LspMessage -----------------------------------------------------------

    #[test]
    fn lsp_message_request() {
        let msg = LspMessage::request(1, "textDocument/hover", None);
        assert!(msg.is_request());
        assert!(!msg.is_notification());
        assert_eq!(msg.id, Some(1));
    }

    #[test]
    fn lsp_message_notification() {
        let msg = LspMessage::notification("textDocument/didOpen", None);
        assert!(!msg.is_request());
        assert!(msg.is_notification());
    }

    #[test]
    fn lsp_message_json_roundtrip() {
        let msg = LspMessage::request(
            42,
            "textDocument/completion",
            Some(serde_json::json!({"query": "fo"})),
        );
        let json = msg.to_json().expect("to_json");
        let back = LspMessage::from_json(&json).expect("from_json");
        assert_eq!(back.id, Some(42));
        assert_eq!(back.method, "textDocument/completion");
    }

    #[test]
    fn lsp_message_from_invalid_json() {
        assert!(LspMessage::from_json("not json").is_err());
    }

    // -- LspResponse ----------------------------------------------------------

    #[test]
    fn lsp_response_success() {
        let resp = LspResponse::success(1, serde_json::json!({"items": []}));
        assert!(resp.is_success());
        assert!(resp.result.is_some());
        assert!(resp.error.is_none());
    }

    #[test]
    fn lsp_response_error() {
        let resp = LspResponse::error(1, -32600, "Invalid Request");
        assert!(!resp.is_success());
        assert!(resp.error.is_some());
        assert_eq!(resp.error.as_ref().expect("error").code, -32600);
    }

    #[test]
    fn lsp_response_json_roundtrip() {
        let resp = LspResponse::success(5, serde_json::json!(null));
        let json = resp.to_json().expect("to_json");
        let back = LspResponse::from_json(&json).expect("from_json");
        assert_eq!(back.id, 5);
        assert!(back.is_success());
    }

    #[test]
    fn lsp_response_from_invalid_json() {
        assert!(LspResponse::from_json("bad").is_err());
    }

    // -- TextEdit -------------------------------------------------------------

    #[test]
    fn text_edit_serialization() {
        let edit = TextEdit {
            range: Range::single_line(1, 0, 5),
            new_text: "hello".to_string(),
        };
        let json = serde_json::to_string(&edit).expect("serialize");
        let back: TextEdit = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(edit.new_text, back.new_text);
    }

    // -- LspError -------------------------------------------------------------

    #[test]
    fn lsp_error_serialization() {
        let err = LspError {
            code: -32600,
            message: "Invalid Request".to_string(),
            data: None,
        };
        let json = serde_json::to_string(&err).expect("serialize");
        let back: LspError = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(err.code, back.code);
        assert_eq!(err.message, back.message);
    }
}
