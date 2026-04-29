//! Custom instructions section.
//!
//! Source: `src/core/prompts/sections/custom-instructions.ts`

use std::fs;
use std::path::{Path, PathBuf};

use crate::types::SystemPromptSettings;

/// Maximum depth for recursive symlink resolution.
const MAX_DEPTH: usize = 5;

/// Language code to human-readable name mapping.
///
/// Source: `src/shared/language.ts` — `LANGUAGES`
const LANGUAGE_NAMES: &[(&str, &str)] = &[
    ("ca", "Català"),
    ("de", "Deutsch"),
    ("en", "English"),
    ("es", "Español"),
    ("fr", "Français"),
    ("hi", "हिन्दी"),
    ("id", "Bahasa Indonesia"),
    ("it", "Italiano"),
    ("ja", "日本語"),
    ("ko", "한국어"),
    ("nl", "Nederlands"),
    ("pl", "Polski"),
    ("pt-BR", "Português"),
    ("ru", "Русский"),
    ("tr", "Türkçe"),
    ("vi", "Tiếng Việt"),
    ("zh-CN", "简体中文"),
    ("zh-TW", "繁體中文"),
];

/// Returns the human-readable language name for a language code.
/// Falls back to the code itself if not found.
///
/// Source: `src/shared/language.ts` — `LANGUAGES` + `isLanguage`
fn get_language_name(code: &str) -> &str {
    LANGUAGE_NAMES
        .iter()
        .find(|(k, _)| *k == code)
        .map(|(_, v)| *v)
        .unwrap_or(code)
}

/// Cache file patterns to exclude from rule compilation.
const CACHE_PATTERNS: &[&str] = &[
    "*.DS_Store",
    "*.bak",
    "*.cache",
    "*.crdownload",
    "*.db",
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
    "Thumbs.db",
];

/// Check if a file should be included in rule compilation.
/// Excludes cache files and system files.
///
/// Source: `src/core/prompts/sections/custom-instructions.ts` — `shouldIncludeRuleFile`
fn should_include_rule_file(filename: &str) -> bool {
    let basename = Path::new(filename)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    !CACHE_PATTERNS.iter().any(|pattern| {
        // TS uses pattern.slice(1) which preserves the leading dot (e.g. "*.lock" → ".lock")
        if let Some(ext) = pattern.strip_prefix("*") {
            basename.ends_with(ext)
        } else {
            basename == *pattern
        }
    })
}

