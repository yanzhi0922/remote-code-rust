/// Validates the sequencing of diff markers (<<<<<<< SEARCH, =======, >>>>>>> REPLACE).
///
/// Port of `validateMarkerSequencing` from `multi-search-replace.ts`.
/// All error messages must match the TypeScript source exactly.

use regex::Regex;

/// State machine states for marker validation.
#[derive(Debug, Clone, Copy, PartialEq)]
enum MarkerState {
    Start,
    AfterSearch,
    AfterSeparator,
}

/// Result of marker sequencing validation.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub success: bool,
    pub error: Option<String>,
}

impl ValidationResult {
    fn ok() -> Self {
        Self {
            success: true,
            error: None,
        }
    }

    fn err(msg: String) -> Self {
        Self {
            success: false,
            error: Some(msg),
        }
    }
}

// Constants matching the TypeScript source
const SEP: &str = "=======";
const REPLACE: &str = ">>>>>>> REPLACE";
const SEARCH_PREFIX: &str = "<<<<<<<";
const REPLACE_PREFIX: &str = ">>>>>>>";

fn report_merge_conflict_error(found: &str, _expected: &str, line: usize) -> ValidationResult {
    // The SEARCH_PATTERN source without regex anchors
    let search = "<<<<<<< SEARCH>?";
    ValidationResult::err(format!(
        "ERROR: Special marker '{}' found in your diff content at line {}:\n\
         \n\
         When removing merge conflict markers like '{}' from files, you MUST escape them\n\
         in your SEARCH section by prepending a backslash (\\) at the beginning of the line:\n\
         \n\
         CORRECT FORMAT:\n\
         \n\
         <<<<<<< SEARCH\n\
         content before\n\
         \\{}    <-- Note the backslash here in this example\n\
         content after\n\
         =======\n\
         replacement content\n\
         >>>>>>> REPLACE\n\
         \n\
         Without escaping, the system confuses your content with diff syntax markers.\n\
         You may use multiple diff blocks in a single diff request, but ANY of ONLY the following separators that occur within SEARCH or REPLACE content must be escaped, as follows:\n\
         \\{}\n\
         \\{}\n\
         \\{}",
        found, line, found, found, search, SEP, REPLACE
    ))
}

fn report_invalid_diff_error(found: &str, expected: &str, line: usize) -> ValidationResult {
    ValidationResult::err(format!(
        "ERROR: Diff block is malformed: marker '{}' found in your diff content at line {}. Expected: {}\n\
         \n\
         CORRECT FORMAT:\n\
         \n\
         <<<<<<< SEARCH\n\
         :start_line: (required) The line number of original content where the search block starts.\n\
         -------\n\
         [exact content to find including whitespace]\n\
         =======\n\
         [new content to replace with]\n\
         >>>>>>> REPLACE",
        found, line, expected
    ))
}

fn report_line_marker_in_replace_error(marker: &str, line: usize) -> ValidationResult {
    ValidationResult::err(format!(
        "ERROR: Invalid line marker '{}' found in REPLACE section at line {}\n\
         \n\
         Line markers (:start_line: and :end_line:) are only allowed in SEARCH sections.\n\
         \n\
         CORRECT FORMAT:\n\
         <<<<<<< SEARCH\n\
         :start_line:5\n\
         content to find\n\
         =======\n\
         replacement content\n\
         >>>>>>> REPLACE\n\
         \n\
         INCORRECT FORMAT:\n\
         <<<<<<< SEARCH\n\
         content to find\n\
         =======\n\
         :start_line:5    <-- Invalid location\n\
         replacement content\n\
         >>>>>>> REPLACE",
        marker, line
    ))
}

