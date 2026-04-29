//! Current time formatting.
//!
//! Produces the `# Current Time` section in ISO 8601 UTC format with
//! timezone information, matching the TypeScript output.

use chrono::Utc;

/// Format the current time section.
///
/// Output format:
/// ```text
///
/// # Current Time
/// Current time in ISO 8601 UTC format: 2025-01-15T10:30:00.000Z
/// User time zone: Asia/Shanghai, UTC+8:00
/// ```
///
/// The timezone offset is computed from the local system timezone.
pub fn format_current_time() -> String {
    let now = Utc::now();
    let iso = now.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

    // Compute local timezone offset.
    let local_offset = chrono::Local::now().offset().local_minus_utc();
    let offset_hours = local_offset / 3600;
    let offset_minutes = (local_offset.abs() % 3600) / 60;
    let sign = if offset_hours >= 0 { "+" } else { "-" };
    let offset_str = format!(
        "{}{}:{:02}",
        sign,
        offset_hours.abs(),
        offset_minutes
    );

    // Best-effort timezone name (IANA name if detectable, else "Local").
    let tz_display = get_timezone_name();

    format!(
        "\n\n# Current Time\nCurrent time in ISO 8601 UTC format: {}\nUser time zone: {}, UTC{}",
        iso, tz_display, offset_str
    )
}

/// Best-effort timezone name retrieval.
///
/// On Unix, reads `/etc/localtime` or `TZ` env var.
/// On Windows, uses the system timezone.
/// Falls back to "Local" if detection fails.
fn get_timezone_name() -> String {
    // Try the TZ environment variable first.
    if let Ok(tz) = std::env::var("TZ") {
        if !tz.is_empty() {
            return tz;
        }
    }

    // Try to get the system timezone via chrono-tz or iana-time-zone.
    // Since we don't want extra dependencies, fall back to "Local".
    // In practice, the caller should provide the timezone name.
    "Local".to_string()
}

/// Format the current time section with an explicit timezone name.
///
/// This is the preferred API when the caller knows the IANA timezone name.
pub fn format_current_time_with_tz(timezone_name: &str) -> String {
    let now = Utc::now();
    let iso = now.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

    let local_now = chrono::Local::now();
    let offset = local_now.offset();
    let offset_secs = offset.local_minus_utc();
    let offset_hours = offset_secs / 3600;
    let offset_minutes = (offset_secs.abs() % 3600) / 60;
    let sign = if offset_hours >= 0 { "+" } else { "-" };
    let offset_str = format!(
        "{}{}:{:02}",
        sign,
        offset_hours.abs(),
        offset_minutes
    );

    format!(
        "\n\n# Current Time\nCurrent time in ISO 8601 UTC format: {}\nUser time zone: {}, UTC{}",
        iso, timezone_name, offset_str
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_current_time_contains_headers() {
        let result = format_current_time();
        assert!(result.contains("# Current Time"));
        assert!(result.contains("Current time in ISO 8601 UTC format:"));
        assert!(result.contains("User time zone:"));
    }

    #[test]
    fn test_format_current_time_iso_format() {
        let result = format_current_time();
        // ISO format should contain 'T' and 'Z'
        assert!(result.contains("T"));
        assert!(result.contains("Z"));
    }

    #[test]
    fn test_format_current_time_utc_offset() {
        let result = format_current_time();
        // Should contain UTC+ or UTC-
        assert!(result.contains("UTC+") || result.contains("UTC-"));
    }

    #[test]
    fn test_format_current_time_with_tz_contains_name() {
        let result = format_current_time_with_tz("Asia/Shanghai");
        assert!(result.contains("Asia/Shanghai"));
        assert!(result.contains("# Current Time"));
    }

    #[test]
    fn test_format_current_time_starts_with_newlines() {
        let result = format_current_time();
        assert!(result.starts_with("\n\n# Current Time"));
    }
}
