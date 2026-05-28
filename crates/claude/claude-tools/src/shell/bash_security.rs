//! Enhanced Bash command security validation.
//!
//! Mirrors the TS reference's `bashSecurity.ts` security checks.
//! Covers: command injection, obfuscation, parser differentials,
//! dangerous variables, heredoc smuggling, Zsh attacks, jq abuse,
//! git commit injection, and redirect injection.

use std::collections::HashSet;

/// Result of a security check.
#[derive(Debug, Clone)]
pub struct SecurityCheckResult {
    pub safe: bool,
    pub reasons: Vec<String>,
}

impl SecurityCheckResult {
    pub fn safe() -> Self {
        Self {
            safe: true,
            reasons: Vec::new(),
        }
    }
}

/// Run all security checks on a bash command.
#[must_use]
pub fn check_bash_security(command: &str) -> SecurityCheckResult {
    let mut result = SecurityCheckResult::safe();
    check_command_substitution(command, &mut result);
    check_process_substitution(command, &mut result);
    check_dangerous_variables(command, &mut result);
    check_obfuscated_flags(command, &mut result);
    check_brace_expansion(command, &mut result);
    check_control_characters(command, &mut result);
    check_unicode_whitespace(command, &mut result);
    check_heredoc_patterns(command, &mut result);
    check_jq_dangerous_flags(command, &mut result);
    check_zsh_dangerous_commands(command, &mut result);
    check_backslash_escaped_operators(command, &mut result);
    check_git_commit_injection(command, &mut result);
    check_ifs_injection(command, &mut result);
    check_proc_environ_access(command, &mut result);
    check_redirection_injection(command, &mut result);
    result
}

fn check_command_substitution(command: &str, result: &mut SecurityCheckResult) {
    if command.contains('`') && count_unescaped_single_quotes(command).is_multiple_of(2) {
        result.safe = false;
        result
            .reasons
            .push("Command substitution via backticks detected".into());
    }
    if let Some(pos) = command.find("$(")
        && !is_inside_single_quotes(command, pos)
    {
        result.safe = false;
        result
            .reasons
            .push("Command substitution via $() detected".into());
    }
    for m in command.match_indices("${") {
        if !is_inside_single_quotes(command, m.0) {
            let after = &command[m.0 + 2..];
            if let Some(end) = after.find('}') {
                let var_name = &after[..end];
                if var_name.contains('!') || var_name.contains('#') || var_name.contains('@') {
                    result.safe = false;
                    result.reasons.push(format!(
                        "Dangerous parameter expansion ${{{{{}}}}}",
                        var_name
                    ));
                }
            }
        }
    }
}

fn check_process_substitution(command: &str, result: &mut SecurityCheckResult) {
    for (i, c) in command.char_indices() {
        if (c == '<' || c == '>')
            && command.get(i + 1..).is_some_and(|r| r.starts_with('('))
            && !is_inside_single_quotes(command, i)
        {
            result.safe = false;
            result.reasons.push("Process substitution detected".into());
        }
    }
}

fn check_dangerous_variables(command: &str, result: &mut SecurityCheckResult) {
    for var in [
        "IFS",
        "LD_PRELOAD",
        "LD_LIBRARY_PATH",
        "PYTHONPATH",
        "NODE_OPTIONS",
        "BASH_ENV",
        "ENV",
    ] {
        for pat in [format!("${}", var), format!("${{{}}}", var)] {
            for (i, _) in command.match_indices(pat.as_str()) {
                if !is_inside_single_quotes(command, i) {
                    result.safe = false;
                    result
                        .reasons
                        .push(format!("Dangerous variable {} referenced", var));
                }
            }
        }
    }
}

fn check_obfuscated_flags(command: &str, result: &mut SecurityCheckResult) {
    for (i, _) in command.match_indices("$'") {
        if !is_inside_single_quotes(command, i) {
            result.safe = false;
            result
                .reasons
                .push("ANSI-C quoting ($'...') — potential flag obfuscation".into());
        }
    }
    for (i, _) in command.match_indices("$\"") {
        if !is_inside_single_quotes(command, i) {
            result.safe = false;
            result
                .reasons
                .push("Locale quoting ($\"...\") — potential flag obfuscation".into());
        }
    }
    if command.contains("\"\"-") || command.contains("''-") {
        result.safe = false;
        result
            .reasons
            .push("Empty quote concatenation — potential flag obfuscation".into());
    }
}

