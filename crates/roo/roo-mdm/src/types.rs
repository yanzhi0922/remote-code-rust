use serde::{Deserialize, Serialize};

/// MDM (Mobile Device Management) configuration pushed by enterprise policies.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MdmConfig {
    /// List of allowed AI providers. `None` means all providers are allowed.
    pub allowed_providers: Option<Vec<String>>,
    /// List of blocked tools. `None` means no tools are blocked.
    pub blocked_tools: Option<Vec<String>>,
    /// Whether actions require approval from an administrator.
    pub require_approval: Option<bool>,
    /// Custom instructions injected by MDM policy.
    pub custom_instructions: Option<String>,
}

/// Result of a compliance check against MDM policy.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ComplianceResult {
    pub is_compliant: bool,
    pub violations: Vec<String>,
}

/// Supported platforms for MDM configuration paths.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MdmPlatform {
    Windows,
    Macos,
    Linux,
}

/// Errors that can occur during MDM operations.
#[derive(Clone, Debug, thiserror::Error)]
pub enum MdmError {
    #[error("MDM configuration not loaded")]
    ConfigNotLoaded,

    #[error("failed to parse MDM config: {0}")]
    ConfigParseError(String),

    #[error("IO error: {0}")]
    IoError(String),
}

impl From<serde_json::Error> for MdmError {
    fn from(err: serde_json::Error) -> Self {
        MdmError::ConfigParseError(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mdm_config_serialization_roundtrip() {
        let config = MdmConfig {
            allowed_providers: Some(vec!["openai".to_string(), "anthropic".to_string()]),
            blocked_tools: Some(vec!["exec".to_string()]),
            require_approval: Some(true),
            custom_instructions: Some("Use only approved providers.".to_string()),
        };
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: MdmConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.allowed_providers, deserialized.allowed_providers);
        assert_eq!(config.blocked_tools, deserialized.blocked_tools);
        assert_eq!(config.require_approval, deserialized.require_approval);
        assert_eq!(
            config.custom_instructions,
            deserialized.custom_instructions
        );
    }

    #[test]
    fn test_mdm_config_none_fields() {
        let config = MdmConfig {
            allowed_providers: None,
            blocked_tools: None,
            require_approval: None,
            custom_instructions: None,
        };
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: MdmConfig = serde_json::from_str(&json).unwrap();
        assert!(deserialized.allowed_providers.is_none());
        assert!(deserialized.blocked_tools.is_none());
        assert!(deserialized.require_approval.is_none());
        assert!(deserialized.custom_instructions.is_none());
    }

    #[test]
    fn test_mdm_config_default() {
        let config = MdmConfig::default();
        assert!(config.allowed_providers.is_none());
        assert!(config.blocked_tools.is_none());
        assert!(config.require_approval.is_none());
        assert!(config.custom_instructions.is_none());
    }

    #[test]
    fn test_compliance_result_serialization() {
        let result = ComplianceResult {
            is_compliant: true,
            violations: vec![],
        };
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: ComplianceResult = serde_json::from_str(&json).unwrap();
        assert!(deserialized.is_compliant);
        assert!(deserialized.violations.is_empty());
    }

    #[test]
    fn test_compliance_result_with_violations() {
        let result = ComplianceResult {
            is_compliant: false,
            violations: vec!["violation 1".to_string(), "violation 2".to_string()],
        };
        assert!(!result.is_compliant);
        assert_eq!(2, result.violations.len());
    }

    #[test]
    fn test_mdm_platform_serialization() {
        let platforms = vec![
            MdmPlatform::Windows,
            MdmPlatform::Macos,
            MdmPlatform::Linux,
        ];
        for p in platforms {
            let json = serde_json::to_string(&p).unwrap();
            let deserialized: MdmPlatform = serde_json::from_str(&json).unwrap();
            assert_eq!(p, deserialized);
        }
    }

    #[test]
    fn test_mdm_error_display() {
        let err = MdmError::ConfigNotLoaded;
        assert_eq!(format!("{err}"), "MDM configuration not loaded");

        let err = MdmError::ConfigParseError("bad json".to_string());
        assert_eq!(format!("{err}"), "failed to parse MDM config: bad json");
    }

    #[test]
    fn test_mdm_error_from_serde_json() {
        let result: Result<serde_json::Value, serde_json::Error> =
            serde_json::from_str("invalid json");
        if let Err(json_err) = result {
            let mdm_err: MdmError = MdmError::from(json_err);
            match mdm_err {
                MdmError::ConfigParseError(_) => {}
                _ => panic!("expected ConfigParseError"),
            }
        }
    }
}
