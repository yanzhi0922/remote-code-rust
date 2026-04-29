//! Local BM25-like search over an in-memory inverted index.
//!
//! The [`SearchIndex`] holds skill documents and supports tokenization,
//! TF-IDF scoring with field boosting, and result highlighting.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use tracing::debug;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A skill document that can be indexed and searched.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillDocument {
    /// Unique identifier / slug for the skill.
    pub slug: String,
    /// Human-readable name.
    pub name: String,
    /// Longer description of the skill.
    pub description: String,
    /// Trigger keywords that should boost matching.
    pub triggers: Vec<String>,
}

/// A single search result returned by [`SearchIndex::search`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// Slug of the matched skill.
    pub skill_slug: String,
    /// Relevance score (higher is better).
    pub score: f64,
    /// Terms from the query that matched.
    pub matched_terms: Vec<String>,
    /// Highlighted fragments showing where the match occurred.
    pub highlights: Vec<String>,
}

// ---------------------------------------------------------------------------
// Internal index structures
// ---------------------------------------------------------------------------

/// Per-term statistics kept inside the inverted index.
#[derive(Debug, Clone)]
struct PostingEntry {
    /// Skill slug this posting belongs to.
    slug: String,
    /// Term frequency in each field.
    tf_name: u32,
    tf_description: u32,
    tf_triggers: u32,
}

/// The in-memory search index.
#[derive(Debug, Clone)]
pub struct SearchIndex {
    /// Maps normalised term → list of postings.
    postings: HashMap<String, Vec<PostingEntry>>,
    /// All indexed skill slugs (for dedup / count).
    indexed_slugs: HashSet<String>,
    /// Total number of documents in the index.
    doc_count: u64,
    /// Sum of token counts across all indexed documents (for average document length).
    total_doc_length: u64,
    /// Per-document token counts (slug → total tokens across all fields).
    doc_lengths: HashMap<String, u64>,
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// BM25 parameters.
const K1: f64 = 1.2;
const B: f64 = 0.75;

/// Field weight multipliers.
const WEIGHT_NAME: f64 = 3.0;
const WEIGHT_DESCRIPTION: f64 = 1.0;
const WEIGHT_TRIGGERS: f64 = 2.5;

// ---------------------------------------------------------------------------
// Implementation
// ---------------------------------------------------------------------------

impl SearchIndex {
    /// Create an empty search index.
    pub fn new() -> Self {
        Self {
            postings: HashMap::new(),
            indexed_slugs: HashSet::new(),
            doc_count: 0,
            total_doc_length: 0,
            doc_lengths: HashMap::new(),
        }
    }

    /// Returns the number of indexed skills.
    pub fn len(&self) -> usize {
        self.indexed_slugs.len()
    }

    /// Returns `true` if the index contains no skills.
    pub fn is_empty(&self) -> bool {
        self.indexed_slugs.is_empty()
    }

    /// Index a skill document, making it searchable.
    ///
    /// If a skill with the same slug already exists it is replaced.
    pub fn index_skill(&mut self, skill: &SkillDocument) {
        // Remove old postings for this slug if re-indexing.
        if self.indexed_slugs.contains(&skill.slug) {
            self.remove_skill_postings(&skill.slug);
        } else {
            self.doc_count += 1;
        }
        self.indexed_slugs.insert(skill.slug.clone());

        let name_tokens = tokenize(&skill.name);
        let desc_tokens = tokenize(&skill.description);
        let trigger_tokens = tokenize(&skill.triggers.join(" "));

        // Compute total document length (sum of all field token counts).
        let doc_length = (name_tokens.len() + desc_tokens.len() + trigger_tokens.len()) as u64;
        self.total_doc_length += doc_length;
        self.doc_lengths.insert(skill.slug.clone(), doc_length);

        // Count term frequencies per field.
        let mut name_tf: HashMap<&str, u32> = HashMap::new();
        for t in &name_tokens {
            *name_tf.entry(t).or_insert(0) += 1;
        }
        let mut desc_tf: HashMap<&str, u32> = HashMap::new();
        for t in &desc_tokens {
            *desc_tf.entry(t).or_insert(0) += 1;
        }
        let mut trigger_tf: HashMap<&str, u32> = HashMap::new();
        for t in &trigger_tokens {
            *trigger_tf.entry(t).or_insert(0) += 1;
        }

        // Collect all unique terms for this document.
        let mut all_terms: HashSet<&&str> = HashSet::new();
        all_terms.extend(name_tf.keys());
        all_terms.extend(desc_tf.keys());
        all_terms.extend(trigger_tf.keys());

        for term in all_terms {
            let posting = PostingEntry {
                slug: skill.slug.clone(),
                tf_name: *name_tf.get(term).unwrap_or(&0),
                tf_description: *desc_tf.get(term).unwrap_or(&0),
                tf_triggers: *trigger_tf.get(term).unwrap_or(&0),
            };
            self.postings
                .entry((*term).to_string())
                .or_default()
                .push(posting);
        }

        debug!(slug = %skill.slug, doc_length = doc_length, "Indexed skill");
    }

