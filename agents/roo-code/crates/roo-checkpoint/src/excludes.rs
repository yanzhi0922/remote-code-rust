//! Exclude patterns for the shadow git checkpoint repository.
//!
//! These patterns are written to `.git/info/exclude` in the shadow git repo
//! to prevent large, generated, or binary files from being tracked.
//! Ported from `src/services/checkpoints/excludes.ts` (213 lines).

/// Build artifact directories and common generated output folders.
/// Matches `getBuildArtifactPatterns()` from the TS source.
pub const BUILD_ARTIFACT_PATTERNS: &[&str] = &[
    ".gradle/",
    ".idea/",
    ".parcel-cache/",
    ".pytest_cache/",
    ".next/",
    ".nuxt/",
    ".sass-cache/",
    ".terraform/",
    ".terragrunt-cache/",
    ".vs/",
    ".vscode/",
    "Pods/",
    "__pycache__/",
    "bin/",
    "build/",
    "bundle/",
    "coverage/",
    "deps/",
    "dist/",
    "env/",
    "node_modules/",
    "obj/",
    "out/",
    "pkg/",
    "pycache/",
    "target/dependency/",
    "temp/",
    "vendor/",
    "venv/",
];

/// Media file extensions (images, audio, video).
/// Matches `getMediaFilePatterns()` from the TS source.
pub const MEDIA_FILE_PATTERNS: &[&str] = &[
    "*.jpg",
    "*.jpeg",
    "*.png",
    "*.gif",
    "*.bmp",
    "*.ico",
    "*.webp",
    "*.tiff",
    "*.tif",
    "*.raw",
    "*.heic",
    "*.avif",
    "*.eps",
    "*.psd",
    "*.3gp",
    "*.aac",
    "*.aiff",
    "*.asf",
    "*.avi",
    "*.divx",
    "*.flac",
    "*.m4a",
    "*.m4v",
    "*.mkv",
    "*.mov",
    "*.mp3",
    "*.mp4",
    "*.mpeg",
    "*.mpg",
    "*.ogg",
    "*.opus",
    "*.rm",
    "*.rmvb",
    "*.vob",
    "*.wav",
    "*.webm",
    "*.wma",
    "*.wmv",
];

/// Cache, temporary, and system file patterns.
/// Matches `getCacheFilePatterns()` from the TS source.
pub const CACHE_FILE_PATTERNS: &[&str] = &[
    "*.DS_Store",
    "*.bak",
    "*.cache",
    "*.crdownload",
    "*.dmp",
    "*.dump",
    "*.eslintcache",
    "*.lock",
    "*.log",
    "*.old",
    "*.part",
    "*.partial",
    "*.pyc",
    "*.pyo",
    "*.stackdump",
    "*.swo",
    "*.swp",
    "*.temp",
    "*.tmp",
    "*.Thumbs.db",
];

/// Configuration and environment file patterns.
/// Matches `getConfigFilePatterns()` from the TS source.
pub const CONFIG_FILE_PATTERNS: &[&str] = &[
    "*.env*",
    "*.local",
    "*.development",
    "*.production",
];

/// Large data and binary archive patterns.
/// Matches `getLargeDataFilePatterns()` from the TS source.
pub const LARGE_DATA_FILE_PATTERNS: &[&str] = &[
    "*.zip",
    "*.tar",
    "*.gz",
    "*.rar",
    "*.7z",
    "*.iso",
    "*.bin",
    "*.exe",
    "*.dll",
    "*.so",
    "*.dylib",
    "*.dat",
    "*.dmg",
    "*.msi",
];

/// Database file patterns.
/// Matches `getDatabaseFilePatterns()` from the TS source.
pub const DATABASE_FILE_PATTERNS: &[&str] = &[
    "*.arrow",
    "*.accdb",
    "*.aof",
    "*.avro",
    "*.bak",
    "*.bson",
    "*.csv",
    "*.db",
    "*.dbf",
    "*.dmp",
    "*.frm",
    "*.ibd",
    "*.mdb",
    "*.myd",
    "*.myi",
    "*.orc",
    "*.parquet",
    "*.pdb",
    "*.rdb",
    "*.sql",
    "*.sqlite",
];

/// Geospatial data file patterns.
/// Matches `getGeospatialPatterns()` from the TS source.
pub const GEOSPATIAL_PATTERNS: &[&str] = &[
    "*.shp",
    "*.shx",
    "*.dbf",
    "*.prj",
    "*.sbn",
    "*.sbx",
    "*.shp.xml",
    "*.cpg",
    "*.gdb",
    "*.mdb",
    "*.gpkg",
    "*.kml",
    "*.kmz",
    "*.gml",
    "*.geojson",
    "*.dem",
    "*.asc",
    "*.img",
    "*.ecw",
    "*.las",
    "*.laz",
    "*.mxd",
    "*.qgs",
    "*.grd",
    "*.csv",
    "*.dwg",
    "*.dxf",
];

