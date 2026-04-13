use std::collections::BTreeSet;
use std::path::PathBuf;

use anyhow::{Result, anyhow};
use rc_config::RuntimeConfig;
use rc_skills::{DEFAULT_SKILL_LOCK_FILE, SkillMetadata, load_skill_lock_file};

use crate::cli::{SkillsCommand, SkillsListArgs, SkillsLockArgs, SkillsShowArgs};

#[derive(Debug, Clone, serde::Serialize)]
struct RuntimeSkillRecord {
    slug: String,
    title: String,
    summary: Option<String>,
    origin_kind: String,
    origin_name: String,
    path: PathBuf,
    root: PathBuf,
    tools: Vec<String>,
    triggers: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct SkillsListOutput {
    warnings: Vec<String>,
    skills: Vec<RuntimeSkillRecord>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct SkillShowOutput {
    warnings: Vec<String>,
    skill: RuntimeSkillRecord,
    #[serde(skip_serializing_if = "Option::is_none")]
    instructions: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct SkillsLockOutput {
    path: PathBuf,
    exists: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    lock: Option<rc_skills::SkillLockFile>,
}

pub(crate) fn run_skills(config: &RuntimeConfig, command: SkillsCommand) -> Result<()> {
    match command {
        SkillsCommand::List(args) => run_skills_list(config, args),
        SkillsCommand::Show(args) => run_skills_show(config, args),
        SkillsCommand::Lock(args) => run_skills_lock(config, args),
    }
}

fn run_skills_list(config: &RuntimeConfig, args: SkillsListArgs) -> Result<()> {
    let output = build_skills_list_output(config, !args.no_plugins);
    if args.json {
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    if output.skills.is_empty() {
        println!("No skills found.");
        for warning in output.warnings {
            println!("  - {warning}");
        }
        return Ok(());
    }

    for warning in &output.warnings {
        println!("warning: {warning}");
    }
    for skill in &output.skills {
        println!(
            "{}  {}  {} ({})",
            skill.slug,
            skill.title,
            skill.origin_kind,
            skill.path.display()
        );
        if let Some(summary) = &skill.summary {
            println!("  summary: {summary}");
        }
        if !skill.triggers.is_empty() {
            println!("  triggers: {}", skill.triggers.join(", "));
        }
        if !skill.tools.is_empty() {
            println!("  tools: {}", skill.tools.join(", "));
        }
        if skill.origin_kind == "plugin" {
            println!("  plugin: {}", skill.origin_name);
        }
    }
    Ok(())
}

fn run_skills_show(config: &RuntimeConfig, args: SkillsShowArgs) -> Result<()> {
    let output = build_skill_show_output(config, &args.skill, !args.no_plugins, args.include_instructions)?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    for warning in &output.warnings {
        println!("warning: {warning}");
    }
    let skill = &output.skill;
    println!("skill: {} ({})", skill.slug, skill.title);
    println!("origin: {} {}", skill.origin_kind, skill.origin_name);
    println!("path: {}", skill.path.display());
    if let Some(summary) = &skill.summary {
        println!("summary: {summary}");
    }
    if !skill.triggers.is_empty() {
        println!("triggers: {}", skill.triggers.join(", "));
    }
    if !skill.tools.is_empty() {
        println!("tools: {}", skill.tools.join(", "));
    }
    if let Some(instructions) = &output.instructions {
        println!("instructions:");
        for line in instructions.lines() {
            println!("  {line}");
        }
    }
    Ok(())
}

fn run_skills_lock(config: &RuntimeConfig, args: SkillsLockArgs) -> Result<()> {
    let path = config.paths.profile_dir.join(DEFAULT_SKILL_LOCK_FILE);
    let output = if path.exists() {
        SkillsLockOutput {
            path: path.clone(),
            exists: true,
            lock: Some(load_skill_lock_file(&path)?),
        }
    } else {
        SkillsLockOutput {
            path,
            exists: false,
            lock: None,
        }
    };

    if args.json {
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    println!("lock path: {}", output.path.display());
    println!("exists: {}", output.exists);
    if let Some(lock) = output.lock {
        println!("version: {}", lock.version);
        println!("installed skills: {}", lock.skills.len());
        for (slug, record) in lock.skills.iter().take(20) {
            println!("  {}  {}  {}", slug, record.source, record.skill_path.display());
        }
    }
    Ok(())
}

fn build_skills_list_output(config: &RuntimeConfig, include_plugins: bool) -> SkillsListOutput {
    let mut warnings = Vec::new();
    let mut skills = Vec::new();
    let mut seen = BTreeSet::new();

    if config.paths.skills_dir.exists() {
        match rc_skills::discover_skills(&config.paths.skills_dir) {
            Ok(discovered) => {
                for skill in discovered {
                    let record = skill_record("profile", "profile", &skill.metadata);
                    seen.insert(record.slug.clone());
                    skills.push(record);
                }
            }
            Err(error) => warnings.push(format!(
                "Failed to discover profile skills in {}: {error}",
                config.paths.skills_dir.display()
            )),
        }
    }

    if include_plugins && config.paths.plugins_dir.exists() {
        match rc_plugins::discover_plugins(&config.paths.plugins_dir) {
            Ok(plugins) => {
                for plugin in plugins {
                    match plugin.discover_bundled_skills() {
                        Ok(discovered) => {
                            for skill in discovered {
                                if seen.contains(&skill.metadata.slug) {
                                    warnings.push(format!(
                                        "Duplicate skill slug `{}` discovered in plugin {}",
                                        skill.metadata.slug, plugin.manifest.name
                                    ));
                                }
                                skills.push(skill_record(
                                    "plugin",
                                    &plugin.manifest.name,
                                    &skill.metadata,
                                ));
                            }
                        }
                        Err(error) => warnings.push(format!(
                            "Failed to discover skills in plugin {}: {error}",
                            plugin.manifest.name
                        )),
                    }
                }
            }
            Err(error) => warnings.push(format!(
                "Failed to discover plugins in {}: {error}",
                config.paths.plugins_dir.display()
            )),
        }
    }

    skills.sort_by(|left, right| {
        left.slug
            .cmp(&right.slug)
            .then_with(|| left.origin_kind.cmp(&right.origin_kind))
            .then_with(|| left.origin_name.cmp(&right.origin_name))
    });

    SkillsListOutput { warnings, skills }
}

fn build_skill_show_output(
    config: &RuntimeConfig,
    slug: &str,
    include_plugins: bool,
    include_instructions: bool,
) -> Result<SkillShowOutput> {
    let mut warnings = Vec::new();
    let mut matches = Vec::new();

    if config.paths.skills_dir.exists() {
        match rc_skills::discover_skills(&config.paths.skills_dir) {
            Ok(discovered) => {
                matches.extend(discovered.into_iter().filter(|skill| skill.metadata.slug == slug).map(|skill| {
                    ("profile".to_owned(), "profile".to_owned(), skill)
                }));
            }
            Err(error) => warnings.push(format!("Failed to discover profile skills: {error}")),
        }
    }

    if include_plugins && config.paths.plugins_dir.exists() {
        match rc_plugins::discover_plugins(&config.paths.plugins_dir) {
            Ok(plugins) => {
                for plugin in plugins {
                    match plugin.discover_bundled_skills() {
                        Ok(discovered) => {
                            matches.extend(
                                discovered
                                    .into_iter()
                                    .filter(|skill| skill.metadata.slug == slug)
                                    .map(|skill| ("plugin".to_owned(), plugin.manifest.name.clone(), skill)),
                            );
                        }
                        Err(error) => warnings.push(format!(
                            "Failed to discover skills in plugin {}: {error}",
                            plugin.manifest.name
                        )),
                    }
                }
            }
            Err(error) => warnings.push(format!("Failed to discover plugins: {error}")),
        }
    }

    match matches.len() {
        0 => Err(anyhow!("No skill named `{slug}` was found")),
        1 => {
            let (origin_kind, origin_name, skill) = matches.pop().expect("single skill");
            Ok(SkillShowOutput {
                warnings,
                skill: skill_record(&origin_kind, &origin_name, &skill.metadata),
                instructions: include_instructions.then_some(skill.instructions),
            })
        }
        _ => Err(anyhow!(
            "Skill `{slug}` is ambiguous across: {}",
            matches
                .into_iter()
                .map(|(origin_kind, origin_name, skill)| format!(
                    "{}:{} ({})",
                    origin_kind,
                    origin_name,
                    skill.metadata.path.display()
                ))
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

fn skill_record(origin_kind: &str, origin_name: &str, metadata: &SkillMetadata) -> RuntimeSkillRecord {
    RuntimeSkillRecord {
        slug: metadata.slug.clone(),
        title: metadata.title.clone(),
        summary: metadata.summary.clone(),
        origin_kind: origin_kind.to_owned(),
        origin_name: origin_name.to_owned(),
        path: metadata.path.clone(),
        root: metadata.root.clone(),
        tools: metadata.tools.clone(),
        triggers: metadata.triggers.clone(),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use rc_config::{ProviderOverrides, RuntimeOverrides, load_runtime_config};
    use tempfile::tempdir;

    use super::{build_skill_show_output, build_skills_list_output};

    fn test_config() -> (tempfile::TempDir, rc_config::RuntimeConfig) {
        let tempdir = tempdir().expect("tempdir");
        let cwd = tempdir.path().join("workspace");
        let profile = tempdir.path().join(".remote-code-rust");
        fs::create_dir_all(cwd.join(".")).expect("cwd");
        fs::create_dir_all(profile.join("skills").join("demo")).expect("skills");
        fs::write(
            profile.join("skills").join("demo").join("SKILL.md"),
            "# Demo\n\nSummary.\n",
        )
        .expect("write skill");
        let config = load_runtime_config(
            Some(cwd),
            Some(profile),
            None,
            rc_core::PermissionMode::Default,
            rc_core::InputFormat::Text,
            rc_core::OutputFormat::Text,
            false,
            false,
            false,
            false,
            4,
            ProviderOverrides::default(),
            RuntimeOverrides::default(),
        )
        .expect("config");
        (tempdir, config)
    }

    #[test]
    fn skills_list_includes_profile_skill() {
        let (_tempdir, config) = test_config();
        let output = build_skills_list_output(&config, false);
        assert_eq!(output.skills.len(), 1);
        assert_eq!(output.skills[0].slug, "demo");
    }

    #[test]
    fn skills_show_returns_single_skill() {
        let (_tempdir, config) = test_config();
        let output = build_skill_show_output(&config, "demo", false, true).expect("skill show");
        assert_eq!(output.skill.slug, "demo");
        assert!(output.instructions.is_some());
    }
}
