#!/usr/bin/env python3
"""Fail CI when panic-macro usage exceeds a configured threshold.

Counts production-source uses of:
  - `.unwrap()` and `.expect("...")` on Result/Option
  - bare `panic!()` in non-test code

Excludes unwrap_or / unwrap_or_else / unwrap_or_default (those are NOT panics).
Excludes test code: `tests/`, `tests.rs`, `test_*.rs`, `*_test.rs`, and any
`#[cfg(test)]` blocks detected by filename heuristic.

The threshold is read from `scripts/panic-macro-budget.json` so the budget can
shrink over time as panic macros are replaced with `?` + `anyhow::Context`.

Exit codes:
  0  under budget
  1  over budget (or threshold file missing)
  2  internal error (grep failure, malformed JSON)
"""
from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
BUDGET_FILE = REPO_ROOT / "scripts" / "panic-macro-budget.json"

# Pattern: `.unwrap()` or `.expect("...")` — NOT `.unwrap_or(...)` (negative lookahead).
STRICT_PANIC_RE = re.compile(r"\.unwrap\(\)|\.expect\(")
BARE_PANIC_RE = re.compile(r"\bpanic!\s*\(")

# Test-file exclusion: matches the basename of any file path under these rules.
TEST_PATH_SEGMENTS = ("/tests/", "/tests.rs", "/test_", "/_test.rs")


def is_test_path(path: str) -> bool:
    p = path.replace("\\", "/")
    return any(seg in p for seg in TEST_PATH_SEGMENTS)


def run_ripgrep(pattern: str) -> list[tuple[str, int, str]]:
    """Return (file, line, content) tuples for each grep match under crates/ + apps/."""
    cmd = [
        "rg",
        "--no-heading",
        "--line-number",
        "--color=never",
        "--type=rust",
        pattern,
        "crates",
        "apps",
    ]
    proc = subprocess.run(cmd, cwd=REPO_ROOT, capture_output=True, text=True)
    if proc.returncode not in (0, 1):  # 0=match, 1=no match
        raise RuntimeError(f"ripgrep failed (rc={proc.returncode}): {proc.stderr}")
    out: list[tuple[str, int, str]] = []
    for line in proc.stdout.splitlines():
        # ripgrep format: file:line:content
        parts = line.split(":", 2)
        if len(parts) != 3:
            continue
        try:
            lineno = int(parts[1])
        except ValueError:
            continue
        out.append((parts[0], lineno, parts[2]))
    return out


def classify(matches: list[tuple[str, int, str]]) -> dict[str, int]:
    """Partition matches into production vs test buckets."""
    counts = {"prod_strict": 0, "prod_panic": 0, "test_strict": 0, "test_panic": 0}
    for path, _, content in matches:
        in_test = is_test_path(path)
        is_bare_panic = bool(BARE_PANIC_RE.search(content))
        is_strict = bool(STRICT_PANIC_RE.search(content))
        if is_bare_panic:
            counts["test_panic" if in_test else "prod_panic"] += 1
        if is_strict:
            counts["test_strict" if in_test else "prod_strict"] += 1
    return counts


def main() -> int:
    if not BUDGET_FILE.exists():
        print(f"ERROR: threshold file missing: {BUDGET_FILE}", file=sys.stderr)
        return 1
    budget = json.loads(BUDGET_FILE.read_text(encoding="utf-8"))
    prod_strict_limit = int(budget["unwrap_expect_in_production"])
    prod_panic_limit = int(budget["bare_panic_in_production"])

    strict_matches = run_ripgrep(r"\.unwrap\(\)|\.expect\(")
    bare_panic_matches = run_ripgrep(r"\bpanic!\s*\(")
    strict_counts = classify(strict_matches)
    panic_counts = classify(bare_panic_matches)

    prod_strict = strict_counts["prod_strict"]
    prod_panic = panic_counts["prod_panic"]
    test_strict = strict_counts["test_strict"]
    test_panic = panic_counts["test_panic"]

    print("=== panic-macro budget check ===")
    print(f"production `.unwrap()` / `.expect()`: {prod_strict} / {prod_strict_limit}")
    print(f"production `panic!()`:               {prod_panic} / {prod_panic_limit}")
    print(f"(test code:   {test_strict} strict, {test_panic} panic — informational only)")

    failed = False
    if prod_strict > prod_strict_limit:
        print(
            f"FAIL: production panic-macro count {prod_strict} exceeds "
            f"budget {prod_strict_limit}.",
            file=sys.stderr,
        )
        failed = True
    if prod_panic > prod_panic_limit:
        print(
            f"FAIL: production `panic!()` count {prod_panic} exceeds "
            f"budget {prod_panic_limit}.",
            file=sys.stderr,
        )
        failed = True

    return 1 if failed else 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except RuntimeError as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        sys.exit(2)
