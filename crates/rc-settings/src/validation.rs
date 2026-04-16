//! Settings validation.

use crate::types::Settings;

/// Validation result containing any warnings or errors.
#[derive(Debug, Default)]
pub struct ValidationResult {
    /// Validation errors (blocking).
    pub errors: Vec<String>,
    /// Validation warnings (non-blocking).
    pub warnings: Vec<String>,
}

impl ValidationResult {
    /// Check if there are any errors.
    #[must_use]
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Check if there are any warnings.
    #[must_use]
    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }

    /// Check if the result is valid (no errors).
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }
}

/// Validate settings and return any errors or warnings.
pub fn validate_settings(settings: &Settings) -> ValidationResult {
    let mut result = ValidationResult::default();

    // Validate feedback survey rate
    if let Some(rate) = settings.feedback_survey_rate
        && (!(0.0..=1.0).contains(&rate)) {
            result.errors.push(format!(
                "feedbackSurveyRate must be between 0 and 1, got {rate}"
            ));
        }

    // Validate cleanup period days
    if let Some(days) = settings.cleanup_period_days
        && days > 365 {
            result.warnings.push(format!(
                "cleanupPeriodDays of {days} is very large (> 1 year)"
            ));
        }

    // Validate auto updates channel
    if let Some(channel) = &settings.auto_updates_channel
        && channel != "latest" && channel != "stable" {
            result.errors.push(format!(
                "autoUpdatesChannel must be 'latest' or 'stable', got '{channel}'"
            ));
        }

    // Validate default shell
    if let Some(shell) = &settings.default_shell
        && shell != "bash" && shell != "powershell" {
            result.errors.push(format!(
                "defaultShell must be 'bash' or 'powershell', got '{shell}'"
            ));
        }

    // Validate force login method
    if let Some(method) = &settings.force_login_method
        && method != "claudeai" && method != "console" {
            result.errors.push(format!(
                "forceLoginMethod must be 'claudeai' or 'console', got '{method}'"
            ));
        }

    // Validate effort level
    if let Some(level) = &settings.effort_level {
        match level.as_str() {
            "low" | "medium" | "high" | "max" => {}
            _ => {
                result.errors.push(format!(
                    "effortLevel must be 'low', 'medium', 'high', or 'max', got '{level}'"
                ));
            }
        }
    }

    // Warn about deprecated fields
    if settings.include_co_authored_by.is_some() {
        result.warnings.push(
            "includeCoAuthoredBy is deprecated; use attribution instead".to_string(),
        );
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_empty_settings() {
        let settings = Settings::new();
        let result = validate_settings(&settings);
        assert!(result.is_valid());
        assert!(!result.has_warnings());
    }

    #[test]
    fn valid_model_setting() {
        let mut settings = Settings::new();
        settings.model = Some("claude-opus-4".to_string());
        let result = validate_settings(&settings);
        assert!(result.is_valid());
    }

    #[test]
    fn invalid_feedback_rate() {
        let mut settings = Settings::new();
        settings.feedback_survey_rate = Some(1.5);
        let result = validate_settings(&settings);
        assert!(result.has_errors());
        assert!(result.errors[0].contains("feedbackSurveyRate"));
    }

    #[test]
    fn valid_feedback_rate() {
        let mut settings = Settings::new();
        settings.feedback_survey_rate = Some(0.5);
        let result = validate_settings(&settings);
        assert!(result.is_valid());
    }

    #[test]
    fn invalid_auto_updates_channel() {
        let mut settings = Settings::new();
        settings.auto_updates_channel = Some("beta".to_string());
        let result = validate_settings(&settings);
        assert!(result.has_errors());
    }

    #[test]
    fn valid_auto_updates_channel() {
        let mut settings = Settings::new();
        settings.auto_updates_channel = Some("stable".to_string());
        let result = validate_settings(&settings);
        assert!(result.is_valid());
    }

    #[test]
    fn invalid_default_shell() {
        let mut settings = Settings::new();
        settings.default_shell = Some("zsh".to_string());
        let result = validate_settings(&settings);
        assert!(result.has_errors());
    }

    #[test]
    fn valid_effort_levels() {
        for level in &["low", "medium", "high", "max"] {
            let mut settings = Settings::new();
            settings.effort_level = Some(level.to_string());
            let result = validate_settings(&settings);
            assert!(result.is_valid(), "effort level '{level}' should be valid");
        }
    }

    #[test]
    fn invalid_effort_level() {
        let mut settings = Settings::new();
        settings.effort_level = Some("extreme".to_string());
        let result = validate_settings(&settings);
        assert!(result.has_errors());
    }

    #[test]
    fn deprecated_field_warning() {
        let mut settings = Settings::new();
        settings.include_co_authored_by = Some(true);
        let result = validate_settings(&settings);
        assert!(result.has_warnings());
        assert!(result.warnings[0].contains("deprecated"));
    }

    #[test]
    fn large_cleanup_days_warning() {
        let mut settings = Settings::new();
        settings.cleanup_period_days = Some(500);
        let result = validate_settings(&settings);
        assert!(result.has_warnings());
    }
}
