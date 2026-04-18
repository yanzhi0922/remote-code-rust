//! Skills system integration tests.
//!
//! Validates the skill pipeline across rc-skills (discovery, metadata, search,
//! bundled registry, executor) and cross-crate integration with rc-tools
//! (BM25 skill discovery engine).

use std::fs;
use std::path::PathBuf;

// ─── Helpers ──────────────────────────────────────────────────────────────

fn ok<T, E: std::fmt::Display>(result: Result<T, E>) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("unexpected error: {error}"),
    }
}

// ─── Skill discovery from filesystem ──────────────────────────────────────

#[test]
fn discover_skills_finds_skill_md_files() {
    let temp = ok(tempfile::tempdir());
    let root = temp.path().join("demo-skill");
    ok(fs::create_dir_all(&root));
    ok(fs::write(
        root.join("SKILL.md"),
        "# Demo Skill\n\nA demonstration skill.\n",
    ));

    let skills = ok(rc_skills::discover_skills(temp.path()));
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].metadata.slug, "demo-skill");
    assert_eq!(skills[0].metadata.title, "Demo Skill");
}

#[test]
fn discover_skills_ignores_non_skill_files() {
    let temp = ok(tempfile::tempdir());
    let root = temp.path().join("notes");
    ok(fs::create_dir_all(&root));
    ok(fs::write(root.join("README.md"), "# Not a skill"));
    ok(fs::write(root.join("notes.txt"), "some notes"));

    let skills = ok(rc_skills::discover_skills(temp.path()));
    assert!(skills.is_empty());
}

#[test]
fn discover_skills_sorts_by_slug() {
    let temp = ok(tempfile::tempdir());
    for name in ["zeta-skill", "alpha-skill", "mid-skill"] {
        let dir = temp.path().join(name);
        ok(fs::create_dir_all(&dir));
        ok(fs::write(
            dir.join("SKILL.md"),
            format!("# {name}\n\nDesc.\n"),
        ));
    }

    let skills = ok(rc_skills::discover_skills(temp.path()));
    assert_eq!(skills.len(), 3);
    assert_eq!(skills[0].metadata.slug, "alpha-skill");
    assert_eq!(skills[1].metadata.slug, "mid-skill");
    assert_eq!(skills[2].metadata.slug, "zeta-skill");
}

#[test]
fn skill_with_front_matter_extracts_metadata() {
    let temp = ok(tempfile::tempdir());
    let root = temp.path().join("frontmatter-skill");
    ok(fs::create_dir_all(&root));
    ok(fs::write(
        root.join("SKILL.md"),
        "+++\nname = \"Custom Skill\"\nsummary = \"A custom skill\"\ntags = [\"alpha\"]\ntools = [\"shell\"]\ntriggers = [\"custom work\"]\n+++\n# Body\n\nBody text.\n",
    ));

    let skill = ok(rc_skills::load_skill(root.join("SKILL.md")));
    assert_eq!(skill.metadata.title, "Custom Skill");
    assert_eq!(skill.metadata.summary.as_deref(), Some("A custom skill"));
    assert_eq!(skill.metadata.tags, ["alpha"]);
    assert_eq!(skill.metadata.tools, ["shell"]);
    assert_eq!(skill.metadata.triggers, ["custom work"]);
}

// ─── BundledSkill registry ────────────────────────────────────────────────

#[test]
fn bundled_skills_count_is_15() {
    assert_eq!(rc_skills::bundled::bundled_skill_count(), 15);
}

#[test]
fn bundled_skills_all_have_names() {
    for skill in rc_skills::bundled::BundledSkill::all() {
        assert!(!skill.name().is_empty());
        assert!(!skill.description().is_empty());
    }
}

#[test]
fn bundled_skill_from_name_round_trip() {
    for skill in rc_skills::bundled::BundledSkill::all() {
        let name = skill.name();
        assert_eq!(
            rc_skills::bundled::BundledSkill::from_name(name),
            Some(*skill)
        );
    }
}

