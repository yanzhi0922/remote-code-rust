use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use rc_config::ProviderConfig;
use rc_core::{ConversationEntry, ProviderResponse, SubAgentCompletion};

use crate::{ProviderClient, StreamingCallbacks};

#[async_trait]
pub trait ConversationBackend: Send + Sync {
    async fn complete(&self, conversation: &[ConversationEntry]) -> Result<ProviderResponse>;

    async fn complete_streaming(
        &self,
        conversation: &[ConversationEntry],
        callbacks: Option<StreamingCallbacks>,
    ) -> Result<ProviderResponse>;

    fn sub_agent_completion(&self) -> Arc<dyn SubAgentCompletion>;
}

#[derive(Clone)]
pub struct ProviderCompatBackend {
    client: Arc<ProviderClient>,
    provider: ProviderConfig,
}

impl ProviderCompatBackend {
    #[must_use]
    pub fn new(client: Arc<ProviderClient>, provider: &ProviderConfig) -> Self {
        Self {
            client,
            provider: provider.clone(),
        }
    }
}

struct ProviderSubAgentCompletion {
    client: Arc<ProviderClient>,
    provider: ProviderConfig,
}

impl ProviderSubAgentCompletion {
    fn new(client: Arc<ProviderClient>, provider: &ProviderConfig) -> Self {
        Self {
            client,
            provider: provider.clone(),
        }
    }
}

#[async_trait]
impl SubAgentCompletion for ProviderSubAgentCompletion {
    async fn complete(
        &self,
        conversation: &[ConversationEntry],
    ) -> anyhow::Result<ProviderResponse> {
        self.client.complete(&self.provider, conversation).await
    }
}

#[async_trait]
impl ConversationBackend for ProviderCompatBackend {
    async fn complete(&self, conversation: &[ConversationEntry]) -> Result<ProviderResponse> {
        self.client.complete(&self.provider, conversation).await
    }

    async fn complete_streaming(
        &self,
        conversation: &[ConversationEntry],
        callbacks: Option<StreamingCallbacks>,
    ) -> Result<ProviderResponse> {
        self.client
            .complete_streaming_with_callbacks(&self.provider, conversation, callbacks)
            .await
    }

    fn sub_agent_completion(&self) -> Arc<dyn SubAgentCompletion> {
        Arc::new(ProviderSubAgentCompletion::new(
            Arc::clone(&self.client),
            &self.provider,
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use rc_config::ProviderConfig;
    use rc_core::{ConversationEntry, ProviderProtocol};

    use super::{ConversationBackend, ProviderCompatBackend};
    use crate::ProviderClient;

    fn mock_provider() -> ProviderConfig {
        ProviderConfig {
            name: "mock".to_owned(),
            base_url: Some("mock://provider".to_owned()),
            api_key: Some("mock".to_owned()),
            model: Some("mock-model".to_owned()),
            protocol: ProviderProtocol::Anthropic,
            timeout_ms: 30_000,
            max_output_tokens: 4_096,
            max_retries: 0,
            retry_initial_backoff_ms: 100,
            retry_max_backoff_ms: 1_000,
            respect_retry_after: true,
            request_header_overrides: BTreeMap::new(),
            request_metadata: BTreeMap::new(),
            thinking_budget: None,
        }
    }

    #[tokio::test]
    async fn provider_compat_backend_delegates_complete() {
        let backend = ProviderCompatBackend::new(
            Arc::new(ProviderClient::new().expect("provider client")),
            &mock_provider(),
        );

        let response = backend
            .complete(&[ConversationEntry::user("hello from backend")])
            .await
            .expect("mock complete should work");

        assert!(response.text.contains("hello from backend"));
    }

    #[tokio::test]
    async fn provider_compat_backend_exposes_sub_agent_completion() {
        let backend = ProviderCompatBackend::new(
            Arc::new(ProviderClient::new().expect("provider client")),
            &mock_provider(),
        );

        let completion = backend.sub_agent_completion();
        let response = completion
            .complete(&[ConversationEntry::user("subagent call")])
            .await
            .expect("mock subagent completion should work");

        assert!(response.text.contains("subagent call"));
    }
}
