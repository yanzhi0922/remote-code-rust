use anyhow::Result;
use rc_config::AppPaths;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerConfig {
    pub runner_id: String,
    pub control_plane_url: Option<String>,
    pub profile_dir: AppPaths,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerStatus {
    pub runner_id: String,
    pub control_plane_url: Option<String>,
    pub profile_dir: String,
    pub phase: &'static str,
}

pub fn describe_status(config: &RunnerConfig) -> Result<RunnerStatus> {
    Ok(RunnerStatus {
        runner_id: config.runner_id.clone(),
        control_plane_url: config.control_plane_url.clone(),
        profile_dir: config.profile_dir.profile_dir.display().to_string(),
        phase: "phase0-skeleton",
    })
}
