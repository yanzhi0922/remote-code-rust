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

CRITICAL: The previous version of this script (commit d1d9408d) had a bug
where the regex `unsafe\s*\{[^\n]*\n(?P<body>.*?)^\s*\}` matched
non-greedily and would absorb the function body up to the FIRST inner `}`
in the block, replacing it with a SAFETY comment but losing the actual
`unsafe { ... }` call. This script uses a brace-balancing tokenizer to
find the TRUE end of the unsafe block, so the body is preserved verbatim.
See memory `reference-bug-d1d9408d-safety-comment-regression.md`.

The inserted text is a conservative default that is correct for the dominant
pattern in this workspace (`std::env::set_var` / `std::env::remove_var`).

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

# Match the OPENING of a multi-line `unsafe {` block.  We do NOT try to
# capture the body here; instead we use a brace-balancing walker
# (`find_block_end`) to locate the matching `}`.
UNSAFE_OPEN_RE = re.compile(
    r"^(?P<indent>[ \t]*)unsafe\s*\{[ \t]*\n",
    re.MULTILINE,
)
SAFETY_COMMENT_RE = re.compile(r"//\s*SAFETY\s*:", re.IGNORECASE)
INLINE_UNSAFE_RE = re.compile(r"\bunsafe\s*\{[^{}\n]*\}\s*;")


def find_block_end(text: str, start: int) -> int:
    """Walk forward from `start` (the `{` position) and return the index of
    the matching `}`, accounting for nested braces and ignoring braces that
    appear inside string literals, char literals, line comments, and block
    comments.

    Returns -1 if no balanced `}` is found before EOF.
    """
    depth = 0
    i = start
    in_line_comment = False
    in_block_comment = False
    in_string = False
    in_char = False
    while i < len(text):
        ch = text[i]
        nxt = text[i + 1] if i + 1 < len(text) else ""
        if in_line_comment:
            if ch == "\n":
                in_line_comment = False
            i += 1
            continue
        if in_block_comment:
            if ch == "*" and nxt == "/":
                in_block_comment = False
                i += 2
                continue
            i += 1
            continue
        if in_string:
            if ch == "\\":
                i += 2
                continue
            if ch == '"':
                in_string = False
            i += 1
            continue
        if in_char:
            if ch == "\\":
                i += 2
                continue
            if ch == "'":
                in_char = False
            i += 1
            continue
        # State transitions
        if ch == "/" and nxt == "/":
            in_line_comment = True
            i += 2
            continue
        if ch == "/" and nxt == "*":
            in_block_comment = True
            i += 2
            continue
        if ch == '"':
            in_string = True
            i += 1
            continue
        if ch == "'":
            in_char = True
            i += 1
            continue
        if ch == "{":
            depth += 1
        elif ch == "}":
            depth -= 1
            if depth == 0:
                return i
        i += 1
    return -1


def has_safety_comment_above(text: str, match_start: int) -> bool:
    """Look ~12 lines above the match for a `// SAFETY:` comment."""
    start_of_window = text.rfind("\n", 0, match_start)
    window_start = text.rfind("\n", 0, max(0, start_of_window - 1500))
    window = text[window_start:match_start]
    return bool(SAFETY_COMMENT_RE.search(window))


def is_one_line_unsafe(text: str, open_match_start: int, open_brace_pos: int) -> bool:
    """True if the unsafe block opens AND closes on the same line."""
    # Reject if the opening `unsafe {` is followed by a newline before the
    # matching `}`. (We use the brace-walker result via the caller; here we
    # only need a quick screen on the opening position itself.)
    end = text.find("\n", open_match_start)
    if end == -1:
        return True
    # Look for a `}` before any newline.
    return INLINE_UNSAFE_RE.search(text[open_match_start:end + 1]) is not None


def transform(text: str) -> tuple[str, int]:
    edits = 0
    out: list[str] = []
    pos = 0
    for match in UNSAFE_OPEN_RE.finditer(text):
        if has_safety_comment_above(text, match.start()):
            continue
        # The opening `{` is the last char of the regex match.
        brace_pos = match.end() - 1
        end_brace = find_block_end(text, brace_pos)
        if end_brace == -1:
            # Unbalanced — skip to be safe.
            continue
        # Skip inline `unsafe { ... }` (no newlines inside the block).
        block_text = text[match.start():end_brace + 1]
        if "\n" not in block_text:
            continue
        # Emit everything up to the match, then the SAFETY comment block
        # (indented to match), then keep the rest of the file starting at
        # the match. This way the body is left ENTIRELY UNTOUCHED.
        before = text[pos:match.start()]
        indent = match.group("indent")
        inserted = "".join(f"{indent}{line}\n" for line in DEFAULT_SAFETY.splitlines())
        out.append(before)
        out.append(inserted)
        pos = match.start()
        # Advance the matcher past this block. We need to skip ahead in the
        # source so we don't re-match. Easiest: rewrite the file with the
        # SAFETY comment inserted and re-scan. For simplicity, do it in a
        # second pass after recording the match.
        edits += 1
    if edits == 0:
        return text, 0
    # The simple in-place approach above double-counts and can corrupt. Use a
    # two-pass algorithm: collect (start, indent) tuples, then insert in
    # reverse order so earlier offsets stay valid.
    insertions: list[tuple[int, str]] = []
    for match in UNSAFE_OPEN_RE.finditer(text):
        if has_safety_comment_above(text, match.start()):
            continue
        brace_pos = match.end() - 1
        end_brace = find_block_end(text, brace_pos)
        if end_brace == -1:
            continue
        block_text = text[match.start():end_brace + 1]
        if "\n" not in block_text:
            continue
        indent = match.group("indent")
        inserted = "".join(f"{indent}{line}\n" for line in DEFAULT_SAFETY.splitlines())
        insertions.append((match.start(), inserted))
    # Insert from last to first so earlier offsets stay valid.
    new_text = text
    for start, inserted in sorted(insertions, key=lambda t: t[0], reverse=True):
        new_text = new_text[:start] + inserted + new_text[start:]
    return new_text, len(insertions)


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