/// Safely read a file and return its trimmed content.
///
/// Source: `src/core/prompts/sections/custom-instructions.ts` — `safeReadFile`
fn safe_read_file(file_path: &Path) -> String {
    fs::read_to_string(file_path)
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

/// Check if a directory exists.
fn directory_exists(dir_path: &Path) -> bool {
    dir_path.is_dir()
}

/// Internal representation tracking both original path (for sorting) and resolved path (for reading).
/// Matches TS `resolveDirectoryEntry` + `resolveSymLink` dual-path tracking.
type FileInfo = (String, String, String); // (original_path, resolved_path, content)

/// Read all text files from a directory in alphabetical order.
///
/// Source: `src/core/prompts/sections/custom-instructions.ts` — `readTextFilesFromDirectory`
fn read_text_files_from_directory(dir_path: &Path) -> Vec<(String, String)> {
    let mut file_info: Vec<FileInfo> = Vec::new();

    if let Ok(entries) = fs::read_dir(dir_path) {
        collect_files_recursive(dir_path, entries, &mut file_info, 0);
    }

    // Sort alphabetically by original filename (case-insensitive).
    // For symlinks, this uses the symlink name, not the target name — matching TS behavior.
    file_info.sort_by(|a, b| {
        let name_a = Path::new(&a.0)
            .file_name()
            .map(|n| n.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let name_b = Path::new(&b.0)
            .file_name()
            .map(|n| n.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        name_a.cmp(&name_b)
    });

    // Map to (resolved_path, content) for external use — display uses resolved path
    file_info
        .into_iter()
        .map(|(_, resolved, content)| (resolved, content))
        .collect()
}

/// Recursively collect files from a directory.
///
/// Tracks both `original_path` (the entry/symlink path, used for sorting) and
/// `resolved_path` (the actual file path, used for reading and display).
/// Matches TS `resolveDirectoryEntry` + `resolveSymLink` dual-path tracking.
fn collect_files_recursive(
    _dir_path: &Path,
    entries: fs::ReadDir,
    file_info: &mut Vec<FileInfo>,
    depth: usize,
) {
    if depth > MAX_DEPTH {
        return;
    }

    for entry in entries.flatten() {
        let original_path = entry.path();

        // Handle symlinks — track both original and resolved paths
        let resolved_path = if original_path.is_symlink() {
            match fs::read_link(&original_path) {
                Ok(target) => {
                    let resolved = if target.is_absolute() {
                        target
                    } else {
                        original_path.parent().unwrap_or(Path::new(".")).join(target)
                    };
                    if resolved.is_dir() {
                        if let Ok(sub_entries) = fs::read_dir(&resolved) {
                            collect_files_recursive(&resolved, sub_entries, file_info, depth + 1);
                        }
                        continue;
                    } else {
                        resolved
                    }
                }
                Err(_) => continue,
            }
        } else {
            original_path.clone()
        };

        if resolved_path.is_file() {
            let path_str = resolved_path.to_string_lossy().to_string();
            if !should_include_rule_file(&path_str) {
                continue;
            }
            let content = safe_read_file(&resolved_path);
            if !content.is_empty() {
                file_info.push((
                    original_path.to_string_lossy().to_string(),
                    resolved_path.to_string_lossy().to_string(),
                    content,
                ));
            }
        } else if resolved_path.is_dir() {
            if let Ok(sub_entries) = fs::read_dir(&resolved_path) {
                collect_files_recursive(&resolved_path, sub_entries, file_info, depth + 1);
            }
        }
    }
}

/// Format content from multiple files with filenames as headers.
///
/// Source: `src/core/prompts/sections/custom-instructions.ts` — `formatDirectoryContent`
fn format_directory_content(files: &[(String, String)], cwd: &str) -> String {
    if files.is_empty() {
        return String::new();
    }

    files
        .iter()
        .map(|(filename, content)| {
            let display_path = Path::new(filename)
                .strip_prefix(cwd)
                .unwrap_or(Path::new(filename))
                .to_string_lossy()
                .replace('\\', "/");
            format!("# Rules from {}:\n{}", display_path, content)
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Returns the global .roo directory path.
fn get_global_roo_directory() -> Option<PathBuf> {
    dirs_home_dir().map(|home| home.join(".roo"))
}

/// Get the home directory using standard environment variables.
fn dirs_home_dir() -> Option<PathBuf> {
    if cfg!(windows) {
        std::env::var("USERPROFILE").ok().map(PathBuf::from)
    } else {
        std::env::var("HOME").ok().map(PathBuf::from)
    }
}

/// Returns the .roo directories for the given cwd (project-local and global).
///
/// Source: `src/services/roo-config.ts` — `getRooDirectoriesForCwd`
fn get_roo_directories_for_cwd(cwd: &str) -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    // Project-local .roo directory
    let local_roo = Path::new(cwd).join(".roo");
    if local_roo.is_dir() {
        dirs.push(local_roo);
    }

    // Global .roo directory
    if let Some(global_roo) = get_global_roo_directory() {
        if global_roo.is_dir() {
            dirs.push(global_roo);
        }
    }

    dirs
}

/// Returns all .roo directories including subdirectories.
///
/// Source: `src/services/roo-config.ts` — `getAllRooDirectoriesForCwd`
fn get_all_roo_directories_for_cwd(cwd: &str) -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    // Project-local .roo directory
    let local_roo = Path::new(cwd).join(".roo");
    if local_roo.is_dir() {
        dirs.push(local_roo);
    }

    // Recursively find .roo directories in subdirectories
    if let Ok(entries) = fs::read_dir(cwd) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                find_roo_dirs_recursive(&path, &mut dirs);
            }
        }
    }

    // Global .roo directory
    if let Some(global_roo) = get_global_roo_directory() {
        if global_roo.is_dir() {
            dirs.push(global_roo);
        }
    }

    dirs
}

/// Recursively find .roo directories.
fn find_roo_dirs_recursive(dir: &Path, result: &mut Vec<PathBuf>) {
    let roo_dir = dir.join(".roo");
    if roo_dir.is_dir() {
        result.push(roo_dir);
    }

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && !path.file_name().map(|n| n == ".git").unwrap_or(false) {
                find_roo_dirs_recursive(&path, result);
            }
        }
    }
}

