/// Cloud API client for making authenticated requests to the Roo Code cloud.
/// Mirrors packages/cloud/src/CloudAPI.ts

use crate::config::get_roo_code_api_url;
use crate::types::CloudError;
use crate::utils::get_user_agent;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Share visibility levels.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShareVisibility {
    Organization,
    Public,
}

/// Response from sharing a task.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareResponse {
    pub success: bool,
    pub share_url: Option<String>,
    pub task_id: Option<String>,
}

/// Bridge configuration response.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeConfig {
    pub user_id: String,
    pub socket_bridge_url: String,
    pub token: String,
}

/// Cloud API client for making authenticated requests.
pub struct CloudApi {
    base_url: String,
    version: String,
}

impl CloudApi {
    /// Create a new CloudAPI client.
    pub fn new(version: Option<&str>) -> Self {
        Self {
            base_url: get_roo_code_api_url(),
            version: version.unwrap_or("unknown").to_string(),
        }
    }

    /// Make an authenticated GET request.
    pub async fn get(&self, endpoint: &str, token: &str) -> Result<Value, CloudError> {
        self.request(endpoint, "GET", None, token).await
    }

    /// Make an authenticated POST request.
    pub async fn post(
        &self,
        endpoint: &str,
        body: Option<Value>,
        token: &str,
    ) -> Result<Value, CloudError> {
        self.request(endpoint, "POST", body, token).await
    }

    /// Share a task with the specified visibility.
    pub async fn share_task(
        &self,
        task_id: &str,
        visibility: ShareVisibility,
        token: &str,
    ) -> Result<ShareResponse, CloudError> {
        let body = serde_json::json!({
            "taskId": task_id,
            "visibility": visibility,
        });

        let response = self.post("/api/extension/share", Some(body), token).await?;

        let share_response: ShareResponse =
            serde_json::from_value(response).map_err(|e| CloudError::SerializationError(e.to_string()))?;

        Ok(share_response)
    }

    /// Get bridge configuration.
    pub async fn bridge_config(&self, token: &str) -> Result<BridgeConfig, CloudError> {
        let response = self.get("/api/extension/bridge/config", token).await?;

        let config: BridgeConfig =
            serde_json::from_value(response).map_err(|e| CloudError::SerializationError(e.to_string()))?;

        Ok(config)
    }

    /// Get the credit balance for the authenticated user.
    pub async fn credit_balance(&self, token: &str) -> Result<f64, CloudError> {
        let response = self.get("/api/extension/credit-balance", token).await?;

        let balance = response["balance"].as_f64().ok_or_else(|| {
            CloudError::SerializationError("Missing 'balance' field in response".to_string())
        })?;

        Ok(balance)
    }

    /// Core request method with authentication and error handling.
    async fn request(
        &self,
        endpoint: &str,
        method: &str,
        body: Option<Value>,
        token: &str,
    ) -> Result<Value, CloudError> {
        let url = format!("{}{}", self.base_url, endpoint);

        let client = reqwest::Client::new();
        let mut request = match method {
            "POST" => client.post(&url),
            "PUT" => client.put(&url),
            "DELETE" => client.delete(&url),
            _ => client.get(&url),
        };

        request = request
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", token))
            .header("User-Agent", get_user_agent(Some(&self.version)));

        if let Some(body) = body {
            request = request.json(&body);
        }

        let response = request.send().await.map_err(|e| {
            if e.is_connect() || e.is_timeout() {
                CloudError::NetworkError(format!("Network error while calling {}: {}", endpoint, e))
            } else {
                CloudError::NetworkError(format!("Request failed: {}", e))
            }
        })?;

        let status = response.status();
        if !status.is_success() {
            let body_text = response.text().await.unwrap_or_default();

            return match status.as_u16() {
                401 => Err(CloudError::NotAuthenticated),
                404 => {
                    if endpoint.contains("/share") {
                        Err(CloudError::TaskNotFound("Task not found".to_string()))
                    } else {
                        Err(CloudError::ApiError(format!(
                            "Resource not found: {}",
                            endpoint
                        ), 404, Some(body_text)))
                    }
                }
                _ => Err(CloudError::ApiError(
                    format!("HTTP {}: {}", status.as_u16(), status.canonical_reason().unwrap_or("Unknown")),
                    status.as_u16(),
                    Some(body_text),
                )),
            };
        }

        let data: Value = response.json().await.map_err(|e| {
            CloudError::SerializationError(format!("Failed to parse response: {}", e))
        })?;

        Ok(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cloud_api_new() {
        let api = CloudApi::new(Some("1.0.0"));
        assert!(!api.base_url.is_empty());
        assert_eq!("1.0.0", api.version);
    }

    #[test]
    fn test_share_visibility_serde() {
        let v = ShareVisibility::Organization;
        let json = serde_json::to_string(&v).unwrap();
        assert!(json.contains("organization"));

        let v2 = ShareVisibility::Public;
        let json2 = serde_json::to_string(&v2).unwrap();
        assert!(json2.contains("public"));
    }

    #[test]
    fn test_share_response_deserialization() {
        let json = r#"{"success": true, "shareUrl": "https://example.com/share/123", "taskId": "task-123"}"#;
        let resp: ShareResponse = serde_json::from_str(json).unwrap();
        assert!(resp.success);
        assert_eq!(Some("https://example.com/share/123".to_string()), resp.share_url);
        assert_eq!(Some("task-123".to_string()), resp.task_id);
    }

    #[test]
    fn test_bridge_config_deserialization() {
        let json = r#"{"userId": "u1", "socketBridgeUrl": "wss://example.com", "token": "tok123"}"#;
        let config: BridgeConfig = serde_json::from_str(json).unwrap();
        assert_eq!("u1", config.user_id);
        assert_eq!("wss://example.com", config.socket_bridge_url);
        assert_eq!("tok123", config.token);
    }
}
