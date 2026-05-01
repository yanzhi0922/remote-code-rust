//! # Roo Editor
//!
//! Editor integration for Roo Code Rust — diff view, file editing, undo stack,
//! markdown export, XLSX text extraction, and image handling.

pub mod types;
pub mod diff_view;
pub mod file_editor;
pub mod undo_stack;
pub mod extract_text;
pub mod export_markdown;
pub mod extract_xlsx;
pub mod image_handler;
pub mod indentation_reader;
pub mod line_counter;
pub mod read_lines;

// Re-export the primary public API at the crate root for convenience.
pub use types::{DiffViewOptions, EditOperation, EditResult, EditorState, FileChange};
pub use diff_view::{DiffViewError, DiffViewProvider};
pub use file_editor::{DiffTag, FileEditor, FileEditorError, LineDiff};
pub use undo_stack::UndoStack;
pub use export_markdown::{
    get_task_file_name, format_content_block_to_markdown, conversation_to_markdown,
    write_markdown_to_file, find_tool_name, ContentBlock, ConversationMessage,
};
pub use extract_xlsx::{extract_text_from_xlsx_file, extract_text_from_xlsx_bytes, XlsxError};
pub use image_handler::{
    is_file_path, parse_data_uri, save_image_to_temp, save_image_to_file,
    image_to_data_uri, resolve_image_path, ImageHandlerError, ParsedDataUri,
};
