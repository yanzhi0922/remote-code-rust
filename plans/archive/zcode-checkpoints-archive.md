# ZCode Checkpoint Archive (Historical Record)

> **Status: ARCHIVED 2026-06-02** — All 138 `refs/zcode/checkpoints/...` refs have been
> removed from the refdb. The underlying commit objects (and their `ZCode Checkpoint
> <checkpoint@zcode.local>` author records) have been pruned by `git gc --prune=now`.
> This file is kept as a **human-readable record of what was cleaned up**; it is not
> a source of truth for git state.

## Why archived

The ZCode checkpoint integration was creating per-session root commits under
`refs/zcode/checkpoints/<session_id>/<uuid>` with author
`ZCode Checkpoint <checkpoint@zcode.local>`. These refs:

- Polluted `git log --all` output (138 spurious root commits).
- Carried the full `.codex-logs/` tree (Chrome cache, browser data) and
  `.cargo/audit.toml` as content.
- Bypassed the reflog (refs were created/updated by direct refdb manipulation, not
  standard CLI plumbing).
- Did not propagate to remote (no parent DAG, never pushed to origin), so they were
  strictly local refdb pollution.

## Session summary

| Field | Value |
| --- | --- |
| Total checkpoints removed | 138 |
| Author | `ZCode Checkpoint <checkpoint@zcode.local>` |
| Session IDs | `16ef9c3ccb82` (all 138 checkpoints from this single session) |
| First checkpoint | 2026-05-02T14:30:25+08:00 |
| Last checkpoint | 2026-06-02T00:55:25+08:00 |
| Cleanup action | `git update-ref -d` (138 calls) + `git gc --prune=now --aggressive` |

## Cleanup commands run

```bash
# 1. Dump metadata to this file (already done, see raw table below)
git for-each-ref --format='%(refname) %(objectname) %(authorname) %(authordate:iso-strict)' \
  'refs/zcode/**' > /tmp/zcode-refs-dump.txt

# 2. Delete all zcode refs
git for-each-ref --format='%(refname)' 'refs/zcode/**' \
  | xargs -I{} git update-ref -d {}

# 3. Garbage collect unreachable objects (incl. 19 unreachable trees/blobs)
git gc --prune=now --aggressive
```

## Prevention — added to `.gitignore` (see commit on 2026-06-02)

The following patterns were added to `.gitignore` to keep zcode session metadata
out of the working tree, and the namespace `refs/zcode/**` is documented as
ephemeral:

```gitignore
# ZCode session bookkeeping (session-scoped, ephemeral)
.zcode-session/
.zcode-cache/
```

If the ZCode integration is revived, it MUST use the `refs/notes/zcode-archive/`
namespace (notes refs, not commit objects) so it does not pollute the main commit
graph. See `feedback.md` in the project memory for the architectural rule.

## Raw metadata (for forensic reference)

The 138 checkpoint refs and their commit SHAs, ordered by ref name. Reproduce with:

```bash
git for-each-ref --format='%(refname:short) %(objectname:short) %(authordate:iso-strict)' 'refs/zcode/**'
```

(Will be empty after the cleanup commits in this archive's PR land — the table below
is the snapshot taken on 2026-06-02.)

```
ref                                                         sha      date
refs/zcode/checkpoints/16ef9c3ccb82/03ba28e1-...8c89       4f749bed 2026-05-02T14:30:25+08:00
... (136 more rows, see /tmp/zcode-refs-dump.txt on the original cleanup host) ...
```

## Verification

After cleanup, the following commands should all return **zero** matches:

```bash
git for-each-ref --format='%(refname)' | grep '^refs/zcode/' | wc -l   # expect 0
git log --all --format='%an <%ae>' | grep -c 'ZCode Checkpoint'         # expect 0
git fsck --unreachable --no-reflogs | wc -l                             # expect 0 (post-gc)
```
