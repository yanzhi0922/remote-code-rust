use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct DenialTracker {
    counts: HashMap<String, usize>,
}

impl DenialTracker {
    pub fn record(&mut self, key: impl Into<String>) -> usize {
        let entry = self.counts.entry(key.into()).or_insert(0);
        *entry += 1;
        *entry
    }

    #[must_use]
    pub fn count(&self, key: &str) -> usize {
        self.counts.get(key).copied().unwrap_or_default()
    }
}
