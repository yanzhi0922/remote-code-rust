//! Diagnostic collection and management.
//!
//! Provides a registry for collecting, querying, and clearing LSP diagnostics
//! published by language servers.

use std::collections::HashMap;

use crate::types::{Diagnostic, DiagnosticSeverity};

// ---------------------------------------------------------------------------
// DiagnosticRegistry
// ---------------------------------------------------------------------------

/// A registry for collecting diagnostics from multiple sources.
#[derive(Debug, Default)]
pub struct DiagnosticRegistry {
    /// Diagnostics keyed by document URI.
    diagnostics: HashMap<String, Vec<Diagnostic>>,
    /// Total error count across all documents.
    error_count: usize,
    /// Total warning count across all documents.
    warning_count: usize,
}

impl DiagnosticRegistry {
    /// Create a new empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Publish diagnostics for a document.
    ///
    /// Replaces any existing diagnostics for the same document.
    pub fn publish(&mut self, uri: &str, diagnostics: Vec<Diagnostic>) {
        self.diagnostics.insert(uri.to_string(), diagnostics);

        // Recompute counts from scratch to avoid drift.
        let (errors, warnings) =
            self.diagnostics
                .values()
                .flatten()
                .fold((0usize, 0usize), |(e, w), d| {
                    if d.is_error() {
                        (e + 1, w)
                    } else if d.is_warning() {
                        (e, w + 1)
                    } else {
                        (e, w)
                    }
                });
        self.error_count = errors;
        self.warning_count = warnings;
    }

    /// Get diagnostics for a specific document.
    #[must_use]
    pub fn get(&self, uri: &str) -> &[Diagnostic] {
        self.diagnostics.get(uri).map_or(&[], Vec::as_slice)
    }

    /// Get all diagnostics across all documents.
    #[must_use]
    pub fn get_all(&self) -> &HashMap<String, Vec<Diagnostic>> {
        &self.diagnostics
    }

    /// Clear diagnostics for a specific document.
    pub fn clear(&mut self, uri: &str) {
        self.diagnostics.remove(uri);

        // Recompute counts from scratch to avoid drift.
        let (errors, warnings) =
            self.diagnostics
                .values()
                .flatten()
                .fold((0usize, 0usize), |(e, w), d| {
                    if d.is_error() {
                        (e + 1, w)
                    } else if d.is_warning() {
                        (e, w + 1)
                    } else {
                        (e, w)
                    }
                });
        self.error_count = errors;
        self.warning_count = warnings;
    }

    /// Clear all diagnostics.
    pub fn clear_all(&mut self) {
        self.diagnostics.clear();
        self.error_count = 0;
        self.warning_count = 0;
    }

    /// Total number of diagnostics across all documents.
    #[must_use]
    pub fn total_count(&self) -> usize {
        self.diagnostics.values().map(Vec::len).sum()
    }

    /// Number of documents with diagnostics.
    #[must_use]
    pub fn document_count(&self) -> usize {
        self.diagnostics.len()
    }

    /// Total error count.
    #[must_use]
    pub fn error_count(&self) -> usize {
        self.error_count
    }

    /// Total warning count.
    #[must_use]
    pub fn warning_count(&self) -> usize {
        self.warning_count
    }

    /// Whether any errors exist.
    #[must_use]
    pub fn has_errors(&self) -> bool {
        self.error_count > 0
    }

    /// Whether any diagnostics exist.
    #[must_use]
    pub fn has_diagnostics(&self) -> bool {
        !self.diagnostics.is_empty()
    }

    /// Get document URIs that have diagnostics.
    #[must_use]
    pub fn document_uris(&self) -> Vec<&str> {
        self.diagnostics.keys().map(String::as_str).collect()
    }

    /// Get diagnostics filtered by severity.
    #[must_use]
    pub fn get_by_severity(&self, uri: &str, severity: DiagnosticSeverity) -> Vec<&Diagnostic> {
        self.diagnostics
            .get(uri)
            .map(|ds| ds.iter().filter(|d| d.severity == Some(severity)).collect())
            .unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------
// publish_diagnostics helper
// ---------------------------------------------------------------------------

/// Convenience function to publish diagnostics for a document.
pub fn publish_diagnostics(
    registry: &mut DiagnosticRegistry,
    uri: &str,
    diagnostics: Vec<Diagnostic>,
) {
    registry.publish(uri, diagnostics);
}

/// Convenience function to get diagnostics for a document.
pub fn get_diagnostics<'a>(registry: &'a DiagnosticRegistry, uri: &str) -> &'a [Diagnostic] {
    registry.get(uri)
}

