//! MCP name utility functions.
//!
//! Corresponds to TS: `src/utils/mcp-name.ts`.
//! Provides sanitization, building, parsing, normalization, and matching of MCP tool names.

/// Separator used between MCP prefix, server name, and tool name.
///
/// We use "--" (double hyphen) because:
/// 1. It's allowed by all providers (dashes are permitted in function names)
/// 2. It won't conflict with underscores in sanitized server/tool names
/// 3. It's unique enough to be a reliable delimiter for parsing
pub const MCP_TOOL_SEPARATOR: &str = "--";

/// Prefix for all MCP tool function names.
pub const MCP_TOOL_PREFIX: &str = "mcp";

/// Maximum length of a built MCP tool name (Gemini limit).
const MAX_TOOL_NAME_LENGTH: usize = 64;

/// Sanitize a name to be safe for use in API function names.
///
/// This removes special characters and ensures the name starts correctly.
/// Hyphens are preserved since they are valid in function names.
///
/// Corresponds to TS: `sanitizeMcpName`
pub fn sanitize_mcp_name(name: &str) -> String {
    if name.is_empty() {
        return "_".to_string();
    }

    // Replace runs of whitespace with a single underscore first
    let mut sanitized = String::new();
    let mut last_was_space = false;
    for ch in name.chars() {
        if ch.is_whitespace() {
            if !last_was_space {
                sanitized.push('_');
                last_was_space = true;
            }
        } else {
            sanitized.push(ch);
            last_was_space = false;
        }
    }

    // Only allow alphanumeric, underscores, and hyphens
    sanitized = sanitized
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .collect();

    // Replace any double-hyphen sequences with single hyphen to avoid separator conflicts
    while sanitized.contains("--") {
        sanitized = sanitized.replace("--", "-");
    }

    // Ensure the name starts with a letter or underscore
    if !sanitized.is_empty() {
        let first = sanitized.chars().next().unwrap();
        if !first.is_ascii_alphabetic() && first != '_' {
            sanitized = format!("_{}", sanitized);
        }
    }

    // If empty after sanitization, use a placeholder
    if sanitized.is_empty() {
        sanitized = "_unnamed".to_string();
    }

    sanitized
}

/// Normalize a string for comparison by treating hyphens and underscores as equivalent.
///
/// This is used to match tool names when models convert hyphens to underscores.
///
/// Corresponds to TS: `normalizeForComparison`
pub fn normalize_for_comparison(name: &str) -> String {
    name.replace('-', "_")
}

/// Normalize an MCP tool name by converting underscore separators back to hyphens.
///
/// This handles the case where models (especially Claude) convert hyphens to underscores
/// in tool names when using native tool calling.
///
/// For example: `"mcp__server__tool"` -> `"mcp--server--tool"`
///
/// Uses fuzzy matching — treats hyphens and underscores as equivalent when normalizing
/// the separator pattern.
///
/// Corresponds to TS: `normalizeMcpToolName`
pub fn normalize_mcp_tool_name(tool_name: &str) -> String {
    // Normalize for comparison to detect MCP tools regardless of separator style
    let normalized = normalize_for_comparison(tool_name);

    // Only normalize if it looks like an MCP tool (starts with mcp__)
    if normalized.starts_with("mcp__") {
        // Find the pattern: mcp{sep}server{sep}tool where sep is -- or __
        // Split on both separator styles
        let parts: Vec<&str> = tool_name.split("__").flat_map(|s| s.split("--")).collect();

        if parts.len() >= 3 && parts[0].eq_ignore_ascii_case("mcp") {
            // Reconstruct with proper -- separators
            let server_name = parts[1];
            let tool_name_part = parts[2..].join(MCP_TOOL_SEPARATOR);
            return format!(
                "{}{}{}{}{}",
                MCP_TOOL_PREFIX, MCP_TOOL_SEPARATOR, server_name, MCP_TOOL_SEPARATOR, tool_name_part
            );
        }
    }
    tool_name.to_string()
}

/// Check if a tool name is an MCP tool (starts with the MCP prefix and separator).
///
/// Uses fuzzy matching to handle both hyphen and underscore separators.
///
/// Corresponds to TS: `isMcpTool`
pub fn is_mcp_tool(tool_name: &str) -> bool {
    let normalized = normalize_for_comparison(tool_name);
    normalized.starts_with("mcp__")
}