#[test]
fn bundled_skill_to_document_has_instructions() {
    let doc = rc_skills::bundled::BundledSkill::Commit.to_document();
    assert_eq!(doc.metadata.slug, "commit");
    assert!(!doc.instructions.is_empty());
    assert!(doc.metadata.tags.contains(&"bundled".to_owned()));
}

#[test]
fn is_bundled_recognizes_known_skills() {
    assert!(rc_skills::bundled::is_bundled("commit"));
    assert!(rc_skills::bundled::is_bundled("review"));
    assert!(!rc_skills::bundled::is_bundled("my-custom"));
}

#[test]
fn resolve_skill_finds_bundled() {
    let result = ok(rc_skills::bundled::resolve_skill("commit", &[]));
    assert!(result.is_some());
    assert_eq!(result.expect("skill").metadata.slug, "commit");
}

#[test]
fn resolve_skill_finds_user_defined() {
    let temp = ok(tempfile::tempdir());
    let root = temp.path().join("my-skill");
    ok(fs::create_dir_all(&root));
    ok(fs::write(root.join("SKILL.md"), "# My Skill\n\nCustom.\n"));

    let result = ok(rc_skills::bundled::resolve_skill(
        "my-skill",
        &[temp.path()],
    ));
    assert!(result.is_some());
    assert_eq!(result.expect("skill").metadata.slug, "my-skill");
}

#[test]
fn list_all_skills_includes_bundled() {
    let skills = ok(rc_skills::bundled::list_all_skills(&[]));
    assert!(skills.len() >= 15);
}

// ─── SkillSearchEngine BM25 search ────────────────────────────────────────

#[test]
fn search_engine_indexes_and_searches() {
    let mut engine = rc_skills::search::SkillSearchEngine::new();
    engine.index_skill(rc_skills::search::SkillDocument {
        slug: "rust-dev".to_owned(),
        title: "Rust Development".to_owned(),
        summary: "Build Rust applications".to_owned(),
        tags: vec!["rust".to_owned()],
        triggers: vec!["rust programming".to_owned()],
        tools: vec!["cargo".to_owned()],
        instructions: String::new(),
    });
    engine.index_skill(rc_skills::search::SkillDocument {
        slug: "python-dev".to_owned(),
        title: "Python Development".to_owned(),
        summary: "Build Python applications".to_owned(),
        tags: vec!["python".to_owned()],
        triggers: vec!["python programming".to_owned()],
        tools: vec!["pip".to_owned()],
        instructions: String::new(),
    });

    assert_eq!(engine.indexed_count(), 2);

    let results = engine.search("rust", 10);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].slug, "rust-dev");
}

#[test]
fn search_engine_disabled_returns_no_results() {
    let mut engine = rc_skills::search::SkillSearchEngine::disabled();
    engine.index_skill(rc_skills::search::SkillDocument::new("test", "Test"));
    assert!(!engine.is_enabled());
    assert_eq!(engine.search("test", 10).len(), 0);
}

#[test]
fn search_engine_prefetch_tracking() {
    let mut engine = rc_skills::search::SkillSearchEngine::new();
    engine.index_skill(rc_skills::search::SkillDocument::new("skill-a", "Skill A"));
    engine.index_skill(rc_skills::search::SkillDocument::new("skill-b", "Skill B"));

    let prefetch = engine.prefetch();
    assert_eq!(prefetch.total(), 2);
    assert_eq!(prefetch.pending_count(), 2);
}

#[test]
fn search_convenience_function() {
    let docs = [rc_skills::search::SkillDocument {
        slug: "deploy".to_owned(),
        title: "Deploy".to_owned(),
        summary: "Deploy to cloud".to_owned(),
        tags: vec!["deploy".to_owned()],
        triggers: vec![],
        tools: vec![],
        instructions: String::new(),
    }];
    let results = rc_skills::search::search_skills(&docs, "deploy", 5);
    assert_eq!(results.len(), 1);
}

// ─── SkillExecutor validation and prompt building ─────────────────────────

