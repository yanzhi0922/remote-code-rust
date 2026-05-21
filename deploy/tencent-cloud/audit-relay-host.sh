#!/usr/bin/env bash
set -euo pipefail

SERVICE_NAME="${REMOTE_CODE_AUDIT_SERVICE:-remote-code-control-plane.service}"
ENV_FILE="${REMOTE_CODE_AUDIT_ENV_FILE:-/etc/remote-code/control-plane.env}"
APP_ROOT="${REMOTE_CODE_AUDIT_APP_ROOT:-/opt/remote-code}"
HEALTH_URL="${REMOTE_CODE_AUDIT_HEALTH_URL:-http://127.0.0.1:8787/healthz}"

failures=()
warnings=()
passes=()

pass() {
  passes+=("$1")
  printf 'PASS: %s\n' "$1"
}

warn() {
  warnings+=("$1")
  printf 'WARN: %s\n' "$1"
}

fail() {
  failures+=("$1")
  printf 'FAIL: %s\n' "$1"
}

env_value() {
  local key="$1"
  if [[ ! -r "$ENV_FILE" ]]; then
    return 1
  fi
  local line
  line="$(grep -E "^[[:space:]]*${key}=" "$ENV_FILE" | tail -n 1 || true)"
  [[ -n "$line" ]] || return 1
  line="${line#*=}"
  line="${line%\"}"
  line="${line#\"}"
  line="${line%\'}"
  line="${line#\'}"
  printf '%s' "$line"
}

check_systemd() {
  if ! command -v systemctl >/dev/null 2>&1; then
    warn "systemctl unavailable; process and health checks still run"
    return
  fi

  if systemctl is-active --quiet "$SERVICE_NAME"; then
    pass "$SERVICE_NAME is active"
  else
    fail "$SERVICE_NAME is not active"
  fi

  local units
  units="$(systemctl list-units --type=service --state=running --no-legend '*remote-code*' 2>/dev/null | awk '{print $1}' || true)"
  if [[ -z "$units" ]]; then
    fail "no running remote-code service found"
  elif [[ "$units" == "$SERVICE_NAME" ]]; then
    pass "only $SERVICE_NAME is running under systemd"
  else
    fail "unexpected running remote-code systemd units: ${units//$'\n'/, }"
  fi
}

check_processes() {
  local bad
  bad="$(ps -eo pid=,comm=,args= | awk '
    /remote-code-control-plane/ { next }
    /audit-relay-host\.sh/ { next }
    /remote-code-runner/ || /\/remote-code([[:space:]]|$)/ || /[[:space:]]cargo([[:space:]]|$)/ || /[[:space:]]rustc([[:space:]]|$)/ || /agents\/(codex|roo|claude)/ || /codex-rs/ || /Roo-Code/ { print }
  ' || true)"
  if [[ -n "$bad" ]]; then
    fail "forbidden runner/build/agent processes are running: ${bad//$'\n'/; }"
  else
    pass "no runner, CLI, cargo/rustc, or agent processes detected"
  fi
}

