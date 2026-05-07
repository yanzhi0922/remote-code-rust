//! Auto-dream — background memory consolidation agent.
//!
//! Mirrors TS `autoDream.ts` with a 5-gate execution chain:
//! 1. Feature toggle (enabled + not remote + auto-memory on)
//! 2. Time gate (min hours since last consolidation)
//! 3. Scan throttle (min minutes since last directory scan)
//! 4. Session gate (enough sessions accumulated)
//! 5. Lock gate (no other process currently dreaming)
//!
//! If all gates pass, launches a forked sub-agent with the 4-phase
//! consolidation prompt (orient → gather → consolidate → prune).
//!
//! This runs as Phase 3 (BackgroundFireAndForget) of the stop-hook pipeline.

pub mod config;
pub mod consolidation_lock;
pub mod consolidation_prompt;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use config::AutoDreamConfig;
use consolidation_lock::ConsolidationLock;
use consolidation_prompt::AUTO_DREAM_SYSTEM_PROMPT;

/// Shared state for auto-dream gating across invocations.
#[derive(Debug)]
pub struct AutoDreamState {
    /// Timestamp of last directory scan (ms since epoch).
    last_session_scan_at: Option<u128>,
    /// Number of sessions found in last scan.
    last_session_count: usize,
}

impl Default for AutoDreamState {
    fn default() -> Self {
        Self {
            last_session_scan_at: None,
            last_session_count: 0,
        }
    }
}

/// Auto-dream executor that manages the 5-gate chain.
#[derive(Debug)]
pub struct AutoDreamExecutor {
    /// Configuration for gating thresholds.
    config: AutoDreamConfig,
    /// Shared mutable state (scan timestamps, counts).
    state: Arc<Mutex<AutoDreamState>>,
    /// Path to the auto-memory directory.
    memory_dir: PathBuf,
    /// Path to the session transcripts directory.
    session_dir: Option<PathBuf>,
}

impl AutoDreamExecutor {
    /// Create a new auto-dream executor.
    pub fn new(
        config: AutoDreamConfig,
        memory_dir: PathBuf,
        session_dir: Option<PathBuf>,
    ) -> Self {
        Self {
            config,
            state: Arc::new(Mutex::new(AutoDreamState::default())),
            memory_dir,
            session_dir,
        }
    }

    /// Returns a reference to the config.
    pub fn config(&self) -> &AutoDreamConfig {
        &self.config
    }

    /// Returns the memory directory path.
    pub fn memory_dir(&self) -> &Path {
        &self.memory_dir
    }

    /// Execute the 5-gate chain. Returns true if auto-dream should proceed.
    ///
    /// # Arguments
    /// * `is_remote` — whether this is a remote session
    /// * `auto_memory_enabled` — whether auto-memory is enabled
    /// * `agent_id` — the agent ID (auto-dream only runs on main agent, not sub-agents)
    pub fn should_trigger(
        &self,
        is_remote: bool,
        auto_memory_enabled: bool,
        agent_id: Option<&str>,
    ) -> bool {
        // Auto-dream only fires on main agent (no agent_id)
        if agent_id.is_some() {
            return false;
        }

        // Gate 1: Feature toggle
        if !self.config.is_enabled(is_remote, auto_memory_enabled) {
            return false;
        }

        // Gate 2: Time gate
        let lock = ConsolidationLock::new(&self.memory_dir);
        let last_ms = lock.read_last_consolidated_at();
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let hours_since = (now_ms.saturating_sub(last_ms)) as f64 / 3_600_000.0;
        if !self.config.time_gate_passed(hours_since) {
            return false;
        }

        // Gate 3: Scan throttle
        let minutes_since_scan = self.state.lock().map_or(None, |s| {
            s.last_session_scan_at.map(|scan_ms| {
                (now_ms.saturating_sub(scan_ms)) as f64 / 60_000.0
            })
        });
        if !self.config.scan_throttle_passed(minutes_since_scan) {
            return false;
        }

        // Gate 4: Session gate — scan sessions
        let session_count = self.count_sessions_since(last_ms);
        {
            if let Ok(mut state) = self.state.lock() {
                state.last_session_scan_at = Some(now_ms);
                state.last_session_count = session_count;
            }
        }
        if !self.config.session_gate_passed(session_count) {
            return false;
        }

        // Gate 5: Lock gate
        match lock.try_acquire() {
            Ok(_) => true,
            Err(_) => false,
        }
    }

