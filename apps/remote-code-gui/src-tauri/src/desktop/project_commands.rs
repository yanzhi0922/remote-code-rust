use super::*;

#[tauri::command]
pub(super) async fn list_projects(
    state: State<'_, AppState>,
) -> std::result::Result<Vec<ProjectInfoDto>, String> {
    let runtime = state.runtime.lock().await;
    let sessions = runtime
        .session_store
        .list_active_sessions()
        .map_err(|error| format!("{error:#}"))?;
    Ok(build_project_infos(&runtime.projects, &sessions))
}

#[tauri::command]
pub(super) async fn add_project(
    state: State<'_, AppState>,
    path: String,
) -> std::result::Result<ProjectInfoDto, String> {
    let mut runtime = state.runtime.lock().await;
    let path = normalize_existing_path(Path::new(&path)).map_err(|error| format!("{error:#}"))?;
    if !path.exists() || !path.is_dir() {
        return Err(format!("project path does not exist: {}", path.display()));
    }
    if !runtime
        .projects
        .iter()
        .any(|project| path_identity(&project.path) == path_identity(&path))
    {
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("project")
            .to_owned();
        runtime.projects.push(ProjectEntry {
            path: path.clone(),
            name: name.clone(),
        });
        runtime
            .projects
            .sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
        persist_runtime_files(&runtime).map_err(|error| format!("{error:#}"))?;
        return Ok(ProjectInfoDto {
            path: path.display().to_string(),
            name,
            session_count: project_session_count(
                &path,
                &runtime
                    .session_store
                    .list_active_sessions()
                    .map_err(|error| format!("{error:#}"))?,
            ),
            is_auto_detected: false,
        });
    }
    let project = runtime
        .projects
        .iter()
        .find(|project| project.path == path)
        .ok_or_else(|| "failed to load project".to_owned())?;
    Ok(ProjectInfoDto {
        path: project.path.display().to_string(),
        name: project.name.clone(),
        session_count: 0,
        is_auto_detected: false,
    })
}

#[tauri::command]
pub(super) async fn remove_project(
    state: State<'_, AppState>,
    path: String,
) -> std::result::Result<(), String> {
    let mut runtime = state.runtime.lock().await;
    let path = normalize_existing_path(Path::new(&path)).unwrap_or_else(|_| PathBuf::from(path));
    let sessions = runtime
        .session_store
        .list_active_sessions()
        .map_err(|error| format!("{error:#}"))?;
    if project_session_count(&path, &sessions) > 0 {
        return Err("该项目下仍有会话，不能移除项目文件夹。".to_owned());
    }
    let path_key = path_identity(&path);
    runtime
        .projects
        .retain(|project| path_identity(&project.path) != path_key);
    persist_runtime_files(&runtime).map_err(|error| format!("{error:#}"))?;
    Ok(())
}

#[tauri::command]
pub(super) async fn pick_folder(app: AppHandle) -> std::result::Result<Option<String>, String> {
    let picked = app.dialog().file().blocking_pick_folder();
    let Some(picked) = picked else {
        return Ok(None);
    };
    let path = picked.into_path().map_err(|error| error.to_string())?;
    Ok(Some(path.display().to_string()))
}