check_source_tree_absent() {
  local bad=()
  local path
  for path in \
    "$APP_ROOT/.git" \
    "$APP_ROOT/Cargo.toml" \
    "$APP_ROOT/Cargo.lock" \
    "$APP_ROOT/crates" \
    "$APP_ROOT/apps" \
    "$APP_ROOT/agents" \
    "$APP_ROOT/src" \
    "$APP_ROOT/.research"; do
    if [[ -e "$path" ]]; then
      bad+=("$path")
    fi
  done

  if ((${#bad[@]} > 0)); then
    fail "source/build tree artifacts found under $APP_ROOT: ${bad[*]}"
  else
    pass "$APP_ROOT contains release artifacts only, not a Rust source tree"
  fi
}

check_env_file() {
  if [[ ! -r "$ENV_FILE" ]]; then
    fail "$ENV_FILE is not readable; cannot prove bootstrap/query/provider-key boundary"
    return
  fi
  pass "$ENV_FILE is readable for audit"

  local bootstrap_secret
  bootstrap_secret="$(env_value REMOTE_CODE_CONTROL_PLANE_BOOTSTRAP_SECRET || true)"
  if [[ ${#bootstrap_secret} -ge 32 && "$bootstrap_secret" != *"<SECRET>"* && "$bootstrap_secret" != *"change-me"* ]]; then
    pass "REMOTE_CODE_CONTROL_PLANE_BOOTSTRAP_SECRET is configured and non-placeholder"
  else
    fail "REMOTE_CODE_CONTROL_PLANE_BOOTSTRAP_SECRET is missing, short, or placeholder"
  fi

  local relay_only
  relay_only="$(env_value REMOTE_CODE_CONTROL_PLANE_RELAY_ONLY || true)"
  if [[ "${relay_only,,}" == "true" ]]; then
    pass "REMOTE_CODE_CONTROL_PLANE_RELAY_ONLY=true"
  else
    fail "REMOTE_CODE_CONTROL_PLANE_RELAY_ONLY must be true on relay hosts"
  fi

  local query_switch
  for query_switch in REMOTE_CODE_ALLOW_QUERY_ACCESS_TOKEN REMOTE_CODE_RUNNER_ALLOW_QUERY_ACCESS_TOKEN; do
    local value
    value="$(env_value "$query_switch" || true)"
    case "${value,,}" in
      true|1|yes|on)
        fail "$query_switch is enabled"
        ;;
      *)
        pass "$query_switch is disabled or unset"
        ;;
    esac
  done

  local provider_key_pattern='^[[:space:]]*(ANTHROPIC_API_KEY|OPENAI_API_KEY|MINIMAX_API_KEY|MINIMAX_TOKEN_PLAN_API_KEY|KUAIKAT_[A-Z0-9_]*KEY|DEEPSEEK_[A-Z0-9_]*KEY|GOOGLE_API_KEY|AWS_ACCESS_KEY_ID|AWS_SECRET_ACCESS_KEY|MCP_[A-Z0-9_]*KEY)='
  if grep -Eiq "$provider_key_pattern" "$ENV_FILE"; then
    fail "$ENV_FILE contains provider, cloud, or MCP key variables"
  else
    pass "$ENV_FILE contains no provider, cloud, or MCP key variables"
  fi

  if grep -Eq '^[[:space:]]*REMOTE_CODE_CONTROL_PLANE_USER_KEY_HASHES=' "$ENV_FILE"; then
    pass "REMOTE_CODE_CONTROL_PLANE_USER_KEY_HASHES is configured; attach hash source and rotation evidence"
  else
    pass "REMOTE_CODE_CONTROL_PLANE_USER_KEY_HASHES is unset"
  fi
}

check_health() {
  if ! command -v curl >/dev/null 2>&1; then
    warn "curl unavailable; skipped $HEALTH_URL"
    return
  fi

  local response
  if ! response="$(curl -fsS --max-time 5 "$HEALTH_URL" 2>&1)"; then
    fail "health check failed for $HEALTH_URL: $response"
    return
  fi

  if printf '%s' "$response" | grep -Eq '"ok"[[:space:]]*:[[:space:]]*true'; then
    pass "$HEALTH_URL returned ok=true"
  else
    fail "$HEALTH_URL did not return ok=true"
  fi

  if printf '%s' "$response" | grep -Eq '"auth_required"[[:space:]]*:[[:space:]]*true'; then
    pass "control plane reports auth_required=true"
  else
    fail "control plane does not report auth_required=true"
  fi
}

printf '# Remote Code Relay Host Audit\n\n'
printf 'Service: %s\nEnv file: %s\nApp root: %s\nHealth URL: %s\n\n' "$SERVICE_NAME" "$ENV_FILE" "$APP_ROOT" "$HEALTH_URL"

check_systemd
check_processes
check_source_tree_absent
check_env_file
check_health

printf '\nSummary: %d pass, %d warning, %d failure\n' "${#passes[@]}" "${#warnings[@]}" "${#failures[@]}"

if ((${#failures[@]} > 0)); then
  exit 1
fi
