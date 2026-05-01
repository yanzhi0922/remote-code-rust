//! Objective section.
//!
//! Source: `src/core/prompts/sections/objective.ts`

/// Returns the objective section.
///
/// Source: `src/core/prompts/sections/objective.ts` — `getObjectiveSection`
pub fn get_objective_section() -> &'static str {
    r#"====

OBJECTIVE

You accomplish a given task iteratively, breaking it down into clear steps and working through them methodically.

1. Analyze the user's task and set clear, achievable goals to accomplish it. Prioritize these goals in a logical order.
2. Work through these goals sequentially, utilizing available tools one at a time as necessary. Each goal should correspond to a distinct step in your problem-solving process. You will be informed on the work completed and what's remaining as you go.
3. Remember, you have extensive capabilities with access to a wide range of tools that can be used in powerful and clever ways as necessary to accomplish each goal. Before calling a tool, do some analysis. First, analyze the file structure provided in environment_details to gain context and insights for proceeding effectively. Next, think about which of the provided tools is the most relevant tool to accomplish the user's task. Go through each of the required parameters of the relevant tool and determine if the user has directly provided or given enough information to infer a value. When deciding if the parameter can be inferred, carefully consider all the context to see if it supports a specific value. If all of the required parameters are present or can be reasonably inferred, proceed with the tool use. BUT, if one of the values for a required parameter is missing, DO NOT invoke the tool (not even with fillers for the missing params) and instead, ask the user to provide the missing parameters using the ask_followup_question tool. DO NOT ask for more information on optional parameters if it is not provided.
4. Once you've completed the user's task, you must use the attempt_completion tool to present the result of the task to the user.
5. The user may provide feedback, which you can use to make improvements and try again. But DO NOT continue in pointless back and forth conversations, i.e. don't end your responses with questions or offers for further assistance."#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_objective_section() {
        let result = get_objective_section();
        assert!(result.starts_with("====\n\nOBJECTIVE"));
        assert!(result.contains("breaking it down into clear steps"));
        assert!(result.contains("working through them methodically"));
    }

    #[test]
    fn test_get_objective_section_all_steps() {
        let result = get_objective_section();
        assert!(result.contains("1. Analyze the user's task"));
        assert!(result.contains("2. Work through these goals sequentially"));
        assert!(result.contains("3. Remember, you have extensive capabilities"));
        assert!(result.contains("4. Once you've completed the user's task"));
        assert!(result.contains("5. The user may provide feedback"));
    }

    #[test]
    fn test_get_objective_section_key_phrases() {
        let result = get_objective_section();
        // Key phrases from TS source
        assert!(result.contains("ask_followup_question tool"));
        assert!(result.contains("attempt_completion tool"));
        assert!(result.contains("DO NOT invoke the tool"));
        assert!(result.contains("not even with fillers"));
        assert!(result.contains("DO NOT ask for more information on optional parameters"));
        assert!(result.contains("pointless back and forth conversations"));
    }
}
