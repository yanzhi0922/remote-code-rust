//! # Roo Editor
//!
//! Editor integration for Roo Code Rust — diff view, file editing, undo stack,
//! markdown export, XLSX text extraction, and image handling.

pub mod diff_view;
pub mod export_markdown;
pub mod extract_text;
pub mod extract_xlsx;
pub mod file_editor;
pub mod image_handler;
pub mod indentation_reader;
pub mod line_counter;
pub mod read_lines;
pub mod types;
pub mod undo_stack;

// Re-export the primary public API at the crate root for convenience.
pub use diff_view::{DiffViewError, DiffViewProvider};
pub use export_markdown::{
    ContentBlock, ConversationMessage, conversation_to_markdown, find_tool_name,
    format_content_block_to_markdown, get_task_file_name, write_markdown_to_file,
};
pub use extract_xlsx::{XlsxError, extract_text_from_xlsx_bytes, extract_text_from_xlsx_file};
pub use file_editor::{DiffTag, FileEditor, FileEditorError, LineDiff};
pub use image_handler::{
    ImageHandlerError, ParsedDataUri, image_to_data_uri, is_file_path, parse_data_uri,
    resolve_image_path, save_image_to_file, save_image_to_temp,
};
pub use types::{DiffViewOptions, EditOperation, EditResult, EditorState, FileChange};
pub use undo_stack::UndoStack;
