//! Command approval logic for auto-approval.
//!
//! Mirrors `commands.ts` — implements the longest-prefix-match strategy
//! for allowlist/denylist resolution, dangerous substitution detection,
//! and command chain parsing.

use std::sync::LazyLock as Lazy;
use regex::Regex;

use crate::types::CommandDecision;

// ---------------------------------------------------------------------------
// Regex patterns for dangerous substitution detection
// ---------------------------------------------------------------------------

static DANGEROUS_PARAMETER_EXPANSION: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\$\{[^}]*@[PQEAa][^}]*\}").unwrap());

static PARAMETER_ASSIGNMENT_OCTAL: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\$\{[^}]*[=+\-?][^}]*\\[0-7]{3}[^}]*\}").unwrap());

static PARAMETER_ASSIGNMENT_HEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\$\{[^}]*[=+\-?][^}]*\\x[0-9a-fA-F]{2}[^}]*\}").unwrap());

static PARAMETER_ASSIGNMENT_UNICODE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\$\{[^}]*[=+\-?][^}]*\\u[0-9a-fA-F]{4}[^}]*\}").unwrap());

static INDIRECT_EXPANSION: Lazy<Regex> = Lazy::new(|| Regex::new(r"\$\{![^}]+\}").unwrap());

static HERE_STRING_WITH_SUBSTITUTION: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"<<<\s*(\$\(|`)").unwrap());

/// Matches zsh process substitution `=(...)` — must be at start of string or
/// preceded by whitespace / `;` / `|` / `&` / `(` / `<`.
static ZSH_PROCESS_SUBSTITUTION: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?:^|[\s;|&(<])=\([^)]+\)").unwrap());

static ZSH_GLOB_QUALIFIER: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"[*?+@!]\(e:[^:]+:\)").unwrap());

// ---------------------------------------------------------------------------
// Dangerous substitution detection
// ---------------------------------------------------------------------------

/// Detect dangerous parameter substitutions that could lead to command
/// execution.
///
/// Detected patterns:
/// - `${var@P}`, `${var@Q}`, `${var@E}`, `${var@A}`, `${var@a}`
/// - `${var=value}` with escape sequences (octal, hex, unicode)
/// - `${!var}` indirect references
/// - `<<<$(...)` or `<<<`...` ` here-string command substitution
/// - `=(...)` zsh process substitution
/// - `*(e:...:)` zsh glob qualifiers with code execution
///
/// Mirrors `containsDangerousSubstitution` from `commands.ts`.
pub fn contains_dangerous_substitution(source: &str) -> bool {
    DANGEROUS_PARAMETER_EXPANSION.is_match(source)
        || PARAMETER_ASSIGNMENT_OCTAL.is_match(source)
        || PARAMETER_ASSIGNMENT_HEX.is_match(source)
        || PARAMETER_ASSIGNMENT_UNICODE.is_match(source)
        || INDIRECT_EXPANSION.is_match(source)
        || HERE_STRING_WITH_SUBSTITUTION.is_match(source)
        || ZSH_PROCESS_SUBSTITUTION.is_match(source)
        || ZSH_GLOB_QUALIFIER.is_match(source)
}

// ---------------------------------------------------------------------------
// Longest prefix match
// ---------------------------------------------------------------------------

/// Find the longest matching prefix from a list of prefixes for a given
/// command.
///
/// - Wildcard `"*"` matches any command.
/// - Matching is case-insensitive.
/// - Returns `None` if no match is found.
///
/// Mirrors `findLongestPrefixMatch` from `commands.ts`.
pub fn find_longest_prefix_match<'a>(
    command: &str,
    prefixes: &'a [String],
) -> Option<&'a str> {
    if command.is_empty() || prefixes.is_empty() {
        return None;
    }

    let trimmed = command.trim().to_lowercase();
    // Track (original prefix ref, lowercase length) so that comparisons
    // are always between lowercase lengths — matching the TS behaviour
    // where `longestMatch` is always a lowercase string.
    let mut best: Option<(&'a str, usize)> = None;

    for prefix in prefixes {
        let lower = prefix.to_lowercase();
        if lower == "*" || trimmed.starts_with(&lower) {
            let lower_len = lower.len();
            match best {
                None => best = Some((prefix.as_str(), lower_len)),
                Some((_, prev_len)) if lower_len > prev_len => {
                    best = Some((prefix.as_str(), lower_len));
                }
                _ => {}
            }
        }
    }

    best.map(|(s, _)| s)
}

