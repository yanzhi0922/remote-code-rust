#!/usr/bin/env python3
"""Collect read-only compatibility fixtures from the legacy remote-code workspace."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import pathlib
import re
import subprocess
import sys
from datetime import datetime, timezone
from typing import Any


DEFAULT_SOURCE_ROOT = pathlib.Path(__file__).resolve().parent.parent / "remote-code"


def sha256_text(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def read_text(path: pathlib.Path) -> str:
    return path.read_text(encoding="utf-8")


def extract_list(source: str, variable_name: str) -> list[str]:
    pattern = rf"{variable_name}\s*=\s*new Set\(\[(.*?)\]\)"
    match = re.search(pattern, source, re.DOTALL)
    if not match:
        return []
    return re.findall(r"'([^']+)'", match.group(1))


def extract_runtime_version(config_source: str) -> str | None:
    match = re.search(r"const RUNTIME_VERSION = '([^']+)'", config_source)
    return match.group(1) if match else None


def extract_tool_names(protocol_source: str) -> list[str]:
    match = re.search(r"tools:\s*\[(.*?)\]", protocol_source, re.DOTALL)
    if not match:
        return []
    return re.findall(r"'([^']+)'", match.group(1))


def write_json(path: pathlib.Path, payload: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def write_text(path: pathlib.Path, payload: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(payload, encoding="utf-8")


def find_node_command() -> list[str] | None:
    candidates = [
        ["node"],
        ["node.exe"],
    ]
    for candidate in candidates:
        try:
            result = subprocess.run(
                candidate + ["--version"],
                capture_output=True,
                text=True,
                check=False,
            )
        except OSError:
            continue
        if result.returncode == 0:
            return candidate
    return None


def run_reference_command(
    node_command: list[str],
    legacy_runtime: pathlib.Path,
    args: list[str],
    env: dict[str, str],
) -> dict[str, Any]:
    completed = subprocess.run(
        node_command + [str(legacy_runtime)] + args,
        capture_output=True,
        text=True,
        check=False,
        env=env,
    )
    return {
        "argv": args,
        "returncode": completed.returncode,
        "stdout": completed.stdout,
        "stderr": completed.stderr,
    }


def seed_protocol_fixture(protocol_tools: list[str], cwd: pathlib.Path) -> dict[str, Any]:
    return {
        "type": "system",
        "subtype": "init",
        "apiKeySource": "user",
        "remote_code_version": "runtime-headless",
        "cwd": str(cwd),
        "tools": protocol_tools,
        "mcp_servers": [],
        "model": None,
        "permissionMode": "default",
        "slash_commands": [],
        "output_style": "default",
        "skills": [],
        "plugins": [],
        "uuid": "fixture-seeded-uuid",
        "session_id": "fixture-session",
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source-root", type=pathlib.Path, default=DEFAULT_SOURCE_ROOT)
    parser.add_argument(
        "--output-root",
        type=pathlib.Path,
        default=pathlib.Path("fixtures") / "reference" / "legacy-runtime-src",
    )
    args = parser.parse_args()

    source_root = args.source_root.resolve()
    output_root = args.output_root.resolve()
    runtime_root = source_root / "remote-code" / "runtime-src"
    config_path = runtime_root / "config.cjs"
    protocol_path = runtime_root / "protocol.cjs"
    entry_path = runtime_root / "index.js"

    if not source_root.exists():
        raise SystemExit(f"Source root does not exist: {source_root}")
    if not config_path.exists() or not protocol_path.exists() or not entry_path.exists():
        raise SystemExit(f"Legacy runtime files not found under: {runtime_root}")

    config_source = read_text(config_path)
    protocol_source = read_text(protocol_path)
    runtime_version = extract_runtime_version(config_source)
    permission_modes = extract_list(config_source, "VALID_PERMISSION_MODES")
    reserved_headers = extract_list(config_source, "RESERVED_PROVIDER_REQUEST_HEADER_NAMES")
    tool_names = extract_tool_names(protocol_source)

    metadata = {
        "captured_at_utc": datetime.now(timezone.utc).isoformat(),
        "source_root": str(source_root),
        "runtime_root": str(runtime_root),
        "runtime_version": runtime_version,
        "permission_modes": permission_modes,
        "reserved_provider_headers": reserved_headers,
        "protocol_tools": tool_names,
        "source_hashes": {
            "config.cjs": sha256_text(config_source),
            "protocol.cjs": sha256_text(protocol_source),
            "index.js": sha256_text(read_text(entry_path)),
        },
    }
    write_json(output_root / "metadata.json", metadata)

    write_json(
        output_root / "seeded" / "stream-json-init.json",
        seed_protocol_fixture(tool_names, pathlib.Path.cwd()),
    )

    write_text(output_root / "source-snapshots" / "config.cjs", config_source)
    write_text(output_root / "source-snapshots" / "protocol.cjs", protocol_source)

    node_command = find_node_command()
    if node_command is None:
        write_json(
            output_root / "runtime-captures" / "node-unavailable.json",
            {"status": "skipped", "reason": "node executable not found"},
        )
        return 0

    env = os.environ.copy()
    env.setdefault("REMOTE_CODE_PROVIDER", "fixture")
    env.setdefault("REMOTE_CODE_BASE_URL", "https://example.invalid")
    env.setdefault("REMOTE_CODE_API_KEY", "fixture-key")
    env.setdefault("REMOTE_CODE_MODEL", "fixture-model")
    env.setdefault("REMOTE_CODE_SESSION_ID", "fixture-session")

    doctor_capture = run_reference_command(node_command, entry_path, ["doctor"], env)
    write_json(output_root / "runtime-captures" / "doctor.json", doctor_capture)

    headless = subprocess.run(
        node_command
        + [
            str(entry_path),
            "-p",
            "--input-format",
            "stream-json",
            "--output-format",
            "stream-json",
            "--session-id",
            "fixture-session",
        ],
        input="",
        capture_output=True,
        text=True,
        check=False,
        env=env,
    )
    write_json(
        output_root / "runtime-captures" / "headless-init.json",
        {
            "argv": [
                "-p",
                "--input-format",
                "stream-json",
                "--output-format",
                "stream-json",
                "--session-id",
                "fixture-session",
            ],
            "returncode": headless.returncode,
            "stdout_lines": [line for line in headless.stdout.splitlines() if line.strip()],
            "stderr": headless.stderr,
        },
    )

    return 0


if __name__ == "__main__":
    sys.exit(main())