/// Build a full MCP tool function name from server and tool names.
///
/// The format is: `mcp--{sanitized_server_name}--{sanitized_tool_name}`
///
/// The total length is capped at 64 characters to conform to API limits.
///
/// Corresponds to TS: `buildMcpToolName`
pub fn build_mcp_tool_name(server_name: &str, tool_name: &str) -> String {
    let sanitized_server = sanitize_mcp_name(server_name);
    let sanitized_tool = sanitize_mcp_name(tool_name);

    // Build the full name: mcp--{server}--{tool}
    let full_name = format!(
        "{}{}{}{}{}",
        MCP_TOOL_PREFIX,
        MCP_TOOL_SEPARATOR,
        sanitized_server,
        MCP_TOOL_SEPARATOR,
        sanitized_tool
    );

    // Truncate if necessary (max 64 chars for Gemini)
    if full_name.len() > MAX_TOOL_NAME_LENGTH {
        full_name[..MAX_TOOL_NAME_LENGTH].to_string()
    } else {
        full_name
    }
}

/// Parse an MCP tool function name back into server and tool names.
///
/// This handles both hyphen and underscore separators using fuzzy matching.
///
/// Returns `Some((server_name, tool_name))` on success, or `None` if parsing fails.
///
/// Corresponds to TS: `parseMcpToolName`
pub fn parse_mcp_tool_name(mcp_tool_name: &str) -> Option<(String, String)> {
    // Normalize the name to handle both separator styles
    let normalized_name = normalize_mcp_tool_name(mcp_tool_name);

    let prefix = format!("{}{}", MCP_TOOL_PREFIX, MCP_TOOL_SEPARATOR);
    if !normalized_name.starts_with(&prefix) {
        return None;
    }

    // Remove the "mcp--" prefix
    let remainder = &normalized_name[prefix.len()..];

    // Split on the separator to get server and tool names
    let separator_index = remainder.find(MCP_TOOL_SEPARATOR)?;
    let server_name = &remainder[..separator_index];
    let tool_name = &remainder[separator_index + MCP_TOOL_SEPARATOR.len()..];

    if server_name.is_empty() || tool_name.is_empty() {
        return None;
    }

    Some((server_name.to_string(), tool_name.to_string()))
}

