use crate::text_utils::normalize_string;

/// Computes the Levenshtein distance between two strings using dynamic programming.
fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a_len = a.len();
    let b_len = b.len();

    if a_len == 0 {
        return b_len;
    }
    if b_len == 0 {
        return a_len;
    }

    // Use a single row for space optimization
    let mut prev_row: Vec<usize> = (0..=b_len).collect();
    let mut curr_row: Vec<usize> = vec![0; b_len + 1];

    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();

    for (i, a_ch) in a_chars.iter().enumerate() {
        curr_row[0] = i + 1;
        for (j, b_ch) in b_chars.iter().enumerate() {
            let cost = if a_ch == b_ch { 0 } else { 1 };
            curr_row[j + 1] = (prev_row[j + 1] + 1)
                .min(curr_row[j] + 1)
                .min(prev_row[j] + cost);
        }
        std::mem::swap(&mut prev_row, &mut curr_row);
    }

    prev_row[b_len]
}

/// Computes the similarity between two strings using Levenshtein distance.
/// Returns a value between 0.0 and 1.0, where 1.0 is an exact match.
/// Empty search returns 0.
///
/// Port of `getSimilarity` from `multi-search-replace.ts`.
pub fn get_similarity(original: &str, search: &str) -> f64 {
    // Empty searches are no longer supported
    if search.is_empty() {
        return 0.0;
    }

    // Use the normalizeString utility to handle smart quotes and other special characters
    let normalized_original = normalize_string(original);
    let normalized_search = normalize_string(search);

    if normalized_original == normalized_search {
        return 1.0;
    }

    // Calculate Levenshtein distance
    let dist = levenshtein_distance(&normalized_original, &normalized_search);

    // Calculate similarity ratio (0 to 1, where 1 is an exact match)
    let max_length = normalized_original.len().max(normalized_search.len());
    if max_length == 0 {
        return 1.0;
    }

    1.0 - (dist as f64) / (max_length as f64)
}

/// Result of a fuzzy search operation.
#[derive(Debug, Clone)]
pub struct FuzzySearchResult {
    pub best_score: f64,
    pub best_match_index: i64,
    pub best_match_content: String,
}

/// Performs a "middle-out" search of `lines` (between [start_index, end_index]) to find
/// the slice that is most similar to `search_chunk`. Returns the best score, index, and matched text.
///
/// Port of `fuzzySearch` from `multi-search-replace.ts`.
pub fn fuzzy_search(
    lines: &[String],
    search_chunk: &str,
    start_index: usize,
    end_index: usize,
) -> FuzzySearchResult {
    let mut best_score = 0.0;
    let mut best_match_index: i64 = -1;
    let mut best_match_content = String::new();
    let search_len = search_chunk.split('\n').count();
    // Handle \r\n as well
    let search_len = if search_chunk.contains("\r\n") {
        search_chunk.split("\r\n").count()
    } else {
        search_len
    };

    // Middle-out from the midpoint
    let mid_point = (start_index + end_index) / 2;
    let mut left_index = mid_point as i64;
    let mut right_index = (mid_point + 1) as i64;

    let start_index_i64 = start_index as i64;
    let end_index_i64 = end_index as i64;
    let search_len_i64 = search_len as i64;

    while left_index >= start_index_i64 || right_index <= end_index_i64 - search_len_i64 {
        if left_index >= start_index_i64 {
            let left = left_index as usize;
            if left + search_len <= lines.len() {
                let original_chunk = lines[left..(left + search_len)].join("\n");
                let similarity = get_similarity(&original_chunk, search_chunk);
                if similarity > best_score {
                    best_score = similarity;
                    best_match_index = left_index;
                    best_match_content = original_chunk;
                }
            }
            left_index -= 1;
        }

        if right_index <= end_index_i64 - search_len_i64 {
            let right = right_index as usize;
            if right + search_len <= lines.len() {
                let original_chunk = lines[right..(right + search_len)].join("\n");
                let similarity = get_similarity(&original_chunk, search_chunk);
                if similarity > best_score {
                    best_score = similarity;
                    best_match_index = right_index;
                    best_match_content = original_chunk;
                }
            }
            right_index += 1;
        }
    }

    FuzzySearchResult {
        best_score,
        best_match_index,
        best_match_content,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_levenshtein_distance_identical() {
        assert_eq!(levenshtein_distance("hello", "hello"), 0);
    }

    #[test]
    fn test_levenshtein_distance_empty() {
        assert_eq!(levenshtein_distance("", "hello"), 5);
        assert_eq!(levenshtein_distance("hello", ""), 5);
    }

    #[test]
    fn test_levenshtein_distance_basic() {
        assert_eq!(levenshtein_distance("kitten", "sitting"), 3);
    }

    #[test]
    fn test_get_similarity_identical() {
        let sim = get_similarity("hello world", "hello world");
        assert!((sim - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_get_similarity_empty_search() {
        let sim = get_similarity("hello", "");
        assert!((sim - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_get_similarity_different() {
        let sim = get_similarity("hello", "world");
        assert!(sim < 0.5);
    }

    #[test]
    fn test_get_similarity_smart_quotes() {
        let sim = get_similarity(
            "\u{201C}hello\u{201D}",
            "\"hello\"",
        );
        assert!((sim - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_fuzzy_search_basic() {
        let lines: Vec<String> = vec![
            "line 0".to_string(),
            "line 1".to_string(),
            "line 2".to_string(),
            "line 3".to_string(),
            "line 4".to_string(),
        ];
        let result = fuzzy_search(&lines, "line 2", 0, 5);
        assert_eq!(result.best_match_index, 2);
        assert!((result.best_score - 1.0).abs() < f64::EPSILON);
    }
}
