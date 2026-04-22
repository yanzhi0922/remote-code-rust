use std::path::{Component, Path, PathBuf};

use rc_config::RuntimeConfig;
use rc_runtime_prompt::RuntimePromptSettings;
use rc_session::session_memory::session_memory_dir;
use rc_tools::current_runtime_agent_prompt_context;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SessionFileType {
    SessionMemory,
    SessionTranscript,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MemoryScope {
    Personal,
    Team,
}

fn is_same_or_child_dir(path: &str, dir: &str) -> bool {
    path == dir || path.starts_with(&format!("{dir}/"))
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

fn claude_config_home_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("CLAUDE_CONFIG_DIR") {
        return PathBuf::from(dir);
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join(".claude");
    }
    if let Some(home) = std::env::var_os("USERPROFILE") {
        return PathBuf::from(home).join(".claude");
    }
    PathBuf::from(".claude")
}

fn comparable_path(path: &Path) -> String {
    let rendered = normalize_path(path).to_string_lossy().replace('\\', "/");
    if cfg!(windows) {
        rendered.to_ascii_lowercase()
    } else {
        rendered
    }
}

fn comparable_str(value: &str) -> String {
    let rendered = value.replace('\\', "/");
    if cfg!(windows) {
        rendered.to_ascii_lowercase()
    } else {
        rendered
    }
}

fn auto_memory_read_dir(config: &RuntimeConfig) -> Option<PathBuf> {
    current_runtime_agent_prompt_context()
        .and_then(|context| context.auto_memory_read_dir.or(context.auto_memory_dir))
        .or_else(|| {
            RuntimePromptSettings::from_config(config)
                .auto_memory_read_dir
                .map(PathBuf::from)
        })
}

fn team_memory_read_dir(config: &RuntimeConfig) -> Option<PathBuf> {
    current_runtime_agent_prompt_context()
        .and_then(|context| context.team_memory_read_dir)
        .or_else(|| {
            RuntimePromptSettings::from_config(config)
                .team_memory_read_dir
                .map(PathBuf::from)
        })
}

fn agent_memory_dirs(config: &RuntimeConfig) -> Vec<PathBuf> {
    let mut dirs = current_runtime_agent_prompt_context()
        .map(|context| context.agent_memory_dirs)
        .unwrap_or_default();
    dirs.push(config.original_cwd.join(".claude").join("agent-memory"));
    dirs.push(
        config
            .original_cwd
            .join(".claude")
            .join("agent-memory-local"),
    );
    dirs.sort();
    dirs.dedup();
    dirs
}

pub(crate) fn detect_session_file_type(
    config: &RuntimeConfig,
    file_path: &Path,
) -> Option<SessionFileType> {
    let comparable = comparable_path(file_path);
    let profile_dir = comparable_path(&config.paths.profile_dir);
    let claude_home = comparable_path(&claude_config_home_dir());
    let sessions_dir = comparable_path(&config.paths.sessions_dir);
    if ((comparable.starts_with(&claude_home) || comparable.starts_with(&profile_dir))
        && comparable.contains("/projects/")
        && comparable.ends_with(".jsonl"))
        || (is_same_or_child_dir(&comparable, &sessions_dir) && comparable.ends_with(".ndjson"))
    {
        return Some(SessionFileType::SessionTranscript);
    }

    let session_memory = comparable_path(&session_memory_dir(config));
    if is_same_or_child_dir(&comparable, &session_memory) && comparable.ends_with(".md") {
        return Some(SessionFileType::SessionMemory);
    }

    None
}

pub(crate) fn detect_session_pattern_type(pattern: &str) -> Option<SessionFileType> {
    let comparable = comparable_str(pattern);
    if comparable.contains("session-memory")
        && (comparable.contains(".md") || comparable.ends_with('*'))
    {
        return Some(SessionFileType::SessionMemory);
    }
    if comparable.contains(".jsonl")
        || (comparable.contains("projects") && comparable.contains("*.jsonl"))
        || (comparable.contains("sessions") && comparable.contains(".ndjson"))
    {
        return Some(SessionFileType::SessionTranscript);
    }
    None
}

pub(crate) fn is_auto_mem_file(config: &RuntimeConfig, file_path: &Path) -> bool {
    auto_memory_read_dir(config)
        .map(|dir| comparable_path(file_path).starts_with(&comparable_path(&dir)))
        .unwrap_or(false)
}

pub(crate) fn is_team_mem_file(config: &RuntimeConfig, file_path: &Path) -> bool {
    team_memory_read_dir(config)
        .map(|dir| comparable_path(file_path).starts_with(&comparable_path(&dir)))
        .unwrap_or(false)
}

pub(crate) fn memory_scope_for_path(
    config: &RuntimeConfig,
    file_path: &Path,
) -> Option<MemoryScope> {
    if is_team_mem_file(config, file_path) {
        return Some(MemoryScope::Team);
    }
    if is_auto_mem_file(config, file_path) {
        return Some(MemoryScope::Personal);
    }
    None
}

pub(crate) fn is_agent_mem_file(config: &RuntimeConfig, file_path: &Path) -> bool {
    let comparable = comparable_path(file_path);
    agent_memory_dirs(config)
        .into_iter()
        .any(|dir| comparable.starts_with(&comparable_path(&dir)))
}

pub(crate) fn is_auto_managed_memory_file(config: &RuntimeConfig, file_path: &Path) -> bool {
    is_auto_mem_file(config, file_path)
        || is_team_mem_file(config, file_path)
        || detect_session_file_type(config, file_path).is_some()
        || is_agent_mem_file(config, file_path)
}

pub(crate) fn is_memory_directory(config: &RuntimeConfig, dir_path: &Path) -> bool {
    let comparable = comparable_path(dir_path);
    let session_memory = comparable_path(&session_memory_dir(config));
    if is_same_or_child_dir(&comparable, &session_memory) {
        return true;
    }

    let sessions_dir = comparable_path(&config.paths.sessions_dir);
    if is_same_or_child_dir(&comparable, &sessions_dir) {
        return true;
    }

    if agent_memory_dirs(config)
        .into_iter()
        .any(|dir| comparable.starts_with(&comparable_path(&dir)))
    {
        return true;
    }

    if let Some(team_dir) = team_memory_read_dir(config) {
        let team = comparable_path(&team_dir);
        if is_same_or_child_dir(&comparable, &team) {
            return true;
        }
    }

    if let Some(auto_dir) = auto_memory_read_dir(config) {
        let auto = comparable_path(&auto_dir);
        if is_same_or_child_dir(&comparable, &auto) {
            return true;
        }
    }

    let profile_dir = comparable_path(&config.paths.profile_dir);
    let claude_home = comparable_path(&claude_config_home_dir());
    if (comparable.starts_with(&claude_home) || comparable.starts_with(&profile_dir))
        && comparable.contains("/projects/")
    {
        return true;
    }
    comparable.contains("/memory/")
}

fn shell_capture_to_path(cleaned: &str) -> PathBuf {
    if cfg!(windows)
        && cleaned.starts_with('/')
        && cleaned.len() > 3
        && cleaned.as_bytes()[2] == b'/'
        && cleaned.as_bytes()[1].is_ascii_alphabetic()
    {
        return PathBuf::from(format!("{}:/{}", &cleaned[1..2], &cleaned[3..]));
    }
    PathBuf::from(cleaned)
}

pub(crate) fn is_shell_command_targeting_memory(config: &RuntimeConfig, command: &str) -> bool {
    let command_cmp = comparable_str(command);
    let mut roots = vec![
        comparable_path(&claude_config_home_dir()),
        comparable_path(&config.paths.profile_dir),
        comparable_path(&config.paths.sessions_dir),
        comparable_path(&session_memory_dir(config)),
    ];
    if let Some(auto_dir) = auto_memory_read_dir(config) {
        roots.push(comparable_path(&auto_dir));
    }
    if let Some(team_dir) = team_memory_read_dir(config) {
        roots.push(comparable_path(&team_dir));
    }
    if !roots.iter().any(|root| command_cmp.contains(root)) {
        return false;
    }

    let path_like_tokens = regex::Regex::new(r#"(?:[A-Za-z]:[/\\]|/)[^\s'"]+"#)
        .expect("valid memory shell path regex");
    path_like_tokens.find_iter(command).any(|capture| {
        let cleaned = capture.as_str().trim_end_matches([',', ';', '|', '&', '>']);
        let path = shell_capture_to_path(cleaned);
        is_auto_managed_memory_file(config, &path) || is_memory_directory(config, &path)
    })
}

pub(crate) fn is_auto_managed_memory_pattern(pattern: &str) -> bool {
    if detect_session_pattern_type(pattern).is_some() {
        return true;
    }
    let comparable = comparable_str(pattern);
    comparable.contains("agent-memory/") || comparable.contains("agent-memory-local/")
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::{
        MemoryScope, SessionFileType, detect_session_file_type, detect_session_pattern_type,
        is_auto_managed_memory_pattern, memory_scope_for_path,
    };

    #[test]
    fn session_pattern_type_matches_research_rules() {
        assert_eq!(
            detect_session_pattern_type("C:/x/session-memory/*.md"),
            Some(SessionFileType::SessionMemory)
        );
        assert_eq!(
            detect_session_pattern_type("C:/x/projects/*.jsonl"),
            Some(SessionFileType::SessionTranscript)
        );
        assert_eq!(
            detect_session_pattern_type("C:/x/sessions/*.ndjson"),
            Some(SessionFileType::SessionTranscript)
        );
    }

    #[test]
    fn auto_managed_pattern_matches_agent_memory() {
        assert!(is_auto_managed_memory_pattern(
            ".claude/agent-memory/**/*.md"
        ));
        assert!(is_auto_managed_memory_pattern(
            ".claude/agent-memory-local/**/*.md"
        ));
    }

    #[test]
    fn memory_scope_prefers_team() {
        let temp = tempdir().expect("tempdir");
        let cwd = temp.path().join("cwd");
        let profile = temp.path().join("profile");
        let auto_dir = temp.path().join("memory");
        let team_dir = auto_dir.join("team");
        fs::create_dir_all(&cwd).expect("cwd dir");
        fs::create_dir_all(&profile).expect("profile dir");
        std::fs::create_dir_all(&team_dir).expect("team dir");
        let config = rc_config::load_runtime_config(
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
            rc_config::ProviderOverrides::default(),
            rc_config::RuntimeOverrides::default(),
        )
        .expect("config");
        let ctx = rc_tools::RuntimeAgentPromptContext {
            auto_memory_read_dir: Some(auto_dir),
            team_memory_read_dir: Some(team_dir.clone()),
            ..rc_tools::RuntimeAgentPromptContext::default()
        };
        let result = rc_tools::with_runtime_agent_prompt_context_provider(
            std::sync::Arc::new(move || ctx.clone()),
            async move { memory_scope_for_path(&config, &team_dir.join("x.md")) },
        );
        let scope = futures::executor::block_on(result);
        assert_eq!(scope, Some(MemoryScope::Team));
    }

    #[test]
    fn session_memory_detection_uses_runtime_path() {
        let temp = tempdir().expect("tempdir");
        let cwd = temp.path().join("cwd");
        let profile = temp.path().join("profile");
        fs::create_dir_all(&cwd).expect("cwd dir");
        fs::create_dir_all(&profile).expect("profile dir");
        let config = rc_config::load_runtime_config(
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
            rc_config::ProviderOverrides::default(),
            rc_config::RuntimeOverrides::default(),
        )
        .expect("config");
        let path = rc_session::session_memory::session_memory_dir(&config).join("summary.md");
        assert_eq!(
            detect_session_file_type(&config, &path),
            Some(SessionFileType::SessionMemory)
        );
    }

    #[test]
    fn session_transcript_detection_uses_runtime_path() {
        let temp = tempdir().expect("tempdir");
        let cwd = temp.path().join("cwd");
        let profile = temp.path().join("profile");
        fs::create_dir_all(&cwd).expect("cwd dir");
        fs::create_dir_all(&profile).expect("profile dir");
        let config = rc_config::load_runtime_config(
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
            rc_config::ProviderOverrides::default(),
            rc_config::RuntimeOverrides::default(),
        )
        .expect("config");
        let path = config
            .paths
            .sessions_dir
            .join(format!("{}.ndjson", config.session_id));
        assert_eq!(
            detect_session_file_type(&config, &path),
            Some(SessionFileType::SessionTranscript)
        );
    }
}
