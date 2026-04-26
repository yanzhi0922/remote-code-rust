#!/usr/bin/env bash
# build-agents.sh — Build agent binaries for multi-agent architecture
# Usage: ./scripts/build-agents.sh [roo-code|codex|all]
set -euo pipefail

AGENT="${1:-all}"
PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
AGENTS_DIR="$PROJECT_ROOT/agents"
OUTPUT_BASE="$PROJECT_ROOT/target/agent-binaries"

build_roo_code() {
    echo -e "\n=== Building Roo Code Agent ==="
    local roo_dir="$AGENTS_DIR/roo-code"
    if [[ ! -d "$roo_dir" ]]; then
        echo "ERROR: Roo Code source not found at $roo_dir" >&2
        return 1
    fi

    pushd "$roo_dir" >/dev/null
    echo "  Compiling roo-server (release)..."
    cargo build --release -p roo-server
    local bin_src="$roo_dir/target/release/roo-server"
    local out_dir="$OUTPUT_BASE/roo-code"
    mkdir -p "$out_dir"
    cp "$bin_src" "$out_dir/"
    echo "  -> Copied to $out_dir/roo-server"
    popd >/dev/null
    return 0
}

build_codex() {
    echo -e "\n=== Building Codex Agent ==="
    local codex_dir="$AGENTS_DIR/codex"
    if [[ ! -d "$codex_dir" ]]; then
        echo "ERROR: Codex source not found at $codex_dir" >&2
        return 1
    fi

    local codex_rs_dir="$codex_dir/codex-rs"
    if [[ ! -d "$codex_rs_dir" ]]; then
        echo "ERROR: codex-rs directory not found at $codex_rs_dir" >&2
        return 1
    fi

    pushd "$codex_rs_dir" >/dev/null
    echo "  Compiling codex-rs/app-server (release)..."
    cargo build --release -p codex-app-server || cargo build --release -p codex-exec
    local bin_src
    bin_src=$(find "$codex_rs_dir/target/release" -maxdepth 1 -type f -executable -name "codex*" | head -1)
    if [[ -z "$bin_src" ]]; then
        echo "ERROR: Could not find codex binary" >&2
        popd >/dev/null
        return 1
    fi
    local out_dir="$OUTPUT_BASE/codex"
    mkdir -p "$out_dir"
    cp "$bin_src" "$out_dir/"
    echo "  -> Copied to $out_dir/$(basename "$bin_src")"
    popd >/dev/null
    return 0
}

echo "Remote Code — Agent Binary Builder"
echo "Project root: $PROJECT_ROOT"
echo "Output:       $OUTPUT_BASE"
mkdir -p "$OUTPUT_BASE"

case "$AGENT" in
    roo-code) build_roo_code ;;
    codex)    build_codex ;;
    all)
        build_roo_code
        build_codex
        ;;
    *)
        echo "Usage: $0 [roo-code|codex|all]" >&2
        exit 1
        ;;
esac

echo -e "\n✓ All requested agents built successfully!"