/// Load rule files from global, project-local, and optionally subfolder directories.
/// Rules are loaded in order: global first, then project-local, then subfolders (alphabetically).
///
/// Source: `src/core/prompts/sections/custom-instructions.ts` — `loadRuleFiles`
pub fn load_rule_files(cwd: &str, enable_subfolder_rules: bool) -> String {
    let mut rules: Vec<String> = Vec::new();

    let roo_directories = if enable_subfolder_rules {
        get_all_roo_directories_for_cwd(cwd)
    } else {
        get_roo_directories_for_cwd(cwd)
    };

    // Check for .roo/rules/ directories in order
    for roo_dir in &roo_directories {
        let rules_dir = roo_dir.join("rules");
        if directory_exists(&rules_dir) {
            let files = read_text_files_from_directory(&rules_dir);
            if !files.is_empty() {
                let content = format_directory_content(&files, cwd);
                rules.push(content);
            }
        }
    }

    // If we found rules in .roo/rules/ directories, return them
    if !rules.is_empty() {
        return format!(
            "\n# Rules from .roo directories:\n\n{}",
            rules.join("\n\n")
        );
    }

    // Fall back to existing behavior for legacy .roorules/.clinerules files
    let rule_files = [".roorules", ".clinerules"];

    for file in &rule_files {
        let content = safe_read_file(&Path::new(cwd).join(file));
        if !content.is_empty() {
            return format!("\n# Rules from {}:\n{}\n", file, content);
        }
    }

    String::new()
}

/// Load mode-specific rules from .roo/rules-{mode}/ directories.
///
/// Source: `src/core/prompts/sections/custom-instructions.ts` — mode rules loading
fn load_mode_rule_files(cwd: &str, mode: &str, enable_subfolder_rules: bool) -> (String, String) {
    let mut mode_rules: Vec<String> = Vec::new();

    let roo_directories = if enable_subfolder_rules {
        get_all_roo_directories_for_cwd(cwd)
    } else {
        get_roo_directories_for_cwd(cwd)
    };

    let mode_rules_dir_name = format!("rules-{}", mode);

    for roo_dir in &roo_directories {
        let mode_rules_dir = roo_dir.join(&mode_rules_dir_name);
        if directory_exists(&mode_rules_dir) {
            let files = read_text_files_from_directory(&mode_rules_dir);
            if !files.is_empty() {
                let content = format_directory_content(&files, cwd);
                mode_rules.push(content);
            }
        }
    }

    // If we found mode-specific rules in .roo/rules-${mode}/ directories, use them
    if !mode_rules.is_empty() {
        let used_rule_file = format!("rules-{} directories", mode);
        return (
            format!("\n{}", mode_rules.join("\n\n")),
            used_rule_file,
        );
    }

    // Fall back to existing behavior for legacy files
    let roo_mode_rule_file = format!(".roorules-{}", mode);
    let content = safe_read_file(&Path::new(cwd).join(&roo_mode_rule_file));
    if !content.is_empty() {
        return (content, roo_mode_rule_file);
    }

    let cline_mode_rule_file = format!(".clinerules-{}", mode);
    let content = safe_read_file(&Path::new(cwd).join(&cline_mode_rule_file));
    if !content.is_empty() {
        return (content, cline_mode_rule_file);
    }

    (String::new(), String::new())
}

