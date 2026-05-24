use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use claude_core::ToolCall;
use claude_query_engine::config::{ProcessUserInputContext, ToolRunResult};

/// Headless tool runner for the server context.
///
/// Executes tools via `claude_tools::execute_tool_call()` with an auto-accept
/// permission broker — no interactive prompts since IM adapters are unattended.
pub struct ServerToolRunner {
    broker: Arc<dyn claude_permissions::PermissionBroker>,
}

impl ServerToolRunner {
    pub fn new() -> Self {
        let broker = Arc::new(claude_permissions::StaticPermissionBroker::new(true));
        Self { broker }
    }
}

#[async_trait]
impl claude_query_engine::config::ToolRunner for ServerToolRunner {
    async fn run_tool(
        &self,
        tool_call: &ToolCall,
        _context: &ProcessUserInputContext,
    ) -> Result<ToolRunResult> {
        let tool_context = claude_tools::ToolExecutionContext::default();

        let result =
            claude_tools::execute_tool_call(tool_call, &tool_context, self.broker.as_ref()).await?;

        Ok(ToolRunResult {
            result,
            pre_messages: Vec::new(),
            post_messages: Vec::new(),
            permission_denial: None,
            output_tokens_consumed: None,
        })
    }
}
