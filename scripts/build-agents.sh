#!/usr/bin/env bash
#
# Build all three Agent binaries for the multi-agent architecture.
# Local development / trusted runner machines only. Do not run this on the
# relay-only cloud control-plane host.
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
    local binary_name="$2"
    local source_path="$3"
    shift 3

    echo -e "\n${CYAN}=== Building $name ===${NC}"
    local start_time
    start_time=$(date +%s)

    if "$@"; then
        local end_time
        end_time=$(date +%s)
        local duration=$((end_time - start_time))

        # Copy binary to output directory
        if [ -f "$source_path" ]; then
            cp -f "$source_path" "$OUTPUT_DIR/"
            echo -e "  ${GREEN}Copied: $OUTPUT_DIR/$binary_name${NC}"
            RESULTS+=("$name | OK | ${duration}s")
        else
            echo -e "  ${RED}Warning: Binary not found at $source_path${NC}"
            RESULTS+=("$name | FAILED | ${duration}s")
        fi
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
    "remote-code" \
    "$ROOT_DIR/target/$PROFILE/remote-code" \
    cargo build --package remote-code $PROFILE_FLAG

# ── 2. Codex Agent ──
build_agent "Codex Agent" \
    "codex" \
    "$ROOT_DIR/agents/codex/codex-rs/target/$PROFILE/codex" \
    cargo build --package codex-cli --manifest-path "$ROOT_DIR/agents/codex/codex-rs/Cargo.toml" $PROFILE_FLAG

# ── 3. Roo-code Agent ──
build_agent "Roo-code Agent" \
    "roo" \
    "$ROOT_DIR/target/$PROFILE/roo" \
    cargo build --package roo-cli --manifest-path "$ROOT_DIR/Cargo.toml" $PROFILE_FLAG

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
