use std::env;
use std::path::PathBuf;

pub(crate) const DEFAULT_PROFILE_DIR_NAME: &str = ".remote-code-rust";
pub(crate) const AGENTS_DIR_NAME: &str = "agents";
pub(crate) const CACHE_DIR_NAME: &str = "cache";
pub(crate) const LOGS_DIR_NAME: &str = "logs";
pub(crate) const REMOTE_CONTROL_FILE_NAME: &str = "remote_control.json";
pub(crate) const GUI_PROJECTS_FILE_NAME: &str = "gui-projects.json";
pub(crate) const GUI_PROVIDERS_FILE_NAME: &str = "gui-providers.json";
pub(crate) const GUI_SETTINGS_FILE_NAME: &str = "gui-settings.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimePathLayout {
    pub(crate) profile_dir: PathBuf,
    pub(crate) sessions_dir: PathBuf,
    pub(crate) artifacts_dir: PathBuf,
    pub(crate) logs_dir: PathBuf,
    pub(crate) cache_dir: PathBuf,
    pub(crate) agents_dir: PathBuf,
    pub(crate) remote_control_file: PathBuf,
    pub(crate) gui_projects_file: PathBuf,
    pub(crate) gui_providers_file: PathBuf,
    pub(crate) gui_settings_file: PathBuf,
}

impl RuntimePathLayout {
    pub(crate) fn from_profile_dir(profile_dir: PathBuf) -> Self {
        Self {
            sessions_dir: profile_dir.join("sessions"),
            artifacts_dir: profile_dir.join("artifacts"),
            logs_dir: profile_dir.join(LOGS_DIR_NAME),
            cache_dir: profile_dir.join(CACHE_DIR_NAME),
            agents_dir: profile_dir.join(AGENTS_DIR_NAME),
            remote_control_file: profile_dir.join(REMOTE_CONTROL_FILE_NAME),
            gui_projects_file: profile_dir.join(GUI_PROJECTS_FILE_NAME),
            gui_providers_file: profile_dir.join(GUI_PROVIDERS_FILE_NAME),
            gui_settings_file: profile_dir.join(GUI_SETTINGS_FILE_NAME),
            profile_dir,
        }
    }

    pub(crate) fn discover() -> Option<Self> {
        Some(Self::from_profile_dir(profile_dir_from_env_or_home()?))
    }

    pub(crate) fn ensure_exists(&self) -> std::io::Result<()> {
        for directory in [
            &self.profile_dir,
            &self.sessions_dir,
            &self.artifacts_dir,
            &self.logs_dir,
            &self.cache_dir,
            &self.agents_dir,
        ] {
            std::fs::create_dir_all(directory)?;
        }
        Ok(())
    }
}

#[cfg(feature = "desktop")]
impl RuntimePathLayout {
    pub(crate) fn from_app_paths(paths: &claude_config::AppPaths) -> Self {
        let mut layout = Self::from_profile_dir(paths.profile_dir.clone());
        layout.sessions_dir = paths.sessions_dir.clone();
        layout.artifacts_dir = paths.artifacts_dir.clone();
        layout.logs_dir = paths.logs_dir.clone();
        layout
    }
}

pub(crate) fn profile_dir_from_env_or_home() -> Option<PathBuf> {
    env::var("REMOTE_CODE_PROFILE_DIR")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| home_dir().map(|home| home.join(DEFAULT_PROFILE_DIR_NAME)))
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(PathBuf::from))
}