/// Convenience function to clear diagnostics for a document.
pub fn clear_diagnostics(registry: &mut DiagnosticRegistry, uri: &str) {
    registry.clear(uri);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Range;

    fn make_error(range: Range, msg: &str) -> Diagnostic {
        Diagnostic::new(range, msg).with_severity(DiagnosticSeverity::Error)
    }

    fn make_warning(range: Range, msg: &str) -> Diagnostic {
        Diagnostic::new(range, msg).with_severity(DiagnosticSeverity::Warning)
    }

    fn make_info(range: Range, msg: &str) -> Diagnostic {
        Diagnostic::new(range, msg).with_severity(DiagnosticSeverity::Information)
    }

    // -- DiagnosticRegistry ---------------------------------------------------

    #[test]
    fn registry_new() {
        let reg = DiagnosticRegistry::default();
        assert_eq!(reg.total_count(), 0);
        assert_eq!(reg.document_count(), 0);
        assert!(!reg.has_diagnostics());
    }

    #[test]
    fn registry_publish_and_get() {
        let mut reg = DiagnosticRegistry::default();
        reg.publish(
            "file:///test.rs",
            vec![make_error(Range::single_line(1, 0, 5), "err")],
        );

        let got = reg.get("file:///test.rs");
        assert_eq!(got.len(), 1);
        assert!(got[0].is_error());
    }

    #[test]
    fn registry_counts() {
        let mut reg = DiagnosticRegistry::default();
        reg.publish(
            "file:///a.rs",
            vec![
                make_error(Range::single_line(1, 0, 5), "e1"),
                make_warning(Range::single_line(2, 0, 5), "w1"),
                make_info(Range::single_line(3, 0, 5), "i1"),
            ],
        );

        assert_eq!(reg.error_count(), 1);
        assert_eq!(reg.warning_count(), 1);
        assert_eq!(reg.total_count(), 3);
        assert!(reg.has_errors());
    }

    #[test]
    fn registry_clear_document() {
        let mut reg = DiagnosticRegistry::default();
        reg.publish(
            "file:///a.rs",
            vec![make_error(Range::single_line(1, 0, 5), "e")],
        );
        reg.clear("file:///a.rs");
        assert_eq!(reg.total_count(), 0);
        assert_eq!(reg.error_count(), 0);
    }

    #[test]
    fn registry_clear_all() {
        let mut reg = DiagnosticRegistry::default();
        reg.publish(
            "file:///a.rs",
            vec![make_error(Range::single_line(1, 0, 5), "e")],
        );
        reg.publish(
            "file:///b.rs",
            vec![make_warning(Range::single_line(1, 0, 5), "w")],
        );
        reg.clear_all();
        assert_eq!(reg.total_count(), 0);
        assert_eq!(reg.error_count(), 0);
        assert_eq!(reg.warning_count(), 0);
    }

    #[test]
    fn registry_replace_diagnostics() {
        let mut reg = DiagnosticRegistry::default();
        reg.publish(
            "file:///a.rs",
            vec![make_error(Range::single_line(1, 0, 5), "e1")],
        );
        assert_eq!(reg.error_count(), 1);

        reg.publish(
            "file:///a.rs",
            vec![make_error(Range::single_line(2, 0, 5), "e2")],
        );
        assert_eq!(reg.error_count(), 1);
        assert_eq!(reg.total_count(), 1);
    }

    #[test]
    fn registry_get_missing_document() {
        let reg = DiagnosticRegistry::default();
        let diags = reg.get("file:///nonexistent.rs");
        assert!(diags.is_empty());
    }

    #[test]
    fn registry_document_uris() {
        let mut reg = DiagnosticRegistry::default();
        reg.publish("file:///a.rs", vec![]);
        reg.publish(
            "file:///b.rs",
            vec![make_error(Range::single_line(1, 0, 5), "e")],
        );
        let mut uris: Vec<&str> = reg.document_uris();
        uris.sort();
        assert_eq!(uris, vec!["file:///a.rs", "file:///b.rs"]);
    }

    #[test]
    fn registry_filter_by_severity() {
        let mut reg = DiagnosticRegistry::default();
        reg.publish(
            "file:///a.rs",
            vec![
                make_error(Range::single_line(1, 0, 5), "e"),
                make_warning(Range::single_line(2, 0, 5), "w"),
                make_info(Range::single_line(3, 0, 5), "i"),
            ],
        );
        let errors = reg.get_by_severity("file:///a.rs", DiagnosticSeverity::Error);
        assert_eq!(errors.len(), 1);
        let warnings = reg.get_by_severity("file:///a.rs", DiagnosticSeverity::Warning);
        assert_eq!(warnings.len(), 1);
        let infos = reg.get_by_severity("file:///a.rs", DiagnosticSeverity::Information);
        assert_eq!(infos.len(), 1);
    }

    #[test]
    fn registry_filter_missing_document() {
        let reg = DiagnosticRegistry::default();
        let errors = reg.get_by_severity("file:///none.rs", DiagnosticSeverity::Error);
        assert!(errors.is_empty());
    }

    #[test]
    fn registry_get_all() {
        let mut reg = DiagnosticRegistry::default();
        reg.publish(
            "file:///a.rs",
            vec![make_error(Range::single_line(1, 0, 5), "e")],
        );
        let all = reg.get_all();
        assert_eq!(all.len(), 1);
    }

    // -- Convenience functions ------------------------------------------------

    #[test]
    fn publish_diagnostics_function() {
        let mut reg = DiagnosticRegistry::default();
        publish_diagnostics(
            &mut reg,
            "file:///test.rs",
            vec![make_error(Range::single_line(1, 0, 5), "e")],
        );
        assert_eq!(reg.total_count(), 1);
    }

    #[test]
    fn get_diagnostics_function() {
        let mut reg = DiagnosticRegistry::default();
        reg.publish(
            "file:///test.rs",
            vec![make_error(Range::single_line(1, 0, 5), "e")],
        );
        let diags = get_diagnostics(&reg, "file:///test.rs");
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn clear_diagnostics_function() {
        let mut reg = DiagnosticRegistry::default();
        reg.publish(
            "file:///test.rs",
            vec![make_error(Range::single_line(1, 0, 5), "e")],
        );
        clear_diagnostics(&mut reg, "file:///test.rs");
        assert_eq!(reg.total_count(), 0);
    }

    #[test]
    fn registry_no_diagnostics_no_errors() {
        let reg = DiagnosticRegistry::default();
        assert!(!reg.has_errors());
        assert!(!reg.has_diagnostics());
    }

    #[test]
    fn registry_only_warnings_no_errors() {
        let mut reg = DiagnosticRegistry::default();
        reg.publish(
            "file:///a.rs",
            vec![make_warning(Range::single_line(1, 0, 5), "w")],
        );
        assert!(!reg.has_errors());
        assert!(reg.has_diagnostics());
        assert_eq!(reg.warning_count(), 1);
    }
}
