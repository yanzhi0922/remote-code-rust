//! ask_followup_question tool implementation.

use crate::types::*;
use roo_types::tool::AskFollowupQuestionParams;

/// Validate ask_followup_question parameters.
pub fn validate_followup_params(params: &AskFollowupQuestionParams) -> Result<(), MiscToolError> {
    if params.question.trim().is_empty() {
        return Err(MiscToolError::Validation(
            "question must not be empty".to_string(),
        ));
    }

    if params.follow_up.is_empty() {
        return Err(MiscToolError::Validation(
            "must provide at least one follow-up option".to_string(),
        ));
    }

    // Check each option has text
    for (i, opt) in params.follow_up.iter().enumerate() {
        if opt.text.trim().is_empty() {
            return Err(MiscToolError::Validation(format!(
                "follow-up option {} must have non-empty text",
                i + 1
            )));
        }
    }

    Ok(())
}

/// Process an ask_followup_question request.
pub fn process_followup(params: &AskFollowupQuestionParams) -> Result<FollowupResult, MiscToolError> {
    validate_followup_params(params)?;

    let suggestions: Vec<String> = params
        .follow_up
        .iter()
        .map(|opt| {
            if let Some(ref mode) = opt.mode {
                format!("{} (mode: {mode})", opt.text)
            } else {
                opt.text.clone()
            }
        })
        .collect();

    Ok(FollowupResult {
        question: params.question.clone(),
        suggestions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use roo_types::tool::FollowUpOption;

    #[test]
    fn test_validate_empty_question() {
        let params = AskFollowupQuestionParams {
            question: "".to_string(),
            follow_up: vec![FollowUpOption {
                text: "option".to_string(),
                mode: None,
            }],
        };
        assert!(validate_followup_params(&params).is_err());
    }

    #[test]
    fn test_validate_no_options() {
        let params = AskFollowupQuestionParams {
            question: "What?".to_string(),
            follow_up: vec![],
        };
        assert!(validate_followup_params(&params).is_err());
    }

    #[test]
    fn test_validate_empty_option_text() {
        let params = AskFollowupQuestionParams {
            question: "What?".to_string(),
            follow_up: vec![FollowUpOption {
                text: "".to_string(),
                mode: None,
            }],
        };
        assert!(validate_followup_params(&params).is_err());
    }

    #[test]
    fn test_validate_valid() {
        let params = AskFollowupQuestionParams {
            question: "Continue?".to_string(),
            follow_up: vec![
                FollowUpOption {
                    text: "Yes".to_string(),
                    mode: None,
                },
                FollowUpOption {
                    text: "No".to_string(),
                    mode: Some("code".to_string()),
                },
            ],
        };
        assert!(validate_followup_params(&params).is_ok());
    }

    #[test]
    fn test_process_followup() {
        let params = AskFollowupQuestionParams {
            question: "Next step?".to_string(),
            follow_up: vec![
                FollowUpOption {
                    text: "Implement".to_string(),
                    mode: Some("code".to_string()),
                },
                FollowUpOption {
                    text: "Plan".to_string(),
                    mode: None,
                },
            ],
        };
        let result = process_followup(&params).unwrap();
        assert_eq!(result.question, "Next step?");
        assert_eq!(result.suggestions.len(), 2);
        assert!(result.suggestions[0].contains("mode: code"));
    }
}
