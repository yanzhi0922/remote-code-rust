//! switch_mode tool implementation.

use crate::helpers::*;
use crate::types::*;
use roo_types::tool::SwitchModeParams;

/// Validate switch_mode parameters.
pub fn validate_switch_mode_params(
    params: &SwitchModeParams,
    current_mode: &str,
) -> Result<ModeSwitchResult, ModeToolError> {
    validate_mode_slug(&params.mode_slug)?;

    let same = is_same_mode(&params.mode_slug, current_mode);

    Ok(ModeSwitchResult {
        mode_slug: params.mode_slug.clone(),
        reason: params.reason.clone(),
        is_same_mode: same,
    })
}

/// Process a switch_mode request.
pub fn process_switch_mode(
    params: &SwitchModeParams,
    current_mode: &str,
) -> Result<ModeSwitchResult, ModeToolError> {
    let result = validate_switch_mode_params(params, current_mode)?;

    if result.is_same_mode {
        return Err(ModeToolError::SameMode(format!(
            "Already in '{}' mode",
            params.mode_slug
        )));
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_valid_switch() {
        let params = SwitchModeParams {
            mode_slug: "code".to_string(),
            reason: Some("need to code".to_string()),
        };
        let result = validate_switch_mode_params(&params, "ask").unwrap();
        assert_eq!(result.mode_slug, "code");
        assert!(!result.is_same_mode);
    }

    #[test]
    fn test_validate_same_mode() {
        let params = SwitchModeParams {
            mode_slug: "code".to_string(),
            reason: None,
        };
        let result = validate_switch_mode_params(&params, "code").unwrap();
        assert!(result.is_same_mode);
    }

    #[test]
    fn test_validate_invalid_mode() {
        let params = SwitchModeParams {
            mode_slug: "invalid".to_string(),
            reason: None,
        };
        assert!(validate_switch_mode_params(&params, "ask").is_err());
    }

    #[test]
    fn test_process_switch_success() {
        let params = SwitchModeParams {
            mode_slug: "architect".to_string(),
            reason: Some("plan needed".to_string()),
        };
        let result = process_switch_mode(&params, "code").unwrap();
        assert_eq!(result.mode_slug, "architect");
    }

    #[test]
    fn test_process_switch_same_mode() {
        let params = SwitchModeParams {
            mode_slug: "code".to_string(),
            reason: None,
        };
        assert!(process_switch_mode(&params, "code").is_err());
    }
}
