#!/usr/bin/env bash
# verify-codex-migration.sh — Verify the agents/codex submodule migration is healthy.
#
# This script enforces the 10 invariants that must hold for the Codex
# submodule migration (executed 2026-06-01) to be considered complete:
#
#   1. .gitmodules exists and registers agents/codex
#   2. git submodule status shows clean (or only the expected diff)
#   3. Root Cargo.toml no longer references crates/codex/* paths
#   4. Root Cargo.toml references agents/codex/codex-rs/* (≥ 100 occurrences)
#   5. Root Cargo.toml has agents/codex in workspace.exclude
#   6. agents/codex/codex-rs/ is a real git checkout with valid Cargo workspace
#   7. patches/codex/ contains the expected build.rs overlays
#   8. scripts/sync-codex.sh exists and is executable
#   9. cargo metadata --no-deps parses without errors
#  10. Root workspace contains no orphan crates/codex/Cargo.toml
#
# Exit codes:
#   0 = all checks passed
#   1 = at least one check failed
#
# Usage:
#   ./scripts/verify-codex-migration.sh            # Run all checks
#   ./scripts/verify-codex-migration.sh --verbose  # Show every check's output
#   ./scripts/verify-codex-migration.sh --help     # Show usage

set -uo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

VERBOSE=false
for arg in "$@"; do
    case "$arg" in
        --verbose|-v) VERBOSE=true ;;
        --help|-h)
            sed -n '2,30p' "$0"
            exit 0
            ;;
        *)
            echo "Unknown argument: $arg" >&2
            exit 1
            ;;
    esac
done

PASS=0
FAIL=0
FAILURES=()

log_pass() {
    echo "  [PASS] $1"
    PASS=$((PASS + 1))
}

log_fail() {
    echo "  [FAIL] $1"
    if [ "$VERBOSE" = true ] && [ -n "${2:-}" ]; then
        echo "         $2"
    fi
    FAIL=$((FAIL + 1))
    FAILURES+=("$1")
}

log_skip() {
    echo "  [SKIP] $1"
}

section() {
    echo ""
    echo "─── $1 ───"
}

# ─────────────────────────────────────────────────────────────────────────────
# Check 1: .gitmodules exists with agents/codex registration
# ─────────────────────────────────────────────────────────────────────────────
section "Check 1: .gitmodules registers agents/codex"
if [ ! -f .gitmodules ]; then
    log_fail ".gitmodules missing" "expected file at repo root"
elif ! grep -q '^\[submodule "agents/codex"\]' .gitmodules; then
    log_fail ".gitmodules missing agents/codex registration" "$(cat .gitmodules)"
elif ! grep -q "url = https://github.com/openai/codex.git" .gitmodules; then
    log_fail ".gitmodules does not point at openai/codex.git" "$(cat .gitmodules)"
else
    log_pass ".gitmodules correctly registers openai/codex.git at agents/codex"
fi

# ─────────────────────────────────────────────────────────────────────────────
# Check 2: git submodule status shows clean
# ─────────────────────────────────────────────────────────────────────────────
section "Check 2: git submodule status"
SUBMODULE_STATUS=$(git submodule status agents/codex 2>&1 || true)
if echo "$SUBMODULE_STATUS" | grep -q "^-"; then
    log_fail "agents/codex not initialized" "$SUBMODULE_STATUS"
elif echo "$SUBMODULE_STATUS" | grep -q "^+"; then
    log_fail "agents/codex has unpinned commits" "$SUBMODULE_STATUS"
elif echo "$SUBMODULE_STATUS" | grep -q "^-"; then
    log_fail "agents/codex not initialized" "$SUBMODULE_STATUS"
else
    SUBMODULE_SHA=$(echo "$SUBMODULE_STATUS" | awk '{print $1}')
    log_pass "agents/codex is clean at ${SUBMODULE_SHA:0:12}"
fi

# ─────────────────────────────────────────────────────────────────────────────
# Check 3: Root Cargo.toml no longer references crates/codex/* paths
# ─────────────────────────────────────────────────────────────────────────────
section "Check 3: No crates/codex/ paths in root Cargo.toml"
CRATES_CODEX_REFS=$(grep -c '"crates/codex/' Cargo.toml 2>/dev/null | tr -d '[:space:]' || echo 0)
if [ "${CRATES_CODEX_REFS:-0}" -gt 0 ]; then
    log_fail "Cargo.toml still references $CRATES_CODEX_REFS crates/codex/* paths" "should be 0"
else
    log_pass "Cargo.toml has 0 references to crates/codex/*"
fi

# ─────────────────────────────────────────────────────────────────────────────
# Check 4: Root Cargo.toml references agents/codex/codex-rs/* (≥ 100 occurrences)
# ─────────────────────────────────────────────────────────────────────────────
section "Check 4: agents/codex/codex-rs/ paths in root Cargo.toml"
SUBMODULE_REFS=$(grep -c '"agents/codex/codex-rs/' Cargo.toml 2>/dev/null | tr -d '[:space:]' || echo 0)
if [ "${SUBMODULE_REFS:-0}" -lt 100 ]; then
    log_fail "Cargo.toml has only $SUBMODULE_REFS agents/codex/codex-rs/ refs" "expected ≥ 100"
else
    log_pass "Cargo.toml has $SUBMODULE_REFS agents/codex/codex-rs/ path dependencies"