    /// Remove all postings for a given slug.
    fn remove_skill_postings(&mut self, slug: &str) {
        // Subtract document length from total.
        if let Some(len) = self.doc_lengths.remove(slug) {
            self.total_doc_length = self.total_doc_length.saturating_sub(len);
        }
        for postings in self.postings.values_mut() {
            postings.retain(|p| p.slug != slug);
        }
        // Clean up empty posting lists.
        self.postings.retain(|_, v| !v.is_empty());
    }

    /// Search the index with a text query, returning up to `limit` results.
    ///
    /// Uses a BM25 scoring function with field boosting and actual average
    /// document length normalisation.
    pub fn search(&self, query: &str, limit: usize) -> Vec<SearchResult> {
        let query_tokens = tokenize(query);
        if query_tokens.is_empty() || self.doc_count == 0 {
            return Vec::new();
        }

        let df: f64 = self.doc_count as f64;
        let avg_dl = if self.doc_count > 0 {
            self.total_doc_length as f64 / self.doc_count as f64
        } else {
            1.0
        };

        // Accumulate scores per slug.
        let mut scores: HashMap<String, f64> = HashMap::new();
        let mut matched: HashMap<String, Vec<String>> = HashMap::new();
        let mut highlights_map: HashMap<String, Vec<String>> = HashMap::new();

        for term in &query_tokens {
            let idf = idf(term, &self.postings, df);

            if let Some(postings) = self.postings.get(term) {
                for posting in postings {
                    let dl = self.doc_lengths.get(&posting.slug).copied().unwrap_or(1) as f64;
                    let score_name = bm25_term(posting.tf_name, idf, WEIGHT_NAME, dl, avg_dl);
                    let score_desc =
                        bm25_term(posting.tf_description, idf, WEIGHT_DESCRIPTION, dl, avg_dl);
                    let score_trig =
                        bm25_term(posting.tf_triggers, idf, WEIGHT_TRIGGERS, dl, avg_dl);

                    let total = score_name + score_desc + score_trig;
                    if total > 0.0 {
                        *scores.entry(posting.slug.clone()).or_insert(0.0) += total;
                        matched
                            .entry(posting.slug.clone())
                            .or_default()
                            .push(term.clone());
                    }
                }
            }

            // Build highlights from the query term.
            if let Some(postings) = self.postings.get(term) {
                for posting in postings {
                    let hl = format_highlight(term, posting);
                    if !hl.is_empty() {
                        highlights_map
                            .entry(posting.slug.clone())
                            .or_default()
                            .push(hl);
                    }
                }
            }
        }

        // Sort by score descending.
        let mut results: Vec<SearchResult> = scores
            .into_iter()
            .map(|(slug, score)| {
                let mt = matched.remove(&slug).unwrap_or_default();
                let hl = highlights_map.remove(&slug).unwrap_or_default();
                SearchResult {
                    skill_slug: slug,
                    score,
                    matched_terms: mt,
                    highlights: hl,
                }
            })
            .collect();

        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(limit);
        results
    }

