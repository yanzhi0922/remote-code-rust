#!/usr/bin/env python3
"""Audit `unsafe {}` blocks in the workspace.

For each `unsafe { ... }` block in `crates/` and `apps/`, verify that a
`// SAFETY: ...` comment immediately precedes the block.  Exits with code 1
if any block lacks the comment, code 0 if every block is annotated.

This is a soft-fail audit: the goal is to surface undocumented unsafe usage
for review, not to enforce annotations in CI (uncomment `fail_on_missing` to
enable the enforcement).

Usage:
  python scripts/audit_unsafe_blocks.py            # report only
  python scripts/audit_unsafe_blocks.py --enforce  # exit 1 on any missing
"""
from __future__ import annotations

import argparse
import re
import subprocess
import sys
from collections import defaultdict
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
SCAN_DIRS = ("crates", "apps")

# Match an `unsafe {` opening token. We don't try to balance braces here; the
# SAFETY comment is expected within 4 lines *before* the opening token.
UNSAFE_OPEN_RE = re.compile(r"\bunsafe\s*\{", re.MULTILINE)

# Match a `// SAFETY: ...` comment. We accept any non-empty trailing text.
SAFETY_COMMENT_RE = re.compile(r"//\s*SAFETY\s*:\s*\S+", re.IGNORECASE)


def run_ripgrep(pattern: str) -> list[tuple[str, int]]:
    cmd = [
        "rg",
        "--no-heading",
        "--line-number",
        "--color=never",
        "--type=rust",
        pattern,
        *SCAN_DIRS,
    ]
    proc = subprocess.run(cmd, cwd=REPO_ROOT, capture_output=True, text=True)
    if proc.returncode not in (0, 1):
        raise RuntimeError(f"ripgrep failed (rc={proc.returncode}): {proc.stderr}")
    out: list[tuple[str, int]] = []
    for line in proc.stdout.splitlines():
        path, lineno_s, _ = line.split(":", 2)
        out.append((path, int(lineno_s)))
    return out


def find_missing_safety_comments() -> dict[str, list[tuple[int, str]]]:
    """Return {file: [(line, snippet), ...]} for unsafe blocks without a SAFETY
    comment in the 4 lines preceding the block."""
    unsafe_sites = run_ripgrep(r"\bunsafe\s*\{")
    missing: dict[str, list[tuple[int, str]]] = defaultdict(list)
    for rel_path, line in unsafe_sites:
        abs_path = REPO_ROOT / rel_path
        try:
            with abs_path.open(encoding="utf-8") as fh:
                text = fh.read()
        except OSError:
            continue
        lines = text.splitlines()
        # The `unsafe {` may be on the same line as other code, but the
        # SAFETY comment is expected to precede it on its own line.
        if line - 1 < 0 or line > len(lines):
            continue
        preceding_window = "\n".join(lines[max(0, line - 12):line])
        if not SAFETY_COMMENT_RE.search(preceding_window):
            snippet = lines[line - 1].strip()[:80]
            missing[rel_path].append((line, snippet))
    return missing


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--enforce",
        action="store_true",
        help="exit 1 if any unsafe block lacks a SAFETY comment",
    )
    args = parser.parse_args()

    missing = find_missing_safety_comments()
    if not missing:
        print("All `unsafe {}` blocks have a `// SAFETY: ...` comment.")
        return 0

    total = sum(len(v) for v in missing.values())
    print(f"Found {total} `unsafe {{` block(s) without a `// SAFETY: ...` "
          f"comment in {len(missing)} file(s):\n")
    for path, sites in sorted(missing.items()):
        for line, snippet in sites:
            print(f"  {path}:{line}  {snippet}")
    print(f"\nTotal: {total} block(s) in {len(missing)} file(s).")
    return 1 if args.enforce else 0


if __name__ == "__main__":
    sys.exit(main())