// ---------------------------------------------------------------------------
// Single command approval / denial
// ---------------------------------------------------------------------------

/// Check if a single command should be auto-approved.
///
/// Mirrors `isAutoApprovedSingleCommand` from `commands.ts`.
pub fn is_auto_approved_single_command(
    command: &str,
    allowed: &[String],
    denied: Option<&[String]>,
) -> bool {
    if command.is_empty() {
        return true;
    }

    if allowed.is_empty() {
        return false;
    }

    let has_wildcard = allowed.iter().any(|cmd| cmd.to_lowercase() == "*");

    // If no denylist provided (None), use simple allowlist logic
    let Some(denied) = denied else {
        let trimmed = command.trim().to_lowercase();
        return allowed.iter().any(|prefix| {
            let lower = prefix.to_lowercase();
            lower == "*" || trimmed.starts_with(&lower)
        });
    };

    let longest_denied = find_longest_prefix_match(command, denied);
    let longest_allowed = find_longest_prefix_match(command, allowed);

    // Wildcard + no deny match → approve
    if has_wildcard && longest_denied.is_none() {
        return true;
    }

    // Must have an allowlist match
    let Some(longest_allowed) = longest_allowed else {
        return false;
    };

    // No deny match → approve
    let Some(longest_denied) = longest_denied else {
        return true;
    };

    // Both match — allowlist must be longer
    longest_allowed.len() > longest_denied.len()
}

/// Check if a single command should be auto-denied.
///
/// Mirrors `isAutoDeniedSingleCommand` from `commands.ts`.
pub fn is_auto_denied_single_command(
    command: &str,
    allowed: &[String],
    denied: Option<&[String]>,
) -> bool {
    if command.is_empty() {
        return false;
    }

    let Some(denied) = denied else {
        return false;
    };

    if denied.is_empty() {
        return false;
    }

    let longest_denied = find_longest_prefix_match(command, denied);
    let longest_allowed = find_longest_prefix_match(command, allowed);

    let Some(longest_denied) = longest_denied else {
        return false;
    };

    let Some(longest_allowed) = longest_allowed else {
        return true;
    };

    // Denylist must be longer or equal to auto-deny
    longest_denied.len() >= longest_allowed.len()
}

// ---------------------------------------------------------------------------
// Single command decision
// ---------------------------------------------------------------------------

/// Get the decision for a single command using longest prefix match rule.
///
/// **Decision Matrix:**
///
/// | Allowlist | Denylist | Result        |
/// |-----------|----------|---------------|
/// | Yes       | No       | AutoApprove   |
/// | No        | Yes      | AutoDeny      |
/// | Yes (longer) | Yes  | AutoApprove   |
/// | Yes (shorter/equal) | Yes | AutoDeny |
/// | No        | No       | AskUser       |
///
/// Mirrors `getSingleCommandDecision` from `commands.ts`.
pub fn get_single_command_decision(
    command: &str,
    allowed: &[String],
    denied: &[String],
) -> CommandDecision {
    if command.is_empty() {
        return CommandDecision::AutoApprove;
    }

    let longest_allowed = find_longest_prefix_match(command, allowed);
    let longest_denied = find_longest_prefix_match(command, denied);

    match (longest_allowed, longest_denied) {
        (Some(_), None) => CommandDecision::AutoApprove,
        (None, Some(_)) => CommandDecision::AutoDeny,
        (Some(allowed), Some(denied)) => {
            if allowed.len() > denied.len() {
                CommandDecision::AutoApprove
            } else {
                CommandDecision::AutoDeny
            }
        }
        (None, None) => CommandDecision::AskUser,
    }
}

// ---------------------------------------------------------------------------
// Command chain parsing (simplified)
// ---------------------------------------------------------------------------