/// Log file patterns.
/// Matches `getLogFilePatterns()` from the TS source.
pub const LOG_FILE_PATTERNS: &[&str] = &[
    "*.error",
    "*.log",
    "*.logs",
    "*.npm-debug.log*",
    "*.out",
    "*.stdout",
    "yarn-debug.log*",
    "yarn-error.log*",
];

/// Returns the combined list of all static exclude patterns (without LFS patterns).
/// This is the synchronous portion of `getExcludePatterns`.
pub fn get_static_exclude_patterns() -> Vec<&'static str> {
    let mut patterns = Vec::new();
    patterns.push(".git/");
    patterns.extend_from_slice(BUILD_ARTIFACT_PATTERNS);
    patterns.extend_from_slice(MEDIA_FILE_PATTERNS);
    patterns.extend_from_slice(CACHE_FILE_PATTERNS);
    patterns.extend_from_slice(CONFIG_FILE_PATTERNS);
    patterns.extend_from_slice(LARGE_DATA_FILE_PATTERNS);
    patterns.extend_from_slice(DATABASE_FILE_PATTERNS);
    patterns.extend_from_slice(GEOSPATIAL_PATTERNS);
    patterns.extend_from_slice(LOG_FILE_PATTERNS);
    patterns
}

/// Reads LFS patterns from `.gitattributes` in the workspace.
/// Matches `getLfsPatterns()` from the TS source.
///
/// Parses lines like `*.psd filter=lfs diff=lfs merge=lfs -text`
/// and extracts the glob pattern (`*.psd`).
pub async fn get_lfs_patterns(workspace_path: &str) -> Vec<String> {
    let attributes_path = std::path::Path::new(workspace_path).join(".gitattributes");

    match tokio::fs::read_to_string(&attributes_path).await {
        Ok(content) => content
            .lines()
            .filter(|line| line.contains("filter=lfs"))
            .filter_map(|line| line.split_whitespace().next())
            .map(|s| s.to_string())
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Returns the full list of exclude patterns for a given workspace.
/// Combines all static patterns with LFS patterns read from `.gitattributes`.
/// Matches `getExcludePatterns()` from the TS source.
pub async fn get_exclude_patterns(workspace_path: &str) -> Vec<String> {
    let mut patterns: Vec<String> = get_static_exclude_patterns()
        .iter()
        .map(|s| s.to_string())
        .collect();

    let lfs_patterns = get_lfs_patterns(workspace_path).await;
    patterns.extend(lfs_patterns);

    patterns
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_artifact_patterns_count() {
        assert_eq!(BUILD_ARTIFACT_PATTERNS.len(), 29);
    }

    #[test]
    fn test_media_file_patterns_count() {
        assert_eq!(MEDIA_FILE_PATTERNS.len(), 38);
    }

    #[test]
    fn test_cache_file_patterns_count() {
        assert_eq!(CACHE_FILE_PATTERNS.len(), 20);
    }

    #[test]
    fn test_config_file_patterns_count() {
        assert_eq!(CONFIG_FILE_PATTERNS.len(), 4);
    }

    #[test]
    fn test_large_data_file_patterns_count() {
        assert_eq!(LARGE_DATA_FILE_PATTERNS.len(), 14);
    }

    #[test]
    fn test_database_file_patterns_count() {
        assert_eq!(DATABASE_FILE_PATTERNS.len(), 21);
    }

    #[test]
    fn test_geospatial_patterns_count() {
        assert_eq!(GEOSPATIAL_PATTERNS.len(), 27);
    }

    #[test]
    fn test_log_file_patterns_count() {
        assert_eq!(LOG_FILE_PATTERNS.len(), 8);
    }

    #[test]
    fn test_static_exclude_patterns_starts_with_git() {
        let patterns = get_static_exclude_patterns();
        assert_eq!(patterns[0], ".git/");
    }

    #[test]
    fn test_static_exclude_patterns_contains_node_modules() {
        let patterns = get_static_exclude_patterns();
        assert!(patterns.contains(&"node_modules/"));
    }

    #[test]
    fn test_static_exclude_patterns_contains_target_dependency() {
        let patterns = get_static_exclude_patterns();
        assert!(patterns.contains(&"target/dependency/"));
    }

    #[test]
    fn test_static_exclude_patterns_total_count() {
        let patterns = get_static_exclude_patterns();
        // 1 (.git/) + 29 + 38 + 20 + 4 + 14 + 21 + 27 + 8 = 162
        assert_eq!(patterns.len(), 162);
    }

    #[test]
    fn test_media_patterns_include_common_formats() {
        assert!(MEDIA_FILE_PATTERNS.contains(&"*.jpg"));
        assert!(MEDIA_FILE_PATTERNS.contains(&"*.png"));
        assert!(MEDIA_FILE_PATTERNS.contains(&"*.mp4"));
        assert!(MEDIA_FILE_PATTERNS.contains(&"*.mp3"));
        assert!(MEDIA_FILE_PATTERNS.contains(&"*.webp"));
    }

    #[test]
    fn test_cache_patterns_include_common_temp_files() {
        assert!(CACHE_FILE_PATTERNS.contains(&"*.tmp"));
        assert!(CACHE_FILE_PATTERNS.contains(&"*.log"));
        assert!(CACHE_FILE_PATTERNS.contains(&"*.DS_Store"));
        assert!(CACHE_FILE_PATTERNS.contains(&"*.lock"));
    }

    #[test]
    fn test_config_patterns_include_env() {
        assert!(CONFIG_FILE_PATTERNS.contains(&"*.env*"));
        assert!(CONFIG_FILE_PATTERNS.contains(&"*.local"));
    }

    #[test]
    fn test_large_data_patterns_include_archives() {
        assert!(LARGE_DATA_FILE_PATTERNS.contains(&"*.zip"));
        assert!(LARGE_DATA_FILE_PATTERNS.contains(&"*.tar"));
        assert!(LARGE_DATA_FILE_PATTERNS.contains(&"*.exe"));
    }

    #[test]
    fn test_database_patterns_include_common_db_files() {
        assert!(DATABASE_FILE_PATTERNS.contains(&"*.db"));
        assert!(DATABASE_FILE_PATTERNS.contains(&"*.sqlite"));
        assert!(DATABASE_FILE_PATTERNS.contains(&"*.csv"));
        assert!(DATABASE_FILE_PATTERNS.contains(&"*.sql"));
    }

    #[test]
    fn test_geospatial_patterns_include_common_gis_files() {
        assert!(GEOSPATIAL_PATTERNS.contains(&"*.shp"));
        assert!(GEOSPATIAL_PATTERNS.contains(&"*.kml"));
        assert!(GEOSPATIAL_PATTERNS.contains(&"*.geojson"));
    }

    #[test]
    fn test_log_patterns_include_common_log_formats() {
        assert!(LOG_FILE_PATTERNS.contains(&"*.error"));
        assert!(LOG_FILE_PATTERNS.contains(&"*.log"));
        assert!(LOG_FILE_PATTERNS.contains(&"*.out"));
    }

    #[tokio::test]
    async fn test_get_lfs_patterns_no_file() {
        let patterns = get_lfs_patterns("/nonexistent/path").await;
        assert!(patterns.is_empty());
    }

    #[tokio::test]
    async fn test_get_lfs_patterns_with_file() {
        let dir = tempfile::tempdir().unwrap();
        let gitattributes = dir.path().join(".gitattributes");
        tokio::fs::write(
            &gitattributes,
            "*.psd filter=lfs diff=lfs merge=lfs -text\n*.zip filter=lfs merge=lfs -text\nnormal.txt text\n",
        )
        .await
        .unwrap();

        let patterns = get_lfs_patterns(dir.path().to_str().unwrap()).await;
        assert_eq!(patterns, vec!["*.psd", "*.zip"]);
    }

    #[tokio::test]
    async fn test_get_exclude_patterns_includes_all_categories() {
        let dir = tempfile::tempdir().unwrap();
        let patterns = get_exclude_patterns(dir.path().to_str().unwrap()).await;

        // Should include .git/
        assert!(patterns.contains(&".git/".to_string()));
        // Should include build artifacts
        assert!(patterns.contains(&"node_modules/".to_string()));
        // Should include media patterns
        assert!(patterns.contains(&"*.png".to_string()));
        // Should include cache patterns
        assert!(patterns.contains(&"*.tmp".to_string()));
        // Should include config patterns
        assert!(patterns.contains(&"*.env*".to_string()));
        // Should include large data patterns
        assert!(patterns.contains(&"*.zip".to_string()));
        // Should include database patterns
        assert!(patterns.contains(&"*.sqlite".to_string()));
        // Should include geospatial patterns
        assert!(patterns.contains(&"*.shp".to_string()));
        // Should include log patterns
        assert!(patterns.contains(&"*.error".to_string()));
    }
}
