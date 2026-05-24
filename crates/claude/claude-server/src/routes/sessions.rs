use std::path::{Path, PathBuf};

use axum::Json;
use axum::extract::{Path as AxumPath, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ServerConfig;
use crate::error::ApiError;
use crate::state::ServerState;

#[derive(Debug, Serialize)]
pub struct SessionResponse {
    pub session_id: Uuid,
    pub title: String,
    pub cwd: String,
    pub provider_name: String,
    pub model: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<claude_session::SessionSummary> for SessionResponse {
    fn from(s: claude_session::SessionSummary) -> Self {
        Self {
            session_id: s.session_id,
            title: s.title,
            cwd: s.cwd.display().to_string(),
            provider_name: s.provider_name,
            model: s.model,
            created_at: s.created_at.to_rfc3339(),
            updated_at: s.updated_at.to_rfc3339(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateSessionRequest {
    pub work_dir: Option<String>,
}

pub async fn list_sessions(
    State(state): State<ServerState>,
) -> Result<Json<Vec<SessionResponse>>, ApiError> {
    let sessions = state.session_store.list_active_sessions()?;
    Ok(Json(
        sessions.into_iter().map(SessionResponse::from).collect(),
    ))
}

pub async fn create_session(
    State(state): State<ServerState>,
    Json(req): Json<CreateSessionRequest>,
) -> Result<(StatusCode, Json<SessionResponse>), ApiError> {
    let session_id = Uuid::new_v4();
    let work_dir = resolve_session_work_dir(&state.config, req.work_dir.as_deref())?;

    state.session_store.ensure_session(
        session_id,
        &work_dir,
        &state.config.default_provider,
        Some(&state.config.default_model),
        None,
    )?;

    let summary = state.session_store.get_session_summary(session_id)?;
    Ok((StatusCode::CREATED, Json(SessionResponse::from(summary))))
}

pub async fn get_session(
    State(state): State<ServerState>,
    AxumPath(id): AxumPath<Uuid>,
) -> Result<Json<SessionResponse>, ApiError> {
    let summary = state
        .session_store
        .get_session_summary(id)
        .map_err(|_| ApiError::not_found(format!("session `{id}` not found")))?;
    Ok(Json(SessionResponse::from(summary)))
}

pub async fn delete_session(
    State(state): State<ServerState>,
    AxumPath(id): AxumPath<Uuid>,
) -> Result<StatusCode, ApiError> {
    state.session_store.set_archived(id, true)?;
    state.remove_active_session(id);
    Ok(StatusCode::NO_CONTENT)
}

pub async fn get_messages(
    State(state): State<ServerState>,
    AxumPath(id): AxumPath<Uuid>,
) -> Result<Json<Vec<serde_json::Value>>, ApiError> {
    let conversation = state.session_store.load_conversation(id)?;
    let entries: Vec<serde_json::Value> = conversation
        .iter()
        .filter_map(|entry| serde_json::to_value(entry).ok())
        .collect();
    Ok(Json(entries))
}

fn resolve_session_work_dir(
    config: &ServerConfig,
    requested_work_dir: Option<&str>,
) -> Result<PathBuf, ApiError> {
    let base = canonical_dir(&config.default_work_dir, "default work_dir")?;
    let requested = requested_work_dir
        .map(PathBuf::from)
        .unwrap_or_else(|| base.clone());
    let candidate = if requested.is_absolute() {
        requested
    } else {
        base.join(requested)
    };
    let canonical = canonical_dir(&candidate, "requested work_dir")?;

    if !canonical.starts_with(&base) {
        return Err(ApiError::bad_request(format!(
            "requested work_dir must be inside {}",
            base.display()
        )));
    }

    Ok(canonical)
}

fn canonical_dir(path: &Path, label: &str) -> Result<PathBuf, ApiError> {
    let canonical = std::fs::canonicalize(path).map_err(|error| {
        ApiError::bad_request(format!(
            "{label} `{}` is not accessible: {error}",
            path.display()
        ))
    })?;
    if !canonical.is_dir() {
        return Err(ApiError::bad_request(format!(
            "{label} `{}` is not a directory",
            path.display()
        )));
    }
    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use super::resolve_session_work_dir;
    use crate::ServerConfig;
    use tempfile::tempdir;

    #[test]
    fn resolve_session_work_dir_allows_default_and_children() {
        let root = tempdir().expect("tempdir");
        let child = root.path().join("child");
        std::fs::create_dir(&child).expect("child dir");
        let config = ServerConfig {
            default_work_dir: root.path().to_path_buf(),
            ..ServerConfig::default()
        };

        let default_dir = resolve_session_work_dir(&config, None).expect("default dir");
        let child_dir = resolve_session_work_dir(&config, Some("child")).expect("child dir");

        assert_eq!(default_dir, std::fs::canonicalize(root.path()).unwrap());
        assert_eq!(child_dir, std::fs::canonicalize(child).unwrap());
    }

    #[test]
    fn resolve_session_work_dir_rejects_external_absolute_path() {
        let root = tempdir().expect("tempdir");
        let outside = tempdir().expect("outside");
        let config = ServerConfig {
            default_work_dir: root.path().to_path_buf(),
            ..ServerConfig::default()
        };

        let error = resolve_session_work_dir(&config, Some(&outside.path().display().to_string()))
            .expect_err("external path should be rejected");

        assert!(format!("{error:?}").contains("must be inside"));
    }
}
