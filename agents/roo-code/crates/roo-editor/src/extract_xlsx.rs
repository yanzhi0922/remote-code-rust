//! Extract Text from XLSX
//!
//! Extracts text content from XLSX spreadsheet files.
//! Mirrors `extract-text-from-xlsx.ts`.

use std::path::Path;

use calamine::{Data, Reader, Sheets, open_workbook_auto, open_workbook_auto_from_rs};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors during XLSX text extraction.
#[derive(Debug, thiserror::Error)]
pub enum XlsxError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("XLSX parsing error: {0}")]
    ParseError(String),
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum number of rows to process per sheet.
const ROW_LIMIT: u32 = 50000;

// ---------------------------------------------------------------------------
// XLSX text extraction
// ---------------------------------------------------------------------------

/// Extract text from an XLSX file at the given path.
///
/// Source: `.research/Roo-Code/src/integrations/misc/extract-text-from-xlsx.ts`
pub fn extract_text_from_xlsx_file(file_path: &Path) -> Result<String, XlsxError> {
    let mut workbook = open_workbook_auto(file_path)
        .map_err(|e| XlsxError::ParseError(e.to_string()))?;

    extract_text_from_workbook(&mut workbook)
}

/// Extract text from XLSX bytes.
pub fn extract_text_from_xlsx_bytes(data: &[u8]) -> Result<String, XlsxError> {
    let cursor = std::io::Cursor::new(data);
    let mut workbook: Sheets<_> = open_workbook_auto_from_rs(cursor)
        .map_err(|e| XlsxError::ParseError(e.to_string()))?;

    extract_text_from_workbook(&mut workbook)
}

fn extract_text_from_workbook<R: std::io::Seek + std::io::Read>(
    workbook: &mut Sheets<R>,
) -> Result<String, XlsxError> {
    let mut excel_text = String::new();

    let sheet_names = workbook.sheet_names().to_vec();
    for name in &sheet_names {
        if let Ok(range) = workbook.worksheet_range(name) {
            excel_text.push_str(&format!("--- Sheet: {} ---\n", name));

            let mut row_count = 0u32;
            for row in range.rows() {
                row_count += 1;
                if row_count > ROW_LIMIT {
                    excel_text.push_str(&format!("[... truncated at row {} ...]\n", row_count));
                    break;
                }

                let row_texts: Vec<String> = row
                    .iter()
                    .map(|cell: &Data| format_cell_value(cell))
                    .collect();

                let has_content = row_texts.iter().any(|t: &String| !t.trim().is_empty());
                if has_content {
                    excel_text.push_str(&row_texts.join("\t"));
                    excel_text.push('\n');
                }
            }

            excel_text.push('\n');
        }
    }

    Ok(excel_text.trim().to_string())
}

/// Format a calamine cell value to a string.
fn format_cell_value(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(s) => s.clone(),
        Data::Float(f) => {
            if *f == (*f as i64) as f64 {
                format!("{}", *f as i64)
            } else {
                format!("{}", f)
            }
        }
        Data::Int(i) => format!("{}", i),
        Data::Bool(b) => format!("{}", b),
        Data::DateTime(dt) => {
            if let Some(date) = dt.as_datetime() {
                date.format("%Y-%m-%d").to_string()
            } else {
                format!("{}", dt)
            }
        }
        Data::DateTimeIso(d) => d.clone(),
        Data::DurationIso(d) => d.clone(),
        Data::Error(e) => format!("[Error: {:?}]", e),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_cell_empty() {
        assert_eq!(format_cell_value(&Data::Empty), "");
    }

    #[test]
    fn test_format_cell_string() {
        assert_eq!(format_cell_value(&Data::String("hello".to_string())), "hello");
    }

    #[test]
    fn test_format_cell_int() {
        assert_eq!(format_cell_value(&Data::Int(42)), "42");
    }

    #[test]
    fn test_format_cell_float() {
        assert_eq!(format_cell_value(&Data::Float(3.14)), "3.14");
    }

    #[test]
    fn test_format_cell_bool() {
        assert_eq!(format_cell_value(&Data::Bool(true)), "true");
    }

    #[test]
    fn test_format_cell_error() {
        let result = format_cell_value(&Data::Error(calamine::CellErrorType::Div0));
        assert!(result.contains("[Error:"));
    }

    #[test]
    fn test_extract_from_nonexistent_file() {
        let result = extract_text_from_xlsx_file(Path::new("/nonexistent.xlsx"));
        assert!(result.is_err());
    }
}