/// Parse a command chain into individual sub-commands.
///
/// Splits by `&&`, `||`, `;`, `|`, `&`, and newlines while respecting
/// single-quoted and double-quoted strings.
///
/// This is a simplified version of the TypeScript `parseCommand` which uses
/// the `shell-quote` library. We do not need a full shell parser here.
pub fn parse_command_chain(command: &str) -> Vec<String> {
    if command.trim().is_empty() {
        return Vec::new();
    }

    let mut result = Vec::new();
    let mut current = String::new();
    let mut chars = command.chars().peekable();
    let mut in_single_quote = false;
    let mut in_double_quote = false;

    while let Some(ch) = chars.next() {
        if in_single_quote {
            if ch == '\'' {
                in_single_quote = false;
            }
            current.push(ch);
            continue;
        }

        if in_double_quote {
            if ch == '"' {
                in_double_quote = false;
            } else if ch == '\\' {
                // Consume the next char as escaped
                current.push(ch);
                if let Some(next) = chars.next() {
                    current.push(next);
                }
                continue;
            }
            current.push(ch);
            continue;
        }

        match ch {
            '\'' => {
                in_single_quote = true;
                current.push(ch);
            }
            '"' => {
                in_double_quote = true;
                current.push(ch);
            }
            '&' => {
                if chars.peek() == Some(&'&') {
                    chars.next();
                    push_trimmed(&mut result, &mut current);
                } else {
                    // Background operator &
                    push_trimmed(&mut result, &mut current);
                }
            }
            '|' => {
                if chars.peek() == Some(&'|') {
                    chars.next();
                    push_trimmed(&mut result, &mut current);
                } else {
                    // Pipe operator |
                    push_trimmed(&mut result, &mut current);
                }
            }
            ';' => {
                push_trimmed(&mut result, &mut current);
            }
            '\n' | '\r' => {
                push_trimmed(&mut result, &mut current);
            }
            _ => {
                current.push(ch);
            }
        }
    }

    // Push remaining
    let trimmed = current.trim().to_string();
    // Remove simple PowerShell-like redirections (e.g. 2>&1)
    let cleaned = remove_powershell_redirections(&trimmed);
    if !cleaned.is_empty() {
        result.push(cleaned);
    }

    // Extract subshell commands from $(...) and `...` and add them as
    // separate commands. This mirrors the TypeScript behavior where
    // subshells are extracted and checked individually.
    let subshells = extract_subshells(command);
    for subshell in subshells {
        // Recursively parse the subshell content (it may contain chains too)
        let sub_commands = parse_command_chain(&subshell);
        result.extend(sub_commands);
    }

    result
}

