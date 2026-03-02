#!/usr/bin/env bash
# .bridge/hooks/gemini_hook.sh
# This script handles Gemini's 'AfterAgent' event.

BRIDGE_DIR=".bridge"
FRAGMENTS_DIR="$BRIDGE_DIR/fragments"
PLAN_FILE="$FRAGMENTS_DIR/plan.ascii"
SYSTEM_FILE="$FRAGMENTS_DIR/system.ascii"
PID_FILE="$BRIDGE_DIR/orchestrator.pid"

# Get current time
TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

# Read input from stdin (JSON metadata)
INPUT=$(cat)

# Gemini CLI uses 'prompt_response' for AfterAgent
LATEST_PLAN=$(echo "$INPUT" | jq -r '.prompt_response // .response // empty')

# Write plan fragment
if [[ -n "$LATEST_PLAN" ]]; then
    echo "# schema: plan v3 (TOON)" > "$PLAN_FILE"
    echo "created: $TIMESTAMP" >> "$PLAN_FILE"
    echo "objective: |" >> "$PLAN_FILE"
    echo "$LATEST_PLAN" | sed 's/^/  /' >> "$PLAN_FILE"

    # Update system state to notify Claude
    sed -i "s/state:.*/state: WAITING_FOR_EXECUTION/" "$SYSTEM_FILE"
    sed -i "s/last_agent:.*/last_agent: Gemini/" "$SYSTEM_FILE"
    sed -i "s/timestamp:.*/timestamp: $TIMESTAMP/" "$SYSTEM_FILE"

    echo "[Gemini Hook] Plan captured and state updated." >&2
fi

# Signal the orchestrator
if [[ -f "$PID_FILE" ]]; then
    PID=$(cat "$PID_FILE")
    kill -USR1 "$PID" 2>/dev/null
fi

# Must output JSON to stdout for Gemini CLI hooks
echo '{"decision": "allow"}'
