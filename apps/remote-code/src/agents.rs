use std::collections::BTreeMap;

use anyhow::{Result, anyhow};
use rc_agents::{AgentIdentity, AgentScheduler, AgentTask};
use rc_config::RuntimeConfig;

use crate::cli::{AgentsCommand, AgentsPlanArgs};

pub(crate) fn parse_agent_spec(spec: &str) -> Result<AgentIdentity> {
    let mut segments = spec.splitn(4, ';').map(str::trim);
    let name = segments
        .next()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("invalid --agent spec `{spec}`; expected name;role;paths;labels"))?;
    let role = segments
        .next()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("invalid --agent spec `{spec}`; role is missing"))?;
    let mut agent = AgentIdentity::new(name, role);
    agent.ownership_paths = segments.next().map(parse_csv_list).unwrap_or_default();
    agent.labels = segments
        .next()
        .map(parse_key_value_pairs)
        .unwrap_or_default();
    Ok(agent)
}

pub(crate) fn parse_task_spec(spec: &str) -> Result<AgentTask> {
    let mut segments = spec.splitn(4, ';').map(str::trim);
    let title = segments
        .next()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            anyhow!("invalid --task spec `{spec}`; expected title;paths;labels;description")
        })?;
    let mut task = AgentTask::new(title);
    task.ownership_paths = segments.next().map(parse_csv_list).unwrap_or_default();
    task.required_labels = segments
        .next()
        .map(parse_key_value_pairs)
        .unwrap_or_default();
    segments
        .next()
        .unwrap_or_default()
        .clone_into(&mut task.description);
    task.budget.read_calls = 32;
    task.budget.edit_calls = 12;
    task.budget.command_calls = 8;
    Ok(task)
}

pub(crate) fn default_agent_specs() -> Vec<AgentIdentity> {
    vec![
        parse_agent_spec("planner;planner;;phase=plan").unwrap_or_else(|_| {
            let mut agent = AgentIdentity::new("planner", "planner");
            agent.labels.insert("phase".to_owned(), "plan".to_owned());
            agent
        }),
        parse_agent_spec("runtime;implementer;apps/remote-code,crates/rc-session,crates/rc-tools;phase=local")
            .unwrap_or_else(|_| AgentIdentity::new("runtime", "implementer")),
        parse_agent_spec(
            "remote;implementer;apps/remote-code-runner,apps/remote-code-control-plane,crates/rc-runner,crates/rc-control-plane;phase=remote",
        )
        .unwrap_or_else(|_| AgentIdentity::new("remote", "implementer")),
        parse_agent_spec("review;reviewer;.;phase=review")
            .unwrap_or_else(|_| AgentIdentity::new("review", "reviewer")),
    ]
}

pub(crate) fn default_task_for_objective(objective: &str, config: &RuntimeConfig) -> AgentTask {
    let mut task = AgentTask::new(objective);
    task.description = format!(
        "Coordinate work for {} in {}",
        objective,
        config.cwd.display()
    );
    task.ownership_paths = vec![config.cwd.display().to_string()];
    task.budget.read_calls = 64;
    task.budget.edit_calls = 16;
    task.budget.command_calls = 12;
    task
}

fn parse_csv_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn parse_key_value_pairs(value: &str) -> BTreeMap<String, String> {
    value
        .split(',')
        .filter_map(|entry| {
            let (key, value) = entry.split_once('=')?;
            let key = key.trim();
            let value = value.trim();
            if key.is_empty() || value.is_empty() {
                None
            } else {
                Some((key.to_owned(), value.to_owned()))
            }
        })
        .collect()
}

pub(crate) fn run_agents(config: &RuntimeConfig, command: AgentsCommand) -> Result<()> {
    match command {
        AgentsCommand::Plan(args) => run_agents_plan(config, &args),
    }
}

pub(crate) fn run_agents_plan(config: &RuntimeConfig, args: &AgentsPlanArgs) -> Result<()> {
    let mut scheduler = AgentScheduler::new(args.lead.clone(), args.objective.clone());
    let agents = if args.agents.is_empty() {
        default_agent_specs()
    } else {
        args.agents
            .iter()
            .map(|spec| parse_agent_spec(spec))
            .collect::<Result<Vec<_>>>()?
    };
    for agent in agents {
        scheduler.register_agent(agent);
    }

    let tasks = if args.tasks.is_empty() {
        vec![default_task_for_objective(&args.objective, config)]
    } else {
        args.tasks
            .iter()
            .map(|spec| parse_task_spec(spec))
            .collect::<Result<Vec<_>>>()?
    };
    for task in tasks {
        scheduler.add_task(task);
    }

    while let Some((task_id, agent_id)) = scheduler.assign_next_task() {
        let agent = scheduler
            .agents()
            .into_iter()
            .find(|agent| agent.agent_id == agent_id)
            .ok_or_else(|| anyhow!("assigned agent {agent_id} was not found"))?;
        let task = scheduler
            .tasks()
            .into_iter()
            .find(|task| task.id == task_id)
            .ok_or_else(|| anyhow!("assigned task {task_id} was not found"))?;
        let _ = scheduler.queue_instruction(
            agent_id,
            args.lead.clone(),
            format!("Task: {}", task.title),
            format!(
                "Objective: {}\nTask: {}\nOwnership: {}",
                args.objective,
                task.title,
                if task.ownership_paths.is_empty() {
                    "(unscoped)".to_owned()
                } else {
                    task.ownership_paths.join(", ")
                }
            ),
        );
        if args.json {
            continue;
        }
        println!(
            "Assigned `{}` -> {} ({})",
            task.title, agent.name, agent.role
        );
    }

    if args.json {
        println!("{}", serde_json::to_string_pretty(&scheduler.snapshot())?);
    } else {
        let summary = scheduler.summary();
        println!(
            "\nTeam {}: {} agent(s), {} task(s), {} pending message(s)",
            summary.team_id, summary.total_agents, summary.total_tasks, summary.pending_messages
        );
    }
    Ok(())
}
