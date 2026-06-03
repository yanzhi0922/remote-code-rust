#!/usr/bin/env bash
# sync-codex.sh — Sync the codex submodule with upstream and apply local overlays.
#
# Usage:
#   ./scripts/sync-codex.sh              # Normal sync
#   ./scripts/sync-codex.sh --dry-run    # Preview without making changes
#   ./scripts/sync-codex.sh --check      # Sync + cargo check verification

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
CODEX_DIR="$ROOT_DIR/agents/codex"
OVERLAY_DIR="$ROOT_DIR/patches/codex"

DRY_RUN=false
DO_CHECK=false

for arg in "$@"; do
    case "$arg" in
        --dry-run) DRY_RUN=true ;;
        --check)   DO_CHECK=true ;;
        --help|-h)
            echo "Usage: $0 [--dry-run] [--check]"
            echo ""
            echo "  --dry-run   Preview changes without applying them"
            echo "  --check     Run cargo check after sync"
            exit 0
            ;;
        *)
            echo "Unknown argument: $arg"
            exit 1
            ;;
    esac
done

run() {
    if $DRY_RUN; then
        echo "[DRY-RUN] $*"
    else
        echo "[EXEC] $*"
        "$@"
    fi
}

echo "=== Codex Submodule Sync ==="
echo "Submodule: $CODEX_DIR"
echo ""

# Step 1: Fetch upstream
echo "--- Step 1: Fetching upstream ---"
run git submodule update --remote agents/codex

# Step 2: Show what changed
echo ""
echo "--- Step 2: Changes from previous pin ---"
OLD_COMMIT=$(git diff --submodule agents/codex | head -1 || true)
run git -C "$CODEX_DIR" log --oneline -5
echo ""

# Step 3: Apply build.rs overlays
echo "--- Step 3: Applying build.rs overlays ---"
for overlay in "$OVERLAY_DIR"/*.rs; do
    if [ -f "$overlay" ]; then
        filename=$(basename "$overlay")
        # Map overlay filename to target crate directory
        case "$filename" in
            app-server-build.rs)
                target="$CODEX_DIR/codex-rs/app-server/build.rs"
                ;;
            tui-build.rs)
                target="$CODEX_DIR/codex-rs/tui/build.rs"
                ;;
            exec-build.rs)
                target="$CODEX_DIR/codex-rs/exec/build.rs"
                ;;
            windows-sandbox-build.rs)
                target="$CODEX_DIR/codex-rs/windows-sandbox-rs/build.rs"
                ;;
            *)
                echo "  Skipping unknown overlay: $filename"
                continue
                ;;
        esac
        run cp "$overlay" "$target"
        echo "  Applied: $filename -> $target"
    fi
done

echo ""
echo "--- Step 4: Verifying submodule status ---"
run git submodule status agents/codex

# Step 5: Optional cargo check
if $DO_CHECK; then
    echo ""
    echo "--- Step 5: Running cargo check (rc-codex-adapter) ---"
    if ! cargo check -p rc-codex-adapter 2>&1; then
        echo ""
        echo "!!! cargo check FAILED — adapter may need API updates for new upstream"
        echo "    Check rc-codex-adapter/src/ for compilation errors"
        exit 1
    fi
    echo "cargo check passed"
fi

echo ""
echo "=== Sync complete ==="
echo "Run 'git add agents/codex && git commit' to pin the new upstream version."
