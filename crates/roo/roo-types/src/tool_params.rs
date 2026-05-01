//! Tool parameter type definitions for native protocol.
//!
//! Derived from `packages/types/src/tool-params.ts`.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// ReadFileMode
// ---------------------------------------------------------------------------

/// Read mode for the read_file tool.
///
/// Source: `packages/types/src/tool-params.ts` — `ReadFileMode`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReadFileMode {
    /// Simple offset/limit reading (default).
    Slice,
    /// Semantic block extraction based on code structure.
    Indentation,
}

impl Default for ReadFileMode {
    fn default() -> Self {
        Self::Slice
    }
}

// ---------------------------------------------------------------------------
// IndentationParams
// ---------------------------------------------------------------------------

/// Indentation-mode configuration for the read_file tool.
///
/// Source: `packages/types/src/tool-params.ts` — `IndentationParams`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndentationParams {
    /// 1-based line number to anchor indentation extraction.
    pub anchor_line: Option<u64>,
    /// Maximum indentation levels to include above anchor (0 = unlimited).
    pub max_levels: Option<u64>,
    /// Include sibling blocks at the same indentation level.
    pub include_siblings: Option<bool>,
    /// Include file header (imports, comments at top).
    pub include_header: Option<bool>,
    /// Hard cap on lines returned for indentation mode.
    pub max_lines: Option<u64>,
}

// ---------------------------------------------------------------------------
// ReadFileParams (new format)
// ---------------------------------------------------------------------------

/// Parameters for the read_file tool (new format).
///
/// Source: `packages/types/src/tool-params.ts` — `ReadFileParams`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadFileParams {
    /// Path to the file, relative to workspace.
    pub path: String,
    /// Reading mode: "slice" (default) or "indentation".
    pub mode: Option<ReadFileMode>,
    /// 1-based line number to start reading from (slice mode, default: 1).
    pub offset: Option<u64>,
    /// Maximum number of lines to read (default: 2000).
    pub limit: Option<u64>,
    /// Indentation-mode configuration (only used when mode === "indentation").
    pub indentation: Option<IndentationParams>,
}

// ---------------------------------------------------------------------------
// Legacy read_file format
// ---------------------------------------------------------------------------

/// Line range specification for legacy read_file format.
///
/// Source: `packages/types/src/tool-params.ts` — `LineRange`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineRange {
    pub start: u64,
    pub end: u64,
}

/// File entry for legacy read_file format.
///
/// Source: `packages/types/src/tool-params.ts` — `FileEntry`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    /// Path to the file, relative to workspace.
    pub path: String,
    /// Optional list of line ranges to read.
    pub line_ranges: Option<Vec<LineRange>>,
}

/// Legacy parameters for the read_file tool (pre-refactor format).
///
/// Source: `packages/types/src/tool-params.ts` — `LegacyReadFileParams`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyReadFileParams {
    /// Array of file entries to read.
    pub files: Vec<FileEntry>,
    /// Discriminant flag for type narrowing.
    pub _legacy_format: bool,
}

// ---------------------------------------------------------------------------
// Coordinate / Size
// ---------------------------------------------------------------------------

/// 2D coordinate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Coordinate {
    pub x: f64,
    pub y: f64,
}

/// Size dimensions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Size {
    pub width: f64,
    pub height: f64,
}

// ---------------------------------------------------------------------------
// GenerateImageParams
// ---------------------------------------------------------------------------

/// Parameters for the generate_image tool.
///
/// Source: `packages/types/src/tool-params.ts` — `GenerateImageParams`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateImageToolParams {
    pub prompt: String,
    pub path: String,
    pub image: Option<String>,
}