    /// Retrieve a skill document by slug (requires all indexed skills to be stored).
    ///
    /// Note: The index only stores postings, not full documents. Use this to check
    /// whether a slug has been indexed.
    pub fn contains_slug(&self, slug: &str) -> bool {
        self.indexed_slugs.contains(slug)
    }
}

impl Default for SearchIndex {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Scoring helpers
// ---------------------------------------------------------------------------

/// Compute IDF for a term.
fn idf(term: &str, postings: &HashMap<String, Vec<PostingEntry>>, total_docs: f64) -> f64 {
    let df = postings.get(term).map(|p| p.len() as f64).unwrap_or(0.0);
    if df == 0.0 {
        return 0.0;
    }
    // Standard IDF with +1 smoothing.
    ((total_docs - df + 0.5) / (df + 0.5) + 1.0).ln()
}

/// BM25 term score for a single field.
///
/// Uses actual document length (`dl`) and average document length (`avgdl`)
/// for proper BM25 length normalisation.
fn bm25_term(tf: u32, idf: f64, weight: f64, dl: f64, avgdl: f64) -> f64 {
    if tf == 0 {
        return 0.0;
    }
    let tf64 = tf as f64;
    let numerator = tf64 * (K1 + 1.0);
    let denominator = tf64 + K1 * (1.0 - B + B * (dl / avgdl));
    idf * (numerator / denominator) * weight
}

/// Build a highlight string for a matched posting.
fn format_highlight(term: &str, posting: &PostingEntry) -> String {
    let mut parts = Vec::new();
    if posting.tf_name > 0 {
        parts.push(format!("name: **{}**", term));
    }
    if posting.tf_description > 0 {
        parts.push(format!("description: …{}…", term));
    }
    if posting.tf_triggers > 0 {
        parts.push(format!("trigger: [{}]", term));
    }
    parts.join(", ")
}

// ---------------------------------------------------------------------------
// Tokenization
// ---------------------------------------------------------------------------

/// Tokenize a string: split on non-alphanumeric, lowercase, apply simple suffix
/// stripping as a light-weight stemmer.
pub fn tokenize(text: &str) -> Vec<String> {
    let re = Regex::new(r"[a-zA-Z0-9]+").expect("valid regex");
    re.find_iter(text)
        .map(|m| {
            let word = m.as_str().to_lowercase();
            stem_light(&word)
        })
        .collect()
}

/// Very lightweight English stemmer — strips common suffixes.
fn stem_light(word: &str) -> String {
    let w = word;
    // Order matters: longest suffix first.
    if w.ends_with("ing") && w.len() > 5 {
        return w[..w.len() - 3].to_string();
    }
    if w.ends_with("tion") && w.len() > 5 {
        return w[..w.len() - 4].to_string();
    }
    if w.ends_with("ment") && w.len() > 5 {
        return w[..w.len() - 4].to_string();
    }
    if w.ends_with("ness") && w.len() > 5 {
        return w[..w.len() - 4].to_string();
    }
    if w.ends_with("able") && w.len() > 5 {
        return w[..w.len() - 4].to_string();
    }
    if w.ends_with("ful") && w.len() > 4 {
        return w[..w.len() - 3].to_string();
    }
    if w.ends_with("ous") && w.len() > 4 {
        return w[..w.len() - 3].to_string();
    }
    if w.ends_with("ive") && w.len() > 4 {
        return w[..w.len() - 3].to_string();
    }
    if w.ends_with("ed") && w.len() > 4 {
        return w[..w.len() - 2].to_string();
    }
    if w.ends_with("ly") && w.len() > 4 {
        return w[..w.len() - 2].to_string();
    }
    if w.ends_with("er") && w.len() > 4 {
        return w[..w.len() - 2].to_string();
    }
    if w.ends_with("es") && w.len() > 4 {
        return w[..w.len() - 2].to_string();
    }
    if w.ends_with('s') && w.len() > 3 {
        return w[..w.len() - 1].to_string();
    }
    w.to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_skill(slug: &str, name: &str, desc: &str, triggers: &[&str]) -> SkillDocument {
        SkillDocument {
            slug: slug.to_string(),
            name: name.to_string(),
            description: desc.to_string(),
            triggers: triggers.iter().map(|s| (*s).to_string()).collect(),
        }
    }

    #[test]
    fn tokenize_basic() {
        let tokens = tokenize("Hello, World! 123");
        assert_eq!(tokens, vec!["hello", "world", "123"]);
    }

    #[test]
    fn tokenize_empty() {
        let tokens = tokenize("");
        assert!(tokens.is_empty());
    }

    #[test]
    fn tokenize_special_chars() {
        let tokens = tokenize("foo-bar_baz.qux");
        assert_eq!(tokens, vec!["foo", "bar", "baz", "qux"]);
    }

    #[test]
    fn stem_removes_s() {
        assert_eq!(stem_light("tests"), "test");
    }

    #[test]
    fn stem_removes_ing() {
        assert_eq!(stem_light("deploying"), "deploy");
    }

    #[test]
    fn stem_removes_ed() {
        assert_eq!(stem_light("deployed"), "deploy");
    }

    #[test]
    fn stem_removes_ly() {
        assert_eq!(stem_light("quickly"), "quick");
    }

    #[test]
    fn stem_short_word_unchanged() {
        assert_eq!(stem_light("the"), "the");
    }

    #[test]
    fn index_and_search_basic() {
        let mut index = SearchIndex::new();
        index.index_skill(&sample_skill(
            "azure-deploy",
            "Azure Deploy",
            "Deploy applications to Azure cloud",
            &["deploy", "azure"],
        ));
        let results = index.search("deploy", 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].skill_slug, "azure-deploy");
    }

