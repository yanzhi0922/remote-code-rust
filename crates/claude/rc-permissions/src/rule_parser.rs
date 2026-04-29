use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::rules::{RuleAction, RuleSource, SourceAwarePermissionRule};

#[derive(Debug, Default, Deserialize)]
struct PermissionDocument {
    #[serde(default)]
    permissions: PermissionLists,
}

#[derive(Debug, Default, Deserialize)]
struct PermissionLists {
    #[serde(default)]
    allow: Vec<String>,
    #[serde(default)]
    ask: Vec<String>,
    #[serde(default)]
    deny: Vec<String>,
}

pub fn discover_permission_rule_files(
    cwd: &Path,
    profile_dir: &Path,
    settings_files: &[PathBuf],
    cli_settings_files: &[PathBuf],
) -> Vec<(PathBuf, RuleSource)> {
    let explicit_cli_paths = cli_settings_files
        .iter()
        .map(|path| normalize_rule_path(cwd, path))
        .collect::<Vec<_>>();
    let mut files = Vec::new();
    files.extend(settings_files.iter().map(|path| {
        (
            path.clone(),
            classify_settings_rule_source(path, cwd, profile_dir, &explicit_cli_paths),
        )
    }));

    for candidate in [
        cwd.join(".remote-code-rust").join("settings.toml"),
        cwd.join(".remote-code-rust").join("settings.json"),
    ] {
        if candidate.exists() && !files.iter().any(|(path, _)| path == &candidate) {
            files.push((candidate, RuleSource::Project));
        }
    }

    for candidate in [
        profile_dir.join("settings.toml"),
        profile_dir.join("settings.json"),
    ] {
        if candidate.exists() && !files.iter().any(|(path, _)| path == &candidate) {
            files.push((candidate, RuleSource::User));
        }
    }
    files
}

fn classify_settings_rule_source(
    path: &Path,
    cwd: &Path,
    profile_dir: &Path,
    explicit_cli_paths: &[PathBuf],
) -> RuleSource {
    if explicit_cli_paths
        .iter()
        .any(|candidate| candidate == &normalize_rule_path(cwd, path))
    {
        return RuleSource::Cli;
    }

    let project_paths = [
        cwd.join(".remote-code").join("settings.json"),
        cwd.join(".remote-code").join("settings.local.json"),
        cwd.join(".remote-code-rust").join("settings.toml"),
        cwd.join(".remote-code-rust").join("settings.json"),
    ];
    if project_paths.iter().any(|candidate| candidate == path) {
        return RuleSource::Project;
    }

    let user_paths = [
        profile_dir.join("settings.toml"),
        profile_dir.join("settings.json"),
        profile_dir
            .join("profiles")
            .join("legacy-import")
            .join("settings.json"),
    ];
    if user_paths.iter().any(|candidate| candidate == path) {
        return RuleSource::User;
    }

    RuleSource::Cli
}

fn normalize_rule_path(cwd: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

pub fn load_permission_rules_from_file(
    path: &Path,
    source: RuleSource,
) -> Result<Vec<SourceAwarePermissionRule>> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read permission settings {}", path.display()))?;
    let parsed = parse_permission_document(path, &raw)?;
    Ok(materialize_rules(parsed.permissions, source))
}

fn parse_permission_document(path: &Path, raw: &str) -> Result<PermissionDocument> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match extension.as_str() {
        "json" => serde_json::from_str(raw)
            .with_context(|| format!("failed to parse JSON settings {}", path.display())),
        "toml" => toml::from_str(raw)
            .with_context(|| format!("failed to parse TOML settings {}", path.display())),
        _ => toml::from_str(raw)
            .or_else(|_| serde_json::from_str(raw))
            .with_context(|| format!("failed to parse permission settings {}", path.display())),
    }
}