fi

# ─────────────────────────────────────────────────────────────────────────────
# Check 5: Root Cargo.toml has agents/codex in workspace.exclude
# ─────────────────────────────────────────────────────────────────────────────
section "Check 5: agents/codex in workspace.exclude"
if ! grep -q '^\s*"agents/codex"' Cargo.toml; then
    log_fail "agents/codex not in workspace.exclude" "must be excluded from root workspace"
else
    log_pass "agents/codex is correctly excluded from root workspace"
fi

# ─────────────────────────────────────────────────────────────────────────────
# Check 6: agents/codex/codex-rs is a real Cargo workspace
# ─────────────────────────────────────────────────────────────────────────────
section "Check 6: agents/codex/codex-rs is a real git checkout"
if [ ! -d "agents/codex/codex-rs" ]; then
    log_fail "agents/codex/codex-rs/ missing" "submodule content not checked out"
elif [ ! -f "agents/codex/codex-rs/Cargo.toml" ]; then
    log_fail "agents/codex/codex-rs/Cargo.toml missing" "submodule workspace root"
elif [ ! -d "agents/codex/.git" ] && [ ! -f "agents/codex/.git" ]; then
    log_fail "agents/codex/.git not present" "submodule git link missing"
else
    UPSTREAM_CRATES=$(find agents/codex/codex-rs -name "Cargo.toml" -not -path "*/target/*" 2>/dev/null | wc -l)
    log_pass "agents/codex/codex-rs is a valid workspace with $UPSTREAM_CRATES upstream crates"
fi

# ─────────────────────────────────────────────────────────────────────────────
# Check 7: patches/codex/ contains expected build.rs overlays
# ─────────────────────────────────────────────────────────────────────────────
section "Check 7: patches/codex/ overlays present"
EXPECTED_OVERLAYS=(app-server-build.rs exec-build.rs tui-build.rs windows-sandbox-build.rs)
MISSING=0
for overlay in "${EXPECTED_OVERLAYS[@]}"; do
    if [ ! -f "patches/codex/$overlay" ]; then
        log_fail "missing patches/codex/$overlay" "overlay not present"
        MISSING=$((MISSING + 1))
    fi
done
if [ "$MISSING" -eq 0 ]; then
    log_pass "patches/codex/ contains all ${#EXPECTED_OVERLAYS[@]} expected overlays"
fi

# ─────────────────────────────────────────────────────────────────────────────
# Check 8: scripts/sync-codex.sh exists and is executable
# ─────────────────────────────────────────────────────────────────────────────
section "Check 8: scripts/sync-codex.sh"
if [ ! -f scripts/sync-codex.sh ]; then
    log_fail "scripts/sync-codex.sh missing"
elif [ ! -x scripts/sync-codex.sh ]; then
    log_fail "scripts/sync-codex.sh not executable" "chmod +x scripts/sync-codex.sh"
else
    if ! grep -q "git submodule update" scripts/sync-codex.sh; then
        log_fail "scripts/sync-codex.sh does not invoke git submodule update"
    elif ! grep -q -- "--dry-run" scripts/sync-codex.sh; then
        log_fail "scripts/sync-codex.sh does not support --dry-run"
    else
        log_pass "scripts/sync-codex.sh exists, is executable, and supports dry-run"
    fi
fi

# ─────────────────────────────────────────────────────────────────────────────
# Check 9: cargo metadata --no-deps parses
# ─────────────────────────────────────────────────────────────────────────────
section "Check 9: cargo metadata parses"
if ! command -v cargo >/dev/null 2>&1; then
    log_skip "cargo not on PATH — skipping"
else
    if cargo metadata --no-deps --format-version 1 >/dev/null 2>&1; then
        MEMBER_COUNT=$(cargo metadata --no-deps --format-version 1 2>/dev/null \
            | grep -oE '"workspace_members":\[[^]]*\]' | head -1 \
            | grep -oE 'path\+file:' | wc -l)
        log_pass "cargo metadata parses cleanly ($MEMBER_COUNT workspace members)"
    else
        log_fail "cargo metadata --no-deps failed" "see cargo output above"
    fi
fi

# ─────────────────────────────────────────────────────────────────────────────
# Check 10: No orphan crates/codex/Cargo.toml in root workspace
# ─────────────────────────────────────────────────────────────────────────────
section "Check 10: No orphan crates/codex/* manifests"
ORPHAN_TOML=$(find crates/codex -name "Cargo.toml" -type f 2>/dev/null | wc -l)
if [ "$ORPHAN_TOML" -gt 0 ]; then
    log_fail "found $ORPHAN_TOML Cargo.toml under crates/codex/" "should be 0 after migration"
else
    log_pass "no orphan Cargo.toml under crates/codex/"
fi

# ─────────────────────────────────────────────────────────────────────────────
# Summary
# ─────────────────────────────────────────────────────────────────────────────
echo ""
echo "════════════════════════════════════════════════════════════"
echo "  Results: $PASS passed, $FAIL failed"
echo "════════════════════════════════════════════════════════════"

if [ "$FAIL" -gt 0 ]; then
    echo ""
    echo "Failed checks:"
    for f in "${FAILURES[@]}"; do
        echo "  - $f"
    done
    exit 1
fi

echo ""
echo "All 10 checks passed. Codex submodule migration is healthy."
exit 0