#[test]
fn executor_validates_valid_skill() {
    let temp = ok(tempfile::tempdir());
    let root = temp.path().join("valid-skill");
    ok(fs::create_dir_all(&root));
    ok(fs::write(
        root.join("SKILL.md"),
        "# Valid\n\nInstructions here.",
    ));

    let skill = rc_skills::SkillDocument {
        metadata: rc_skills::SkillMetadata {
            slug: "valid-skill".to_owned(),
            title: "Valid Skill".to_owned(),
            summary: None,
            path: root.join("SKILL.md"),
            root: root.clone(),
            tags: vec![],
            tools: vec![],
            triggers: vec![],
            references: vec![],
            scripts: vec![],
            assets: vec![],
        },
        instructions: "Do the thing.".to_owned(),
    };

    let executor = rc_skills::executor::SkillExecutor::new();
    ok(executor.validate_skill(&skill));
}

#[test]
fn executor_rejects_empty_slug() {
    let temp = ok(tempfile::tempdir());
    let root = temp.path().join("empty-slug");
    ok(fs::create_dir_all(&root));

    let skill = rc_skills::SkillDocument {
        metadata: rc_skills::SkillMetadata {
            slug: String::new(),
            title: "No Slug".to_owned(),
            summary: None,
            path: root.join("SKILL.md"),
            root: root.clone(),
            tags: vec![],
            tools: vec![],
            triggers: vec![],
            references: vec![],
            scripts: vec![],
            assets: vec![],
        },
        instructions: "Content".to_owned(),
    };

    let executor = rc_skills::executor::SkillExecutor::new();
    let err = executor
        .validate_skill(&skill)
        .expect_err("empty slug should fail validation");
    assert!(err.to_string().contains("slug"));
}

#[test]
fn executor_builds_prompt_with_references() {
    let temp = ok(tempfile::tempdir());
    let root = temp.path().join("ref-skill");
    let refs_dir = root.join("references");
    ok(fs::create_dir_all(&refs_dir));
    ok(fs::write(refs_dir.join("guide.md"), "This is the guide."));
    ok(fs::write(
        root.join("SKILL.md"),
        "# Ref Skill\n\nUse the guide.",
    ));

    let skill = rc_skills::SkillDocument {
        metadata: rc_skills::SkillMetadata {
            slug: "ref-skill".to_owned(),
            title: "Ref Skill".to_owned(),
            summary: None,
            path: root.join("SKILL.md"),
            root: root.clone(),
            tags: vec![],
            tools: vec![],
            triggers: vec![],
            references: vec![refs_dir.join("guide.md")],
            scripts: vec![],
            assets: vec![],
        },
        instructions: "Use the guide.".to_owned(),
    };

    let executor = rc_skills::executor::SkillExecutor::new();
    let ctx = rc_skills::executor::SkillExecutionContext::new(&root);
    let result = ok(executor.execute_skill(&skill, &ctx));
    assert!(result.valid);
    assert_eq!(result.references_loaded, 1);
    assert!(result.prompt.contains("guide.md"));
}

#[test]
fn executor_interpolates_env_vars() {
    let temp = ok(tempfile::tempdir());
    let root = temp.path().join("env-skill");
    ok(fs::create_dir_all(&root));
    ok(fs::write(
        root.join("SKILL.md"),
        "# Env Skill\n\nDeploy to {{ENV}}.\n",
    ));

    let skill = rc_skills::SkillDocument {
        metadata: rc_skills::SkillMetadata {
            slug: "env-skill".to_owned(),
            title: "Env Skill".to_owned(),
            summary: None,
            path: root.join("SKILL.md"),
            root: root.clone(),
            tags: vec![],
            tools: vec![],
            triggers: vec![],
            references: vec![],
            scripts: vec![],
            assets: vec![],
        },
        instructions: "Deploy to {{ENV}}.".to_owned(),
    };

    let executor = rc_skills::executor::SkillExecutor::new();
    let ctx = rc_skills::executor::SkillExecutionContext::new(&root).with_env("ENV", "production");
    let result = ok(executor.execute_skill(&skill, &ctx));
    assert_eq!(result.env_vars_interpolated, 1);
    assert!(result.prompt.contains("production"));
}

