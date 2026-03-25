#!/bin/bash
# Prompt Queue Manager CLI
# Usage: ./queue.sh status|enqueue|process|clear|retry|dashboard

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Check if dashboard command
if [ "$1" = "dashboard" ]; then
    shift
    node "$SCRIPT_DIR/.ouroboros/queue_dashboard.js" "$@"
else
    node "$SCRIPT_DIR/src/ouroboros/v2/prompt_queue.js" "$@"
fi