/// Load AGENTS.md or AGENT.md file from a specific directory.
///
/// Source: `src/core/prompts/sections/custom-instructions.ts` — `loadAgentRulesFileFromDirectory`
fn load_agent_rules_file_from_directory(
    directory: &Path,
    show_path: bool,
    cwd: &str,
) -> String {
    let filenames = ["AGENTS.md", "AGENT.md"];
    let mut results: Vec<String> = Vec::new();
    let display_path = Path::new(directory)
        .strip_prefix(cwd)
        .unwrap_or(directory)
        .to_string_lossy()
        .replace('\\', "/");

    for filename in &filenames {
        let agent_path = directory.join(filename);
        let content = safe_read_file(&agent_path);

        if !content.is_empty() {
            let header = if show_path {
                format!(
                    "# Agent Rules Standard ({}) from {}:",
                    filename, display_path
                )
            } else {
                format!("# Agent Rules Standard ({}):", filename)
            };
            results.push(format!("{}\n{}", header, content));
            break;
        }
    }

    // Always try to load AGENTS.local.md for personal overrides
    let local_filename = "AGENTS.local.md";
    let local_path = directory.join(local_filename);
    let local_content = safe_read_file(&local_path);

    if !local_content.is_empty() {
        let local_header = if show_path {
            format!(
                "# Agent Rules Local ({}) from {}:",
                local_filename, display_path
            )
        } else {
            format!("# Agent Rules Local ({}):", local_filename)
        };
        results.push(format!("{}\n{}", local_header, local_content));
    }

    results.join("\n\n")
}

/// Load all AGENTS.md files from project root and optionally subdirectories.
///
/// Source: `src/core/prompts/sections/custom-instructions.ts` — `loadAllAgentRulesFiles`
fn load_all_agent_rules_files(cwd: &str, enable_subfolder_rules: bool) -> String {
    let mut agent_rules: Vec<String> = Vec::new();

    if !enable_subfolder_rules {
        let content =
            load_agent_rules_file_from_directory(Path::new(cwd), false, cwd);
        let trimmed = content.trim();
        if !trimmed.is_empty() {
            agent_rules.push(trimmed.to_string());
        }
        return agent_rules.join("\n\n");
    }

    // When enabled, load from root and all subdirectories with .roo folders
    let directories = get_agents_directories_for_cwd(cwd);

    for directory in &directories {
        let show_path = directory != Path::new(cwd);
        let content = load_agent_rules_file_from_directory(directory, show_path, cwd);
        let trimmed = content.trim();
        if !trimmed.is_empty() {
            agent_rules.push(trimmed.to_string());
        }
    }

    agent_rules.join("\n\n")
}

/// Returns directories that may contain AGENTS.md files.
fn get_agents_directories_for_cwd(cwd: &str) -> Vec<PathBuf> {
    let mut dirs = vec![PathBuf::from(cwd)];

    // Find subdirectories with .roo folders
    if let Ok(entries) = fs::read_dir(cwd) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && path.join(".roo").is_dir() {
                dirs.push(path);
            }
        }
    }

    dirs
}

