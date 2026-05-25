use anyhow::{anyhow, Result};
use dirs_next::home_dir;
use std::collections::HashMap;
use std::env;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

pub fn normalize_null_device_env(env_map: &mut HashMap<String, String>) {
    let keys: Vec<String> = env_map.keys().cloned().collect();
    for k in keys {
        if let Some(v) = env_map.get(&k).cloned() {
            let t = v.trim().to_ascii_lowercase();
            if t == "/dev/null" || t == "\\\\\\\\dev\\\\\\\\null" {
                env_map.insert(k, "NUL".to_string());
            }
        }
    }
}

pub fn ensure_non_interactive_pager(env_map: &mut HashMap<String, String>) {
    env_map
        .entry("GIT_PAGER".into())
        .or_insert_with(|| "more.com".into());
    env_map
        .entry("PAGER".into())
        .or_insert_with(|| "more.com".into());
    env_map.entry("LESS".into()).or_insert_with(|| "".into());
}

// Keep PATH and PATHEXT stable for callers that rely on inheriting the parent process env.
pub fn inherit_path_env(env_map: &mut HashMap<String, String>) {
    if !env_map.contains_key("PATH")
        && let Ok(path) = env::var("PATH")
    {
        env_map.insert("PATH".into(), path);
    }
    if !env_map.contains_key("PATHEXT")
        && let Ok(pathext) = env::var("PATHEXT")
    {
        env_map.insert("PATHEXT".into(), pathext);
    }
}

fn prepend_path(env_map: &mut HashMap<String, String>, prefix: &str) {
    let existing = env_map
        .get("PATH")
        .cloned()
        .or_else(|| env::var("PATH").ok())
        .unwrap_or_default();
    let parts: Vec<String> = existing.split(';').map(ToString::to_string).collect();
    if parts
        .first()
        .map(|p| p.eq_ignore_ascii_case(prefix))
        .unwrap_or(false)
    {
        return;
    }
    let mut new_path = String::new();
    new_path.push_str(prefix);
    if !existing.is_empty() {
        new_path.push(';');
        new_path.push_str(&existing);
    }
    env_map.insert("PATH".into(), new_path);
}

fn reorder_pathext_for_stubs(env_map: &mut HashMap<String, String>) {
    let default = env_map
        .get("PATHEXT")
        .cloned()
        .or_else(|| env::var("PATHEXT").ok())
        .unwrap_or(".COM;.EXE;.BAT;.CMD".to_string());
    let exts: Vec<String> = default
        .split(';')
        .filter(|e| !e.is_empty())
        .map(ToString::to_string)
        .collect();
    let exts_norm: Vec<String> = exts.iter().map(|e| e.to_ascii_uppercase()).collect();
    let want = [".BAT", ".CMD"];
    let mut front: Vec<String> = Vec::new();
    for w in want {
        if let Some(idx) = exts_norm.iter().position(|e| e == w) {
            front.push(exts[idx].clone());
        }
    }
    let rest: Vec<String> = exts
        .into_iter()
        .enumerate()
        .filter(|(i, _)| {
            let up = &exts_norm[*i];
            up != ".BAT" && up != ".CMD"
        })
        .map(|(_, e)| e)
        .collect();
    let mut combined = Vec::new();
    combined.extend(front);
    combined.extend(rest);
    env_map.insert("PATHEXT".into(), combined.join(";"));
}

fn ensure_denybin(tools: &[&str], denybin_dir: Option<&Path>) -> Result<PathBuf> {
    let base = match denybin_dir {
        Some(p) => p.to_path_buf(),
        None => {
            let home = home_dir().ok_or_else(|| anyhow!("no home dir"))?;
            home.join(".sbx-denybin")
        }
    };
    fs::create_dir_all(&base)?;
    for tool in tools {
        for ext in [".bat", ".cmd"] {
            let path = base.join(format!("{tool}{ext}"));
            if !path.exists() {
                let mut f = File::create(&path)?;
                f.write_all(b"@echo off\\r\\nexit /b 1\\r\\n")?;
            }
        }
    }
    Ok(base)
}

// ---------------------------------------------------------------------------
// Environment variable isolation
// ---------------------------------------------------------------------------

