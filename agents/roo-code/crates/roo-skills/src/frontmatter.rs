//! Simplified YAML frontmatter parsing for SKILL.md files.
//!
//! Instead of using a full YAML parser, we use simple string splitting
//! to extract the frontmatter between `---` delimiters and parse
//! key-value pairs line by line.

use crate::types::SkillSource;

// ---------------------------------------------------------------------------
// FrontMatter
// ---------------------------------------------------------------------------

/// Parsed frontmatter fields from a SKILL.md file.
#[derive(Debug, Clone, Default)]
pub struct FrontMatter {
    pub name: Option<String>,
    pub description: Option<String>,
    pub mode: Option<String>,
    pub mode_slugs: Option<Vec<String>>,
}

// ---------------------------------------------------------------------------
// parse_skill_md
// ---------------------------------------------------------------------------

/// Parse a SKILL.md file content into frontmatter and body.
///
/// Returns `None` if the content does not contain a valid frontmatter block
/// (i.e., doesn't start with `---`).
///
/// # Format
///
/// ```markdown
/// ---
/// name: my-skill
/// description: A description
/// modeSlugs:
///   - code
///   - architect
/// ---
/// Skill instructions go here.
/// ```
pub fn parse_skill_md(content: &str) -> Option<(FrontMatter, String)> {
    let trimmed = content.trim_start();

    // Must start with ---
    if !trimmed.starts_with("---") {
        return None;
    }

    // Find the closing ---
    // Skip the opening ---
    let after_opening = &trimmed[3..];
    let rest = after_opening.trim_start_matches(['\r', '\n']);

    // Find the closing ---
    let closing_pos = rest.find("\n---").or_else(|| {
        // Handle case where closing --- is at the end without trailing newline
        if rest.ends_with("---") && rest.len() > 3 {
            Some(rest.len() - 3)
        } else {
            None
        }
    });

    let (frontmatter_str, body) = match closing_pos {
        Some(pos) => {
            let fm = &rest[..pos];
            // Body starts after the closing --- and any whitespace
            let after_closing = &rest[pos + 4..]; // skip \n---
            let body = after_closing.trim_start_matches(['\r', '\n']);
            (fm, body.to_string())
        }
        None => return None,
    };

    let frontmatter = parse_frontmatter_str(frontmatter_str);

    Some((frontmatter, body))
}

/// Parse a frontmatter string into a [`FrontMatter`] struct.
fn parse_frontmatter_str(s: &str) -> FrontMatter {
    let mut fm = FrontMatter::default();
    let mut in_mode_slugs = false;

    for line in s.lines() {
        let trimmed_line = line.trim();

        if trimmed_line.is_empty() {
            in_mode_slugs = false;
            continue;
        }

        // Handle YAML list items for modeSlugs
        if in_mode_slugs {
            if let Some(value) = trimmed_line.strip_prefix("- ") {
                if let Some(ref mut slugs) = fm.mode_slugs {
                    slugs.push(value.trim().to_string());
                }
                continue;
            } else {
                in_mode_slugs = false;
            }
        }

        // Handle key: value pairs
        if let Some((key, value)) = trimmed_line.split_once(':') {
            let key = key.trim();
            let value = value.trim();

            match key {
                "name" => {
                    fm.name = Some(value.to_string());
                }
                "description" => {
                    fm.description = Some(value.to_string());
                }
                "mode" => {
                    fm.mode = Some(value.to_string());
                }
                "modeSlugs" | "mode_slugs" => {
                    // modeSlugs might be on the same line as an empty value,
                    // or just the key with values on subsequent lines
                    if value.is_empty() {
                        fm.mode_slugs = Some(Vec::new());
                        in_mode_slugs = true;
                    } else {
                        // Could be a single value or comma-separated
                        fm.mode_slugs = Some(
                            value
                                .split(',')
                                .map(|s| s.trim().to_string())
                                .filter(|s| !s.is_empty())
                                .collect(),
                        );
                    }
                }
                _ => {
                    // Ignore unknown keys
                }
            }
        }
    }

    fm
}

// ---------------------------------------------------------------------------
// generate_skill_md
// ---------------------------------------------------------------------------

/// Generate a SKILL.md file content from the given fields.
///
/// Creates a file with YAML frontmatter containing name, description,
/// and optional modeSlugs, followed by the instructions body.
pub fn generate_skill_md(
    name: &str,
    description: &str,
    mode_slugs: Option<&[String]>,
    instructions: &str,
) -> String {
    let mut content = String::from("---\n");
    content.push_str(&format!("name: {}\n", name));
    content.push_str(&format!("description: {}\n", description));

    if let Some(slugs) = mode_slugs {
        if !slugs.is_empty() {
            content.push_str("modeSlugs:\n");
            for slug in slugs {
                content.push_str(&format!("  - {}\n", slug));
            }
        }
    }

    content.push_str("---\n");

    if !instructions.is_empty() {
        content.push_str(instructions);
    }

    content
}