fn materialize_rules(
    permissions: PermissionLists,
    source: RuleSource,
) -> Vec<SourceAwarePermissionRule> {
    let mut rules = Vec::new();
    rules.extend(
        permissions
            .allow
            .into_iter()
            .map(|tool_pattern| SourceAwarePermissionRule {
                tool_pattern,
                action: RuleAction::Allow,
                source,
            }),
    );
    rules.extend(
        permissions
            .ask
            .into_iter()
            .map(|tool_pattern| SourceAwarePermissionRule {
                tool_pattern,
                action: RuleAction::Ask,
                source,
            }),
    );
    rules.extend(
        permissions
            .deny
            .into_iter()
            .map(|tool_pattern| SourceAwarePermissionRule {
                tool_pattern,
                action: RuleAction::Deny,
                source,
            }),
    );
    rules
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use crate::rules::{RuleAction, RuleSource};

    use super::{
        classify_settings_rule_source, discover_permission_rule_files,
        load_permission_rules_from_file,
    };

    #[test]
    fn loads_json_permission_rules() {
        let tempdir = tempdir().expect("tempdir");
        let path = tempdir.path().join("settings.json");
        fs::write(
            &path,
            r#"{"permissions":{"allow":["Bash(git *)"],"ask":["Edit"],"deny":["Bash(rm -rf *)"]}}"#,
        )
        .expect("write settings");

        let rules = load_permission_rules_from_file(&path, RuleSource::Cli).expect("load rules");
        assert_eq!(rules.len(), 3);
        assert_eq!(rules[0].action, RuleAction::Allow);
        assert_eq!(rules[1].action, RuleAction::Ask);
        assert_eq!(rules[2].action, RuleAction::Deny);
    }

    #[test]
    fn discovers_cli_project_and_user_files() {
        let tempdir = tempdir().expect("tempdir");
        let cwd = tempdir.path().join("workspace");
        let profile = tempdir.path().join("profile");
        fs::create_dir_all(cwd.join(".remote-code-rust")).expect("project dir");
        fs::create_dir_all(&profile).expect("profile dir");
        let cli = tempdir.path().join("cli.toml");
        fs::write(&cli, "").expect("cli");
        fs::write(cwd.join(".remote-code-rust").join("settings.json"), "{}").expect("project");
        fs::write(profile.join("settings.toml"), "").expect("user");

        let files = discover_permission_rule_files(&cwd, &profile, &[cli], &[]);
        assert_eq!(files.len(), 3);
    }

    #[test]
    fn classifies_runtime_settings_paths_by_scope() {
        let tempdir = tempdir().expect("tempdir");
        let cwd = tempdir.path().join("workspace");
        let profile = tempdir.path().join("profile");
        let cli = tempdir.path().join("custom.json");

        assert_eq!(
            classify_settings_rule_source(
                &cwd.join(".remote-code").join("settings.json"),
                &cwd,
                &profile,
                &[]
            ),
            RuleSource::Project
        );
        assert_eq!(
            classify_settings_rule_source(
                &cwd.join(".remote-code").join("settings.local.json"),
                &cwd,
                &profile,
                &[]
            ),
            RuleSource::Project
        );
        assert_eq!(
            classify_settings_rule_source(&profile.join("settings.json"), &cwd, &profile, &[]),
            RuleSource::User
        );
        assert_eq!(
            classify_settings_rule_source(
                &profile
                    .join("profiles")
                    .join("legacy-import")
                    .join("settings.json"),
                &cwd,
                &profile,
                &[]
            ),
            RuleSource::User
        );
        assert_eq!(
            classify_settings_rule_source(&cli, &cwd, &profile, &[]),
            RuleSource::Cli
        );
    }

    #[test]
    fn explicit_cli_settings_paths_keep_cli_priority_even_for_standard_locations() {
        let tempdir = tempdir().expect("tempdir");
        let cwd = tempdir.path().join("workspace");
        let profile = tempdir.path().join("profile");
        let project_settings = cwd.join(".remote-code").join("settings.local.json");

        assert_eq!(
            classify_settings_rule_source(
                &project_settings,
                &cwd,
                &profile,
                std::slice::from_ref(&project_settings)
            ),
            RuleSource::Cli
        );
    }
}
