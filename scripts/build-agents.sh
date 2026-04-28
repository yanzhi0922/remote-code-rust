#!/usr/bin/env bash
#
# Build all three Agent binaries for the multi-agent architecture.
#
# Usage:
#   ./scripts/build-agents.sh          # Release build (default)
#   ./scripts/build-agents.sh --debug  # Debug build

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# Parse arguments
PROFILE="release"
PROFILE_FLAG="--release"
for arg in "$@"; do
    case "$arg" in
        --debug)
            PROFILE="debug"
            PROFILE_FLAG=""
            ;;
        --release)
            PROFILE="release"
            PROFILE_FLAG="--release"
            ;;
        *)
            echo "Unknown argument: $arg"
            echo "Usage: $0 [--debug|--release]"
            exit 1
            ;;
    esac
done

# Output directory for unified binaries
OUTPUT_DIR="$ROOT_DIR/target/agent-binaries"
mkdir -p "$OUTPUT_DIR"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Build results
declare -a RESULTS

build_agent() {
    local name="$1"
    local build_cmd="$2"
    local binary_name="$3"
    local source_path="$4"

    echo -e "\n${CYAN}=== Building $name ===${NC}"
    local start_time
    start_time=$(date +%s)

    if eval "$build_cmd"; then
        local end_time
        end_time=$(date +%s)
        local duration=$((end_time - start_time))

        # Copy binary to output directory
        if [ -f "$source_path" ]; then
            cp -f "$source_path" "$OUTPUT_DIR/"
            echo -e "  ${GREEN}Copied: $OUTPUT_DIR/$binary_name${NC}"
        else
            echo -e "  ${RED}Warning: Binary not found at $source_path${NC}"
        fi

        RESULTS+=("$name | OK | ${duration}s")
    else
        local end_time
        end_time=$(date +%s)
        local duration=$((end_time - start_time))
        echo -e "  ${RED}FAILED${NC}"
        RESULTS+=("$name | FAILED | ${duration}s")
    fi
}

# ── 1. Claude Code Agent ──
build_agent "Claude Code Agent" \
    "cd $ROOT_DIR && cargo build --package remote-code $PROFILE_FLAG" \
    "remote-code" \
    "$ROOT_DIR/target/$PROFILE/remote-code"

# ── 2. Codex Agent ──
build_agent "Codex Agent" \
    "cd $ROOT_DIR/agents/codex/codex-rs && cargo build --package codex-cli $PROFILE_FLAG" \
    "codex" \
    "$ROOT_DIR/agents/codex/codex-rs/target/$PROFILE/codex"

# ── 3. Roo-code Agent ──
build_agent "Roo-code Agent" \
    "cd $ROOT_DIR/agents/roo-code && cargo build --package roo-cli $PROFILE_FLAG" \
    "roo" \
    "$ROOT_DIR/agents/roo-code/target/$PROFILE/roo"

# ── Summary ──
echo -e "\n========================================"
echo -e "  Build Summary (Profile: $PROFILE)"
echo -e "========================================"
printf "%-25s %-10s %-10s\n" "Agent" "Status" "Duration"
printf "%-25s %-10s %-10s\n" "-----" "------" "--------"
for result in "${RESULTS[@]}"; do
    echo "$result" | awk -F'|' '{printf "%-25s %-10s %-10s\n", $1, $2, $3}'
done
echo ""
echo -e "${CYAN}Output directory: $OUTPUT_DIR${NC}"
echo ""
