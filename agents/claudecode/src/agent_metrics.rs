//! Prometheus-compatible metrics for the claudecode agent.
//!
//! Tracks session count, prompt processing latency, tool call latency,
//! and token usage. Self-contained using `std::sync::LazyLock` -- no
//! external metrics crate required. Output is exposed to the control
//! plane via runtime events so the `/metrics` endpoint can scrape it.

use std::collections::BTreeMap;
use std::fmt::Write;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};

// ---------------------------------------------------------------------------
// Internal types
// ---------------------------------------------------------------------------

struct LabeledCounter {
    counters: Mutex<BTreeMap<String, AtomicU64>>,
}

impl LabeledCounter {
    fn new() -> Self {
        Self {
            counters: Mutex::new(BTreeMap::new()),
        }
    }
    fn inc_by(&self, labels: &str, delta: u64) {
        let mut map = self.counters.lock().unwrap();
        map.entry(labels.to_owned())
            .or_insert_with(|| AtomicU64::new(0))
            .fetch_add(delta, Ordering::Relaxed);
    }
    fn snapshot(&self) -> Vec<(String, u64)> {
        let map = self.counters.lock().unwrap();
        map.iter()
            .map(|(k, v)| (k.clone(), v.load(Ordering::Relaxed)))
            .collect()
    }
}

struct SimpleHistogram {
    buckets: &'static [f64],
    data: Mutex<HistogramData>,
}

struct HistogramData {
    bucket_counts: Vec<AtomicU64>,
    sum_micros: AtomicU64,
    count: AtomicU64,
}

impl HistogramData {
    fn new(n: usize) -> Self {
        let bucket_counts = (0..=n).map(|_| AtomicU64::new(0)).collect();
        Self {
            bucket_counts,
            sum_micros: AtomicU64::new(0),
            count: AtomicU64::new(0),
        }
    }
}

impl SimpleHistogram {
    fn new(buckets: &'static [f64]) -> Self {
        Self {
            buckets,
            data: Mutex::new(HistogramData::new(buckets.len())),
        }
    }
    fn observe(&self, value: f64) {
        let data = self.data.lock().unwrap();
        for (i, &bound) in self.buckets.iter().enumerate() {
            if value <= bound {
                data.bucket_counts[i].fetch_add(1, Ordering::Relaxed);
            }
        }
        data.bucket_counts[self.buckets.len()].fetch_add(1, Ordering::Relaxed);
        data.sum_micros
            .fetch_add((value * 1_000_000.0) as u64, Ordering::Relaxed);
        data.count.fetch_add(1, Ordering::Relaxed);
    }
}

// ---------------------------------------------------------------------------
// Bucket definitions
// ---------------------------------------------------------------------------

const PROMPT_BUCKETS: [f64; 10] = [0.1, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0, 60.0, 120.0, 300.0];
const TOOL_BUCKETS: [f64; 9] = [0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0];

// ---------------------------------------------------------------------------
// Global metric instances
// ---------------------------------------------------------------------------

static SESSION_COUNT: AtomicI64 = AtomicI64::new(0);
static PROMPT_LATENCY: LazyLock<SimpleHistogram> =
    LazyLock::new(|| SimpleHistogram::new(&PROMPT_BUCKETS));
static TOOL_LATENCY: LazyLock<SimpleHistogram> =
    LazyLock::new(|| SimpleHistogram::new(&TOOL_BUCKETS));
static TOKEN_INPUT_TOTAL: AtomicU64 = AtomicU64::new(0);
static TOKEN_OUTPUT_TOTAL: AtomicU64 = AtomicU64::new(0);
static TOKEN_CACHE_READ_TOTAL: AtomicU64 = AtomicU64::new(0);
static TOKEN_CACHE_CREATION_TOTAL: AtomicU64 = AtomicU64::new(0);
static TOOL_CALL_COUNTER: LazyLock<LabeledCounter> = LazyLock::new(LabeledCounter::new);

// ---------------------------------------------------------------------------
// Public helpers
// ---------------------------------------------------------------------------

/// Increment active session count.
pub fn session_start() {
    SESSION_COUNT.fetch_add(1, Ordering::Relaxed);
}

/// Decrement active session count.
pub fn session_end() {
    SESSION_COUNT.fetch_sub(1, Ordering::Relaxed);
}

/// Record prompt processing latency.
pub fn record_prompt_latency(duration_secs: f64) {
    PROMPT_LATENCY.observe(duration_secs);
}

/// Record a tool call with its latency.
pub fn record_tool_call(tool_name: &str, duration_secs: f64) {
    TOOL_LATENCY.observe(duration_secs);
    TOOL_CALL_COUNTER.inc_by(tool_name, 1);
}

/// Record token usage from a provider response.
pub fn record_token_usage(input: u64, output: u64, cache_read: u64, cache_creation: u64) {
    TOKEN_INPUT_TOTAL.fetch_add(input, Ordering::Relaxed);
    TOKEN_OUTPUT_TOTAL.fetch_add(output, Ordering::Relaxed);
    TOKEN_CACHE_READ_TOTAL.fetch_add(cache_read, Ordering::Relaxed);
    TOKEN_CACHE_CREATION_TOTAL.fetch_add(cache_creation, Ordering::Relaxed);
}

