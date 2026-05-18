#!/usr/bin/env python3
"""Run cargo commands for a stable workspace slice.

The workspace is large enough that a single all-workspace test job is brittle on
Windows and slow everywhere. This script keeps CI slicing data-driven by reading
`cargo metadata` instead of hard-coding the package list in workflow YAML.
"""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
from typing import Iterable


SLICE_NAMES = ("claude", "codex", "roo", "apps-shared")
CHUNK_SIZE = 24
GUI_PACKAGE = "remote-code-gui"


def run_json(command: list[str]) -> dict:
    completed = subprocess.run(
        command,
        check=True,
        stdout=subprocess.PIPE,
        stderr=None,
        text=True,
        encoding="utf-8",
    )
    return json.loads(completed.stdout)


def normalized_manifest_dir(package: dict) -> str:
    manifest = Path(package["manifest_path"]).parent
    return manifest.as_posix()


def package_slice(package: dict) -> str | None:
    name = package["name"]
    path = normalized_manifest_dir(package)
    if name == GUI_PACKAGE:
        return None
    if "/crates/claude/" in path or path.endswith("/agents/claudecode"):
        return "claude"
    if "/crates/codex/" in path:
        return "codex"
    if "/crates/roo/" in path:
        return "roo"
    if "/apps/" in path or "/crates/shared/" in path or "/crates/adapters/" in path:
        return "apps-shared"
    return None


def chunks(values: list[str], size: int) -> Iterable[list[str]]:
    for index in range(0, len(values), size):
        yield values[index : index + size]


def cargo_command(kind: str, packages: list[str]) -> list[str]:
    command = ["cargo", kind]
    jobs = os.environ.get("CARGO_TEST_JOBS" if kind == "test" else "CARGO_BUILD_JOBS")
    if jobs:
        command.extend(["-j", jobs])
    for package in packages:
        command.extend(["-p", package])
    command.append("--all-targets")
    if kind == "clippy":
        command.extend(["--", "-D", "warnings"])
    return command


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("kind", choices=("check", "clippy", "test"))
    parser.add_argument("slice", choices=SLICE_NAMES)
    args = parser.parse_args()

    metadata = run_json(["cargo", "metadata", "--no-deps", "--format-version", "1"])
    workspace_ids = set(metadata["workspace_members"])
    packages = [
        package["name"]
        for package in metadata["packages"]
        if package["id"] in workspace_ids and package_slice(package) == args.slice
    ]
    packages = sorted(set(packages))
    if not packages:
        print(f"No packages matched slice {args.slice}", file=sys.stderr)
        return 1

    print(f"Running cargo {args.kind} for {args.slice}: {len(packages)} packages")
    for group in chunks(packages, CHUNK_SIZE):
        print(f"::group::cargo {args.kind} {args.slice} ({len(group)} packages)")
        command = cargo_command(args.kind, group)
        print(" ".join(command))
        completed = subprocess.run(command)
        print("::endgroup::")
        if completed.returncode != 0:
            return completed.returncode
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