/// Check if two tool names match using fuzzy comparison.
///
/// Treats hyphens and underscores as equivalent.
///
/// Corresponds to TS: `toolNamesMatch`
pub fn tool_names_match(name1: &str, name2: &str) -> bool {
    normalize_for_comparison(name1) == normalize_for_comparison(name2)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- sanitize_mcp_name tests ----

    #[test]
    fn test_sanitize_empty() {
        assert_eq!(sanitize_mcp_name(""), "_");
    }

    #[test]
    fn test_sanitize_spaces_to_underscores() {
        assert_eq!(sanitize_mcp_name("my server"), "my_server");
        assert_eq!(sanitize_mcp_name("server name here"), "server_name_here");
    }

    #[test]
    fn test_sanitize_removes_invalid_chars() {
        assert_eq!(sanitize_mcp_name("server@name!"), "servername");
        assert_eq!(sanitize_mcp_name("test#$%^&*()"), "test");
    }

    #[test]
    fn test_sanitize_keeps_valid_chars() {
        assert_eq!(sanitize_mcp_name("server_name"), "server_name");
        assert_eq!(sanitize_mcp_name("server-name"), "server-name");
        assert_eq!(sanitize_mcp_name("Server123"), "Server123");
    }

    #[test]
    fn test_sanitize_removes_dots_and_colons() {
        assert_eq!(sanitize_mcp_name("server.name"), "servername");
        assert_eq!(sanitize_mcp_name("server:name"), "servername");
        assert_eq!(
            sanitize_mcp_name("awslabs.aws-documentation-mcp-server"),
            "awslabsaws-documentation-mcp-server"
        );
    }

    #[test]
    fn test_sanitize_prepends_underscore_for_digit_start() {
        assert_eq!(sanitize_mcp_name("123server"), "_123server");
        assert_eq!(sanitize_mcp_name("-server"), "_-server");
        assert_eq!(sanitize_mcp_name(".server"), "server");
    }

    #[test]
    fn test_sanitize_no_modify_valid_start() {
        assert_eq!(sanitize_mcp_name("server"), "server");
        assert_eq!(sanitize_mcp_name("_server"), "_server");
        assert_eq!(sanitize_mcp_name("Server"), "Server");
    }

    #[test]
    fn test_sanitize_double_hyphen() {
        assert_eq!(sanitize_mcp_name("server--name"), "server-name");
        assert_eq!(sanitize_mcp_name("test---server"), "test-server");
        assert_eq!(sanitize_mcp_name("my----tool"), "my-tool");
    }

    #[test]
    fn test_sanitize_complex_names() {
        assert_eq!(sanitize_mcp_name("My Server @ Home!"), "My_Server__Home");
        assert_eq!(sanitize_mcp_name("123-test server"), "_123-test_server");
    }

    #[test]
    fn test_sanitize_all_invalid_becomes_unnamed() {
        assert_eq!(sanitize_mcp_name("@#$%"), "_unnamed");
    }

    #[test]
    fn test_sanitize_preserves_hyphens() {
        assert_eq!(sanitize_mcp_name("atlassian-jira_search"), "atlassian-jira_search");
        assert_eq!(
            sanitize_mcp_name("atlassian-confluence_search"),
            "atlassian-confluence_search"
        );
    }

    #[test]
    fn test_sanitize_whitespace_only() {
        assert_eq!(sanitize_mcp_name("   "), "_");
    }

    // ---- normalize_for_comparison tests ----

    #[test]
    fn test_normalize_for_comparison() {
        assert_eq!(normalize_for_comparison("get-user"), "get_user");
        assert_eq!(normalize_for_comparison("get_user"), "get_user");
        assert_eq!(normalize_for_comparison("mcp--server--tool"), "mcp__server__tool");
    }

    // ---- normalize_mcp_tool_name tests ----

    #[test]
    fn test_normalize_underscore_to_hyphen_separators() {
        assert_eq!(
            normalize_mcp_tool_name("mcp__server__tool"),
            "mcp--server--tool"
        );
    }

    #[test]
    fn test_normalize_no_modify_hyphen_separators() {
        assert_eq!(
            normalize_mcp_tool_name("mcp--server--tool"),
            "mcp--server--tool"
        );
    }

    #[test]
    fn test_normalize_no_modify_non_mcp_names() {
        assert_eq!(normalize_mcp_tool_name("read_file"), "read_file");
        assert_eq!(normalize_mcp_tool_name("some__tool"), "some__tool");
    }

    #[test]
    fn test_normalize_preserves_underscores_in_names() {
        // Should become: mcp--my_server--get_user_profile
        assert_eq!(
            normalize_mcp_tool_name("mcp__my_server__get_user_profile"),
            "mcp--my_server--get_user_profile"
        );
    }

    #[test]
    fn test_normalize_with_underscore_tool_name() {
        assert_eq!(
            normalize_mcp_tool_name("mcp__server__get_user_profile"),
            "mcp--server--get_user_profile"
        );
    }

    // ---- is_mcp_tool tests ----

    #[test]
    fn test_is_mcp_tool_hyphen_separators() {
        assert!(is_mcp_tool("mcp--server--tool"));
        assert!(is_mcp_tool("mcp--my_server--get_forecast"));
        assert!(is_mcp_tool("mcp--server--get-user-profile"));
    }

    #[test]
    fn test_is_mcp_tool_underscore_separators() {
        // Models may convert hyphens to underscores
        assert!(is_mcp_tool("mcp__server__tool"));
        assert!(is_mcp_tool("mcp__my_server__get_forecast"));
    }

    #[test]
    fn test_is_mcp_tool_false_for_non_mcp() {
        assert!(!is_mcp_tool("server--tool"));
        assert!(!is_mcp_tool("tool"));
        assert!(!is_mcp_tool("read_file"));
        assert!(!is_mcp_tool(""));
    }

    #[test]
    fn test_is_mcp_tool_false_for_old_format() {
        // Old single-underscore format: mcp_server_tool
        assert!(!is_mcp_tool("mcp_server_tool"));
    }

    #[test]
    fn test_is_mcp_tool_false_for_partial_prefix() {
        assert!(!is_mcp_tool("mcp-server"));
        assert!(!is_mcp_tool("mcp"));
    }

    // ---- build_mcp_tool_name tests ----

    #[test]
    fn test_build_basic() {
        assert_eq!(build_mcp_tool_name("server", "tool"), "mcp--server--tool");
    }

    #[test]
    fn test_build_sanitizes_both_names() {
        assert_eq!(
            build_mcp_tool_name("my server", "my tool"),
            "mcp--my_server--my_tool"
        );
    }

    #[test]
    fn test_build_handles_special_chars() {
        assert_eq!(
            build_mcp_tool_name("server@name", "tool!name"),
            "mcp--servername--toolname"
        );
    }

    #[test]
    fn test_build_truncates_long_names() {
        let long_server = "a".repeat(50);
        let long_tool = "b".repeat(50);
        let result = build_mcp_tool_name(&long_server, &long_tool);
        assert!(result.len() <= 64);
    }

    #[test]
    fn test_build_handles_digit_start() {
        assert_eq!(
            build_mcp_tool_name("123server", "456tool"),
            "mcp--_123server--_456tool"
        );
    }

    #[test]
    fn test_build_preserves_underscores() {
        assert_eq!(
            build_mcp_tool_name("my_server", "my_tool"),
            "mcp--my_server--my_tool"
        );
    }

    #[test]
    fn test_build_preserves_hyphens_in_tool() {
        assert_eq!(
            build_mcp_tool_name("onellm", "atlassian-jira_search"),
            "mcp--onellm--atlassian-jira_search"
        );
    }

    #[test]
    fn test_build_multiple_hyphens_in_tool() {
        assert_eq!(
            build_mcp_tool_name("server", "get-user-profile"),
            "mcp--server--get-user-profile"
        );
    }

    // ---- parse_mcp_tool_name tests ----

    #[test]
    fn test_parse_hyphen_separators() {
        assert_eq!(
            parse_mcp_tool_name("mcp--server--tool"),
            Some(("server".to_string(), "tool".to_string()))
        );
    }

    #[test]
    fn test_parse_underscore_separators() {
        // Models may convert hyphens to underscores
        assert_eq!(
            parse_mcp_tool_name("mcp__server__tool"),
            Some(("server".to_string(), "tool".to_string()))
        );
    }

    #[test]
    fn test_parse_non_mcp_returns_none() {
        assert_eq!(parse_mcp_tool_name("server--tool"), None);
        assert_eq!(parse_mcp_tool_name("tool"), None);
    }

    #[test]
    fn test_parse_old_format_returns_none() {
        assert_eq!(parse_mcp_tool_name("mcp_server_tool"), None);
    }

    #[test]
    fn test_parse_tool_with_underscores() {
        assert_eq!(
            parse_mcp_tool_name("mcp--server--tool_name"),
            Some(("server".to_string(), "tool_name".to_string()))
        );
    }

    #[test]
    fn test_parse_server_with_underscores() {
        assert_eq!(
            parse_mcp_tool_name("mcp--my_server--tool"),
            Some(("my_server".to_string(), "tool".to_string()))
        );
    }

    #[test]
    fn test_parse_both_with_underscores() {
        assert_eq!(
            parse_mcp_tool_name("mcp--my_server--get_forecast"),
            Some(("my_server".to_string(), "get_forecast".to_string()))
        );
    }

    #[test]
    fn test_parse_tool_with_hyphens() {
        assert_eq!(
            parse_mcp_tool_name("mcp--onellm--atlassian-jira_search"),
            Some(("onellm".to_string(), "atlassian-jira_search".to_string()))
        );
    }

    #[test]
    fn test_parse_malformed_returns_none() {
        assert_eq!(parse_mcp_tool_name("mcp--"), None);
        assert_eq!(parse_mcp_tool_name("mcp--server"), None);
    }

    // ---- roundtrip tests ----

    #[test]
    fn test_roundtrip_basic() {
        let tool_name = build_mcp_tool_name("server", "tool");
        let parsed = parse_mcp_tool_name(&tool_name);
        assert_eq!(
            parsed,
            Some(("server".to_string(), "tool".to_string()))
        );
    }

    #[test]
    fn test_roundtrip_with_underscores() {
        let tool_name = build_mcp_tool_name("my_server", "my_tool");
        let parsed = parse_mcp_tool_name(&tool_name);
        assert_eq!(
            parsed,
            Some(("my_server".to_string(), "my_tool".to_string()))
        );
    }

    #[test]
    fn test_roundtrip_spaces_converted() {
        let tool_name = build_mcp_tool_name("my server", "get tool");
        let parsed = parse_mcp_tool_name(&tool_name);
        assert_eq!(
            parsed,
            Some(("my_server".to_string(), "get_tool".to_string()))
        );
    }

    #[test]
    fn test_roundtrip_complex_names() {
        let tool_name = build_mcp_tool_name("Weather API", "get_current_forecast");
        let parsed = parse_mcp_tool_name(&tool_name);
        assert_eq!(
            parsed,
            Some(("Weather_API".to_string(), "get_current_forecast".to_string()))
        );
    }

    #[test]
    fn test_roundtrip_hyphens_in_tool() {
        let tool_name = build_mcp_tool_name("onellm", "atlassian-jira_search");
        assert_eq!(tool_name, "mcp--onellm--atlassian-jira_search");

        let parsed = parse_mcp_tool_name(&tool_name);
        assert_eq!(
            parsed,
            Some(("onellm".to_string(), "atlassian-jira_search".to_string()))
        );
    }

    #[test]
    fn test_roundtrip_multiple_hyphens() {
        let tool_name = build_mcp_tool_name("server", "get-user-profile");
        let parsed = parse_mcp_tool_name(&tool_name);
        assert_eq!(
            parsed,
            Some(("server".to_string(), "get-user-profile".to_string()))
        );
    }

    #[test]
    fn test_model_converts_hyphens_to_underscores() {
        // Build with hyphens
        let built_name = build_mcp_tool_name("onellm", "atlassian-jira_search");
        assert_eq!(built_name, "mcp--onellm--atlassian-jira_search");

        // Model outputs underscores instead of hyphens
        let model_output = "mcp__onellm__atlassian-jira_search";

        // Normalize and parse
        let normalized = normalize_mcp_tool_name(model_output);
        assert_eq!(normalized, "mcp--onellm--atlassian-jira_search");

        let parsed = parse_mcp_tool_name(&normalized);
        assert_eq!(
            parsed,
            Some(("onellm".to_string(), "atlassian-jira_search".to_string()))
        );
    }

    #[test]
    fn test_model_converts_all_to_underscores() {
        let built_name = build_mcp_tool_name("onellm", "atlassian-jira_search");
        assert_eq!(built_name, "mcp--onellm--atlassian-jira_search");

        // Model converts everything to underscores
        let model_output = "mcp__onellm__atlassian_jira_search";
        let normalized = normalize_mcp_tool_name(model_output);
        assert_eq!(normalized, "mcp--onellm--atlassian_jira_search");

        let parsed = parse_mcp_tool_name(&normalized);
        // Note: tool name now has underscore instead of hyphen
        assert_eq!(
            parsed,
            Some(("onellm".to_string(), "atlassian_jira_search".to_string()))
        );
    }

    #[test]
    fn test_roundtrip_long_names() {
        let long_server = "very-long-server-name-that-exceeds-normal-length";
        let long_tool = "very-long-tool-name-that-also-exceeds";
        let result = build_mcp_tool_name(long_server, long_tool);

        // Should still be parseable
        let parsed = parse_mcp_tool_name(&result);
        assert!(parsed.is_some());
    }

    #[test]
    fn test_server_name_with_hyphens() {
        let tool_name = build_mcp_tool_name("my-server", "tool");
        assert_eq!(tool_name, "mcp--my-server--tool");

        let parsed = parse_mcp_tool_name(&tool_name);
        assert_eq!(
            parsed,
            Some(("my-server".to_string(), "tool".to_string()))
        );
    }

    #[test]
    fn test_both_names_with_hyphens() {
        let tool_name = build_mcp_tool_name("my-server", "get-user");
        assert_eq!(tool_name, "mcp--my-server--get-user");

        // Model converts to underscores
        let model_output = "mcp__my_server__get_user";
        let parsed = parse_mcp_tool_name(model_output);
        // After normalization, hyphens in names become underscores too
        // because normalize splits on both __ and --
        assert!(parsed.is_some());
    }

    // ---- tool_names_match tests ----

    #[test]
    fn test_match_identical_names() {
        assert!(tool_names_match("get_user", "get_user"));
        assert!(tool_names_match("get-user", "get-user"));
    }

    #[test]
    fn test_match_hyphens_vs_underscores() {
        assert!(tool_names_match("get-user", "get_user"));
        assert!(tool_names_match("get_user", "get-user"));
    }

    #[test]
    fn test_match_complex_mcp_names() {
        assert!(tool_names_match(
            "mcp--server--get-user-profile",
            "mcp__server__get_user_profile"
        ));
    }

    #[test]
    fn test_no_match_different_names() {
        assert!(!tool_names_match("get_user", "get_profile"));
    }
}
