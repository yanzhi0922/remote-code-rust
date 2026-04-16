//! Exponential backoff reconnect scheduler.
//!
//! Manages reconnect attempts for MCP servers that have disconnected,
//! using exponential backoff with configurable parameters. Only remote
//! transports (SSE/HTTP/WS) are eligible for automatic reconnection;
//! stdio and SDK transports are not reconnected automatically.

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Default maximum reconnect attempts.
const DEFAULT_MAX_ATTEMPTS: u32 = 5;
/// Default initial backoff in milliseconds.
const DEFAULT_INITIAL_BACKOFF_MS: u64 = 1000;
/// Default maximum backoff in milliseconds.
const DEFAULT_MAX_BACKOFF_MS: u64 = 30_000;

/// Action returned when scheduling a reconnect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReconnectAction {
    /// The server should attempt to connect immediately.
    ConnectNow,
    /// Wait for the specified duration before the next attempt.
    WaitFor(Duration),
    /// The maximum number of attempts has been reached; give up.
    GiveUp,
}

/// State tracking a single server's reconnect progress.
#[derive(Debug, Clone)]
pub struct ReconnectState {
    /// Current attempt number (1-based).
    pub attempt: u32,
    /// The time at which the next reconnect attempt should be made.
    pub next_attempt_at: Instant,
    /// Whether this reconnect has been aborted.
    pub aborted: bool,
}

/// Exponential backoff reconnect scheduler.
#[derive(Debug)]
pub struct ReconnectScheduler {
    max_attempts: u32,
    initial_backoff_ms: u64,
    max_backoff_ms: u64,
    pending: HashMap<String, ReconnectState>,
}

impl ReconnectScheduler {
    /// Create a new scheduler with default parameters.
    ///
    /// Defaults: max 5 attempts, 1 s initial backoff, 30 s max backoff.
    #[must_use]
    pub fn new() -> Self {
        Self {
            max_attempts: DEFAULT_MAX_ATTEMPTS,
            initial_backoff_ms: DEFAULT_INITIAL_BACKOFF_MS,
            max_backoff_ms: DEFAULT_MAX_BACKOFF_MS,
            pending: HashMap::new(),
        }
    }

    /// Create a new scheduler with custom parameters.
    #[must_use]
    pub fn with_params(
        max_attempts: u32,
        initial_backoff_ms: u64,
        max_backoff_ms: u64,
    ) -> Self {
        Self {
            max_attempts,
            initial_backoff_ms,
            max_backoff_ms,
            pending: HashMap::new(),
        }
    }

    /// Schedule a reconnect for a server.
    ///
    /// If this is the first attempt, returns [`ReconnectAction::ConnectNow`].
    /// If the server has previous failed attempts, returns
    /// [`ReconnectAction::WaitFor`] with the backoff duration.
    /// If the maximum attempts have been exceeded, returns
    /// [`ReconnectAction::GiveUp`].
    pub fn schedule_reconnect(&mut self, server_name: String) -> ReconnectAction {
        let state = self.pending.entry(server_name.clone()).or_insert_with(|| {
            ReconnectState {
                attempt: 0,
                next_attempt_at: Instant::now(),
                aborted: false,
            }
        });

        if state.aborted {
            return ReconnectAction::GiveUp;
        }

        state.attempt += 1;

        if state.attempt > self.max_attempts {
            return ReconnectAction::GiveUp;
        }

        if state.attempt == 1 {
            state.next_attempt_at = Instant::now();
            ReconnectAction::ConnectNow
        } else {
            // Compute backoff before borrowing to avoid borrow conflicts.
            let backoff = compute_backoff(
                state.attempt - 1,
                self.initial_backoff_ms,
                self.max_backoff_ms,
            );
            state.next_attempt_at = Instant::now() + backoff;
            ReconnectAction::WaitFor(backoff)
        }
    }

    /// Report that a reconnect succeeded. Removes the server from the
    /// pending set.
    pub fn report_success(&mut self, server_name: &str) {
        self.pending.remove(server_name);
    }

    /// Report that a reconnect failed. Returns the duration to wait before
    /// the next attempt, or `None` if the maximum attempts have been exceeded.
    pub fn report_failure(&self, server_name: &str) -> Option<Duration> {
        let state = self.pending.get(server_name)?;
        if state.attempt >= self.max_attempts || state.aborted {
            None
        } else {
            Some(compute_backoff(
                state.attempt,
                self.initial_backoff_ms,
                self.max_backoff_ms,
            ))
        }
    }

    /// Cancel reconnect for a specific server.
    pub fn cancel(&mut self, server_name: &str) {
        if let Some(state) = self.pending.get_mut(server_name) {
            state.aborted = true;
        }
    }

    /// Cancel all pending reconnects.
    pub fn cancel_all(&mut self) {
        for state in self.pending.values_mut() {
            state.aborted = true;
        }
    }

    /// Return `true` if the server has a pending (non-aborted) reconnect.
    #[must_use]
    pub fn is_reconnecting(&self, server_name: &str) -> bool {
        self.pending
            .get(server_name)
            .is_some_and(|s| !s.aborted && s.attempt <= self.max_attempts)
    }

