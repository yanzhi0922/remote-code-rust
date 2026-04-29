//! Main environment details builder.
//!
//! Assembles all sections into the final `<environment_details>` XML string.
//! Ported from `src/core/environment/getEnvironmentDetails.ts`.

use crate::reminder::format_reminder_section;
use crate::terminal::{format_active_terminals, format_inactive_terminals};
use crate::time::format_current_time;
use crate::types::EnvironmentInput;

/// Build the complete `<environment_details>` XML string.
///
/// This is a pure function — all data is provided via `input`.
/// The output is wrapped in `<environment_details>...</environment_details>` tags.
pub fn build_environment_details(input: &EnvironmentInput) -> String {
    let mut details = String::new();

    // 1. VSCode Visible Files
    if !input.visible_files.is_empty() {
        details.push_str("\n\n# VSCode Visible Files");
        details.push_str(&format!("\n{}", input.visible_files.join("\n")));
    }

    // 2. VSCode Open Tabs
    if !input.open_tabs.is_empty() {
        details.push_str("\n\n# VSCode Open Tabs");
        details.push_str(&format!("\n{}", input.open_tabs.join("\n")));
    }

    // 3. Recently Modified Files
    if !input.recently_modified_files.is_empty() {
        details.push_str(
            "\n\n# Recently Modified Files\nThese files have been modified since you last accessed them (file was just edited so you may need to re-read it before editing):",
        );
        for file_path in &input.recently_modified_files {
            details.push_str(&format!("\n{}", file_path));
        }
    }

    // 4. Terminal details (active + inactive)
    let terminal_details = build_terminal_details(input);
    if !terminal_details.is_empty() {
        details.push_str(&terminal_details);
    }

    // 5. Current Time
    if input.settings.include_current_time {
        details.push_str(&format_current_time());
    }

    // 6. Git Status
    if input.settings.max_git_status_files > 0 {
        if let Some(ref git_status) = input.git_status {
            if !git_status.is_empty() {
                details.push_str(&format!("\n\n# Git Status\n{}", git_status));
            }
        }
    }

    // 7. Current Cost
    if input.settings.include_current_cost {
        let cost_str = match input.total_cost {
            Some(cost) => format!("${:.2}", cost),
            None => "(Not available)".to_string(),
        };
        details.push_str(&format!("\n\n# Current Cost\n{}", cost_str));
    }

    // 8. Current Mode
    details.push_str("\n\n# Current Mode\n");
    details.push_str(&format!("<slug>{}</slug>\n", input.mode_info.slug));
    details.push_str(&format!("<name>{}</name>\n", input.mode_info.name));
    details.push_str(&format!("<model>{}</model>\n", input.mode_info.model_id));

    // 9. Workspace Files (only when workspace_files is Some and not desktop)
    if let Some(ref workspace_files) = input.workspace_files {
        details.push_str(&format!(
            "\n\n# Current Workspace Directory ({}) Files\n",
            input.cwd
        ));

        if input.is_desktop {
            details.push_str(
                "(Desktop files not shown automatically. Use list_files to explore if needed.)",
            );
        } else {
            let max_files = input.settings.max_workspace_files;

            if max_files == 0 {
                details.push_str(
                    "(Workspace files context disabled. Use list_files to explore if needed.)",
                );
            } else {
                let files_list = workspace_files.files.join("\n");
                details.push_str(&files_list);

                if workspace_files.did_hit_limit {
                    details.push_str("\n(File list truncated. Use list_files to see all files.)");
                }
            }
        }
    }

    // 10. Reminder section
    let reminder_section = if input.settings.todo_list_enabled {
        format_reminder_section(input.todo_list.as_deref())
    } else {
        String::new()
    };

    format!(
        "<environment_details>\n{}\n{}\n</environment_details>",
        details.trim(),
        reminder_section
    )
}

