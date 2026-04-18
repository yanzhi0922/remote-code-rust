//! Shared helpers for persistent team and mailbox-backed collaboration tools.

use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{Context, Result, anyhow};
use serde_json::{Value, json};
use uuid::Uuid;

use rc_swarm::{SwarmError, TeamAllowedPath, TeamFile, TeamMember, mailbox, team_helpers};

fn requested_team_name(input: &Value) -> Option<String> {
    input
        .get("team_name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(ToOwned::to_owned)
}

fn sanitize_team_name(raw: &str) -> String {
    let sanitized: String = raw
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = sanitized.trim_matches('_').trim_matches('-').to_owned();
    let candidate = if trimmed.is_empty() {
        "team".to_owned()
    } else {
        trimmed
    };
    let starts_with_alnum = candidate
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_alphanumeric());
    let mut normalized = if starts_with_alnum {
        candidate
    } else {
        format!("team_{candidate}")
    };
    if normalized.len() > 64 {
        normalized.truncate(64);
    }
    normalized
}

async fn all_team_names() -> Result<Vec<String>> {
    team_helpers::list_teams()
        .await
        .context("failed to list teams")
        .map(|mut teams| {
            teams.sort();
            teams
        })
}

async fn unique_team_name(base: &str) -> Result<String> {
    let taken = all_team_names().await?;
    if !taken.iter().any(|name| name == base) {
        return Ok(base.to_owned());
    }

    for suffix in 2..=999 {
        let max_base_len = 64usize.saturating_sub(suffix.to_string().len() + 1);
        let mut candidate_base = base.to_owned();
        if candidate_base.len() > max_base_len {
            candidate_base.truncate(max_base_len);
        }
        let candidate = format!("{candidate_base}-{suffix}");
        if !taken.iter().any(|name| name == &candidate) {
            return Ok(candidate);
        }
    }

    Ok(format!("team-{}", Uuid::new_v4().simple()))
}

fn objective_from_team(team: &TeamFile) -> Option<&str> {
    team.description.as_deref()
}

async fn unread_count(team_name: &str, agent_name: &str) -> Result<usize> {
    mailbox::count_unread(team_name, agent_name)
        .await
        .map_err(anyhow::Error::from)
}

pub(crate) async fn resolve_single_team_name(explicit: Option<&str>) -> Result<String> {
    if let Some(name) = explicit {
        return Ok(sanitize_team_name(name));
    }

    let teams = all_team_names().await?;
    match teams.as_slice() {
        [] => Err(anyhow!(
            "no active team found; create one with team_create or pass team_name explicitly"
        )),
        [single] => Ok(single.clone()),
        _ => Err(anyhow!(
            "multiple teams are available; pass team_name explicitly"
        )),
    }
}

pub(crate) async fn load_team(team_name: &str) -> Result<TeamFile> {
    team_helpers::read_team(team_name)
        .await
        .map_err(|error| match error {
            SwarmError::TeamNotFound(_) => anyhow!("team '{team_name}' was not found"),
            other => anyhow!(other),
        })
}

pub(crate) fn team_name_from_input(input: &Value) -> Option<String> {
    requested_team_name(input)
}

fn build_member(
    agent_def: &Value,
    cwd: &Path,
    index: usize,
    fallback_role: &str,
) -> Result<TeamMember> {
    let name = agent_def
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .ok_or_else(|| anyhow!("agent definition is missing a non-empty `name`"))?;
    team_helpers::validate_agent_name(name).map_err(anyhow::Error::from)?;

    let mut member = TeamMember::new(
        format!("agent-{}", Uuid::new_v4().simple()),
        name,
        format!("pane-{index}"),
        agent_def
            .get("cwd")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| cwd.to_string_lossy().to_string()),
    );
    member.agent_type = Some(
        agent_def
            .get("role")
            .and_then(Value::as_str)
            .unwrap_or(fallback_role)
            .to_owned(),
    );
    member.model = agent_def
        .get("model")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    member.color = agent_def
        .get("color")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    member.worktree_path = agent_def
        .get("worktree_path")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    member.session_id = agent_def
        .get("session_id")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    Ok(member)
}

pub(crate) async fn create_team(input: &Value, cwd: &Path) -> Result<String> {
    let objective = input
        .get("objective")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|objective| !objective.is_empty())
        .ok_or_else(|| anyhow!("objective is required"))?;
    let lead = input
        .get("lead")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|lead| !lead.is_empty())
        .unwrap_or("lead");

    let requested = requested_team_name(input)
        .map(|name| sanitize_team_name(&name))
        .unwrap_or_else(|| sanitize_team_name(objective));
    team_helpers::validate_team_name(&requested).map_err(anyhow::Error::from)?;

    let existing = match team_helpers::read_team(&requested).await {
        Ok(team) => Some(team),
        Err(SwarmError::TeamNotFound(_)) => None,
        Err(other) => return Err(anyhow!(other)),
    };

    let existed = existing.is_some();
    let team_name = if existed {
        requested
    } else {
        unique_team_name(&requested).await?
    };

    let mut team = existing.unwrap_or_else(|| TeamFile::new(&team_name, lead));
    team.name = team_name.clone();
    team.lead_agent_id = lead.to_owned();
    team.description = Some(objective.to_owned());
    team.members.clear();
    team.hidden_pane_ids.clear();
    team.team_allowed_paths = vec![TeamAllowedPath {
        path: cwd.to_string_lossy().to_string(),
        read_only: false,
    }];

    let mut seen_members = BTreeSet::new();
    if let Some(agents) = input.get("agents").and_then(Value::as_array) {
        for (index, agent_def) in agents.iter().enumerate() {
            let member = build_member(agent_def, cwd, index, "worker")?;
            if member.name == team.lead_agent_id {
                return Err(anyhow!(
                    "agent '{}' cannot reuse the team lead name",
                    member.name
                ));
            }
            if !seen_members.insert(member.name.clone()) {
                return Err(anyhow!("duplicate agent name '{}'", member.name));
            }
            team.members.push(member);
        }
    }

    let status = if existed {
        team_helpers::update_team(&team)
            .await
            .with_context(|| format!("failed to update team '{}'", team.name))?;
        "updated"
    } else {
        team_helpers::create_team(&team)
            .await
            .with_context(|| format!("failed to create team '{}'", team.name))?;
        "created"
    };

    let peers = peer_entries(&team);
    Ok(json!({
        "type": "team_create",
        "status": status,
        "team_name": team.name,
        "objective": objective_from_team(&team),
        "lead": team.lead_agent_id,
        "member_count": team.members.len(),
        "active_member_count": team.active_member_count(),
        "peers": peers,
    })
    .to_string())
}