/// Sensitive environment variable prefixes and exact names that must **never**
/// leak into a sandboxed child process. Covers:
///
/// - Cloud-provider credentials (AWS, Azure, GCP)
/// - CI/CD tokens (GitHub, GitLab, Buildkite)
/// - Generic secrets (SECRET_KEY, PRIVATE_KEY)
/// - Windows credential vault integration
/// - API keys, bearer tokens, session cookies
const SENSITIVE_ENV_PREFIXES: &[&str] = &[
    // AWS
    "AWS_",
    // Azure
    "AZURE_",
    "ARM_",
    // GCP
    "GOOGLE_",
    "GCLOUD_",
    "CLOUDSDK_",
    "GKE_",
    // GitHub / CI
    "GITHUB_TOKEN",
    "GITHUB_PAT",
    "GH_TOKEN",
    "GH_ENTERPRISE_TOKEN",
    "GHES_TOKEN",
    "GITLAB_TOKEN",
    "BUILDKITE_",
    "HEROKU_",
    "VERCEL_",
    "NETLIFY_",
    "DENO_",
    // Vercel / Supabase
    "SUPABASE_",
    "POSTGRES_",
    "DATABASE_",
    // Generic secret patterns
    "SECRET_",
    "PRIVATE_",
    "API_KEY",
    "API_SECRET",
    "AUTH_TOKEN",
    "ACCESS_TOKEN",
    "REFRESH_TOKEN",
    "BEARER_",
    "SESSION_",
    "CSRF_",
    "XSRF_",
    // Windows credential / DPAPI
    "DPAPI_",
    // Sentry / observability (may contain DSNs with keys)
    "SENTRY_DSN",
    "SENTRY_AUTH_TOKEN",
    // OpenAI / Anthropic / LLM keys
    "OPENAI_",
    "ANTHROPIC_",
    "CODEX_",
    // SSH / GPG agent sockets (Unix-like env on Windows via Git Bash, WSL)
    "SSH_AUTH_SOCK",
    "SSH_PRIVATE_KEY",
    "GPG_TTY",
    // Docker / container registry
    "DOCKER_",
];

/// Exact env-var names that are always stripped regardless of prefix.
const SENSITIVE_ENV_EXACT: &[&str] = &[
    "TOKEN",
    "PASSWORD",
    "PASSPHRASE",
    "CREDENTIAL",
    "SECRET",
    "PRIVATE_KEY",
    "KEY",
    "AUTH",
    "COOKIE",
    "SESSION",
    "PAT",
];

/// Return `true` if an environment variable key looks sensitive and should be
/// stripped from the sandbox child environment.
fn is_sensitive_env_key(key: &str) -> bool {
    let upper = key.to_ascii_uppercase();

    // Check exact matches.
    for exact in SENSITIVE_ENV_EXACT {
        if upper == *exact {
            return true;
        }
    }

    // Check prefix matches.
    for prefix in SENSITIVE_ENV_PREFIXES {
        if upper.starts_with(prefix) {
            return true;
        }
    }

    false
}

/// Remove sensitive environment variables from the map that will be passed to
/// the sandboxed child process. This is a **deny-list** approach: we strip
/// known-sensitive keys while preserving everything else (PATH, HOME, etc.).
///
/// Returns the number of keys stripped so callers can log the fact.
pub fn strip_sensitive_env_vars(env_map: &mut HashMap<String, String>) -> usize {
    let before = env_map.len();
    env_map.retain(|k, _| !is_sensitive_env_key(k));
    before - env_map.len()
}

// ---------------------------------------------------------------------------
// Network-offline environment rewrites
// ---------------------------------------------------------------------------