/// Build the terminal details section (active + inactive).
fn build_terminal_details(input: &EnvironmentInput) -> String {
    let mut result = String::new();

    let active = format_active_terminals(&input.active_terminals);
    if !active.is_empty() {
        result.push_str(&active);
    }

    let inactive = format_inactive_terminals(&input.inactive_terminals);
    if !inactive.is_empty() {
        result.push_str(&inactive);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;

    /// Helper to create a minimal valid `EnvironmentInput` with defaults.
    fn default_input() -> EnvironmentInput {
        EnvironmentInput {
            cwd: "/test/path".to_string(),
            visible_files: vec![],
            open_tabs: vec![],
            active_terminals: vec![],
            inactive_terminals: vec![],
            recently_modified_files: vec![],
            git_status: None,
            total_cost: Some(0.25),
            mode_info: ModeDisplayInfo {
                slug: "code".to_string(),
                name: "💻 Code".to_string(),
                model_id: "test-model".to_string(),
            },
            settings: EnvironmentSettings::default(),
            todo_list: None,
            workspace_files: None,
            is_desktop: false,
        }
    }

    // ---- Basic structure ----

    #[test]
    fn test_basic_environment_details() {
        let input = default_input();
        let result = build_environment_details(&input);

        assert!(result.starts_with("<environment_details>"));
        assert!(result.ends_with("</environment_details>"));
        assert!(result.contains("# Current Time"));
        assert!(result.contains("# Current Cost"));
        assert!(result.contains("# Current Mode"));
        assert!(result.contains("<slug>code</slug>"));
        assert!(result.contains("<name>💻 Code</name>"));
        assert!(result.contains("<model>test-model</model>"));
    }

    #[test]
    fn test_xml_wrapping() {
        let input = default_input();
        let result = build_environment_details(&input);
        assert!(result.starts_with("<environment_details>\n"));
        assert!(result.ends_with("\n</environment_details>"));
    }

    // ---- Visible Files ----

    #[test]
    fn test_visible_files_section() {
        let mut input = default_input();
        input.visible_files = vec!["src/main.rs".to_string(), "Cargo.toml".to_string()];
        let result = build_environment_details(&input);
        assert!(result.contains("# VSCode Visible Files"));
        assert!(result.contains("src/main.rs"));
        assert!(result.contains("Cargo.toml"));
    }

    #[test]
    fn test_no_visible_files_section_when_empty() {
        let input = default_input();
        let result = build_environment_details(&input);
        assert!(!result.contains("# VSCode Visible Files"));
    }

    // ---- Open Tabs ----

    #[test]
    fn test_open_tabs_section() {
        let mut input = default_input();
        input.open_tabs = vec!["lib.rs".to_string(), "mod.rs".to_string()];
        let result = build_environment_details(&input);
        assert!(result.contains("# VSCode Open Tabs"));
        assert!(result.contains("lib.rs"));
        assert!(result.contains("mod.rs"));
    }

    #[test]
    fn test_no_open_tabs_section_when_empty() {
        let input = default_input();
        let result = build_environment_details(&input);
        assert!(!result.contains("# VSCode Open Tabs"));
    }

    // ---- Recently Modified Files ----

    #[test]
    fn test_recently_modified_files() {
        let mut input = default_input();
        input.recently_modified_files = vec![
            "modified1.ts".to_string(),
            "modified2.ts".to_string(),
        ];
        let result = build_environment_details(&input);
        assert!(result.contains("# Recently Modified Files"));
        assert!(result.contains("modified1.ts"));
        assert!(result.contains("modified2.ts"));
        assert!(result.contains("file was just edited so you may need to re-read it before editing"));
    }

    #[test]
    fn test_no_recently_modified_when_empty() {
        let input = default_input();
        let result = build_environment_details(&input);
        assert!(!result.contains("# Recently Modified Files"));
    }

    // ---- Active Terminals ----

    #[test]
    fn test_active_terminal_info() {
        let mut input = default_input();
        input.active_terminals = vec![TerminalInfo {
            id: "terminal-1".to_string(),
            working_directory: "/test/path/src".to_string(),
            last_command: "npm test".to_string(),
            new_output: Some("Test output".to_string()),
        }];
        let result = build_environment_details(&input);
        assert!(result.contains("# Actively Running Terminals"));
        assert!(result.contains("## Terminal terminal-1 (Active)"));
        assert!(result.contains("### Working Directory: `/test/path/src`"));
        assert!(result.contains("### Original command: `npm test`"));
        assert!(result.contains("Test output"));
    }

    // ---- Inactive Terminals ----

    #[test]
    fn test_inactive_terminal_with_output() {
        let mut input = default_input();
        input.inactive_terminals = vec![InactiveTerminalInfo {
            id: "terminal-2".to_string(),
            working_directory: "/test/path/build".to_string(),
            completed_processes: vec![CompletedProcess {
                command: "npm build".to_string(),
                output: "Build output".to_string(),
            }],
        }];
        let result = build_environment_details(&input);
        assert!(result.contains("# Inactive Terminals with Completed Process Output"));
        assert!(result.contains("## Terminal terminal-2 (Inactive)"));
        assert!(result.contains("### Working Directory: `/test/path/build`"));
        assert!(result.contains("Command: `npm build`"));
        assert!(result.contains("Build output"));
    }

    // ---- Time ----

    #[test]
    fn test_time_disabled() {
        let mut input = default_input();
        input.settings.include_current_time = false;
        let result = build_environment_details(&input);
        assert!(!result.contains("# Current Time"));
    }

    #[test]
    fn test_time_enabled() {
        let input = default_input();
        let result = build_environment_details(&input);
        assert!(result.contains("# Current Time"));
        assert!(result.contains("Current time in ISO 8601 UTC format:"));
        assert!(result.contains("User time zone:"));
    }

    // ---- Git Status ----

    #[test]
    fn test_git_status_disabled() {
        let mut input = default_input();
        input.settings.max_git_status_files = 0;
        input.git_status = Some("## main".to_string());
        let result = build_environment_details(&input);
        assert!(!result.contains("# Git Status"));
    }

    #[test]
    fn test_git_status_enabled_with_status() {
        let mut input = default_input();
        input.settings.max_git_status_files = 10;
        input.git_status = Some("## main\nM  file1.ts".to_string());
        let result = build_environment_details(&input);
        assert!(result.contains("# Git Status"));
        assert!(result.contains("## main"));
        assert!(result.contains("M  file1.ts"));
    }

    #[test]
    fn test_git_status_enabled_but_none() {
        let mut input = default_input();
        input.settings.max_git_status_files = 10;
        input.git_status = None;
        let result = build_environment_details(&input);
        assert!(!result.contains("# Git Status"));
    }

    #[test]
    fn test_git_status_enabled_but_empty() {
        let mut input = default_input();
        input.settings.max_git_status_files = 10;
        input.git_status = Some(String::new());
        let result = build_environment_details(&input);
        assert!(!result.contains("# Git Status"));
    }

    // ---- Cost ----

    #[test]
    fn test_cost_enabled() {
        let mut input = default_input();
        input.total_cost = Some(1.50);
        let result = build_environment_details(&input);
        assert!(result.contains("# Current Cost"));
        assert!(result.contains("$1.50"));
    }

    #[test]
    fn test_cost_none() {
        let mut input = default_input();
        input.total_cost = None;
        let result = build_environment_details(&input);
        assert!(result.contains("# Current Cost"));
        assert!(result.contains("(Not available)"));
    }

    #[test]
    fn test_cost_disabled() {
        let mut input = default_input();
        input.settings.include_current_cost = false;
        let result = build_environment_details(&input);
        assert!(!result.contains("# Current Cost"));
    }

    // ---- Mode ----

    #[test]
    fn test_mode_info() {
        let input = default_input();
        let result = build_environment_details(&input);
        assert!(result.contains("<slug>code</slug>"));
        assert!(result.contains("<name>💻 Code</name>"));
        assert!(result.contains("<model>test-model</model>"));
    }

    // ---- Workspace Files ----

    #[test]
    fn test_workspace_files_included() {
        let mut input = default_input();
        input.workspace_files = Some(WorkspaceFilesInfo {
            files: vec!["file1.ts".to_string(), "file2.ts".to_string()],
            did_hit_limit: false,
        });
        let result = build_environment_details(&input);
        assert!(result.contains("# Current Workspace Directory"));
        assert!(result.contains("file1.ts"));
        assert!(result.contains("file2.ts"));
    }

    #[test]
    fn test_workspace_files_not_included_when_none() {
        let input = default_input();
        let result = build_environment_details(&input);
        assert!(!result.contains("# Current Workspace Directory"));
    }

    #[test]
    fn test_desktop_directory_special() {
        let mut input = default_input();
        input.is_desktop = true;
        input.workspace_files = Some(WorkspaceFilesInfo {
            files: vec![],
            did_hit_limit: false,
        });
        let result = build_environment_details(&input);
        assert!(result.contains("Desktop files not shown automatically"));
    }

    #[test]
    fn test_max_workspace_files_zero() {
        let mut input = default_input();
        input.settings.max_workspace_files = 0;
        input.workspace_files = Some(WorkspaceFilesInfo {
            files: vec![],
            did_hit_limit: false,
        });
        let result = build_environment_details(&input);
        assert!(result.contains("Workspace files context disabled"));
    }

    #[test]
    fn test_workspace_files_hit_limit() {
        let mut input = default_input();
        input.workspace_files = Some(WorkspaceFilesInfo {
            files: vec!["a.rs".to_string()],
            did_hit_limit: true,
        });
        let result = build_environment_details(&input);
        assert!(result.contains("File list truncated"));
    }

    // ---- Todo / Reminders ----

    #[test]
    fn test_todo_list_enabled_with_items() {
        let mut input = default_input();
        input.todo_list = Some(vec![TodoItemInput {
            content: "test".to_string(),
            status: "pending".to_string(),
        }]);
        let result = build_environment_details(&input);
        assert!(result.contains("REMINDERS"));
    }

    #[test]
    fn test_todo_list_disabled() {
        let mut input = default_input();
        input.settings.todo_list_enabled = false;
        input.todo_list = Some(vec![TodoItemInput {
            content: "test".to_string(),
            status: "pending".to_string(),
        }]);
        let result = build_environment_details(&input);
        assert!(!result.contains("REMINDERS"));
    }

    #[test]
    fn test_todo_list_enabled_but_empty() {
        let mut input = default_input();
        input.todo_list = Some(vec![]);
        let result = build_environment_details(&input);
        // Empty list shows the creation prompt, not "REMINDERS"
        assert!(result.contains("You have not created a todo list yet"));
    }

    #[test]
    fn test_todo_list_none_shows_prompt() {
        let input = default_input();
        let result = build_environment_details(&input);
        assert!(result.contains("You have not created a todo list yet"));
    }

    // ---- Full integration ----

    #[test]
    fn test_full_integration_all_sections() {
        let input = EnvironmentInput {
            cwd: "/project".to_string(),
            visible_files: vec!["src/main.rs".to_string()],
            open_tabs: vec!["Cargo.toml".to_string()],
            active_terminals: vec![TerminalInfo {
                id: "t1".to_string(),
                working_directory: "/project".to_string(),
                last_command: "cargo build".to_string(),
                new_output: Some("Compiling...".to_string()),
            }],
            inactive_terminals: vec![InactiveTerminalInfo {
                id: "t2".to_string(),
                working_directory: "/project".to_string(),
                completed_processes: vec![CompletedProcess {
                    command: "cargo test".to_string(),
                    output: "All tests passed".to_string(),
                }],
            }],
            recently_modified_files: vec!["src/lib.rs".to_string()],
            git_status: Some("## main\nM src/lib.rs".to_string()),
            total_cost: Some(0.42),
            mode_info: ModeDisplayInfo {
                slug: "architect".to_string(),
                name: "🏗️ Architect".to_string(),
                model_id: "claude-3".to_string(),
            },
            settings: EnvironmentSettings {
                include_current_time: true,
                include_current_cost: true,
                max_git_status_files: 10,
                todo_list_enabled: true,
                max_workspace_files: 200,
                max_open_tabs: 20,
            },
            todo_list: Some(vec![
                TodoItemInput {
                    content: "Design API".to_string(),
                    status: "completed".to_string(),
                },
                TodoItemInput {
                    content: "Implement".to_string(),
                    status: "in_progress".to_string(),
                },
            ]),
            workspace_files: Some(WorkspaceFilesInfo {
                files: vec!["Cargo.toml".to_string(), "src/main.rs".to_string()],
                did_hit_limit: false,
            }),
            is_desktop: false,
        };

        let result = build_environment_details(&input);

        // Verify all sections
        assert!(result.contains("<environment_details>"));
        assert!(result.contains("# VSCode Visible Files"));
        assert!(result.contains("# VSCode Open Tabs"));
        assert!(result.contains("# Recently Modified Files"));
        assert!(result.contains("# Actively Running Terminals"));
        assert!(result.contains("# Inactive Terminals with Completed Process Output"));
        assert!(result.contains("# Current Time"));
        assert!(result.contains("# Git Status"));
        assert!(result.contains("# Current Cost"));
        assert!(result.contains("$0.42"));
        assert!(result.contains("# Current Mode"));
        assert!(result.contains("<slug>architect</slug>"));
        assert!(result.contains("<name>🏗️ Architect</name>"));
        assert!(result.contains("<model>claude-3</model>"));
        assert!(result.contains("# Current Workspace Directory"));
        assert!(result.contains("REMINDERS"));
        assert!(result.contains("| 1 | Design API | Completed |"));
        assert!(result.contains("| 2 | Implement | In Progress |"));
        assert!(result.contains("</environment_details>"));
    }

    #[test]
    fn test_minimal_details_no_optional_sections() {
        let mut input = default_input();
        input.settings.include_current_time = false;
        input.settings.include_current_cost = false;
        input.settings.max_git_status_files = 0;
        input.settings.todo_list_enabled = false;
        let result = build_environment_details(&input);

        assert!(result.contains("<environment_details>"));
        assert!(result.contains("# Current Mode"));
        assert!(!result.contains("# Current Time"));
        assert!(!result.contains("# Current Cost"));
        assert!(!result.contains("# Git Status"));
        assert!(!result.contains("REMINDERS"));
        assert!(result.contains("</environment_details>"));
    }
}
