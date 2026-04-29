/// RefreshTimer - A utility for executing a callback with configurable retry behavior.
/// Mirrors packages/cloud/src/RefreshTimer.ts

use std::time::Duration;
use tokio::time::sleep;

/// Configuration options for the RefreshTimer.
#[derive(Clone, Debug)]
pub struct RefreshTimerOptions {
    /// Time in milliseconds to wait before next attempt after success (default: 50000).
    pub success_interval_ms: u64,
    /// Initial backoff time in milliseconds for the first failure (default: 1000).
    pub initial_backoff_ms: u64,
    /// Maximum backoff time in milliseconds (default: 300000).
    pub max_backoff_ms: u64,
}

impl Default for RefreshTimerOptions {
    fn default() -> Self {
        Self {
            success_interval_ms: 50_000,
            initial_backoff_ms: 1_000,
            max_backoff_ms: 300_000,
        }
    }
}

/// A timer utility that executes a callback with configurable retry behavior.
///
/// - If the callback succeeds (returns true), it schedules the next attempt after a fixed interval.
/// - If the callback fails (returns false), it uses exponential backoff up to a maximum interval.
pub struct RefreshTimer {
    options: RefreshTimerOptions,
    current_backoff_ms: u64,
    attempt_count: u32,
    is_running: bool,
}

impl RefreshTimer {
    /// Create a new RefreshTimer with the given options.
    pub fn new(options: RefreshTimerOptions) -> Self {
        Self {
            current_backoff_ms: options.initial_backoff_ms,
            attempt_count: 0,
            is_running: false,
            options,
        }
    }

    /// Check if the timer is currently running.
    pub fn is_running(&self) -> bool {
        self.is_running
    }

    /// Get the current attempt count.
    pub fn attempt_count(&self) -> u32 {
        self.attempt_count
    }

    /// Get the current backoff duration.
    pub fn current_backoff(&self) -> Duration {
        Duration::from_millis(self.current_backoff_ms)
    }

    /// Calculate the delay before the next execution based on the result of the previous callback.
    ///
    /// Returns the delay duration and updates internal state.
    pub fn next_delay(&mut self, success: bool) -> Duration {
        if success {
            self.current_backoff_ms = self.options.initial_backoff_ms;
            self.attempt_count = 0;
            Duration::from_millis(self.options.success_interval_ms)
        } else {
            self.attempt_count += 1;
            let delay = Duration::from_millis(self.current_backoff_ms);
            // Exponential backoff: double the backoff, capped at max
            self.current_backoff_ms = (self.current_backoff_ms * 2).min(self.options.max_backoff_ms);
            delay
        }
    }

    /// Reset the backoff state and attempt count.
    pub fn reset(&mut self) {
        self.current_backoff_ms = self.options.initial_backoff_ms;
        self.attempt_count = 0;
    }

    /// Start the timer loop. This runs the callback repeatedly with appropriate delays.
    /// The callback should return `true` on success and `false` on failure.
    pub async fn run<F, Fut>(&mut self, callback: F)
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = bool>,
    {
        self.is_running = true;

        while self.is_running {
            let success = callback().await;
            let delay = self.next_delay(success);
            sleep(delay).await;
        }
    }

    /// Stop the timer.
    pub fn stop(&mut self) {
        self.is_running = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_options() {
        let opts = RefreshTimerOptions::default();
        assert_eq!(50_000, opts.success_interval_ms);
        assert_eq!(1_000, opts.initial_backoff_ms);
        assert_eq!(300_000, opts.max_backoff_ms);
    }

    #[test]
    fn test_next_delay_success() {
        let opts = RefreshTimerOptions::default();
        let mut timer = RefreshTimer::new(opts);

        let delay = timer.next_delay(true);
        assert_eq!(Duration::from_millis(50_000), delay);
        assert_eq!(0, timer.attempt_count());
    }

    #[test]
    fn test_next_delay_failure_exponential_backoff() {
        let opts = RefreshTimerOptions {
            initial_backoff_ms: 1000,
            max_backoff_ms: 10000,
            ..Default::default()
        };
        let mut timer = RefreshTimer::new(opts);

        // First failure: 1000ms
        let delay1 = timer.next_delay(false);
        assert_eq!(Duration::from_millis(1000), delay1);
        assert_eq!(1, timer.attempt_count());

        // Second failure: 2000ms
        let delay2 = timer.next_delay(false);
        assert_eq!(Duration::from_millis(2000), delay2);

        // Third failure: 4000ms
        let delay3 = timer.next_delay(false);
        assert_eq!(Duration::from_millis(4000), delay3);

        // Fourth failure: 8000ms
        let delay4 = timer.next_delay(false);
        assert_eq!(Duration::from_millis(8000), delay4);

        // Fifth failure: capped at 10000ms
        let delay5 = timer.next_delay(false);
        assert_eq!(Duration::from_millis(10000), delay5);
    }

    #[test]
    fn test_reset() {
        let opts = RefreshTimerOptions::default();
        let mut timer = RefreshTimer::new(opts);

        timer.next_delay(false);
        timer.next_delay(false);
        assert_eq!(2, timer.attempt_count());

        timer.reset();
        assert_eq!(0, timer.attempt_count());
        assert_eq!(Duration::from_millis(1000), timer.current_backoff());
    }

    #[test]
    fn test_success_resets_backoff() {
        let opts = RefreshTimerOptions {
            initial_backoff_ms: 1000,
            max_backoff_ms: 10000,
            ..Default::default()
        };
        let mut timer = RefreshTimer::new(opts);

        timer.next_delay(false);
        timer.next_delay(false);
        assert!(timer.current_backoff_ms > 1000);

        // Success resets everything
        timer.next_delay(true);
        assert_eq!(0, timer.attempt_count());
        assert_eq!(1000, timer.current_backoff_ms);
    }

    #[test]
    fn test_stop() {
        let opts = RefreshTimerOptions {
            success_interval_ms: 10,
            initial_backoff_ms: 10,
            max_backoff_ms: 10,
        };
        let mut timer = RefreshTimer::new(opts);

        // Timer starts not running
        assert!(!timer.is_running());

        // Stop on non-running timer is fine
        timer.stop();
        assert!(!timer.is_running());
    }
}
