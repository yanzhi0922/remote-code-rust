//! Skills section.
//!
//! Source: `src/core/prompts/sections/skills.ts`

use crate::types::SkillInfo;

/// Escapes special XML characters in a string.
fn escape_xml(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => {
                result.push('&');
                result.push_str("amp;");
            }
            '<' => {
                result.push('&');
                result.push_str("lt;");
            }
            '>' => {
                result.push('&');
                result.push_str("gt;");
            }
            '"' => {
                result.push('&');
                result.push_str("quot;");
            }
            '\'' => {
                result.push('&');
                result.push_str("apos;");
            }
            _ => result.push(ch),
        }
    }
    result
}

/// Generate the skills section for the system prompt.
/// Only includes skills relevant to the current mode.
///
/// Source: `src/core/prompts/sections/skills.ts` — `getSkillsSection`
pub fn get_skills_section(skills: &[SkillInfo], current_mode: &str) -> String {
    if skills.is_empty() || current_mode.is_empty() {
        return String::new();
    }

    let skills_xml = skills
        .iter()
        .map(|skill| {
            let name = escape_xml(&skill.name);
            let description = escape_xml(&skill.description);
            let location_line = format!("\n    <location>{}</location>", escape_xml(&skill.path));
            format!(
                "  <skill>\n    <name>{name}</name>\n    <description>{description}</description>{location_line}\n  </skill>"
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"====

AVAILABLE SKILLS

<available_skills>
{skills_xml}
</available_skills>

<mandatory_skill_check>
REQUIRED PRECONDITION

Before producing ANY user-facing response, you MUST perform a skill applicability check.

Step 1: Skill Evaluation
- Evaluate the user's request against ALL available skill <description> entries in <available_skills>.
- Determine whether at least one skill clearly and unambiguously applies.

Step 2: Branching Decision

<if_skill_applies>
- Select EXACTLY ONE skill.
- Prefer the most specific skill when multiple skills match.
- Use the skill tool to load the skill by name.
- Load the skill's instructions fully into context BEFORE continuing.
- Follow the skill instructions precisely.
- Do NOT respond outside the skill-defined flow.
</if_skill_applies>

<if_no_skill_applies>
- Proceed with a normal response.
- Do NOT load any SKILL.md files.
</if_no_skill_applies>

CONSTRAINTS:
- Do NOT load every skill up front.
- Load skills ONLY after a skill is selected.
- Do NOT reload a skill whose instructions already appear in this conversation.
- Do NOT skip this check.
- FAILURE to perform this check is an error.
</mandatory_skill_check>

<linked_file_handling>
- When a skill is loaded, ONLY the skill instructions are present.
- Files linked from the skill are NOT loaded automatically.
- The model MUST explicitly decide to read a linked file based on task relevance.
- Do NOT assume the contents of linked files unless they have been explicitly read.
- Prefer reading the minimum necessary linked file.
- Avoid reading multiple linked files unless required.
- Treat linked files as progressive disclosure, not mandatory context.
</linked_file_handling>

<context_notes>
- The skill list is already filtered for the current mode: "{current_mode}".
- Mode-specific skills may come from skills-{current_mode}/ with project-level overrides taking precedence over global skills.
</context_notes>

<internal_verification>
This section is for internal control only.
Do NOT include this section in user-facing output.

After completing the evaluation, internally confirm:
<skill_check_completed>true|false</skill_check_completed>
</internal_verification>
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn amp_entity() -> String {
        let mut s = String::new();
        s.push('&');
        s.push_str("amp;");
        s
    }

    fn lt_entity() -> String {
        let mut s = String::new();
        s.push('&');
        s.push_str("lt;");
        s
    }

    fn gt_entity() -> String {
        let mut s = String::new();
        s.push('&');
        s.push_str("gt;");
        s
    }

    fn quot_entity() -> String {
        let mut s = String::new();
        s.push('&');
        s.push_str("quot;");
        s
    }

    fn apos_entity() -> String {
        let mut s = String::new();
        s.push('&');
        s.push_str("apos;");
        s
    }

    #[test]
    fn test_escape_xml() {
        assert_eq!(escape_xml("hello"), "hello");
        assert_eq!(escape_xml("a&b"), format!("a{}b", amp_entity()));
        assert_eq!(escape_xml("a<b"), format!("a{}b", lt_entity()));
        assert_eq!(escape_xml("a>b"), format!("a{}b", gt_entity()));
        assert_eq!(escape_xml("a\"b"), format!("a{}b", quot_entity()));
        assert_eq!(escape_xml("a'b"), format!("a{}b", apos_entity()));
    }

    #[test]
    fn test_get_skills_section_empty() {
        let result = get_skills_section(&[], "code");
        assert!(result.is_empty());
    }

    #[test]
    fn test_get_skills_section_with_skills() {
        let skills = vec![SkillInfo {
            name: "my-skill".to_string(),
            description: "A great skill".to_string(),
            path: "/path/to/skill.md".to_string(),
        }];
        let result = get_skills_section(&skills, "code");
        assert!(result.contains("AVAILABLE SKILLS"));
        assert!(result.contains("my-skill"));
        assert!(result.contains("A great skill"));
        assert!(result.contains("mandatory_skill_check"));
    }
}
