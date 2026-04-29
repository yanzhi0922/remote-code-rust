use crate::types::{ComplianceResult, MdmConfig, MdmPlatform};

/// Service for managing MDM (Mobile Device Management) configuration and compliance.
pub struct MdmService {
    config: Option<MdmConfig>,
    #[allow(dead_code)]
    platform: MdmPlatform,
}

impl MdmService {
    /// Create a new MDM service for the given platform with no configuration loaded.
    pub fn new(platform: MdmPlatform) -> Self {
        Self {
            config: None,
            platform,
        }
    }

    /// Load an MDM configuration, replacing any existing configuration.
    pub fn load_config(&mut self, config: MdmConfig) {
        self.config = Some(config);
    }

    /// Check compliance against the loaded MDM configuration.
    ///
    /// If no configuration is loaded, the service is considered compliant.
    pub fn is_compliant(&self) -> ComplianceResult {
        let Some(ref config) = self.config else {
            return ComplianceResult {
                is_compliant: true,
                violations: vec![],
            };
        };

        let mut violations = Vec::new();

        if config.allowed_providers.as_ref().is_some_and(|p| p.is_empty()) {
            violations.push("No providers are allowed".to_string());
        }

        if config
            .require_approval
            .is_some_and(|req| req && config.allowed_providers.is_none())
        {
            violations.push("Approval required but no providers configured".to_string());
        }

        ComplianceResult {
            is_compliant: violations.is_empty(),
            violations,
        }
    }

    /// Check whether a specific provider is allowed under the current MDM policy.
    ///
    /// Returns `true` if no configuration is loaded or if `allowed_providers` is `None`.
    pub fn is_provider_allowed(&self, provider: &str) -> bool {
        self.config
            .as_ref()
            .and_then(|c| c.allowed_providers.as_ref())
            .map_or(true, |providers| {
                providers
                    .iter()
                    .any(|p| p.eq_ignore_ascii_case(provider))
            })
    }

    /// Check whether a specific tool is allowed under the current MDM policy.
    ///
    /// Returns `true` if no configuration is loaded or if the tool is not in the blocked list.
    pub fn is_tool_allowed(&self, tool: &str) -> bool {
        self.config
            .as_ref()
            .and_then(|c| c.blocked_tools.as_ref())
            .map_or(true, |blocked| {
                !blocked.iter().any(|t| t.eq_ignore_ascii_case(tool))
            })
    }

    /// Check whether the MDM policy requires approval for actions.
    ///
    /// Returns `false` if no configuration is loaded.
    pub fn requires_approval(&self) -> bool {
        self.config
            .as_ref()
            .and_then(|c| c.require_approval)
            .unwrap_or(false)
    }

    /// Get custom instructions from the MDM policy, if any.
    pub fn get_custom_instructions(&self) -> Option<&str> {
        self.config
            .as_ref()
            .and_then(|c| c.custom_instructions.as_deref())
    }

    /// Get the platform-specific configuration file path.
    pub fn get_config_path(platform: MdmPlatform) -> String {
        match platform {
            MdmPlatform::Windows => r"C:\ProgramData\Roo\mdm.json".to_string(),
            MdmPlatform::Macos => "/Library/Application Support/Roo/mdm.json".to_string(),
            MdmPlatform::Linux => "/etc/roo/mdm.json".to_string(),
        }
    }

