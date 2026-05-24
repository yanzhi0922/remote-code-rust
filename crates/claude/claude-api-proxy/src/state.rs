use std::collections::HashMap;
use std::sync::Arc;

use crate::config::{ProviderEntry, ProxySettings};

#[derive(Clone)]
pub struct ProxyState {
    pub settings: ProxySettings,
    pub http: reqwest::Client,
    pub model_index: Arc<HashMap<String, ProviderEntry>>,
}

impl ProxyState {
    pub fn new(settings: ProxySettings, model_index: Arc<HashMap<String, ProviderEntry>>) -> Self {
        Self {
            settings,
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(300))
                .build()
                .expect("failed to build reqwest client"),
            model_index,
        }
    }
}
