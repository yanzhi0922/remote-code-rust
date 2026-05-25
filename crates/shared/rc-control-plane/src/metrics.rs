//! Prometheus-compatible metrics for the control plane.
//!
//! Self-contained implementation using only `std::sync` primitives. Exposes a
//! `/metrics` HTTP endpoint that outputs the standard Prometheus text exposition
//! format. No external metrics crate required.

use std::collections::BTreeMap;
use std::fmt::Write;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};

// ---------------------------------------------------------------------------
// Atomic gauge
// ---------------------------------------------------------------------------

struct AtomicGauge {
    value: AtomicI64,
}

impl AtomicGauge {
    const fn new() -> Self {
        Self {
            value: AtomicI64::new(0),
        }
    }
    fn set(&self, v: i64) {
        self.value.store(v, Ordering::Relaxed);
    }
    fn get(&self) -> i64 {
        self.value.load(Ordering::Relaxed)
    }
    #[allow(dead_code)]
    fn inc(&self) {
        self.value.fetch_add(1, Ordering::Relaxed);
    }
    #[allow(dead_code)]
    fn dec(&self) {
        self.value.fetch_sub(1, Ordering::Relaxed);
    }
}

// ---------------------------------------------------------------------------
// Labeled counter
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
        let mut map = self.counters.lock().unwrap_or_else(|e| e.into_inner());
        map.entry(labels.to_owned())
            .or_insert_with(|| AtomicU64::new(0))
            .fetch_add(delta, Ordering::Relaxed);
    }
    fn snapshot(&self) -> Vec<(String, u64)> {
        let map = self.counters.lock().unwrap_or_else(|e| e.into_inner());
        map.iter()
            .map(|(k, v)| (k.clone(), v.load(Ordering::Relaxed)))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Labeled gauge
// ---------------------------------------------------------------------------

struct LabeledGauge {
    gauges: Mutex<BTreeMap<String, AtomicI64>>,
}

impl LabeledGauge {
    fn new() -> Self {
        Self {
            gauges: Mutex::new(BTreeMap::new()),
        }
    }
    fn inc(&self, labels: &str) {
        let mut map = self.gauges.lock().unwrap_or_else(|e| e.into_inner());
        map.entry(labels.to_owned())
            .or_insert_with(|| AtomicI64::new(0))
            .fetch_add(1, Ordering::Relaxed);
    }
    fn dec(&self, labels: &str) {
        let mut map = self.gauges.lock().unwrap_or_else(|e| e.into_inner());
        // insert 0 first so we go to -1 (matches prometheus behavior).
        map.entry(labels.to_owned())
            .or_insert_with(|| AtomicI64::new(0))
            .fetch_sub(1, Ordering::Relaxed);
    }
    fn snapshot(&self) -> Vec<(String, i64)> {
        let map = self.gauges.lock().unwrap_or_else(|e| e.into_inner());
        map.iter()
            .map(|(k, v)| (k.clone(), v.load(Ordering::Relaxed)))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Labeled histogram
// ---------------------------------------------------------------------------

const HISTOGRAM_BUCKETS: [f64; 11] = [
    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];

const DISPATCH_BUCKETS: [f64; 8] = [0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0];

struct LabeledHistogram {
    buckets: &'static [f64],
    data: Mutex<BTreeMap<String, HistogramData>>,
}

struct HistogramData {
    bucket_counts: Vec<AtomicU64>,
    sum: AtomicU64, // stores sum as fixed-point microseconds
    count: AtomicU64,
}

impl HistogramData {
    fn new(buckets: usize) -> Self {
        let bucket_counts = (0..=buckets).map(|_| AtomicU64::new(0)).collect();
        Self {
            bucket_counts,
            sum: AtomicU64::new(0),
            count: AtomicU64::new(0),
        }
    }
}

impl LabeledHistogram {
    fn new(buckets: &'static [f64]) -> Self {
        Self {
            buckets,
            data: Mutex::new(BTreeMap::new()),
        }
    }

    fn observe(&self, labels: &str, value: f64) {
        let mut map = self.data.lock().unwrap_or_else(|e| e.into_inner());
        let data = map
            .entry(labels.to_owned())
            .or_insert_with(|| HistogramData::new(self.buckets.len()));
        for (i, &bound) in self.buckets.iter().enumerate() {
            if value <= bound {
                data.bucket_counts[i].fetch_add(1, Ordering::Relaxed);
            }
        }
        data.bucket_counts[self.buckets.len()].fetch_add(1, Ordering::Relaxed);
        data.sum
            .fetch_add((value * 1_000_000.0) as u64, Ordering::Relaxed);
        data.count.fetch_add(1, Ordering::Relaxed);
    }

    fn snapshot(&self) -> Vec<(String, HistogramSnapshot)> {
        let map = self.data.lock().unwrap_or_else(|e| e.into_inner());
        map.iter()
            .map(|(k, d)| {
                let counts: Vec<u64> = d
                    .bucket_counts
                    .iter()
                    .map(|c| c.load(Ordering::Relaxed))
                    .collect();
                let sum_us = d.sum.load(Ordering::Relaxed);
                let count = d.count.load(Ordering::Relaxed);
                (
                    k.clone(),
                    HistogramSnapshot {
                        bucket_counts: counts,
                        sum_secs: sum_us as f64 / 1_000_000.0,
                        count,
                    },
                )
            })
            .collect()
    }
}

struct HistogramSnapshot {
    bucket_counts: Vec<u64>,
    sum_secs: f64,
    count: u64,
}

// ---------------------------------------------------------------------------
// Global metric instances (LazyLock for non-const types)
// ---------------------------------------------------------------------------

static RUNNER_CONNECTIONS: AtomicGauge = AtomicGauge::new();
static ACTIVE_SESSIONS: AtomicGauge = AtomicGauge::new();
static AUTH_COUNTER: LazyLock<LabeledCounter> = LazyLock::new(LabeledCounter::new);
static ERROR_COUNTER: LazyLock<LabeledCounter> = LazyLock::new(LabeledCounter::new);
static REQUEST_DURATION: LazyLock<LabeledHistogram> =
    LazyLock::new(|| LabeledHistogram::new(&HISTOGRAM_BUCKETS));
static COMMAND_DISPATCH: LazyLock<LabeledHistogram> =
    LazyLock::new(|| LabeledHistogram::new(&DISPATCH_BUCKETS));
static WS_GAUGE: LazyLock<LabeledGauge> = LazyLock::new(LabeledGauge::new);

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

pub fn encode_metrics() -> String {
    let mut out = String::with_capacity(4096);

    // Gauges
    write_gauge(
        &mut out,
        "rc_control_plane_runner_connections",
        "Number of currently registered runner connections",
        RUNNER_CONNECTIONS.get(),
    );
    write_gauge(
        &mut out,
        "rc_control_plane_active_sessions",
        "Number of sessions currently tracked",
        ACTIVE_SESSIONS.get(),
    );

    for (labels, value) in WS_GAUGE.snapshot() {
        let _ = writeln!(
            out,
            "# HELP rc_control_plane_ws_connections Number of currently open WebSocket connections"
        );
        let _ = writeln!(
            out,
            "# TYPE rc_control_plane_ws_connections gauge"
        );
        let _ = writeln!(
            out,
            "rc_control_plane_ws_connections{{stream_type=\"{}\"}} {}",
            labels, value
        );
    }

    // Counters
    write_counter_header(
        &mut out,
        "rc_control_plane_auth_total",
        "Authentication attempts by outcome",
    );
    for (labels, value) in AUTH_COUNTER.snapshot() {
        let parts: Vec<&str> = labels.splitn(2, ',').collect();
        let mechanism = parts.first().copied().unwrap_or("unknown");
        let outcome = parts.get(1).copied().unwrap_or("unknown");
        let _ = writeln!(
            out,
            "rc_control_plane_auth_total{{mechanism=\"{}\",outcome=\"{}\"}} {}",
            mechanism, outcome, value
        );
    }

    write_counter_header(
        &mut out,
        "rc_control_plane_request_errors_total",
        "Total number of HTTP error responses",
    );
    for (labels, value) in ERROR_COUNTER.snapshot() {
        let parts: Vec<&str> = labels.splitn(3, ',').collect();
        let method = parts.first().copied().unwrap_or("unknown");
        let path = parts.get(1).copied().unwrap_or("unknown");
        let status = parts.get(2).copied().unwrap_or("0");
        let _ = writeln!(
            out,
            "rc_control_plane_request_errors_total{{method=\"{}\",path=\"{}\",status=\"{}\"}} {}",
            method, path, status, value
        );
    }

    // Histograms
    write_histogram(
        &mut out,
        "rc_control_plane_request_duration_seconds",
        "HTTP request latency",
        &HISTOGRAM_BUCKETS,
        REQUEST_DURATION.snapshot(),
    );
    write_histogram(
        &mut out,
        "rc_control_plane_command_dispatch_duration_seconds",
        "Latency for dispatching commands to runners",
        &DISPATCH_BUCKETS,
        COMMAND_DISPATCH.snapshot(),
    );

    out
}

// ---------------------------------------------------------------------------
// Convenience helpers
// ---------------------------------------------------------------------------

pub fn record_request(method: &str, path: &str, duration_secs: f64, status: u16) {
    let key = format!("{method},{path}");
    REQUEST_DURATION.observe(&key, duration_secs);
    if status >= 400 {
        ERROR_COUNTER.inc_by(&format!("{method},{path},{status}"), 1);
    }
}

pub fn record_auth(mechanism: &str, success: bool) {
    let outcome = if success { "success" } else { "failure" };
    AUTH_COUNTER.inc_by(&format!("{mechanism},{outcome}"), 1);
}

pub fn record_command_dispatch(command: &str, duration_secs: f64) {
    COMMAND_DISPATCH.observe(command, duration_secs);
}

pub fn set_runner_connections(count: usize) {
    RUNNER_CONNECTIONS.set(count as i64);
}

pub fn set_active_sessions(count: usize) {
    ACTIVE_SESSIONS.set(count as i64);
}

pub fn ws_connect(stream_type: &str) {
    WS_GAUGE.inc(stream_type);
}

pub fn ws_disconnect(stream_type: &str) {
    WS_GAUGE.dec(stream_type);
}

// ---------------------------------------------------------------------------
// Formatting helpers
// ---------------------------------------------------------------------------

// SAFETY: All writeln! calls below write to a String, which is infallible
// (String's fmt::Write impl never errors). The .unwrap() is safe.

fn write_gauge(out: &mut String, name: &str, help: &str, value: i64) {
    let _ = writeln!(out, "# HELP {name} {help}");
    let _ = writeln!(out, "# TYPE {name} gauge");
    let _ = writeln!(out, "{name} {value}");
}

fn write_counter_header(out: &mut String, name: &str, help: &str) {
    let _ = writeln!(out, "# HELP {name} {help}");
    let _ = writeln!(out, "# TYPE {name} counter");
}

fn write_histogram(
    out: &mut String,
    name: &str,
    help: &str,
    bucket_bounds: &[f64],
    data: Vec<(String, HistogramSnapshot)>,
) {
    let _ = writeln!(out, "# HELP {name} {help}");
    let _ = writeln!(out, "# TYPE {name} histogram");

    if data.is_empty() {
        let _ = writeln!(out, "{name}_bucket{{le=\"+Inf\"}} 0");
        let _ = writeln!(out, "{name}_sum 0");
        let _ = writeln!(out, "{name}_count 0");
        return;
    }

    for (_labels, snap) in &data {
        let cumulative = cumulative_counts(&snap.bucket_counts);
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
        let _ = writeln!(out, "{name}_sum {}", snap.sum_secs);
        let _ = writeln!(out, "{name}_count {}", snap.count);
    }
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
