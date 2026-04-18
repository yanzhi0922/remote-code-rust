use serde_json::Value;

use crate::{PermissionClass, PermissionRequest, classify_tool};

#[must_use]
pub fn rule_matches_request(pattern: &str, request: &PermissionRequest) -> bool {
    let (name_part, input_pattern) = split_pattern(pattern);
    if !name_matches(name_part, &request.tool_name) {
        return false;
    }
    let Some(input_pattern) = input_pattern else {
        return true;
    };

    if let Some(command) = extract_shell_command(&request.tool_input) {
        return wildcard_match(input_pattern, command);
    }
    wildcard_match_values(input_pattern, &request.tool_input)
}

fn split_pattern(pattern: &str) -> (&str, Option<&str>) {
    if let Some(open) = pattern.find('(') {
        let close = pattern.rfind(')').unwrap_or(pattern.len());
        let name = &pattern[..open];
        let sub = &pattern[open + 1..close];
        (name.trim(), Some(sub.trim()))
    } else {
        (pattern.trim(), None)
    }
}

fn name_matches(pattern: &str, tool_name: &str) -> bool {
    let normalized = pattern.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "bash" => tool_name.eq_ignore_ascii_case("bash_command"),
        "powershell" => tool_name.eq_ignore_ascii_case("powershell"),
        "read" => classify_tool(tool_name) == PermissionClass::Read,
        "edit" => classify_tool(tool_name) == PermissionClass::Edit,
        "command" => classify_tool(tool_name) == PermissionClass::Bash,
        _ => pattern.eq_ignore_ascii_case(tool_name),
    }
}

fn extract_shell_command(input: &Value) -> Option<&str> {
    input.get("command").and_then(Value::as_str)
}

fn wildcard_match_values(pattern: &str, value: &Value) -> bool {
    match value {
        Value::String(s) => wildcard_match(pattern, s),
        Value::Array(values) => values
            .iter()
            .any(|value| wildcard_match_values(pattern, value)),
        Value::Object(object) => object
            .values()
            .any(|value| wildcard_match_values(pattern, value)),
        _ => false,
    }
}

#[must_use]
pub fn wildcard_match(pattern: &str, text: &str) -> bool {
    let pattern = pattern.as_bytes();
    let text = text.as_bytes();
    let mut dp = vec![vec![false; text.len() + 1]; pattern.len() + 1];
    dp[0][0] = true;

    for i in 1..=pattern.len() {
        if pattern[i - 1] == b'*' {
            dp[i][0] = dp[i - 1][0];
        } else {
            break;
        }
    }

    for i in 1..=pattern.len() {
        for j in 1..=text.len() {
            if pattern[i - 1] == b'*' {
                dp[i][j] = dp[i - 1][j] || dp[i][j - 1];
            } else if pattern[i - 1] == b'?' || pattern[i - 1] == text[j - 1] {
                dp[i][j] = dp[i - 1][j - 1];
            }
        }
    }

    dp[pattern.len()][text.len()]
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::PermissionRequest;

    use super::rule_matches_request;

    fn request(tool_name: &str, input: serde_json::Value) -> PermissionRequest {
        PermissionRequest {
            tool_name: tool_name.to_owned(),
            permission_class: None,
            tool_input: input,
            working_directory: None,
            tool_use_id: None,
            title: None,
            description: None,
            blocked_path: None,
        }
    }

    #[test]
    fn read_class_rule_matches_read_tools() {
        assert!(rule_matches_request(
            "Read",
            &request("read_file", json!({"path":"a"}))
        ));
    }

    #[test]
    fn bash_alias_matches_shell_command_content() {
        assert!(rule_matches_request(
            "Bash(git *)",
            &request("bash_command", json!({"command":"git status"}))
        ));
        assert!(!rule_matches_request(
            "Bash(git *)",
            &request("bash_command", json!({"command":"cargo test"}))
        ));
    }
}
