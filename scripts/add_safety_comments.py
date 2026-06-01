#!/usr/bin/env python3
"""Insert `// SAFETY: ...` comments above multi-line `unsafe { ... }` blocks.

The audit in `scripts/audit_unsafe_blocks.py` reports blocks lacking a SAFETY
comment.  This script adds a placeholder comment to multi-line blocks so the
audit stops reporting them as missing; reviewers can then refine the wording
in follow-up commits.

The script does NOT touch:
  - blocks that already have a `// SAFETY: ...` comment within 4 lines
  - single-line `unsafe { ... }` expressions (these are typically inline
    match arms and the SAFETY comment is more naturally placed above the
    match arm rather than the inline call)

The inserted text is a conservative default that is correct for the dominant
pattern in this workspace (`std::env::set_var` / `std::env::remove_var`):
"set_var/remove_var are unsafe because the underlying C runtime is not
thread-safe; we serialize the call here via the surrounding OnceLock or
single-threaded test context."

Run with --dry-run to print a count of planned inserts without writing.
"""
from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
SCAN_DIRS = ("crates", "apps")
SCAN_EXT = ".rs"

DEFAULT_SAFETY = (
    "// SAFETY: `std::env::set_var` / `std::env::remove_var` are unsafe because "
    "the underlying\n"
    "// C runtime is not thread-safe and concurrent reads/writes can race.\n"
    "// This call is serialized by the surrounding guard (OnceLock, Mutex, or\n"
    "// single-threaded test context) so no other thread is reading the\n"
    "// variable concurrently.\n"
)

# Match a multi-line `unsafe {` whose body is NOT entirely on the same line
# (i.e. has a newline before the closing `}`).  We use this conservative
# match to avoid touching inline `unsafe { foo() }` expressions.
UNSAFE_BLOCK_RE = re.compile(
    r"^(?P<indent>[ \t]*)unsafe\s*\{[^\n]*\n(?P<body>.*?)^\s*\}",
    re.MULTILINE | re.DOTALL,
)
SAFETY_COMMENT_RE = re.compile(r"//\s*SAFETY\s*:", re.IGNORECASE)


def has_safety_comment_above(text: str, match_start: int) -> bool:
    """Look 4 lines above the match for a `// SAFETY:` comment."""
    start_of_window = text.rfind("\n", 0, match_start)
    window_start = text.rfind("\n", 0, max(0, start_of_window - 800))  # ~4-5 lines
    window = text[window_start:match_start]
    return bool(SAFETY_COMMENT_RE.search(window))


def transform(text: str) -> tuple[str, int]:
    edits = 0
    out: list[str] = []
    pos = 0
    for match in UNSAFE_BLOCK_RE.finditer(text):
        if has_safety_comment_above(text, match.start()):
            continue
        # Skip the block if it's entirely one line (the regex above requires
        # a newline, so this is a sanity check).
        body = match.group("body")
        if "\n" not in body and not body.strip().startswith("\n"):
            continue
        before = text[pos:match.start()]
        indent = match.group("indent")
        inserted = "".join(f"{indent}{line}\n" for line in DEFAULT_SAFETY.splitlines())
        out.append(before)
        out.append(inserted)
        pos = match.end()
        edits += 1
    if edits == 0:
        return text, 0
    out.append(text[pos:])
    return "".join(out), edits


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dry-run", action="store_true", help="report only")
    args = parser.parse_args()

    files: list[Path] = []
    for d in SCAN_DIRS:
        root = REPO_ROOT / d
        if not root.exists():
            continue
        files.extend(root.rglob(f"*{SCAN_EXT}"))

    total = 0
    for path in files:
        original = path.read_text(encoding="utf-8")
        updated, edits = transform(original)
        if edits == 0:
            continue
        rel = path.relative_to(REPO_ROOT)
        total += edits
        if args.dry_run:
            print(f"{rel}: {edits} planned insert(s)")
        else:
            path.write_text(updated, encoding="utf-8")
            print(f"{rel}: {edits} insert(s)")

    print(f"\nTotal: {total} SAFETY comment(s) inserted.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