// ---------------------------------------------------------------------------
// generate_new_skill_md
// ---------------------------------------------------------------------------

/// Generate SKILL.md content for a new skill being created.
///
/// This generates a minimal SKILL.md with just the frontmatter and a placeholder body.
pub fn generate_new_skill_md(
    name: &str,
    description: &str,
    mode_slugs: Option<&[String]>,
) -> String {
    generate_skill_md(name, description, mode_slugs, "")
}

// ---------------------------------------------------------------------------
// get_skill_dir_path
// ---------------------------------------------------------------------------

/// Build the filesystem path for a skill directory.
///
/// Skills are stored under `<base>/<source>/[<mode>/]skills/<name>/`.
pub fn get_skill_dir_path(
    base_dir: &str,
    source: SkillSource,
    mode: Option<&str>,
    name: &str,
) -> std::path::PathBuf {
    let _ = source; // Both Global and Project use .roo

    let mut path = std::path::PathBuf::from(base_dir);
    path.push(".roo");

    if let Some(mode_slug) = mode {
        path.push(format!("skills-{}", mode_slug));
    } else {
        path.push("skills");
    }

    path.push(name);
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_skill_md() {
        let content = "\
---
name: my-skill
description: A test skill
---
Some instructions here.";

        let result = parse_skill_md(content);
        assert!(result.is_some());

        let (fm, body) = result.unwrap();
        assert_eq!(fm.name, Some("my-skill".to_string()));
        assert_eq!(fm.description, Some("A test skill".to_string()));
        assert_eq!(body, "Some instructions here.");
    }

    #[test]
    fn test_parse_skill_md_with_mode_slugs() {
        let content = "\
---
name: test-skill
description: Test
modeSlugs:
  - code
  - architect
---
Instructions.";

        let result = parse_skill_md(content);
        assert!(result.is_some());

        let (fm, body) = result.unwrap();
        assert_eq!(fm.name, Some("test-skill".to_string()));
        assert_eq!(fm.mode_slugs, Some(vec!["code".to_string(), "architect".to_string()]));
        assert_eq!(body, "Instructions.");
    }

    #[test]
    fn test_parse_skill_md_with_deprecated_mode() {
        let content = "\
---
name: old-skill
description: Old
mode: code
---
Old instructions.";

        let (fm, _) = parse_skill_md(content).unwrap();
        assert_eq!(fm.mode, Some("code".to_string()));
    }

    #[test]
    fn test_parse_skill_md_no_frontmatter() {
        let content = "Just some text without frontmatter.";
        assert!(parse_skill_md(content).is_none());
    }

    #[test]
    fn test_parse_skill_md_empty_body() {
        let content = "\
---
name: empty
description: Empty body
---";

        let result = parse_skill_md(content);
        assert!(result.is_some());

        let (_, body) = result.unwrap();
        assert_eq!(body, "");
    }

    #[test]
    fn test_generate_skill_md_basic() {
        let result = generate_skill_md("my-skill", "A test skill", None, "Do something.");
        assert!(result.starts_with("---\n"));
        assert!(result.contains("name: my-skill"));
        assert!(result.contains("description: A test skill"));
        assert!(result.contains("Do something."));
        assert!(result.contains("---\n"));
    }

    #[test]
    fn test_generate_skill_md_with_mode_slugs() {
        let slugs = vec!["code".to_string(), "architect".to_string()];
        let result = generate_skill_md("test", "Test", Some(&slugs), "Instructions");
        assert!(result.contains("modeSlugs:\n"));
        assert!(result.contains("  - code\n"));
        assert!(result.contains("  - architect\n"));
    }

    #[test]
    fn test_generate_skill_md_roundtrip() {
        let slugs = vec!["code".to_string()];
        let original = generate_skill_md("my-skill", "A test", Some(&slugs), "Do stuff.");

        let (fm, body) = parse_skill_md(&original).unwrap();
        assert_eq!(fm.name, Some("my-skill".to_string()));
        assert_eq!(fm.description, Some("A test".to_string()));
        assert_eq!(fm.mode_slugs, Some(slugs));
        assert_eq!(body, "Do stuff.");
    }

    #[test]
    fn test_get_skill_dir_path_global() {
        let path = get_skill_dir_path("/home/user", SkillSource::Global, None, "my-skill");
        // Platform-agnostic assertions
        assert!(path.ends_with("my-skill"));
        assert!(path.to_string_lossy().contains(".roo"));
        assert!(path.to_string_lossy().contains("skills"));
        assert!(!path.to_string_lossy().contains("skills-"));
    }

    #[test]
    fn test_get_skill_dir_path_project_with_mode() {
        let path = get_skill_dir_path("/project", SkillSource::Project, Some("code"), "my-skill");
        assert!(path.ends_with("my-skill"));
        assert!(path.to_string_lossy().contains(".roo"));
        assert!(path.to_string_lossy().contains("skills-code"));
    }
}
