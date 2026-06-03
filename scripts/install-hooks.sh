#!/usr/bin/env bash
# install-hooks.sh — Symlink scripts/git-hooks/* into .git/hooks/.
#
# Run once after `git clone` to enable the ZCode-refs pre-receive guard
# locally.  Safe to re-run; existing hooks are not overwritten.
#
# On Windows (Git Bash), symlinks may require admin; this script falls back
# to copying the hook files when symlink fails.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
HOOK_SRC_DIR="$ROOT_DIR/scripts/git-hooks"
HOOK_DST_DIR="$ROOT_DIR/.git/hooks"

if [ ! -d "$HOOK_DST_DIR" ]; then
    echo "No .git/hooks/ directory found at $HOOK_DST_DIR" >&2
    exit 1
fi

for src in "$HOOK_SRC_DIR"/*; do
    [ -f "$src" ] || continue
    name="$(basename "$src")"
    dst="$HOOK_DST_DIR/$name"
    if [ -f "$dst" ] && [ ! -L "$dst" ]; then
        echo "  Skipping $name (already exists, not a symlink)"
        continue
    fi
    # Try symlink first; fall back to copy on Windows / non-symlink FS.
    if ln -sf "../../scripts/git-hooks/$name" "$dst" 2>/dev/null; then
        echo "  Symlinked $name"
    else
        cp "$src" "$dst"
        chmod +x "$dst" 2>/dev/null || true
        echo "  Copied $name (symlink not supported on this FS)"
    fi
done

echo ""
echo "Hooks installed. Verify with:  ls -la $HOOK_DST_DIR"
