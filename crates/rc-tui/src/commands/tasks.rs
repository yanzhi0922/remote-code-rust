use std::collections::HashMap;

use rc_tools::tasks::{BackgroundTask, task_snapshots};

pub fn render() {
    let tasks = task_snapshots();
    if tasks.is_empty() {
        println!("No tasks found.");
        return;
    }

    println!("Tasks:");

    let mut by_parent: HashMap<Option<String>, Vec<BackgroundTask>> = HashMap::new();
    for task in tasks {
        by_parent
            .entry(task.parent_task_id.clone())
            .or_default()
            .push(task);
    }

    let mut roots = by_parent.remove(&None).unwrap_or_default();
    roots.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
    for task in roots {
        render_task(&task, &by_parent);
    }
}

fn render_task(task: &BackgroundTask, by_parent: &HashMap<Option<String>, Vec<BackgroundTask>>) {
    let indent = "  ".repeat(task.depth as usize);
    let kind = task.kind.as_str();
    let summary = if task.summary.trim().is_empty() {
        String::new()
    } else {
        format!(" — {}", task.summary)
    };
    println!(
        "{indent}{}  {:<10} {:<11} {}{}",
        task.id,
        task.status.as_str(),
        kind,
        task.title,
        summary
    );
    if let Some(path) = &task.output_path {
        println!("{indent}    output: {path}");
    }

    let mut children = by_parent
        .get(&Some(task.id.clone()))
        .cloned()
        .unwrap_or_default();
    children.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
    for child in children {
        render_task(&child, by_parent);
    }
}
