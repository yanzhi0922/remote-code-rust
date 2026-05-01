//! System information section.
//!
//! Source: `src/core/prompts/sections/system-info.ts`

/// Converts backslashes to forward slashes (Posix-style paths).
fn to_posix(path: &str) -> String {
    path.replace('\\', "/")
}

/// Returns the system information section.
///
/// Source: `src/core/prompts/sections/system-info.ts` — `getSystemInfoSection`
pub fn get_system_info_section(os_info: &str, shell: &str, home_dir: &str, cwd: &str) -> String {
    let home_dir_posix = to_posix(home_dir);
    let cwd_posix = to_posix(cwd);

    format!(
        r#"====

SYSTEM INFORMATION

Operating System: {os_info}
Default Shell: {shell}
Home Directory: {home_dir_posix}
Current Workspace Directory: {cwd_posix}

The Current Workspace Directory is the active VS Code project directory, and is therefore the default directory for all tool operations. New terminals will be created in the current workspace directory, however if you change directories in a terminal it will then have a different working directory; changing directories in a terminal does not modify the workspace directory, because you do not have access to change the workspace directory. When the user initially gives you a task, a recursive list of all filepaths in the current workspace directory ('/test/path') will be included in environment_details. This provides an overview of the project's file structure, offering key insights into the project from directory/file names (how developers conceptualize and organize their code) and file extensions (the language used). This can also guide decision-making on which files to explore further. If you need to further explore directories such as outside the current workspace directory, you can use the list_files tool. If you pass 'true' for the recursive parameter, it will list files recursively. Otherwise, it will list files at the top level, which is better suited for generic directories where you don't necessarily need the nested structure, like the Desktop."#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_system_info_section_basic() {
        let result = get_system_info_section("Linux 6.1", "/bin/bash", "/home/user", "/home/user/project");
        assert!(result.starts_with("====\n\nSYSTEM INFORMATION"));
        assert!(result.contains("Operating System: Linux 6.1"));
        assert!(result.contains("Default Shell: /bin/bash"));
        assert!(result.contains("Home Directory: /home/user"));
        assert!(result.contains("Current Workspace Directory: /home/user/project"));
    }

    #[test]
    fn test_get_system_info_section_windows_paths() {
        let result = get_system_info_section(
            "Windows 11",
            r"C:\WINDOWS\system32\cmd.exe",
            r"C:\Users\test",
            r"C:\Users\test\project",
        );
        // Windows paths should be converted to posix
        assert!(result.contains("Home Directory: C:/Users/test"));
        assert!(result.contains("Current Workspace Directory: C:/Users/test/project"));
        assert!(result.contains("Default Shell: C:\\WINDOWS\\system32\\cmd.exe"));
    }

    #[test]
    fn test_get_system_info_section_contains_workspace_description() {
        let result = get_system_info_section("macOS 14.0", "/bin/zsh", "/Users/test", "/Users/test/dev");
        assert!(result.contains("The Current Workspace Directory is the active VS Code project directory"));
        assert!(result.contains("list_files tool"));
        assert!(result.contains("recursive parameter"));
    }

    #[test]
    fn test_to_posix() {
        assert_eq!(to_posix(r"C:\Users\test"), "C:/Users/test");
        assert_eq!(to_posix("/home/user"), "/home/user");
        assert_eq!(to_posix(r"src\lib\mod.rs"), "src/lib/mod.rs");
    }
}