    #[test]
    fn search_empty_index() {
        let index = SearchIndex::new();
        let results = index.search("anything", 10);
        assert!(results.is_empty());
    }

    #[test]
    fn search_empty_query() {
        let mut index = SearchIndex::new();
        index.index_skill(&sample_skill("x", "X", "desc", &[]));
        let results = index.search("", 10);
        assert!(results.is_empty());
    }

    #[test]
    fn search_no_match() {
        let mut index = SearchIndex::new();
        index.index_skill(&sample_skill("x", "X", "desc", &[]));
        let results = index.search("zzzzz", 10);
        assert!(results.is_empty());
    }

    #[test]
    fn search_ranking_by_field_boost() {
        let mut index = SearchIndex::new();
        // Skill A: "deploy" only in description
        index.index_skill(&sample_skill("a", "Something Else", "deploy things", &[]));
        // Skill B: "deploy" in name (higher boost)
        index.index_skill(&sample_skill("b", "Deploy Tool", "a tool", &[]));
        let results = index.search("deploy", 10);
        assert!(results.len() >= 2, "should have 2 results");
        assert_eq!(results[0].skill_slug, "b", "name match should rank higher");
    }

    #[test]
    fn search_trigger_boost() {
        let mut index = SearchIndex::new();
        index.index_skill(&sample_skill("a", "Skill A", "a description", &["deploy"]));
        index.index_skill(&sample_skill("b", "Skill B", "a description", &[]));
        let results = index.search("deploy", 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].skill_slug, "a");
    }

    #[test]
    fn reindex_replaces() {
        let mut index = SearchIndex::new();
        index.index_skill(&sample_skill("s", "Old Name", "old desc", &[]));
        index.index_skill(&sample_skill("s", "New Name", "new desc", &[]));
        assert_eq!(index.len(), 1);
        let results = index.search("new", 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].skill_slug, "s");
    }

    #[test]
    fn search_limit() {
        let mut index = SearchIndex::new();
        for i in 0..10 {
            index.index_skill(&sample_skill(
                &format!("skill-{i}"),
                &format!("Deploy {i}"),
                "deployment tool",
                &[],
            ));
        }
        let results = index.search("deploy", 3);
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn matched_terms_populated() {
        let mut index = SearchIndex::new();
        index.index_skill(&sample_skill("x", "Deploy", "desc", &["azure"]));
        let results = index.search("deploy azure", 10);
        assert!(!results.is_empty());
        let r = &results[0];
        assert!(
            r.matched_terms.contains(&"deploy".to_string())
                || r.matched_terms.contains(&"azure".to_string())
        );
    }

    #[test]
    fn highlights_populated() {
        let mut index = SearchIndex::new();
        index.index_skill(&sample_skill("x", "Deploy Tool", "desc", &[]));
        let results = index.search("deploy", 10);
        assert!(!results.is_empty());
        assert!(!results[0].highlights.is_empty());
    }

    #[test]
    fn contains_slug() {
        let mut index = SearchIndex::new();
        index.index_skill(&sample_skill("abc", "Abc", "desc", &[]));
        assert!(index.contains_slug("abc"));
        assert!(!index.contains_slug("xyz"));
    }

    #[test]
    fn default_is_empty() {
        let index = SearchIndex::default();
        assert!(index.is_empty());
    }

    #[test]
    fn multi_term_query() {
        let mut index = SearchIndex::new();
        index.index_skill(&sample_skill(
            "full",
            "Azure Deploy",
            "Deploy to Azure cloud",
            &["azure", "deploy", "cloud"],
        ));
        let results = index.search("azure deploy cloud", 10);
        assert_eq!(results.len(), 1);
        assert!(results[0].score > 0.0);
    }
}