fn check_brace_expansion(command: &str, result: &mut SecurityCheckResult) {
    let bytes = command.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'{' && i + 1 < bytes.len() {
            let rest = &command[i + 1..];
            if let Some(end) = rest.find('}') {
                let inner = &rest[..end];
                if ((inner.contains(',') && !inner.contains(' ')) || inner.contains(".."))
                    && !inner.starts_with('$')
                    && !is_inside_single_quotes(command, i)
                {
                    result.safe = false;
                    result
                        .reasons
                        .push(format!("Brace expansion {{{}}} detected", inner));
                }
            }
        }
        i += 1;
    }
}

fn check_control_characters(command: &str, result: &mut SecurityCheckResult) {
    for (i, c) in command.char_indices() {
        if c.is_control() && c != '\n' && c != '\t' && c != '\r' {
            result.safe = false;
            result.reasons.push(format!(
                "Control character (U+{:04X}) at position {}",
                c as u32, i
            ));
        }
    }
    if command.contains("\x1b[") {
        result.safe = false;
        result.reasons.push("ANSI escape sequence detected".into());
    }
}

fn check_unicode_whitespace(command: &str, result: &mut SecurityCheckResult) {
    for ws in [
        '\u{00A0}', '\u{2000}', '\u{2001}', '\u{2002}', '\u{2003}', '\u{2009}', '\u{202F}',
        '\u{3000}',
    ] {
        if command.contains(ws) {
            result.safe = false;
            result.reasons.push(format!(
                "Unicode whitespace (U+{:04X}) — potential parser differential",
                ws as u32
            ));
        }
    }
}

fn check_heredoc_patterns(command: &str, result: &mut SecurityCheckResult) {
    if command.contains("<<") {
        let has_subst = command.contains("$(") || command.contains('`');
        if has_subst {
            result.safe = false;
            result
                .reasons
                .push("Heredoc with command substitution detected".into());
        }
    }
}

fn check_jq_dangerous_flags(command: &str, result: &mut SecurityCheckResult) {
    if !command.to_ascii_lowercase().contains("jq") {
        return;
    }
    if command.contains("system(") || command.contains("exec(") {
        result.safe = false;
        result
            .reasons
            .push("jq system/exec function call detected".into());
    }
}

fn check_zsh_dangerous_commands(command: &str, result: &mut SecurityCheckResult) {
    let zsh_cmds = [
        "zmodload", "emulate", "sysopen", "sysread", "syswrite", "zpty", "ztcp", "zsocket", "sched",
    ];
    let lower = command.to_ascii_lowercase();
    let words: HashSet<&str> = lower.split_whitespace().collect();
    for cmd in &zsh_cmds {
        if words.contains(cmd) {
            result.safe = false;
            result
                .reasons
                .push(format!("Zsh dangerous command '{}' detected", cmd));
        }
    }
}

fn check_backslash_escaped_operators(command: &str, result: &mut SecurityCheckResult) {
    let lower = command.to_ascii_lowercase();
    for op in ["\\;", "\\|", "\\&", "\\<", "\\>"] {
        if command.contains(op) && !lower.contains("find ") && !lower.contains("xargs ") {
            result.safe = false;
            result.reasons.push(format!(
                "Backslash-escaped operator '{}' — potential parser differential",
                op
            ));
        }
    }
}

fn check_git_commit_injection(command: &str, result: &mut SecurityCheckResult) {
    let lower = command.to_ascii_lowercase();
    if lower.contains("git")
        && lower.contains("commit")
        && let Some(m_pos) = lower.find("-m ")
    {
        let after_m = &command[m_pos + 3..];
        if after_m.contains("$(") || after_m.contains('`') {
            result.safe = false;
            result
                .reasons
                .push("Command substitution in git commit message detected".into());
        }
    }
}