pub fn apply_no_network_to_env(env_map: &mut HashMap<String, String>) -> Result<()> {
    env_map.insert("SBX_NONET_ACTIVE".into(), "1".into());
    env_map
        .entry("HTTP_PROXY".into())
        .or_insert_with(|| "http://127.0.0.1:9".into());
    env_map
        .entry("HTTPS_PROXY".into())
        .or_insert_with(|| "http://127.0.0.1:9".into());
    env_map
        .entry("ALL_PROXY".into())
        .or_insert_with(|| "http://127.0.0.1:9".into());
    env_map
        .entry("NO_PROXY".into())
        .or_insert_with(|| "localhost,127.0.0.1,::1".into());
    env_map
        .entry("PIP_NO_INDEX".into())
        .or_insert_with(|| "1".into());
    env_map
        .entry("PIP_DISABLE_PIP_VERSION_CHECK".into())
        .or_insert_with(|| "1".into());
    env_map
        .entry("NPM_CONFIG_OFFLINE".into())
        .or_insert_with(|| "true".into());
    env_map
        .entry("CARGO_NET_OFFLINE".into())
        .or_insert_with(|| "true".into());
    env_map
        .entry("GIT_HTTP_PROXY".into())
        .or_insert_with(|| "http://127.0.0.1:9".into());
    env_map
        .entry("GIT_HTTPS_PROXY".into())
        .or_insert_with(|| "http://127.0.0.1:9".into());
    env_map
        .entry("GIT_SSH_COMMAND".into())
        .or_insert_with(|| "cmd /c exit 1".into());
    env_map
        .entry("GIT_ALLOW_PROTOCOLS".into())
        .or_insert_with(|| "".into());

    let base = ensure_denybin(&["ssh", "scp"], /*denybin_dir*/ None)?;
    for tool in ["curl", "wget"] {
        for ext in [".bat", ".cmd"] {
            let p = base.join(format!("{tool}{ext}"));
            if p.exists() {
                let _ = fs::remove_file(&p);
            }
        }
    }
    prepend_path(env_map, &base.to_string_lossy());
    reorder_pathext_for_stubs(env_map);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn strips_aws_credentials() {
        let mut env = HashMap::from([
            ("AWS_ACCESS_KEY_ID".into(), "AKIA...".into()),
            ("AWS_SECRET_ACCESS_KEY".into(), "wJalr...".into()),
            ("AWS_SESSION_TOKEN".into(), "FQoG...".into()),
            ("PATH".into(), "/usr/bin".into()),
        ]);
        let stripped = strip_sensitive_env_vars(&mut env);
        assert_eq!(stripped, 3);
        assert!(env.contains_key("PATH"));
        assert!(!env.contains_key("AWS_ACCESS_KEY_ID"));
        assert!(!env.contains_key("AWS_SECRET_ACCESS_KEY"));
        assert!(!env.contains_key("AWS_SESSION_TOKEN"));
    }

    #[test]
    fn strips_github_token() {
        let mut env = HashMap::from([
            ("GITHUB_TOKEN".into(), "ghp_...".into()),
            ("HOME".into(), "/home/user".into()),
        ]);
        let stripped = strip_sensitive_env_vars(&mut env);
        assert_eq!(stripped, 1);
        assert!(env.contains_key("HOME"));
        assert!(!env.contains_key("GITHUB_TOKEN"));
    }

    #[test]
    fn strips_exact_password_key() {
        let mut env = HashMap::from([
            ("PASSWORD".into(), "hunter2".into()),
            ("KEY".into(), "secret".into()),
            ("PAT".into(), "ghp_abc".into()),
            ("USER".into(), "alice".into()),
        ]);
        let stripped = strip_sensitive_env_vars(&mut env);
        assert_eq!(stripped, 3);
        assert!(env.contains_key("USER"));
    }

    #[test]
    fn preserves_safe_vars() {
        let mut env = HashMap::from([
            ("PATH".into(), "/usr/bin".into()),
            ("HOME".into(), "/home/user".into()),
            ("TEMP".into(), "/tmp".into()),
            ("LANG".into(), "en_US.UTF-8".into()),
            ("GIT_PAGER".into(), "cat".into()),
            ("EDITOR".into(), "vim".into()),
        ]);
        let stripped = strip_sensitive_env_vars(&mut env);
        assert_eq!(stripped, 0);
        assert_eq!(env.len(), 6);
    }

    #[test]
    fn case_insensitive_matching() {
        let mut env = HashMap::from([
            ("password".into(), "lower".into()),
            ("Password".into(), "mixed".into()),
            ("aws_secret_access_key".into(), "lower_prefix".into()),
        ]);
        let stripped = strip_sensitive_env_vars(&mut env);
        assert_eq!(stripped, 3);
    }

    #[test]
    fn is_sensitive_detects_prefix_matches() {
        assert!(is_sensitive_env_key("OPENAI_API_KEY"));
        assert!(is_sensitive_env_key("ANTHROPIC_API_KEY"));
        assert!(is_sensitive_env_key("AZURE_SUBSCRIPTION_ID"));
        assert!(is_sensitive_env_key("DOCKER_HOST"));
    }

    #[test]
    fn is_sensitive_allows_safe_keys() {
        assert!(!is_sensitive_env_key("PATH"));
        assert!(!is_sensitive_env_key("HOME"));
        assert!(!is_sensitive_env_key("TEMP"));
        assert!(!is_sensitive_env_key("LANG"));
        assert!(!is_sensitive_env_key("GIT_PAGER"));
        assert!(!is_sensitive_env_key("EDITOR"));
        assert!(!is_sensitive_env_key("SBX_NONET_ACTIVE"));
    }
}