    /// Clear the loaded MDM configuration.
    pub fn clear_config(&mut self) {
        self.config = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_config() -> MdmConfig {
        MdmConfig {
            allowed_providers: Some(vec!["openai".to_string(), "anthropic".to_string()]),
            blocked_tools: Some(vec!["exec".to_string(), "shell".to_string()]),
            require_approval: Some(true),
            custom_instructions: Some("Enterprise policy".to_string()),
        }
    }

    #[test]
    fn test_new_service_has_no_config() {
        let svc = MdmService::new(MdmPlatform::Linux);
        assert!(svc.get_custom_instructions().is_none());
        assert!(!svc.requires_approval());
    }

    #[test]
    fn test_load_config() {
        let mut svc = MdmService::new(MdmPlatform::Windows);
        svc.load_config(sample_config());
        assert_eq!(
            Some("Enterprise policy"),
            svc.get_custom_instructions()
        );
    }

    #[test]
    fn test_clear_config() {
        let mut svc = MdmService::new(MdmPlatform::Linux);
        svc.load_config(sample_config());
        svc.clear_config();
        assert!(svc.get_custom_instructions().is_none());
        assert!(!svc.requires_approval());
    }

    #[test]
    fn test_is_provider_allowed_with_config() {
        let mut svc = MdmService::new(MdmPlatform::Macos);
        svc.load_config(sample_config());

        assert!(svc.is_provider_allowed("openai"));
        assert!(svc.is_provider_allowed("anthropic"));
        assert!(!svc.is_provider_allowed("ollama"));
    }

    #[test]
    fn test_is_provider_allowed_without_config() {
        let svc = MdmService::new(MdmPlatform::Linux);
        assert!(svc.is_provider_allowed("anything"));
    }

    #[test]
    fn test_is_provider_allowed_case_insensitive() {
        let mut svc = MdmService::new(MdmPlatform::Linux);
        svc.load_config(sample_config());
        assert!(svc.is_provider_allowed("OpenAI"));
        assert!(svc.is_provider_allowed("ANTHROPIC"));
    }

    #[test]
    fn test_is_provider_allowed_none_list() {
        let mut svc = MdmService::new(MdmPlatform::Linux);
        svc.load_config(MdmConfig {
            allowed_providers: None,
            ..Default::default()
        });
        assert!(svc.is_provider_allowed("anything"));
    }

    #[test]
    fn test_is_tool_allowed_with_config() {
        let mut svc = MdmService::new(MdmPlatform::Windows);
        svc.load_config(sample_config());

        assert!(!svc.is_tool_allowed("exec"));
        assert!(!svc.is_tool_allowed("shell"));
        assert!(svc.is_tool_allowed("read_file"));
    }

    #[test]
    fn test_is_tool_allowed_without_config() {
        let svc = MdmService::new(MdmPlatform::Linux);
        assert!(svc.is_tool_allowed("anything"));
    }

    #[test]
    fn test_is_tool_allowed_case_insensitive() {
        let mut svc = MdmService::new(MdmPlatform::Linux);
        svc.load_config(sample_config());
        assert!(!svc.is_tool_allowed("Exec"));
        assert!(!svc.is_tool_allowed("SHELL"));
    }

    #[test]
    fn test_requires_approval() {
        let mut svc = MdmService::new(MdmPlatform::Linux);
        assert!(!svc.requires_approval());

        svc.load_config(sample_config());
        assert!(svc.requires_approval());
    }

    #[test]
    fn test_requires_approval_false_when_set() {
        let mut svc = MdmService::new(MdmPlatform::Linux);
        svc.load_config(MdmConfig {
            require_approval: Some(false),
            ..Default::default()
        });
        assert!(!svc.requires_approval());
    }

    #[test]
    fn test_get_custom_instructions() {
        let mut svc = MdmService::new(MdmPlatform::Linux);
        svc.load_config(sample_config());
        assert_eq!(Some("Enterprise policy"), svc.get_custom_instructions());
    }

    #[test]
    fn test_get_custom_instructions_none() {
        let svc = MdmService::new(MdmPlatform::Linux);
        assert!(svc.get_custom_instructions().is_none());
    }

    #[test]
    fn test_config_path_windows() {
        assert_eq!(
            r"C:\ProgramData\Roo\mdm.json",
            MdmService::get_config_path(MdmPlatform::Windows)
        );
    }

    #[test]
    fn test_config_path_macos() {
        assert_eq!(
            "/Library/Application Support/Roo/mdm.json",
            MdmService::get_config_path(MdmPlatform::Macos)
        );
    }

    #[test]
    fn test_config_path_linux() {
        assert_eq!(
            "/etc/roo/mdm.json",
            MdmService::get_config_path(MdmPlatform::Linux)
        );
    }

    #[test]
    fn test_is_compliant_no_config() {
        let svc = MdmService::new(MdmPlatform::Linux);
        let result = svc.is_compliant();
        assert!(result.is_compliant);
        assert!(result.violations.is_empty());
    }

    #[test]
    fn test_is_compliant_with_valid_config() {
        let mut svc = MdmService::new(MdmPlatform::Linux);
        svc.load_config(sample_config());
        let result = svc.is_compliant();
        assert!(result.is_compliant);
    }

    #[test]
    fn test_is_compliant_empty_providers() {
        let mut svc = MdmService::new(MdmPlatform::Linux);
        svc.load_config(MdmConfig {
            allowed_providers: Some(vec![]),
            blocked_tools: None,
            require_approval: None,
            custom_instructions: None,
        });
        let result = svc.is_compliant();
        assert!(!result.is_compliant);
        assert!(result
            .violations
            .iter()
            .any(|v| v.contains("No providers")));
    }
}