/// Add custom instructions to the system prompt.
///
/// Source: `src/core/prompts/sections/custom-instructions.ts` — `addCustomInstructions`
pub fn add_custom_instructions(
    mode_custom_instructions: &str,
    global_custom_instructions: &str,
    cwd: &str,
    mode: &str,
    language: Option<&str>,
    roo_ignore_instructions: Option<&str>,
    settings: Option<&SystemPromptSettings>,
) -> String {
    let mut sections: Vec<String> = Vec::new();

    let enable_subfolder_rules = settings
        .map(|s| s.enable_subfolder_rules)
        .unwrap_or(false);
    let use_agent_rules = settings
        .map(|s| s.use_agent_rules)
        .unwrap_or(true);

    // Load mode-specific rules if mode is provided
    let (mode_rule_content, used_rule_file) = if !mode.is_empty() {
        load_mode_rule_files(cwd, mode, enable_subfolder_rules)
    } else {
        (String::new(), String::new())
    };

    // Add language preference if provided
    if let Some(lang) = language {
        let language_name = get_language_name(lang);
        sections.push(format!(
            "Language Preference:\nYou should always speak and think in the \"{}\" ({}) language unless the user gives you instructions below to do otherwise.",
            language_name, lang
        ));
    }

    // Add global instructions first
    let global_trimmed = global_custom_instructions.trim();
    if !global_trimmed.is_empty() {
        sections.push(format!("Global Instructions:\n{}", global_trimmed));
    }

    // Add mode-specific instructions after
    let mode_trimmed = mode_custom_instructions.trim();
    if !mode_trimmed.is_empty() {
        sections.push(format!("Mode-specific Instructions:\n{}", mode_trimmed));
    }

    // Add rules - include both mode-specific and generic rules if they exist
    let mut rules: Vec<String> = Vec::new();

    // Add mode-specific rules first if they exist
    let mode_rule_trimmed = mode_rule_content.trim();
    if !mode_rule_trimmed.is_empty() {
        // Match TS: usedRuleFile.includes(path.join(".roo", `rules-${mode}`))
        // path.join(".roo", "rules-{mode}") produces ".roo/rules-{mode}" on Unix,
        // ".roo\rules-{mode}" on Windows. None of the possible usedRuleFile values
        // contain this substring, so this check always fails (matching TS behavior).
        let roo_rules_mode_unix = format!(".roo/rules-{}", mode);
        let roo_rules_mode_win = format!(".roo\\rules-{}", mode);
        if used_rule_file.contains(&roo_rules_mode_unix) || used_rule_file.contains(&roo_rules_mode_win) {
            rules.push(mode_rule_trimmed.to_string());
        } else {
            // TS uses untrimmed modeRuleContent (which starts with \n for directory rules),
            // preserving the leading newline for consistent formatting.
            rules.push(format!("# Rules from {}:\n{}", used_rule_file, mode_rule_content));
        }
    }

    if let Some(roo_ignore) = roo_ignore_instructions {
        if !roo_ignore.trim().is_empty() {
            rules.push(roo_ignore.to_string());
        }
    }

    // Add AGENTS.md content if enabled (default: true)
    if use_agent_rules {
        let agent_rules_content = load_all_agent_rules_files(cwd, enable_subfolder_rules);
        let trimmed = agent_rules_content.trim();
        if !trimmed.is_empty() {
            rules.push(trimmed.to_string());
        }
    }

    // Add generic rules
    let generic_rule_content = load_rule_files(cwd, enable_subfolder_rules);
    let generic_trimmed = generic_rule_content.trim();
    if !generic_trimmed.is_empty() {
        rules.push(generic_trimmed.to_string());
    }

    if !rules.is_empty() {
        sections.push(format!("Rules:\n\n{}", rules.join("\n\n")));
    }

    let joined_sections = sections.join("\n\n");

    if joined_sections.is_empty() {
        String::new()
    } else {
        format!(
            r#"

====

USER'S CUSTOM INSTRUCTIONS

The following additional instructions are provided by the user, and should be followed to the best of your ability.

{joined_sections}
"#
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_include_rule_file() {
        assert!(should_include_rule_file("rules.md"));
        assert!(should_include_rule_file("my-rules.txt"));
        assert!(!should_include_rule_file("Thumbs.db"));
        assert!(!should_include_rule_file("cache.lock"));
        assert!(!should_include_rule_file("output.log"));
        assert!(!should_include_rule_file(".DS_Store"));
    }

    #[test]
    fn test_add_custom_instructions_empty() {
        let result = add_custom_instructions(
            "",
            "",
            "/tmp/test",
            "code",
            None,
            None,
            None,
        );
        assert!(result.is_empty() || result.trim().is_empty());
    }

    #[test]
    fn test_add_custom_instructions_with_language() {
        let result = add_custom_instructions(
            "",
            "",
            "/tmp/test",
            "code",
            Some("zh-CN"),
            None,
            None,
        );
        assert!(result.contains("Language Preference"));
        assert!(result.contains("zh-CN"));
    }

    #[test]
    fn test_add_custom_instructions_with_global_instructions() {
        let result = add_custom_instructions(
            "",
            "Always use TypeScript",
            "/tmp/test",
            "code",
            None,
            None,
            None,
        );
        assert!(result.contains("Global Instructions"));
        assert!(result.contains("Always use TypeScript"));
    }

    #[test]
    fn test_add_custom_instructions_with_mode_instructions() {
        let result = add_custom_instructions(
            "Focus on testing",
            "",
            "/tmp/test",
            "code",
            None,
            None,
            None,
        );
        assert!(result.contains("Mode-specific Instructions"));
        assert!(result.contains("Focus on testing"));
    }
}
