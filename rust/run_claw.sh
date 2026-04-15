#!/bin/bash
# AI-Code v0.3.1 - One-click launcher for Linux
# Based on Claw-Code (https://github.com/instructkr/claw-code)
#
# Usage:
#   ./run_claw.sh              # Start interactive REPL
#   ./run_claw.sh "your question"  # Single prompt mode
#   ./run_claw.sh --help       # Show help
#
# Environment variables (required):
#   OPENAI_API_KEY    - Your API key
#   OPENAI_BASE_URL   - API base URL (optional, default: OpenAI)
#   OPENAI_MODEL      - Model name (optional, default: gpt-4o)

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BINARY_PATH="$SCRIPT_DIR/target/release/claw"

# Check if binary exists
if [ ! -f "$BINARY_PATH" ]; then
    echo "Error: Binary not found at $BINARY_PATH"
    echo "Please build the project first:"
    echo "  cd $SCRIPT_DIR && cargo build --release"
    exit 1
fi

# Check for API key
if [ -z "$OPENAI_API_KEY" ]; then
    echo "Error: OPENAI_API_KEY environment variable is not set"
    echo ""
    echo "Please set your API key:"
    echo "  export OPENAI_API_KEY='your-api-key'"
    echo ""
    echo "For custom API endpoints, also set:"
    echo "  export OPENAI_BASE_URL='https://your-api-endpoint'"
    echo "  export OPENAI_MODEL='your-model-name'"
    exit 1
fi

# Set default model if not specified
: "${OPENAI_MODEL:=gpt-4o}"

# Fix terminal settings for Backspace/Delete keys
# This ensures Backspace sends ^? instead of ^H
if [ -t 0 ]; then
    stty erase '^?' 2>/dev/null || true
fi

# Run the binary
exec "$BINARY_PATH" "$@"
