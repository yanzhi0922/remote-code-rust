//! Skill prefetching — pre-load skills likely to be relevant based on query hints.
//!
//! [`SkillPrefetcher`] analyses incoming queries and proactively loads skill
//! documents that are likely to be needed, using background tokio tasks.

use crate::local_search::SearchIndex;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Prefetch result containing slugs that were identified as relevant.
#[derive(Debug, Clone)]
pub struct PrefetchResult {
    /// Slugs that were identified as potentially relevant.
    pub slugs: Vec<String>,
    /// Whether the prefetch was a cache hit (already loaded).
    pub cache_hit: bool,
}

/// Background prefetcher for skill search.
#[derive(Debug, Clone)]
pub struct SkillPrefetcher {
    index: Arc<RwLock<SearchIndex>>,
    /// Slugs that have already been prefetched.
    prefetched: Arc<RwLock<Vec<String>>>,
}

impl SkillPrefetcher {
    /// Create a new prefetcher backed by the given search index.
    pub fn new(index: Arc<RwLock<SearchIndex>>) -> Self {
        Self {
            index,
            prefetched: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Determine which skills are likely to be relevant for `query` and
    /// return their slugs.  This is a synchronous analysis step.
    pub async fn prefetch_relevant(&self, query: &str, limit: usize) -> PrefetchResult {
        let index = self.index.read().await;
        let results = index.search(query, limit);
        let slugs: Vec<String> = results.iter().map(|r| r.skill_slug.clone()).collect();

        let mut prefetched = self.prefetched.write().await;
        let cache_hit = slugs.iter().all(|s| prefetched.contains(s));

        for slug in &slugs {
            if !prefetched.contains(slug) {
                debug!(slug = %slug, "Prefetching skill");
                prefetched.push(slug.clone());
            }
        }

        PrefetchResult { slugs, cache_hit }
    }

    /// Spawn a background task to prefetch skills for the given query.
    pub fn spawn_prefetch(&self, query: String, limit: usize) {
        let prefetcher = self.clone();
        tokio::spawn(async move {
            let result = prefetcher.prefetch_relevant(&query, limit).await;
            debug!(
                slugs = ?result.slugs,
                cache_hit = result.cache_hit,
                "Background prefetch completed"
            );
        });
    }

    /// Return the list of already-prefetched slugs.
    pub async fn prefetched_slugs(&self) -> Vec<String> {
        self.prefetched.read().await.clone()
    }

    /// Clear the prefetch cache.
    pub async fn clear(&self) {
        self.prefetched.write().await.clear();
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::local_search::SkillDocument;

    fn sample_skill(slug: &str, name: &str, desc: &str, triggers: &[&str]) -> SkillDocument {
        SkillDocument {
            slug: slug.to_string(),
            name: name.to_string(),
            description: desc.to_string(),
            triggers: triggers.iter().map(|s| (*s).to_string()).collect(),
        }
    }

    async fn make_prefetcher() -> SkillPrefetcher {
        let index = Arc::new(RwLock::new(SearchIndex::new()));
        {
            let mut idx = index.write().await;
            idx.index_skill(&sample_skill("deploy", "Deploy", "Deploy things", &["deploy"]));
            idx.index_skill(&sample_skill("build", "Build", "Build things", &["build"]));
        }
        SkillPrefetcher::new(index)
    }

    #[tokio::test]
    async fn prefetch_finds_relevant() {
        let pf = make_prefetcher().await;
        let result = pf.prefetch_relevant("deploy", 5).await;
        assert!(result.slugs.contains(&"deploy".to_string()));
        assert!(!result.cache_hit);
    }

    #[tokio::test]
    async fn second_prefetch_is_cache_hit() {
        let pf = make_prefetcher().await;
        let _ = pf.prefetch_relevant("deploy", 5).await;
        let result = pf.prefetch_relevant("deploy", 5).await;
        assert!(result.cache_hit);
    }

    #[tokio::test]
    async fn spawn_prefetch_runs() {
        let pf = make_prefetcher().await;
        pf.spawn_prefetch("build".to_string(), 5);
        // Give the spawned task time to complete.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let slugs = pf.prefetched_slugs().await;
        assert!(slugs.contains(&"build".to_string()));
    }

    #[tokio::test]
    async fn clear_resets() {
        let pf = make_prefetcher().await;
        let _ = pf.prefetch_relevant("deploy", 5).await;
        assert!(!pf.prefetched_slugs().await.is_empty());
        pf.clear().await;
        assert!(pf.prefetched_slugs().await.is_empty());
    }

    #[tokio::test]
    async fn prefetch_empty_query() {
        let pf = make_prefetcher().await;
        let result = pf.prefetch_relevant("", 5).await;
        assert!(result.slugs.is_empty());
    }

    #[tokio::test]
    async fn prefetch_no_match() {
        let pf = make_prefetcher().await;
        let result = pf.prefetch_relevant("zzzzz", 5).await;
        assert!(result.slugs.is_empty());
    }
}
