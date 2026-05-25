use super::*;

// ---------------------------------------------------------------------------
// Agent binary management helpers
// ---------------------------------------------------------------------------

/// Base directory for agent installations: `~/.remote-code/agents/`.
fn agents_base_dir() -> PathBuf {
    directories::BaseDirs::new()
        .map(|dirs| dirs.home_dir().join(".remote-code").join("agents"))
        .unwrap_or_else(|| PathBuf::from(".remote-code").join("agents"))
}

/// Directory name for a given agent type (matches serde serialization).
fn agent_type_dir_name(agent_type: &ProtocolAgentType) -> &'static str {
    match agent_type {
        ProtocolAgentType::RemoteClaude => "remote_claude",
        ProtocolAgentType::RemoteRoo => "remote_roo",
        ProtocolAgentType::RemoteCodex => "remote_codex",
        _ => "unknown",
    }
}

/// Expected binary name for a given agent type.
fn agent_binary_name(agent_type: &ProtocolAgentType) -> String {
    let name = match agent_type {
        ProtocolAgentType::RemoteClaude => "remote-claude",
        ProtocolAgentType::RemoteRoo => "remote-roo",
        ProtocolAgentType::RemoteCodex => "remote-codex",
        _ => "unknown-agent",
    };
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    }
}

#[tauri::command]
pub(super) async fn list_available_agents(
    _state: State<'_, AppState>,
) -> std::result::Result<Vec<AgentTypeInfoDto>, String> {
    let specs: [(ProtocolAgentType, &str); 3] = [
        (ProtocolAgentType::RemoteClaude, "Remote Claude"),
        (ProtocolAgentType::RemoteRoo, "Remote Roo"),
        (ProtocolAgentType::RemoteCodex, "Remote Codex"),
    ];
    let agents = specs
        .iter()
        .map(|(agent_type, display_name)| {
            // All agents are now in-process — no external binary required.
            // RemoteClaude was always built-in; RemoteRoo and RemoteCodex are
            // now also built-in via callback-based in-process adapters.
            let installed = true;
            AgentTypeInfoDto {
                agent_type: agent_type_dir_name(agent_type).to_owned(),
                display_name: display_name.to_string(),
                available: true,
                installed,
            }
        })
        .collect();
    Ok(agents)
}

#[tauri::command]
pub(super) async fn install_agent(agent_type: String) -> std::result::Result<(), String> {
    let parsed: ProtocolAgentType = serde_json::from_str(&format!("\"{}\"", agent_type))
        .map_err(|e| format!("无效的 agent_type: {e}"))?;

    // All agents are now in-process — no external binary to install.
    // RemoteClaude was always built-in; RemoteRoo and RemoteCodex are now
    // also built-in via callback-based in-process adapters.
    if matches!(parsed, ProtocolAgentType::RemoteClaude)
        || matches!(parsed, ProtocolAgentType::RemoteRoo)
        || matches!(parsed, ProtocolAgentType::RemoteCodex)
    {
        return Ok(());
    }

    // Fix #8: append platform-specific suffix to download URLs.
    let platform_suffix = if cfg!(windows) {
        "-windows.exe"
    } else if cfg!(target_os = "macos") {
        "-macos"
    } else {
        "-linux"
    };

    // Attempt to download from a configurable URL.
    let env_key = format!("REMOTE_CODE_{}_DOWNLOAD_URL", agent_type.to_uppercase());
    let download_url = env::var(&env_key).ok().or_else(|| match parsed {
        ProtocolAgentType::RemoteRoo => Some(format!(
            "https://github.com/roo-code/roo-code/releases/latest/download/roo-code-cli{platform_suffix}"
        )),
        ProtocolAgentType::RemoteCodex => Some(format!(
            "https://github.com/openai/codex/releases/latest/download/codex{platform_suffix}"
        )),
        _ => None,
    });

    let Some(url) = download_url else {
        return Err(format!(
            "未配置 {agent_type} 的下载地址。请手动安装到 ~/.remote-code/agents/{}/bin/",
            agent_type_dir_name(&parsed)
        ));
    };

    // Create installation directory.
    let install_dir = agents_base_dir()
        .join(agent_type_dir_name(&parsed))
        .join("bin");
    std::fs::create_dir_all(&install_dir).map_err(|e| format!("创建安装目录失败: {e}"))?;

    let target_path = install_dir.join(agent_binary_name(&parsed));

    // Download.
    let response = reqwest::get(&url)
        .await
        .map_err(|e| format!("下载失败: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("下载失败: HTTP {}", response.status()));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("读取下载内容失败: {e}"))?;

    // Validate: must be non-empty.
    if bytes.is_empty() {
        return Err("下载内容为空".to_owned());
    }

    std::fs::write(&target_path, &bytes).map_err(|e| format!("写入文件失败: {e}"))?;

    // Set executable permissions on Unix.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&target_path, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("设置执行权限失败: {e}"))?;
    }

    Ok(())
}

#[tauri::command]
pub(super) async fn uninstall_agent(
    _state: State<'_, AppState>,
    agent_type: String,
) -> std::result::Result<(), String> {
    let _parsed: ProtocolAgentType = serde_json::from_str(&format!("\"{}\"", agent_type))
        .map_err(|e| format!("无效的 agent_type: {e}"))?;

    // All agents are now built-in (in-process) — cannot uninstall any of them.
    Err("所有 Agent 均为内置进程内适配器，无法卸载".to_owned())
}
