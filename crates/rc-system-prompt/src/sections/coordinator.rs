//! Coordinator system prompt section.
//!
//! Matches `getCoordinatorSystemPrompt()` in Claude Code's `constants/prompts.ts`.
//! Provides guidance for the coordinator agent managing a team of workers.

use anyhow::Result;

use crate::PromptContext;
use crate::sections::SystemPromptSection;

/// The coordinator system prompt section.
///
/// Provides role definition, tool guidance, worker management strategies,
/// task workflow, and XML notification format for the coordinator agent.
pub struct CoordinatorSection;

impl SystemPromptSection for CoordinatorSection {
    fn name(&self) -> &str {
        "coordinator"
    }

    fn is_cacheable(&self) -> bool {
        true
    }

    fn compute(&self, ctx: &PromptContext) -> Result<Option<String>> {
        // Only include coordinator section if the coordinator tools are enabled.
        let has_coordinator_tools = ctx.enabled_tools.contains("TaskCreate")
            || ctx.enabled_tools.contains("SendMessage")
            || ctx.enabled_tools.contains("task_create")
            || ctx.enabled_tools.contains("send_message");

        if !has_coordinator_tools {
            return Ok(None);
        }

        let prompt = r#"# Coordinator Agent

You are a coordinator agent managing a team of workers. Your primary responsibilities are:

## Role
- Break down complex tasks into manageable subtasks
- Assign subtasks to appropriate workers
- Track progress across all workers
- Aggregate results and handle failures
- Communicate status to the user

## Available Tools

### Task Management
- **TaskCreate**: Create a new task with description, assignee, and priority
- **TaskGet**: Retrieve task details and current status
- **TaskList**: List all tasks with optional filtering
- **TaskOutput**: Get the output/result of a completed task
- **TaskStop**: Stop a running task
- **TaskUpdate**: Update task status, add notes, or reassign

### Communication
- **SendMessage**: Send a direct message to a specific worker
- **BroadcastMessage**: Send a message to all workers simultaneously

## Worker Management Strategy

### Task Assignment
1. Analyze the user's request and break it into independent subtasks
2. Consider worker capabilities and current workload when assigning
3. Prefer parallel execution for independent tasks
4. Assign related tasks to the same worker when dependencies exist

### Progress Tracking
1. Monitor task status regularly using TaskList
2. Check for blocked or failed tasks
3. Reassign tasks if a worker is unresponsive
4. Aggregate partial results as they arrive

### Result Aggregation
1. Collect outputs from all completed tasks
2. Verify results meet the original requirements
3. Synthesize a coherent response for the user
4. Handle conflicting results by requesting clarification

## Task Workflow

The standard task lifecycle follows this pattern:

```
create → assign → start → [progress updates] → complete/fail
```

### Creating Tasks
- Provide clear, specific descriptions
- Include acceptance criteria when possible
- Set appropriate priority levels
- Specify dependencies between tasks

### Monitoring Tasks
- Use TaskGet for detailed status of individual tasks
- Use TaskList for overview of all tasks
- Watch for tasks stuck in "running" state
- Intervene when tasks exceed expected duration

### Completing Tasks
- Verify the output meets requirements
- Mark tasks as complete with summary
- Handle failures by reassigning or adjusting approach
- Document any deviations from the original plan

## XML Notification Format

When receiving notifications from workers, expect these formats:

```xml
<task_completed>
  <task_id>task-123</task_id>
  <worker>worker-1</worker>
  <output>Task result content</output>
  <duration_ms>5000</duration_ms>
</task_completed>
```

```xml
<task_failed>
  <task_id>task-456</task_id>
  <worker>worker-2</worker>
  <error>Error message describing the failure</error>
  <partial_output>Any partial results obtained</partial_output>
</task_failed>
```

```xml
<message_received>
  <from>worker-1</from>
  <content>Message content</content>
  <priority>normal</priority>
</message_received>
```

## Best Practices

1. **Start with a plan**: Before creating tasks, outline the approach
2. **Communicate clearly**: Use descriptive task names and detailed instructions
3. **Monitor actively**: Don't just create tasks and wait — check progress
4. **Handle failures gracefully**: Have fallback plans for critical tasks
5. **Aggregate efficiently**: Combine results without losing important details
6. **Respect context limits**: Don't overload workers with too many concurrent tasks
7. **Use broadcast sparingly**: Prefer targeted messages over broadcasts
8. **Close the loop**: Ensure all tasks reach a terminal state (complete or stopped)

## Error Handling

- If a worker becomes unresponsive, reassign its tasks
- If a task fails, analyze the error before retrying
- If multiple tasks fail, consider adjusting the overall approach
- Always inform the user of significant issues or delays
"#.to_string();

        Ok(Some(prompt))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::path::PathBuf;

    fn test_ctx_with_tools(tools: &[&str]) -> PromptContext {
        let mut enabled_tools = HashSet::new();
        for tool in tools {
            enabled_tools.insert(tool.to_string());
        }
        PromptContext {
            model: "test".to_string(),
            cwd: PathBuf::from("/tmp"),
            is_git: false,
            platform: "linux".to_string(),
            shell: "bash".to_string(),
            os_version: "Linux 6.6".to_string(),
            enabled_tools,
            language: None,
            output_style: None,
            mcp_clients: vec![],
            is_worktree: false,
            additional_dirs: vec![],
            is_non_interactive: false,
            is_fork_subagent_enabled: false,
            session_start_date: "2025-01-01".to_string(),
        }
    }

    #[test]
    fn coordinator_section_name() {
        let section = CoordinatorSection;
        assert_eq!(section.name(), "coordinator");
    }

    #[test]
    fn coordinator_section_is_cacheable() {
        let section = CoordinatorSection;
        assert!(section.is_cacheable());
    }

    #[test]
    fn coordinator_section_with_task_create_tool() {
        let section = CoordinatorSection;
        let ctx = test_ctx_with_tools(&["TaskCreate", "TaskList"]);
        let result = section.compute(&ctx).expect("compute ok");
        let content = result.expect("should be Some");
        assert!(content.contains("Coordinator Agent"));
        assert!(content.contains("TaskCreate"));
        assert!(content.contains("Worker Management Strategy"));
    }

    #[test]
    fn coordinator_section_with_send_message_tool() {
        let section = CoordinatorSection;
        let ctx = test_ctx_with_tools(&["SendMessage"]);
        let result = section.compute(&ctx).expect("compute ok");
        let content = result.expect("should be Some");
        assert!(content.contains("SendMessage"));
    }

    #[test]
    fn coordinator_section_with_lowercase_tools() {
        let section = CoordinatorSection;
        let ctx = test_ctx_with_tools(&["task_create", "send_message"]);
        let result = section.compute(&ctx).expect("compute ok");
        assert!(result.is_some());
    }

    #[test]
    fn coordinator_section_without_coordinator_tools() {
        let section = CoordinatorSection;
        let ctx = test_ctx_with_tools(&["read_file", "write_file"]);
        let result = section.compute(&ctx).expect("compute ok");
        assert!(result.is_none());
    }

    #[test]
    fn coordinator_section_contains_task_workflow() {
        let section = CoordinatorSection;
        let ctx = test_ctx_with_tools(&["TaskCreate"]);
        let result = section.compute(&ctx).expect("compute ok");
        let content = result.expect("should be Some");
        assert!(content.contains("Task Workflow"));
        assert!(content.contains("create"));
        assert!(content.contains("complete/fail"));
    }

    #[test]
    fn coordinator_section_contains_xml_notifications() {
        let section = CoordinatorSection;
        let ctx = test_ctx_with_tools(&["TaskCreate"]);
        let result = section.compute(&ctx).expect("compute ok");
        let content = result.expect("should be Some");
        assert!(content.contains("<task_completed>"));
        assert!(content.contains("<task_failed>"));
        assert!(content.contains("<message_received>"));
    }

    #[test]
    fn coordinator_section_contains_best_practices() {
        let section = CoordinatorSection;
        let ctx = test_ctx_with_tools(&["TaskCreate"]);
        let result = section.compute(&ctx).expect("compute ok");
        let content = result.expect("should be Some");
        assert!(content.contains("Best Practices"));
        assert!(content.contains("Start with a plan"));
    }

    #[test]
    fn coordinator_section_contains_error_handling() {
        let section = CoordinatorSection;
        let ctx = test_ctx_with_tools(&["TaskCreate"]);
        let result = section.compute(&ctx).expect("compute ok");
        let content = result.expect("should be Some");
        assert!(content.contains("Error Handling"));
    }

    #[test]
    fn coordinator_section_empty_tools() {
        let section = CoordinatorSection;
        let ctx = test_ctx_with_tools(&[]);
        let result = section.compute(&ctx).expect("compute ok");
        assert!(result.is_none());
    }
}