/// Extract subshell commands from `$(...)` and `` `...` `` patterns.
/// Respects quoting — does not extract from inside single-quoted strings.
fn extract_subshells(command: &str) -> Vec<String> {
    let mut subshells = Vec::new();
    let mut chars = command.chars().peekable();
    let mut in_single_quote = false;
    let mut in_double_quote = false;

    while let Some(ch) = chars.next() {
        if in_single_quote {
            if ch == '\'' {
                in_single_quote = false;
            }
            continue;
        }

        if in_double_quote {
            if ch == '"' {
                in_double_quote = false;
            } else if ch == '\\' {
                chars.next();
            }
            continue;
        }

        match ch {
            '\'' => {
                in_single_quote = true;
            }
            '"' => {
                in_double_quote = true;
            }
            '`' => {
                // Backtick subshell
                let mut content = String::new();
                while let Some(c) = chars.next() {
                    if c == '`' {
                        break;
                    }
                    content.push(c);
                }
                let trimmed = content.trim().to_string();
                if !trimmed.is_empty() {
                    subshells.push(trimmed);
                }
            }
            '$' => {
                if chars.peek() == Some(&'(') {
                    chars.next(); // consume '('
                    let mut depth = 1i32;
                    let mut content = String::new();
                    while let Some(c) = chars.next() {
                        if c == '(' {
                            depth += 1;
                        } else if c == ')' {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        content.push(c);
                    }
                    let trimmed = content.trim().to_string();
                    if !trimmed.is_empty() {
                        subshells.push(trimmed);
                    }
                }
            }
            _ => {}
        }
    }

    subshells
}

/// Push a trimmed command segment to the result list, clearing `current`.
fn push_trimmed(result: &mut Vec<String>, current: &mut String) {
    let trimmed = current.trim().to_string();
    let cleaned = remove_powershell_redirections(&trimmed);
    if !cleaned.is_empty() {
        result.push(cleaned);
    }
    current.clear();
}

/// Remove simple PowerShell-like redirections (e.g. `2>&1`).
fn remove_powershell_redirections(s: &str) -> String {
    static REDIR: Lazy<Regex> = Lazy::new(|| Regex::new(r"\d*>&\d*").unwrap());
    REDIR.replace_all(s, "").trim().to_string()
}

// ---------------------------------------------------------------------------
// Full command decision
// ---------------------------------------------------------------------------

/// Unified command validation that implements the longest prefix match rule.
///
/// **Decision Logic:**
/// 1. Empty command → `AutoApprove`
/// 2. Parse into sub-commands (split by `&&`, `||`, `;`, `|`, `&`)
/// 3. Check each sub-command with longest prefix match
/// 4. If any sub-command is denied → `AutoDeny`
/// 5. If dangerous substitution detected → `AskUser`
/// 6. If all sub-commands approved → `AutoApprove`
/// 7. Otherwise → `AskUser`
///
/// Mirrors `getCommandDecision` from `commands.ts`.
pub fn get_command_decision(
    command: &str,
    allowed: &[String],
    denied: &[String],
) -> CommandDecision {
    if command.trim().is_empty() {
        return CommandDecision::AutoApprove;
    }

    let sub_commands = parse_command_chain(command);

    let decisions: Vec<CommandDecision> = sub_commands
        .iter()
        .map(|cmd| get_single_command_decision(cmd, allowed, denied))
        .collect();

    // If any sub-command is denied, deny the whole command
    if decisions.iter().any(|d| *d == CommandDecision::AutoDeny) {
        return CommandDecision::AutoDeny;
    }

    // Require explicit user approval for dangerous patterns
    if contains_dangerous_substitution(command) {
        return CommandDecision::AskUser;
    }

    // If all sub-commands are approved, approve the whole command
    if decisions.iter().all(|d| *d == CommandDecision::AutoApprove) {
        return CommandDecision::AutoApprove;
    }

    CommandDecision::AskUser
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ---- contains_dangerous_substitution ----

    #[test]
    fn test_dangerous_parameter_expansion_at_p() {
        assert!(contains_dangerous_substitution(
            "echo \"${var@P}\""
        ));
    }

    #[test]
    fn test_dangerous_parameter_expansion_at_q() {
        assert!(contains_dangerous_substitution(
            "echo \"${var@Q}\""
        ));
    }

    #[test]
    fn test_dangerous_parameter_expansion_at_e() {
        assert!(contains_dangerous_substitution(
            "echo \"${var@E}\""
        ));
    }

    #[test]
    fn test_dangerous_parameter_expansion_at_a() {
        assert!(contains_dangerous_substitution(
            "echo \"${var@A}\""
        ));
    }

    #[test]
    fn test_dangerous_parameter_expansion_at_lowercase_a() {
        assert!(contains_dangerous_substitution(
            "echo \"${var@a}\""
        ));
    }

    #[test]
    fn test_dangerous_indirect_expansion() {
        assert!(contains_dangerous_substitution("echo ${!prefix}"));
    }

    #[test]
    fn test_dangerous_here_string_dollar() {
        assert!(contains_dangerous_substitution("cat <<<$(whoami)"));
    }

    #[test]
    fn test_dangerous_here_string_backtick() {
        assert!(contains_dangerous_substitution("cat <<<`whoami`"));
    }

    #[test]
    fn test_dangerous_zsh_process_substitution_standalone() {
        assert!(contains_dangerous_substitution("=(whoami)"));
    }

    #[test]
    fn test_dangerous_zsh_process_substitution_with_space() {
        assert!(contains_dangerous_substitution(" =(ls)"));
    }

    #[test]
    fn test_dangerous_zsh_process_substitution_echo() {
        assert!(contains_dangerous_substitution(
            "echo =(cat /etc/passwd)"
        ));
    }

    #[test]
    fn test_dangerous_zsh_glob_qualifier() {
        assert!(contains_dangerous_substitution("ls *(e:whoami:)"));
    }

    #[test]
    fn test_dangerous_parameter_assignment_octal() {
        // \140 is backtick in octal
        assert!(contains_dangerous_substitution(
            "echo ${var=\\140whoami\\140}"
        ));
    }

    #[test]
    fn test_dangerous_parameter_assignment_hex() {
        assert!(contains_dangerous_substitution(
            "echo ${var=\\x60whoami\\x60}"
        ));
    }

    #[test]
    fn test_dangerous_parameter_assignment_unicode() {
        assert!(contains_dangerous_substitution(
            "echo ${var=\\u0060whoami\\u0060}"
        ));
    }

    // ---- safe patterns (should NOT be flagged) ----

    #[test]
    fn test_safe_zsh_array_assignment() {
        assert!(!contains_dangerous_substitution(
            "files=(a b c)"
        ));
    }

    #[test]
    fn test_safe_zsh_array_assignment_var() {
        assert!(!contains_dangerous_substitution(
            "var=(item1 item2)"
        ));
    }

    #[test]
    fn test_safe_zsh_array_assignment_single() {
        assert!(!contains_dangerous_substitution("x=(hello)"));
    }

    #[test]
    fn test_safe_node_arrow_function() {
        assert!(!contains_dangerous_substitution(
            "node -e \"const a=(b)=>b\""
        ));
    }

    #[test]
    fn test_safe_node_spaced_arrow_function() {
        assert!(!contains_dangerous_substitution(
            "node -e \"const fn = (x) => x * 2\""
        ));
    }

    #[test]
    fn test_safe_node_arrow_in_filter() {
        assert!(!contains_dangerous_substitution(
            "node -e \"arr.filter(i=>!set.has(i))\""
        ));
    }

    // ---- find_longest_prefix_match ----

    #[test]
    fn test_find_longest_prefix_basic() {
        let prefixes: Vec<String> = vec!["git".into(), "git push".into()];
        let result = find_longest_prefix_match("git push origin", &prefixes);
        assert_eq!(result, Some("git push"));
    }

    #[test]
    fn test_find_longest_prefix_wildcard() {
        let prefixes: Vec<String> = vec!["*".into(), "npm".into()];
        let result = find_longest_prefix_match("npm install", &prefixes);
        // "npm" is longer than "*", so "npm" should win
        assert_eq!(result, Some("npm"));
    }

    #[test]
    fn test_find_longest_prefix_no_match() {
        let prefixes: Vec<String> = vec!["git".into(), "npm".into()];
        let result = find_longest_prefix_match("unknown command", &prefixes);
        assert_eq!(result, None);
    }

    #[test]
    fn test_find_longest_prefix_empty_command() {
        let prefixes: Vec<String> = vec!["git".into()];
        assert_eq!(find_longest_prefix_match("", &prefixes), None);
    }

    #[test]
    fn test_find_longest_prefix_empty_prefixes() {
        assert_eq!(find_longest_prefix_match("git", &[]), None);
    }

    // ---- get_single_command_decision ----

    #[test]
    fn test_single_decision_only_allowlist() {
        let allowed: Vec<String> = vec!["git".into()];
        let denied: Vec<String> = vec!["npm".into()];
        assert_eq!(
            get_single_command_decision("git status", &allowed, &denied),
            CommandDecision::AutoApprove
        );
    }

    #[test]
    fn test_single_decision_only_denylist() {
        let allowed: Vec<String> = vec!["git".into()];
        let denied: Vec<String> = vec!["npm".into()];
        assert_eq!(
            get_single_command_decision("npm install", &allowed, &denied),
            CommandDecision::AutoDeny
        );
    }

    #[test]
    fn test_single_decision_denylist_more_specific() {
        let allowed: Vec<String> = vec!["git".into()];
        let denied: Vec<String> = vec!["git push".into()];
        assert_eq!(
            get_single_command_decision("git push origin", &allowed, &denied),
            CommandDecision::AutoDeny
        );
    }

    #[test]
    fn test_single_decision_allowlist_more_specific() {
        let allowed: Vec<String> = vec!["git push --dry-run".into()];
        let denied: Vec<String> = vec!["git push".into()];
        assert_eq!(
            get_single_command_decision(
                "git push --dry-run",
                &allowed,
                &denied
            ),
            CommandDecision::AutoApprove
        );
    }

    #[test]
    fn test_single_decision_no_match() {
        let allowed: Vec<String> = vec!["git".into()];
        let denied: Vec<String> = vec!["npm".into()];
        assert_eq!(
            get_single_command_decision("unknown", &allowed, &denied),
            CommandDecision::AskUser
        );
    }

    #[test]
    fn test_single_decision_empty_command() {
        assert_eq!(
            get_single_command_decision("", &[], &[]),
            CommandDecision::AutoApprove
        );
    }

    #[test]
    fn test_single_decision_case_insensitive() {
        let allowed: Vec<String> = vec!["Git".into()];
        let denied: Vec<String> = vec![];
        assert_eq!(
            get_single_command_decision("GIT status", &allowed, &denied),
            CommandDecision::AutoApprove
        );
    }

    // ---- get_command_decision ----

    #[test]
    fn test_empty_command_auto_approve() {
        assert_eq!(
            get_command_decision("", &[], &[]),
            CommandDecision::AutoApprove
        );
    }

    #[test]
    fn test_wildcard_allowlist() {
        let allowed: Vec<String> = vec!["*".into()];
        let denied: Vec<String> = vec![];
        assert_eq!(
            get_command_decision("anything here", &allowed, &denied),
            CommandDecision::AutoApprove
        );
    }

    #[test]
    fn test_simple_allowlist_match() {
        let allowed: Vec<String> = vec!["git".into()];
        let denied: Vec<String> = vec![];
        assert_eq!(
            get_command_decision("git status", &allowed, &denied),
            CommandDecision::AutoApprove
        );
    }

    #[test]
    fn test_no_allowlist_match_ask_user() {
        let allowed: Vec<String> = vec!["git".into()];
        let denied: Vec<String> = vec![];
        assert_eq!(
            get_command_decision("npm install", &allowed, &denied),
            CommandDecision::AskUser
        );
    }

    #[test]
    fn test_denylist_overrides_allowlist() {
        let allowed: Vec<String> = vec!["git".into()];
        let denied: Vec<String> = vec!["git push".into()];
        assert_eq!(
            get_command_decision("git push origin", &allowed, &denied),
            CommandDecision::AutoDeny
        );
    }

    #[test]
    fn test_longest_prefix_match_wins() {
        let allowed: Vec<String> = vec!["git push --dry-run".into()];
        let denied: Vec<String> = vec!["git push".into()];
        assert_eq!(
            get_command_decision(
                "git push --dry-run",
                &allowed,
                &denied
            ),
            CommandDecision::AutoApprove
        );
    }

    #[test]
    fn test_command_chain_all_approved() {
        let allowed: Vec<String> = vec!["git".into()];
        let denied: Vec<String> = vec![];
        assert_eq!(
            get_command_decision("git status && git log", &allowed, &denied),
            CommandDecision::AutoApprove
        );
    }

    #[test]
    fn test_command_chain_any_denied() {
        let allowed: Vec<String> = vec!["git".into()];
        let denied: Vec<String> = vec!["rm".into()];
        assert_eq!(
            get_command_decision(
                "git status && rm file",
                &allowed,
                &denied
            ),
            CommandDecision::AutoDeny
        );
    }

    #[test]
    fn test_dangerous_substitution_blocks_approval() {
        let allowed: Vec<String> = vec!["echo".into()];
        let denied: Vec<String> = vec![];
        assert_eq!(
            get_command_decision(
                "echo \"${var@P}\"",
                &allowed,
                &denied
            ),
            CommandDecision::AskUser
        );
    }

    #[test]
    fn test_wildcard_with_denylist() {
        let allowed: Vec<String> = vec!["*".into()];
        let denied: Vec<String> = vec!["rm".into()];
        assert_eq!(
            get_command_decision("rm -rf /", &allowed, &denied),
            CommandDecision::AutoDeny
        );
    }

    #[test]
    fn test_wildcard_with_denylist_allows_non_denied() {
        let allowed: Vec<String> = vec!["*".into()];
        let denied: Vec<String> = vec!["rm".into()];
        assert_eq!(
            get_command_decision("git status", &allowed, &denied),
            CommandDecision::AutoApprove
        );
    }

    #[test]
    fn test_case_insensitive_matching() {
        let allowed: Vec<String> = vec!["GIT".into()];
        let denied: Vec<String> = vec![];
        assert_eq!(
            get_command_decision("git status", &allowed, &denied),
            CommandDecision::AutoApprove
        );
    }

    #[test]
    fn test_command_chain_with_pipe() {
        let allowed: Vec<String> = vec!["git".into(), "grep".into()];
        let denied: Vec<String> = vec![];
        assert_eq!(
            get_command_decision("git log | grep foo", &allowed, &denied),
            CommandDecision::AutoApprove
        );
    }

    #[test]
    fn test_command_chain_with_semicolon() {
        let allowed: Vec<String> = vec!["git".into()];
        let denied: Vec<String> = vec!["rm".into()];
        assert_eq!(
            get_command_decision("git status; rm file", &allowed, &denied),
            CommandDecision::AutoDeny
        );
    }

    #[test]
    fn test_subshell_not_in_allowlist() {
        let allowed: Vec<String> = vec!["echo".into()];
        let denied: Vec<String> = vec![];
        // echo $(whoami) — subshell "whoami" is not in allowlist
        assert_eq!(
            get_command_decision("echo $(whoami)", &allowed, &denied),
            CommandDecision::AskUser
        );
    }

    // ---- parse_command_chain ----

    #[test]
    fn test_parse_empty() {
        assert!(parse_command_chain("").is_empty());
        assert!(parse_command_chain("   ").is_empty());
    }

    #[test]
    fn test_parse_single_command() {
        let result = parse_command_chain("git status");
        assert_eq!(result, vec!["git status"]);
    }

    #[test]
    fn test_parse_double_ampersand() {
        let result = parse_command_chain("git status && git log");
        assert_eq!(result, vec!["git status", "git log"]);
    }

    #[test]
    fn test_parse_double_pipe() {
        let result = parse_command_chain("false || true");
        assert_eq!(result, vec!["false", "true"]);
    }

    #[test]
    fn test_parse_semicolon() {
        let result = parse_command_chain("git status; git log");
        assert_eq!(result, vec!["git status", "git log"]);
    }

    #[test]
    fn test_parse_pipe() {
        let result = parse_command_chain("git log | grep foo");
        assert_eq!(result, vec!["git log", "grep foo"]);
    }

    #[test]
    fn test_parse_respects_quotes() {
        let result = parse_command_chain("echo \"hello && world\"");
        assert_eq!(result, vec!["echo \"hello && world\""]);
    }

    #[test]
    fn test_parse_newline_separator() {
        let result = parse_command_chain("git status\ngit log");
        assert_eq!(result, vec!["git status", "git log"]);
    }

    // ---- is_auto_approved_single_command ----

    #[test]
    fn test_is_auto_approved_empty_command() {
        assert!(is_auto_approved_single_command("", &[], None));
    }

    #[test]
    fn test_is_auto_approved_no_allowlist() {
        assert!(!is_auto_approved_single_command(
            "git",
            &[],
            None
        ));
    }

    #[test]
    fn test_is_auto_approved_wildcard_no_deny() {
        let allowed: Vec<String> = vec!["*".into()];
        assert!(is_auto_approved_single_command(
            "anything",
            &allowed,
            None
        ));
    }

    #[test]
    fn test_is_auto_approved_wildcard_with_deny_match() {
        let allowed: Vec<String> = vec!["*".into()];
        let denied: Vec<String> = vec!["rm".into()];
        assert!(!is_auto_approved_single_command(
            "rm -rf /",
            &allowed,
            Some(&denied)
        ));
    }

    #[test]
    fn test_is_auto_approved_wildcard_with_deny_no_match() {
        let allowed: Vec<String> = vec!["*".into()];
        let denied: Vec<String> = vec!["rm".into()];
        assert!(is_auto_approved_single_command(
            "git status",
            &allowed,
            Some(&denied)
        ));
    }

    // ---- is_auto_denied_single_command ----

    #[test]
    fn test_is_auto_denied_empty_command() {
        assert!(!is_auto_denied_single_command("", &[], None));
    }

    #[test]
    fn test_is_auto_denied_no_denylist() {
        assert!(!is_auto_denied_single_command(
            "rm",
            &[],
            None
        ));
    }

    #[test]
    fn test_is_auto_denied_denylist_match() {
        let denied: Vec<String> = vec!["rm".into()];
        assert!(is_auto_denied_single_command(
            "rm -rf /",
            &[],
            Some(&denied)
        ));
    }

    #[test]
    fn test_is_auto_denied_allowlist_longer() {
        let allowed: Vec<String> = vec!["git push --dry-run".into()];
        let denied: Vec<String> = vec!["git push".into()];
        assert!(!is_auto_denied_single_command(
            "git push --dry-run",
            &allowed,
            Some(&denied)
        ));
    }
}