/// Validates that diff markers appear in the correct sequence.
///
/// The valid sequence is: `<<<<<<< SEARCH` → `=======` → `>>>>>>> REPLACE`
/// Multiple blocks are allowed. Various error conditions are detected and
/// reported with helpful messages matching the TypeScript source exactly.
pub fn validate_marker_sequencing(diff_content: &str) -> ValidationResult {
    let search_pattern = Regex::new(r"^<<<<<<< SEARCH>?$").unwrap();
    let mut state = MarkerState::Start;
    let mut line_num: usize = 0;

    let lines: Vec<&str> = diff_content.split('\n').collect();

    // Count markers to detect likely bad structure
    let search_count = lines
        .iter()
        .filter(|l| search_pattern.is_match(l.trim()))
        .count();
    let sep_count = lines.iter().filter(|l| l.trim() == SEP).count();
    let replace_count = lines
        .iter()
        .filter(|l| l.trim() == REPLACE)
        .count();

    let likely_bad_structure = search_count != replace_count || sep_count < search_count;

    for line in &lines {
        line_num += 1;
        let marker = line.trim();

        // Check for line markers in REPLACE sections (but allow escaped ones)
        if state == MarkerState::AfterSeparator {
            if marker.starts_with(":start_line:") && !line.trim().starts_with("\\:start_line:") {
                return report_line_marker_in_replace_error(":start_line:", line_num);
            }
            if marker.starts_with(":end_line:") && !line.trim().starts_with("\\:end_line:") {
                return report_line_marker_in_replace_error(":end_line:", line_num);
            }
        }

        match state {
            MarkerState::Start => {
                if marker == SEP {
                    return if likely_bad_structure {
                        report_invalid_diff_error(SEP, "<<<<<<< SEARCH>", line_num)
                    } else {
                        report_merge_conflict_error(SEP, "<<<<<<< SEARCH>", line_num)
                    };
                }
                if marker == REPLACE {
                    return report_invalid_diff_error(REPLACE, "<<<<<<< SEARCH>", line_num);
                }
                if marker.starts_with(REPLACE_PREFIX) {
                    return report_merge_conflict_error(marker, "<<<<<<< SEARCH>", line_num);
                }
                if search_pattern.is_match(marker) {
                    state = MarkerState::AfterSearch;
                } else if marker.starts_with(SEARCH_PREFIX) {
                    return report_merge_conflict_error(marker, "<<<<<<< SEARCH>", line_num);
                }
            }
            MarkerState::AfterSearch => {
                if search_pattern.is_match(marker) {
                    return report_invalid_diff_error("<<<<<<< SEARCH>?", SEP, line_num);
                }
                if marker.starts_with(SEARCH_PREFIX) {
                    return report_merge_conflict_error(marker, "<<<<<<< SEARCH>", line_num);
                }
                if marker == REPLACE {
                    return report_invalid_diff_error(REPLACE, SEP, line_num);
                }
                if marker.starts_with(REPLACE_PREFIX) {
                    return report_merge_conflict_error(marker, "<<<<<<< SEARCH>", line_num);
                }
                if marker == SEP {
                    state = MarkerState::AfterSeparator;
                }
            }
            MarkerState::AfterSeparator => {
                if search_pattern.is_match(marker) {
                    return report_invalid_diff_error("<<<<<<< SEARCH>?", REPLACE, line_num);
                }
                if marker.starts_with(SEARCH_PREFIX) {
                    return report_merge_conflict_error(marker, REPLACE, line_num);
                }
                if marker == SEP {
                    return if likely_bad_structure {
                        report_invalid_diff_error(SEP, REPLACE, line_num)
                    } else {
                        report_merge_conflict_error(SEP, REPLACE, line_num)
                    };
                }
                if marker == REPLACE {
                    state = MarkerState::Start;
                } else if marker.starts_with(REPLACE_PREFIX) {
                    return report_merge_conflict_error(marker, REPLACE, line_num);
                }
            }
        }
    }

    if state == MarkerState::Start {
        ValidationResult::ok()
    } else {
        let expected = if state == MarkerState::AfterSearch {
            "======="
        } else {
            ">>>>>>> REPLACE"
        };
        ValidationResult::err(format!(
            "ERROR: Unexpected end of sequence: Expected '{}' was not found.",
            expected
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validates_correct_marker_sequence() {
        let diff = "<<<<<<< SEARCH\nsome content\n=======\nnew content\n>>>>>>> REPLACE";
        let result = validate_marker_sequencing(diff);
        assert!(result.success);
    }

    #[test]
    fn test_validates_correct_marker_sequence_with_extra_arrow() {
        let diff = "<<<<<<< SEARCH>\nsome content\n=======\nnew content\n>>>>>>> REPLACE";
        let result = validate_marker_sequencing(diff);
        assert!(result.success);
    }

    #[test]
    fn test_validates_multiple_correct_marker_sequences() {
        let diff = "\
<<<<<<< SEARCH
content1
=======
new content1
>>>>>>> REPLACE
<<<<<<< SEARCH
content2
=======
new content2
>>>>>>> REPLACE";
        let result = validate_marker_sequencing(diff);
        assert!(result.success);
    }

    #[test]
    fn test_detects_separator_before_search() {
        let diff = "=======\ncontent\n>>>>>>> REPLACE";
        let result = validate_marker_sequencing(diff);
        assert!(!result.success);
        assert!(result.error.as_ref().unwrap().contains("======="));
    }

    #[test]
    fn test_detects_missing_separator() {
        let diff = "<<<<<<< SEARCH\ncontent\n>>>>>>> REPLACE";
        let result = validate_marker_sequencing(diff);
        assert!(!result.success);
    }

    #[test]
    fn test_detects_two_separators() {
        let diff = "<<<<<<< SEARCH\ncontent\n=======\n=======\n>>>>>>> REPLACE";
        let result = validate_marker_sequencing(diff);
        assert!(!result.success);
    }

    #[test]
    fn test_detects_replace_before_separator() {
        let diff = "<<<<<<< SEARCH\ncontent\n>>>>>>>";
        let result = validate_marker_sequencing(diff);
        assert!(!result.success);
    }

    #[test]
    fn test_detects_incomplete_sequence() {
        let diff = "<<<<<<< SEARCH\ncontent\n=======\nnew content";
        let result = validate_marker_sequencing(diff);
        assert!(!result.success);
        assert!(result.error.as_ref().unwrap().contains(">>>>>>> REPLACE"));
    }

    #[test]
    fn test_rejects_start_line_in_replace() {
        let diff = "\
<<<<<<< SEARCH
content
=======
:start_line:5
replacement
>>>>>>> REPLACE";
        let result = validate_marker_sequencing(diff);
        assert!(!result.success);
        assert!(result.error.as_ref().unwrap().contains(":start_line:"));
    }

    #[test]
    fn test_allows_escaped_start_line_in_replace() {
        let diff = "\
<<<<<<< SEARCH
content
=======
\\:start_line:5
replacement
>>>>>>> REPLACE";
        let result = validate_marker_sequencing(diff);
        assert!(result.success);
    }

    #[test]
    fn test_rejects_end_line_in_replace() {
        let diff = "\
<<<<<<< SEARCH
content
=======
:end_line:10
replacement
>>>>>>> REPLACE";
        let result = validate_marker_sequencing(diff);
        assert!(!result.success);
        assert!(result.error.as_ref().unwrap().contains(":end_line:"));
    }

    #[test]
    fn test_allows_escaped_end_line_in_replace() {
        let diff = "\
<<<<<<< SEARCH
content
=======
\\:end_line:10
replacement
>>>>>>> REPLACE";
        let result = validate_marker_sequencing(diff);
        assert!(result.success);
    }
}