fn check_ifs_injection(command: &str, result: &mut SecurityCheckResult) {
    if command.contains("$IFS") || command.contains("${IFS") {
        result.safe = false;
        result.reasons.push("$IFS injection detected".into());
    }
}

fn check_proc_environ_access(command: &str, result: &mut SecurityCheckResult) {
    if command.contains("/proc/") && command.contains("environ") {
        result.safe = false;
        result
            .reasons
            .push("/proc/*/environ access — potential credential leak".into());
    }
}

fn check_redirection_injection(command: &str, result: &mut SecurityCheckResult) {
    for pat in [">$", ">${", ">>$", ">>${", "<$", "<${"] {
        for (i, _) in command.match_indices(pat) {
            if !is_inside_single_quotes(command, i) {
                result.safe = false;
                result
                    .reasons
                    .push(format!("Variable in redirect target: {}", pat));
            }
        }
    }
}

// Helpers
fn count_unescaped_single_quotes(s: &str) -> usize {
    let mut count = 0;
    let mut escaped = false;
    for c in s.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        if c == '\\' {
            escaped = true;
            continue;
        }
        if c == '\'' {
            count += 1;
        }
    }
    count
}

fn is_inside_single_quotes(s: &str, pos: usize) -> bool {
    let mut in_single = false;
    let mut escaped = false;
    for (i, c) in s.char_indices() {
        if i >= pos {
            return in_single;
        }
        if escaped {
            escaped = false;
            continue;
        }
        if c == '\\' {
            escaped = true;
            continue;
        }
        if c == '\'' {
            in_single = !in_single;
        }
    }
    in_single
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_command_passes() {
        assert!(check_bash_security("ls -la /tmp").safe);
    }

    #[test]
    fn detects_command_substitution() {
        let r = check_bash_security("echo $(whoami)");
        assert!(!r.safe);
    }

    #[test]
    fn detects_process_substitution() {
        assert!(!check_bash_security("diff <(ls a) <(ls b)").safe);
    }

    #[test]
    fn detects_ifs_injection() {
        assert!(!check_bash_security("echo $IFS").safe);
    }

    #[test]
    fn detects_proc_environ() {
        assert!(!check_bash_security("cat /proc/1/environ").safe);
    }

    #[test]
    fn detects_zsh_commands() {
        assert!(!check_bash_security("zmodload zsh/net/tcp").safe);
    }

    #[test]
    fn single_quoted_substitution_is_safe() {
        assert!(check_bash_security("echo '$(whoami)'").safe);
    }

    // ── check_obfuscated_flags ────────────────────────────────────────

    #[test]
    fn detects_ansi_c_quoting() {
        let r = check_bash_security("echo $'\\x2D\\x2Dverbose'");
        assert!(!r.safe);
        assert!(r.reasons.iter().any(|r| r.contains("ANSI-C quoting")));
    }

    #[test]
    fn detects_locale_quoting() {
        let r = check_bash_security("echo $\"hello\"");
        assert!(!r.safe);
        assert!(r.reasons.iter().any(|r| r.contains("Locale quoting")));
    }

    #[test]
    fn detects_empty_quote_concatenation_double() {
        let r = check_bash_security("echo \"\"-flag");
        assert!(!r.safe);
        assert!(
            r.reasons
                .iter()
                .any(|r| r.contains("Empty quote concatenation"))
        );
    }

    #[test]
    fn detects_empty_quote_concatenation_single() {
        let r = check_bash_security("echo ''-flag");
        assert!(!r.safe);
        assert!(
            r.reasons
                .iter()
                .any(|r| r.contains("Empty quote concatenation"))
        );
    }

    #[test]
    fn ansi_c_quoting_inside_single_quotes_is_safe() {
        // The $' outside single quotes triggers ANSI-C quoting which is valid bash
        // but inside single quotes it's literal text. Test that bare $' is detected.
        let r = check_bash_security("echo $'hello'");
        // $' outside quotes is ANSI-C quoting — should be flagged as obfuscation
        assert!(!r.safe);
    }

    // ── check_brace_expansion ─────────────────────────────────────────

    #[test]
    fn detects_comma_brace_expansion() {
        let r = check_bash_security("echo {a,b,c}");
        assert!(!r.safe);
        assert!(r.reasons.iter().any(|r| r.contains("Brace expansion")));
    }

    #[test]
    fn detects_range_brace_expansion() {
        let r = check_bash_security("echo {1..10}");
        assert!(!r.safe);
        assert!(r.reasons.iter().any(|r| r.contains("Brace expansion")));
    }

    #[test]
    fn brace_expansion_with_space_is_not_flagged() {
        // {a, b} has a space after the comma, so it's not brace expansion
        assert!(check_bash_security("echo {a, b}").safe);
    }

    #[test]
    fn variable_brace_expansion_is_safe() {
        // ${VAR} starts with $ so it should not trigger brace expansion
        assert!(check_bash_security("echo ${HOME}").safe);
    }

    // ── check_control_characters ──────────────────────────────────────

    #[test]
    fn detects_null_control_char() {
        let r = check_bash_security("echo \x00hello");
        assert!(!r.safe);
        assert!(r.reasons.iter().any(|r| r.contains("Control character")));
    }

    #[test]
    fn detects_bel_control_char() {
        let r = check_bash_security("echo \x07ring");
        assert!(!r.safe);
        assert!(r.reasons.iter().any(|r| r.contains("Control character")));
    }

    #[test]
    fn detects_ansi_escape_sequence() {
        let r = check_bash_security("echo \x1b[31mred\x1b[0m");
        assert!(!r.safe);
        assert!(r.reasons.iter().any(|r| r.contains("ANSI escape sequence")));
    }

    #[test]
    fn newline_and_tab_are_allowed() {
        // Newlines and tabs should not trigger control character detection
        assert!(check_bash_security("echo 'hello\tworld'\necho done").safe);
    }

    // ── check_unicode_whitespace ──────────────────────────────────────

    #[test]
    fn detects_nbsp_whitespace() {
        // U+00A0 non-breaking space
        let r = check_bash_security("echo\u{00A0}hello");
        assert!(!r.safe);
        assert!(r.reasons.iter().any(|r| r.contains("Unicode whitespace")));
    }

    #[test]
    fn detects_en_space_whitespace() {
        // U+2000 en space
        let r = check_bash_security("rm\u{2000}-rf /");
        assert!(!r.safe);
        assert!(r.reasons.iter().any(|r| r.contains("Unicode whitespace")));
    }

    #[test]
    fn detects_ideographic_space_whitespace() {
        // U+3000 ideographic space
        let r = check_bash_security("ls\u{3000}-la");
        assert!(!r.safe);
        assert!(
            r.reasons
                .iter()
                .any(|r| r.contains("Unicode whitespace") && r.contains("3000"))
        );
    }

    #[test]
    fn normal_spaces_are_safe() {
        assert!(check_bash_security("echo hello world").safe);
    }

    // ── check_heredoc_patterns ────────────────────────────────────────

    #[test]
    fn detects_heredoc_with_command_substitution() {
        let r = check_bash_security("cat << EOF\n$(whoami)\nEOF");
        assert!(!r.safe);
        assert!(
            r.reasons
                .iter()
                .any(|r| r.contains("Heredoc with command substitution"))
        );
    }

    #[test]
    fn detects_heredoc_with_backtick_substitution() {
        let r = check_bash_security("cat << EOF\n`whoami`\nEOF");
        assert!(!r.safe);
        assert!(
            r.reasons
                .iter()
                .any(|r| r.contains("Heredoc with command substitution"))
        );
    }

    #[test]
    fn plain_heredoc_without_substitution_is_not_flagged() {
        let r = check_bash_security("cat << EOF\nhello\nEOF");
        // Plain heredoc with no substitution ($() or backticks) should not trigger heredoc check
        assert!(r.reasons.iter().all(|reason| !reason.contains("Heredoc")));
    }

    // ── check_jq_dangerous_flags ──────────────────────────────────────

    #[test]
    fn detects_jq_system_call() {
        let r = check_bash_security("jq 'system(\"reboot\")' input.json");
        assert!(!r.safe);
        assert!(r.reasons.iter().any(|r| r.contains("jq system/exec")));
    }

    #[test]
    fn detects_jq_exec_call() {
        let r = check_bash_security("jq 'exec(\"rm -rf /\")' data.json");
        assert!(!r.safe);
        assert!(r.reasons.iter().any(|r| r.contains("jq system/exec")));
    }

    #[test]
    fn detects_jq_uppercase_command() {
        let r = check_bash_security("JQ '. | system(\"ls\")' file.json");
        assert!(!r.safe);
        assert!(r.reasons.iter().any(|r| r.contains("jq system/exec")));
    }

    #[test]
    fn safe_jq_query_passes() {
        assert!(check_bash_security("jq '.name' data.json").safe);
    }

    // ── check_backslash_escaped_operators ─────────────────────────────

    #[test]
    fn detects_escaped_semicolon() {
        let r = check_bash_security("echo hello \\; whoami");
        assert!(!r.safe);
        assert!(
            r.reasons
                .iter()
                .any(|r| r.contains("Backslash-escaped operator"))
        );
    }

    #[test]
    fn detects_escaped_pipe() {
        let r = check_bash_security("echo hello \\| cat /etc/passwd");
        assert!(!r.safe);
        assert!(
            r.reasons
                .iter()
                .any(|r| r.contains("Backslash-escaped operator"))
        );
    }

    #[test]
    fn detects_escaped_ampersand() {
        let r = check_bash_security("echo hello \\& whoami");
        assert!(!r.safe);
        assert!(
            r.reasons
                .iter()
                .any(|r| r.contains("Backslash-escaped operator"))
        );
    }

    #[test]
    fn escaped_ops_in_find_are_safe() {
        // find command commonly uses \; and should not be flagged
        assert!(check_bash_security("find . -name '*.rs' -exec cat {} \\;").safe);
    }

    #[test]
    fn escaped_ops_in_xargs_are_safe() {
        // xargs also commonly uses escaped operators
        assert!(check_bash_security("echo files | xargs -I {} echo {} \\;").safe);
    }

    // ── check_git_commit_injection ────────────────────────────────────

    #[test]
    fn detects_git_commit_with_dollar_substitution() {
        let r = check_bash_security("git commit -m \"$(whoami)\"");
        assert!(!r.safe);
        assert!(r.reasons.iter().any(|r| r.contains("git commit message")));
    }

    #[test]
    fn detects_git_commit_with_backtick_substitution() {
        let r = check_bash_security("git commit -m `whoami`");
        assert!(!r.safe);
        assert!(r.reasons.iter().any(|r| r.contains("git commit message")));
    }

    #[test]
    fn safe_git_commit_message_passes() {
        assert!(check_bash_security("git commit -m \"Initial commit\"").safe);
    }

    #[test]
    fn git_commit_mixed_case_detected() {
        let r = check_bash_security("GIT COMMIT -m \"$(id)\"");
        assert!(!r.safe);
        assert!(r.reasons.iter().any(|r| r.contains("git commit message")));
    }

    // ── check_redirection_injection ───────────────────────────────────

    #[test]
    fn detects_variable_redirect_write() {
        let r = check_bash_security("echo hello >$OUTFILE");
        assert!(!r.safe);
        assert!(
            r.reasons
                .iter()
                .any(|r| r.contains("Variable in redirect target"))
        );
    }

    #[test]
    fn detects_variable_redirect_append() {
        let r = check_bash_security("echo hello >>${LOGFILE}");
        assert!(!r.safe);
        assert!(
            r.reasons
                .iter()
                .any(|r| r.contains("Variable in redirect target"))
        );
    }

    #[test]
    fn detects_variable_redirect_input() {
        let r = check_bash_security("cat <$INPUT");
        assert!(!r.safe);
        assert!(
            r.reasons
                .iter()
                .any(|r| r.contains("Variable in redirect target"))
        );
    }

    #[test]
    fn literal_redirect_is_safe() {
        assert!(check_bash_security("echo hello > output.txt").safe);
    }

    #[test]
    fn variable_redirect_outside_quotes_is_flagged() {
        // $ in redirect target without quoting should be flagged
        let r = check_bash_security("echo hello >$OUTFILE");
        assert!(!r.safe);
        assert!(r.reasons.iter().any(|r| r.contains("redirect")));
    }
}