// ─── Cross-crate: rc-skills types → rc-tools BM25 engine ─────────────────

#[test]
fn rc_tools_bm25_engine_indexes_and_searches() {
    let mut engine = rc_tools::discover_skills::Bm25SkillSearchEngine::new();
    engine.add_skill(rc_tools::discover_skills::SkillMetadata {
        name: "deploy".to_owned(),
        description: "Deploy applications to cloud".to_owned(),
        triggers: vec!["deploy".to_owned(), "cloud".to_owned()],
        path: PathBuf::from("/skills/deploy"),
    });
    engine.add_skill(rc_tools::discover_skills::SkillMetadata {
        name: "test".to_owned(),
        description: "Run unit tests".to_owned(),
        triggers: vec!["test".to_owned(), "testing".to_owned()],
        path: PathBuf::from("/skills/test"),
    });

    let results = engine.search("deploy", 5);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].skill.name, "deploy");
    assert!(results[0].score > 0.0);
}

#[test]
fn rc_tools_bm25_search_returns_empty_for_no_match() {
    let mut engine = rc_tools::discover_skills::Bm25SkillSearchEngine::new();
    engine.add_skill(rc_tools::discover_skills::SkillMetadata {
        name: "deploy".to_owned(),
        description: "Deploy".to_owned(),
        triggers: vec![],
        path: PathBuf::from("/skills/deploy"),
    });

    let results = engine.search("nonexistent", 5);
    assert!(results.is_empty());
}

// ─── SkillMetadata serialization ─────────────────────────────────────────

#[test]
fn skill_metadata_serialization_round_trip() {
    let meta = rc_skills::SkillMetadata {
        slug: "test-meta".to_owned(),
        title: "Test Meta".to_owned(),
        summary: Some("A test".to_owned()),
        path: PathBuf::from("/skills/test"),
        root: PathBuf::from("/skills/test"),
        tags: vec!["test".to_owned()],
        tools: vec!["shell".to_owned()],
        triggers: vec!["test".to_owned()],
        references: vec![],
        scripts: vec![],
        assets: vec![],
    };
    let json = serde_json::to_string(&meta).expect("serialize");
    let decoded: rc_skills::SkillMetadata = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(decoded.slug, "test-meta");
    assert_eq!(decoded.tags, ["test"]);
}

// ─── SkillLockFile parsing ───────────────────────────────────────────────

#[test]
fn skill_lock_file_parsing() {
    let temp = ok(tempfile::tempdir());
    let lock_path = temp.path().join(".skill-lock.json");
    ok(fs::write(
        &lock_path,
        r#"{
            "version": 3,
            "skills": {
                "demo": {
                    "source": "example/demo",
                    "sourceType": "github",
                    "sourceUrl": "https://example.com/demo.git",
                    "skillPath": "skills/demo/SKILL.md",
                    "skillFolderHash": "abc123",
                    "installedAt": "2026-04-07T00:00:00Z",
                    "updatedAt": "2026-04-07T01:00:00Z"
                }
            }
        }"#,
    ));

    let lock_file = ok(rc_skills::load_skill_lock_file(lock_path));
    assert_eq!(lock_file.version, 3);
    assert!(lock_file.skills.contains_key("demo"));
    let record = lock_file.skills.get("demo").expect("record");
    assert_eq!(record.source_type, rc_skills::SkillSourceKind::Github);
}

#[test]
fn filter_skills_by_query() {
    let temp = ok(tempfile::tempdir());
    let root = temp.path().join("filter-skill");
    ok(fs::create_dir_all(&root));
    ok(fs::write(
        root.join("SKILL.md"),
        "# Deploy Skill\n\nDeploy apps.",
    ));

    let skill = ok(rc_skills::load_skill(root.join("SKILL.md")));
    let skills = [skill];
    let filtered = rc_skills::executor::filter_skills_by_query(&skills, "deploy");
    assert_eq!(filtered.len(), 1);

    let empty = rc_skills::executor::filter_skills_by_query(&skills, "nonexistent");
    assert!(empty.is_empty());
}
