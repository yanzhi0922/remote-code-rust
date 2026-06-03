#!/usr/bin/env bash
# purge_zcode_refs.sh — Remove any `refs/zcode/**` references and gc.
#
# The Z.ai ZCode integration (used by `claude-checkpoint` and
# `claude-specialized-agents` for inspiration) writes a `ZCode Checkpoint
# <checkpoint@zcode.local>` commit per session into `refs/zcode/checkpoints/...`.
# This pollutes `git log --all`, `git fsck`, and the object store with hundreds
# of root commits that carry Chrome cache, browser data, and audit.toml.
#
# Run this script periodically (or via cron / git hook) to clean up. It is
# safe to run on a working repo; it only deletes refs, never reachable
# commits.  After deletion, `git gc --prune=now` reclaims disk.
#
# Usage:
#   ./scripts/purge_zcode_refs.sh         # remove + gc
#   ./scripts/purge_zcode_refs.sh --dry   # show what would be removed
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

DRY_RUN=false
for arg in "$@"; do
    case "$arg" in
        --dry)    DRY_RUN=true ;;
        --help|-h)
            sed -n '3,17p' "$0"
            exit 0
            ;;
        *) echo "Unknown argument: $arg" >&2; exit 2 ;;
    esac
done

# Collect refs to delete
ZCODE_REFS="$(git for-each-ref --format='%(refname)' 'refs/zcode/**' || true)"
if [ -z "$ZCODE_REFS" ]; then
    echo "No refs/zcode/** refs to purge."
    exit 0
fi

COUNT="$(printf '%s\n' "$ZCODE_REFS" | wc -l | tr -d ' ')"
echo "Found $COUNT refs/zcode/** ref(s):"
printf '  %s\n' $ZCODE_REFS

if $DRY_RUN; then
    echo ""
    echo "Dry run: no changes made. Re-run without --dry to apply."
    exit 0
fi

# Delete each ref
while IFS= read -r ref; do
    [ -z "$ref" ] && continue
    git update-ref -d "$ref"
done <<< "$ZCODE_REFS"

# Reclaim disk
git reflog expire --expire=now --all 2>/dev/null || true
git gc --prune=now --quiet

REMAINING="$(git for-each-ref --format='%(refname)' 'refs/zcode/**' | wc -l | tr -d ' ')"
echo ""
echo "Purge complete: $REMAINING refs/zcode/** remaining."
