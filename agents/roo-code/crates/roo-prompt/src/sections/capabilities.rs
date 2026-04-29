//! Capabilities section.
//!
//! Source: `src/core/prompts/sections/capabilities.ts`

/// Converts backslashes to forward slashes (Posix-style paths).
fn to_posix(path: &str) -> String {
    path.replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_capabilities_section_basic() {
        let result = get_capabilities_section("/home/user/project", false);
        assert!(result.starts_with("====\n\nCAPABILITIES"));
        assert!(result.contains("execute CLI commands"));
        assert!(result.contains("list files"));
        assert!(result.contains("read and write files"));
        assert!(result.contains("/home/user/project"));
    }

    #[test]
    fn test_get_capabilities_section_no_mcp() {
        let result = get_capabilities_section("/home/user/project", false);
        assert!(!result.contains("MCP servers"));
    }

    #[test]
    fn test_get_capabilities_section_with_mcp() {
        let result = get_capabilities_section("/home/user/project", true);
        assert!(result.contains("MCP servers"));
        assert!(result.contains("additional tools and resources"));
    }

    #[test]
    fn test_get_capabilities_section_windows_paths() {
        let result = get_capabilities_section(r"C:\Users\test\project", false);
        assert!(result.contains("C:/Users/test/project"));
    }

    #[test]
    fn test_get_capabilities_section_execute_command_description() {
        let result = get_capabilities_section("/home/user/project", false);
        assert!(result.contains("execute_command tool"));
        assert!(result.contains("VSCode terminal"));
        assert!(result.contains("new terminal instance"));
    }
}

/// Returns the capabilities section.
///
/// Source: `src/core/prompts/sections/capabilities.ts` — `getCapabilitiesSection`
pub fn get_capabilities_section(cwd: &str, has_mcp: bool) -> String {
    let cwd_posix = to_posix(cwd);
    let mcp_section = if has_mcp {
        "\n- You have access to MCP servers that may provide additional tools and resources. Each server may provide different capabilities that you can use to accomplish tasks more effectively.\n"
    } else {
        ""
    };

    format!(
        r#"====

CAPABILITIES

- You have access to tools that let you execute CLI commands on the user's computer, list files, view source code definitions, regex search, read and write files, and ask follow-up questions. These tools help you effectively accomplish a wide range of tasks, such as writing code, making edits or improvements to existing files, understanding the current state of a project, performing system operations, and much more.
- When the user initially gives you a task, a recursive list of all filepaths in the current workspace directory ('{cwd_posix}') will be included in environment_details. This provides an overview of the project's file structure, offering key insights into the project from directory/file names (how developers conceptualize and organize their code) and file extensions (the language used). This can also guide decision-making on which files to explore further. If you need to further explore directories such as outside the current workspace directory, you can use the list_files tool. If you pass 'true' for the recursive parameter, it will list files recursively. Otherwise, it will list files at the top level, which is better suited for generic directories where you don't necessarily need the nested structure, like the Desktop.
- You can use the execute_command tool to run commands on the user's computer whenever you feel it can help accomplish the user's task. When you need to execute a CLI command, you must provide a clear explanation of what the command does. Prefer to execute complex CLI commands over creating executable scripts, since they are more flexible and easier to run. Interactive and long-running commands are allowed, since the commands are run in the user's VSCode terminal. The user may keep commands running in the background and you will be kept updated on their status along the way. Each command you execute is run in a new terminal instance.{mcp_section}"#
    )
}