pub(crate) fn peer_entries(team: &TeamFile) -> Vec<Value> {
    let mut peers = Vec::with_capacity(team.members.len() + 1);
    peers.push(json!({
        "name": team.lead_agent_id,
        "role": "lead",
        "team": team.name,
        "is_lead": true,
        "cwd": Value::Null,
        "active": true,
    }));
    peers.extend(team.members.iter().map(|member| {
        json!({
            "name": member.name,
            "role": member.agent_type.as_deref().unwrap_or("worker"),
            "team": team.name,
            "is_lead": false,
            "cwd": member.cwd,
            "active": member.is_active.unwrap_or(false),
            "model": member.model,
            "color": member.color,
        })
    }));
    peers
}

async fn detail_status(team: &TeamFile) -> Result<Value> {
    let lead_unread = unread_count(&team.name, &team.lead_agent_id).await?;
    let mut members = Vec::with_capacity(team.members.len());
    for member in &team.members {
        members.push(json!({
            "name": member.name,
            "role": member.agent_type.as_deref().unwrap_or("worker"),
            "cwd": member.cwd,
            "active": member.is_active.unwrap_or(false),
            "model": member.model,
            "color": member.color,
            "unread_messages": unread_count(&team.name, &member.name).await?,
        }));
    }
    Ok(json!({
        "team_name": team.name,
        "objective": objective_from_team(team),
        "lead": {
            "name": team.lead_agent_id,
            "unread_messages": lead_unread,
        },
        "members": members,
        "member_count": team.members.len(),
        "active_member_count": team.active_member_count(),
    }))
}

async fn summary_status(team: &TeamFile) -> Result<Value> {
    let mut unread_members = 0usize;
    for member in &team.members {
        unread_members += unread_count(&team.name, &member.name).await?;
    }
    let lead_unread = unread_count(&team.name, &team.lead_agent_id).await?;
    Ok(json!({
        "team_name": team.name,
        "objective": objective_from_team(team),
        "lead": team.lead_agent_id,
        "member_count": team.members.len(),
        "active_member_count": team.active_member_count(),
        "unread_messages": unread_members + lead_unread,
    }))
}

pub(crate) async fn team_status(input: &Value) -> Result<String> {
    if let Some(explicit) = requested_team_name(input) {
        let team = load_team(&explicit).await?;
        return Ok(json!({
            "type": "team_status",
            "count": 1,
            "teams": [detail_status(&team).await?],
        })
        .to_string());
    }

    let teams = all_team_names().await?;
    if teams.is_empty() {
        return Ok(json!({
            "type": "team_status",
            "teams": [],
            "count": 0,
            "message": "No active team in current context. Use team_create to create a team."
        })
        .to_string());
    }

    if teams.len() == 1 {
        let team = load_team(&teams[0]).await?;
        return Ok(json!({
            "type": "team_status",
            "count": 1,
            "teams": [detail_status(&team).await?],
        })
        .to_string());
    }

    let mut summaries = Vec::with_capacity(teams.len());
    for team_name in teams {
        let team = load_team(&team_name).await?;
        summaries.push(summary_status(&team).await?);
    }
    let count = summaries.len();
    Ok(json!({
        "type": "team_status",
        "teams": summaries,
        "count": count,
    })
    .to_string())
}

pub(crate) async fn list_peers(input: &Value) -> Result<String> {
    let peers = if let Some(explicit) = requested_team_name(input) {
        let team = load_team(&explicit).await?;
        peer_entries(&team)
    } else {
        let teams = all_team_names().await?;
        if teams.is_empty() {
            Vec::new()
        } else if teams.len() == 1 {
            let team = load_team(&teams[0]).await?;
            peer_entries(&team)
        } else {
            let mut all_peers = Vec::new();
            for team_name in teams {
                let team = load_team(&team_name).await?;
                all_peers.extend(peer_entries(&team));
            }
            all_peers
        }
    };

    if peers.is_empty() {
        Ok(json!({
            "peers": [],
            "count": 0,
            "message": "No peers registered in current context. Use team_create to create a team."
        })
        .to_string())
    } else {
        let count = peers.len();
        Ok(json!({
            "peers": peers,
            "count": count,
        })
        .to_string())
    }
}