    /// Count sessions modified since the given timestamp.
    fn count_sessions_since(&self, since_ms: u128) -> usize {
        let Some(ref session_dir) = self.session_dir else {
            return 0;
        };

        let since_time = UNIX_EPOCH + std::time::Duration::from_millis(since_ms as u64);

        std::fs::read_dir(session_dir)
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .filter(|e| {
                        e.metadata()
                            .ok()
                            .and_then(|m| m.modified().ok())
                            .is_some_and(|mtime| mtime > since_time)
                    })
                    .count()
            })
            .unwrap_or(0)
    }

    /// Build the dream prompt for the forked agent.
    pub fn build_prompt(&self) -> String {
        consolidation_prompt::build_dream_prompt(
            self.memory_dir.to_str().unwrap_or("."),
            self.session_dir.as_deref().and_then(|p| p.to_str()),
        )
    }

    /// Get the system prompt for the dream agent.
    pub fn system_prompt(&self) -> &'static str {
        AUTO_DREAM_SYSTEM_PROMPT
    }

    /// Roll back the consolidation lock after failure.
    pub fn rollback_lock(&self, prior_mtime_ms: u128) {
        let lock = ConsolidationLock::new(&self.memory_dir);
        lock.rollback(prior_mtime_ms);
    }

    /// Record successful consolidation.
    pub fn record_consolidation(&self) -> anyhow::Result<()> {
        let lock = ConsolidationLock::new(&self.memory_dir);
        lock.record_consolidation()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn executor_skips_sub_agents() {
        let dir = tempfile::tempdir().unwrap();
        let executor = AutoDreamExecutor::new(
            AutoDreamConfig::default(),
            dir.path().to_path_buf(),
            None,
        );
        assert!(!executor.should_trigger(false, true, Some("agent-123")));
    }

    #[test]
    fn executor_skips_remote_sessions() {
        let dir = tempfile::tempdir().unwrap();
        let executor = AutoDreamExecutor::new(
            AutoDreamConfig::default(),
            dir.path().to_path_buf(),
            None,
        );
        assert!(!executor.should_trigger(true, true, None));
    }

    #[test]
    fn executor_skips_when_auto_memory_disabled() {
        let dir = tempfile::tempdir().unwrap();
        let executor = AutoDreamExecutor::new(
            AutoDreamConfig::default(),
            dir.path().to_path_buf(),
            None,
        );
        assert!(!executor.should_trigger(false, false, None));
    }

    #[test]
    fn build_prompt_returns_nonempty() {
        let dir = tempfile::tempdir().unwrap();
        let executor = AutoDreamExecutor::new(
            AutoDreamConfig::default(),
            dir.path().to_path_buf(),
            Some(dir.path().to_path_buf()),
        );
        let prompt = executor.build_prompt();
        assert!(!prompt.is_empty());
        assert!(prompt.contains("Phase 1"));
    }

    #[test]
    fn system_prompt_is_dream_prompt() {
        let dir = tempfile::tempdir().unwrap();
        let executor = AutoDreamExecutor::new(
            AutoDreamConfig::default(),
            dir.path().to_path_buf(),
            None,
        );
        assert!(executor.system_prompt().contains("Phase 4"));
    }

    #[test]
    fn session_count_with_no_dir() {
        let dir = tempfile::tempdir().unwrap();
        let executor = AutoDreamExecutor::new(
            AutoDreamConfig::default(),
            dir.path().to_path_buf(),
            None,
        );
        assert_eq!(executor.count_sessions_since(0), 0);
    }

    #[test]
    fn session_count_counts_recent_files() {
        let dir = tempfile::tempdir().unwrap();
        let session_dir = dir.path().join("sessions");
        fs::create_dir_all(&session_dir).unwrap();
        fs::write(session_dir.join("sess1.jsonl"), "test").unwrap();
        fs::write(session_dir.join("sess2.jsonl"), "test").unwrap();

        let executor = AutoDreamExecutor::new(
            AutoDreamConfig::default(),
            dir.path().to_path_buf(),
            Some(session_dir),
        );
        // Since since_ms = 0, all files should be counted
        assert!(executor.count_sessions_since(0) >= 2);
    }
}
