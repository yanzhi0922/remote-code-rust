//! Versioned settings resolution for dynamic router providers.
//!
//! Derived from `src/api/providers/fetchers/versionedSettings.ts`.
//!
//! Resolves version-keyed settings based on the current plugin version.
//! Finds the highest version key that is ≤ the current version.

use serde_json::Value;
use std::collections::HashMap;

/// Compares two semantic version strings.
///
/// Returns `Ordering`: `Less` if v1 < v2, `Equal` if same, `Greater` if v1 > v2.
/// Strips pre-release suffixes before comparing.
pub fn compare_semver(v1: &str, v2: &str) -> std::cmp::Ordering {
    let a = v1.split('-').next().unwrap_or(v1);
    let b = v2.split('-').next().unwrap_or(v2);

    let pa: Vec<u64> = a.split('.').filter_map(|s| s.parse().ok()).collect();
    let pb: Vec<u64> = b.split('.').filter_map(|s| s.parse().ok()).collect();

    for i in 0..pa.len().max(pb.len()) {
        let na = pa.get(i).copied().unwrap_or(0);
        let nb = pb.get(i).copied().unwrap_or(0);
        match na.cmp(&nb) {
            std::cmp::Ordering::Equal => continue,
            other => return other,
        }
    }
    std::cmp::Ordering::Equal
}

/// Finds the highest version from `versioned_settings` that is ≤ `current_version`.
///
/// For nightly builds (version contains "nightly"), always picks the highest available.
pub fn find_highest_matching_version(
    versioned_settings: &HashMap<String, Value>,
    current_version: &str,
) -> Option<String> {
    if versioned_settings.is_empty() {
        return None;
    }

    let is_nightly = current_version.to_lowercase().contains("nightly");

    let mut versions: Vec<&String> = versioned_settings.keys().collect();
    versions.sort_by(|a, b| compare_semver(b, a));

    if is_nightly {
        return versions.into_iter().next().cloned();
    }

    versions
        .into_iter()
        .filter(|v| compare_semver(current_version, v) >= std::cmp::Ordering::Equal)
        .next()
        .cloned()
}

/// Resolves versioned settings by finding the highest matching version.
///
/// Returns the settings for the highest version key that is ≤ `current_version`,
/// or an empty `Value::Object` if none match.
pub fn resolve_versioned_settings(
    versioned_settings: &HashMap<String, Value>,
    current_version: &str,
) -> Value {
    if let Some(version) = find_highest_matching_version(versioned_settings, current_version) {
        versioned_settings
            .get(&version)
            .cloned()
            .unwrap_or(Value::Object(Default::default()))
    } else {
        Value::Object(Default::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_compare_semver_equal() {
        assert_eq!(
            compare_semver("3.36.4", "3.36.4"),
            std::cmp::Ordering::Equal
        );
    }

    #[test]
    fn test_compare_semver_greater() {
        assert_eq!(
            compare_semver("3.37.0", "3.36.4"),
            std::cmp::Ordering::Greater
        );
    }

    #[test]
    fn test_compare_semver_less() {
        assert_eq!(compare_semver("3.35.0", "3.36.4"), std::cmp::Ordering::Less);
    }

    #[test]
    fn test_compare_semver_prerelease() {
        assert_eq!(
            compare_semver("3.36.4-beta.1", "3.36.4"),
            std::cmp::Ordering::Equal
        );
    }

    #[test]
    fn test_find_highest_matching_version_basic() {
        let mut settings = HashMap::new();
        settings.insert("3.36.4".to_string(), Value::String("a".to_string()));
        settings.insert("3.35.0".to_string(), Value::String("b".to_string()));

        assert_eq!(
            find_highest_matching_version(&settings, "3.36.4"),
            Some("3.36.4".to_string())
        );
        assert_eq!(
            find_highest_matching_version(&settings, "3.36.0"),
            Some("3.35.0".to_string())
        );
        assert_eq!(find_highest_matching_version(&settings, "3.34.0"), None);
    }

    #[test]
    fn test_find_highest_matching_version_nightly() {
        let mut settings = HashMap::new();
        settings.insert("3.36.4".to_string(), Value::String("a".to_string()));
        settings.insert("3.35.0".to_string(), Value::String("b".to_string()));

        // Nightly picks the highest regardless of current version
        assert_eq!(
            find_highest_matching_version(&settings, "3.30.0-nightly"),
            Some("3.36.4".to_string())
        );
    }

    #[test]
    fn test_resolve_versioned_settings() {
        let mut settings = HashMap::new();
        settings.insert(
            "3.36.4".to_string(),
            serde_json::json!({"includedTools": ["apply_diff"]}),
        );
        settings.insert(
            "3.35.0".to_string(),
            serde_json::json!({"includedTools": ["search_replace"]}),
        );

        let result = resolve_versioned_settings(&settings, "3.36.4");
        assert_eq!(result["includedTools"][0], "apply_diff");

        let result = resolve_versioned_settings(&settings, "3.34.0");
        assert!(result.as_object().unwrap().is_empty());
    }
}