/// Encode all agent metrics in Prometheus text exposition format.
pub fn encode_metrics() -> String {
    // Force-init lazy statics.
    LazyLock::force(&PROMPT_LATENCY);
    LazyLock::force(&TOOL_LATENCY);
    LazyLock::force(&TOOL_CALL_COUNTER);

    let mut out = String::with_capacity(2048);

    let _ = writeln!(out, "# HELP rc_agent_sessions Number of active agent sessions");
    let _ = writeln!(out, "# TYPE rc_agent_sessions gauge");
    let _ = writeln!(
        out,
        "rc_agent_sessions {}",
        SESSION_COUNT.load(Ordering::Relaxed)
    );

    write_histogram_section(
        &mut out,
        "rc_agent_prompt_duration_seconds",
        "Prompt processing latency",
        &PROMPT_BUCKETS,
        &PROMPT_LATENCY,
    );
    write_histogram_section(
        &mut out,
        "rc_agent_tool_call_duration_seconds",
        "Tool call latency",
        &TOOL_BUCKETS,
        &TOOL_LATENCY,
    );

    let _ = writeln!(out, "# HELP rc_agent_tokens_total Token usage counters");
    let _ = writeln!(out, "# TYPE rc_agent_tokens_total counter");
    let _ = writeln!(
        out,
        "rc_agent_tokens_total{{kind=\"input\"}} {}",
        TOKEN_INPUT_TOTAL.load(Ordering::Relaxed)
    );
    let _ = writeln!(
        out,
        "rc_agent_tokens_total{{kind=\"output\"}} {}",
        TOKEN_OUTPUT_TOTAL.load(Ordering::Relaxed)
    );
    let _ = writeln!(
        out,
        "rc_agent_tokens_total{{kind=\"cache_read\"}} {}",
        TOKEN_CACHE_READ_TOTAL.load(Ordering::Relaxed)
    );
    let _ = writeln!(
        out,
        "rc_agent_tokens_total{{kind=\"cache_creation\"}} {}",
        TOKEN_CACHE_CREATION_TOTAL.load(Ordering::Relaxed)
    );

    let _ = writeln!(out, "# HELP rc_agent_tool_calls_total Tool calls by name");
    let _ = writeln!(out, "# TYPE rc_agent_tool_calls_total counter");
    for (tool_name, count) in TOOL_CALL_COUNTER.snapshot() {
        let _ = writeln!(out, "rc_agent_tool_calls_total{{tool=\"{}\"}} {}", tool_name, count);
    }

    out
}

// ---------------------------------------------------------------------------
// Internal formatting
// ---------------------------------------------------------------------------

fn write_histogram_section(
    out: &mut String,
    name: &str,
    help: &str,
    bucket_bounds: &[f64],
    hist: &SimpleHistogram,
) {
    let _ = writeln!(out, "# HELP {name} {help}");
    let _ = writeln!(out, "# TYPE {name} histogram");

    let data = hist.data.lock().unwrap();
    let counts: Vec<u64> = data
        .bucket_counts
        .iter()
        .map(|c| c.load(Ordering::Relaxed))
        .collect();
    let sum_us = data.sum_micros.load(Ordering::Relaxed);
    let count = data.count.load(Ordering::Relaxed);

    let cumulative = cumulative_counts(&counts);
    for (i, bound) in bucket_bounds.iter().enumerate() {
        let _ = writeln!(
            out,
            "{name}_bucket{{le=\"{}\"}} {}",
            format_bound(*bound),
            cumulative[i]
        );
    }
    let _ = writeln!(
        out,
        "{name}_bucket{{le=\"+Inf\"}} {}",
        cumulative[bucket_bounds.len()]
    );
    let _ = writeln!(out, "{name}_sum {}", sum_us as f64 / 1_000_000.0);
    let _ = writeln!(out, "{name}_count {count}");
}

fn cumulative_counts(counts: &[u64]) -> Vec<u64> {
    let mut cum = Vec::with_capacity(counts.len());
    let mut total: u64 = 0;
    for &c in counts {
        total += c;
        cum.push(total);
    }
    cum
}

fn format_bound(b: f64) -> String {
    if b < 0.01 {
        format!("{:.3}", b)
    } else if b < 1.0 {
        format!("{:.2}", b)
    } else {
        format!("{:.1}", b)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_metrics_produces_valid_output() {
        let output = encode_metrics();
        assert!(output.contains("rc_agent_sessions"));
        assert!(output.contains("rc_agent_prompt_duration_seconds"));
        assert!(output.contains("rc_agent_tool_call_duration_seconds"));
        assert!(output.contains("rc_agent_tokens_total{kind=\"input\"}"));
        assert!(output.contains("# TYPE rc_agent_prompt_duration_seconds histogram"));
    }

    #[test]
    fn session_count_tracks_lifecycle() {
        session_start();
        session_start();
        assert_eq!(SESSION_COUNT.load(Ordering::Relaxed), 2);
        session_end();
        assert_eq!(SESSION_COUNT.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn token_usage_accumulates() {
        let before = TOKEN_INPUT_TOTAL.load(Ordering::Relaxed);
        record_token_usage(100, 50, 200, 30);
        assert_eq!(
            TOKEN_INPUT_TOTAL.load(Ordering::Relaxed),
            before + 100
        );
    }
}