    /// Return the reconnect state for a server, if any.
    #[must_use]
    pub fn reconnect_state(&self, server_name: &str) -> Option<&ReconnectState> {
        self.pending.get(server_name)
    }

    /// Return the number of servers with pending reconnects.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.pending
            .values()
            .filter(|s| !s.aborted)
            .count()
    }
}

/// Compute the backoff duration for a given attempt number (1-based).
fn compute_backoff(attempt: u32, initial_backoff_ms: u64, max_backoff_ms: u64) -> Duration {
    let exponent = attempt.saturating_sub(1);
    let multiplier = 2_u64.saturating_pow(exponent);
    let backoff_ms = initial_backoff_ms.saturating_mul(multiplier).min(max_backoff_ms);
    Duration::from_millis(backoff_ms)
}

impl Default for ReconnectScheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_attempt_returns_connect_now() {
        let mut scheduler = ReconnectScheduler::new();
        let action = scheduler.schedule_reconnect("test-server".to_owned());
        assert_eq!(action, ReconnectAction::ConnectNow);
    }

    #[test]
    fn second_attempt_returns_wait() {
        let mut scheduler = ReconnectScheduler::new();
        scheduler.schedule_reconnect("test-server".to_owned());
        let action = scheduler.schedule_reconnect("test-server".to_owned());
        match action {
            ReconnectAction::WaitFor(d) => assert_eq!(d, Duration::from_secs(1)),
            other => panic!("expected WaitFor, got {other:?}"),
        }
    }

    #[test]
    fn max_attempts_returns_give_up() {
        let mut scheduler = ReconnectScheduler::with_params(2, 100, 1000);
        scheduler.schedule_reconnect("srv".to_owned()); // attempt 1
        scheduler.schedule_reconnect("srv".to_owned()); // attempt 2
        let action = scheduler.schedule_reconnect("srv".to_owned()); // attempt 3 > max
        assert_eq!(action, ReconnectAction::GiveUp);
    }

    #[test]
    fn report_success_removes_state() {
        let mut scheduler = ReconnectScheduler::new();
        scheduler.schedule_reconnect("srv".to_owned());
        assert!(scheduler.is_reconnecting("srv"));
        scheduler.report_success("srv");
        assert!(!scheduler.is_reconnecting("srv"));
        assert!(scheduler.reconnect_state("srv").is_none());
    }

    #[test]
    fn report_failure_returns_next_backoff() {
        let mut scheduler = ReconnectScheduler::new();
        scheduler.schedule_reconnect("srv".to_owned());
        let next = scheduler.report_failure("srv");
        assert!(next.is_some());
        assert_eq!(next, Some(Duration::from_secs(1)));
    }

    #[test]
    fn cancel_marks_aborted() {
        let mut scheduler = ReconnectScheduler::new();
        scheduler.schedule_reconnect("srv".to_owned());
        assert!(scheduler.is_reconnecting("srv"));
        scheduler.cancel("srv");
        assert!(!scheduler.is_reconnecting("srv"));
        let state = scheduler.reconnect_state("srv").expect("state exists");
        assert!(state.aborted);
    }

    #[test]
    fn cancel_all_aborts_everything() {
        let mut scheduler = ReconnectScheduler::new();
        scheduler.schedule_reconnect("a".to_owned());
        scheduler.schedule_reconnect("b".to_owned());
        scheduler.cancel_all();
        assert!(!scheduler.is_reconnecting("a"));
        assert!(!scheduler.is_reconnecting("b"));
        assert_eq!(scheduler.pending_count(), 0);
    }

    #[test]
    fn backoff_increases_exponentially() {
        assert_eq!(compute_backoff(1, 1000, 30_000), Duration::from_millis(1000));
        assert_eq!(compute_backoff(2, 1000, 30_000), Duration::from_millis(2000));
        assert_eq!(compute_backoff(3, 1000, 30_000), Duration::from_millis(4000));
        assert_eq!(compute_backoff(4, 1000, 30_000), Duration::from_millis(8000));
        assert_eq!(compute_backoff(5, 1000, 30_000), Duration::from_millis(16_000));
        assert_eq!(compute_backoff(6, 1000, 30_000), Duration::from_millis(30_000)); // capped
    }

    #[test]
    fn default_scheduler_has_expected_params() {
        let scheduler = ReconnectScheduler::default();
        assert_eq!(scheduler.max_attempts, 5);
        assert_eq!(scheduler.initial_backoff_ms, 1000);
        assert_eq!(scheduler.max_backoff_ms, 30_000);
        assert_eq!(scheduler.pending_count(), 0);
    }

    #[test]
    fn cancelled_server_schedule_returns_give_up() {
        let mut scheduler = ReconnectScheduler::new();
        scheduler.schedule_reconnect("srv".to_owned());
        scheduler.cancel("srv");
        let action = scheduler.schedule_reconnect("srv".to_owned());
        assert_eq!(action, ReconnectAction::GiveUp);
    }

    #[test]
    fn report_failure_after_max_attempts_returns_none() {
        let mut scheduler = ReconnectScheduler::with_params(1, 100, 1000);
        scheduler.schedule_reconnect("srv".to_owned()); // attempt 1
        let next = scheduler.report_failure("srv");
        assert!(next.is_none());
    }
}
