#!/usr/bin/env bash
# .bridge/hooks/claude_hook.sh
# Enhanced with Ralph Loop autonomous iteration logic.

BRIDGE_DIR=".bridge"
FRAGMENTS_DIR="$BRIDGE_DIR/fragments"
RESULTS_FILE="$FRAGMENTS_DIR/results.ascii"
SYSTEM_FILE="$FRAGMENTS_DIR/system.ascii"
PID_FILE="$BRIDGE_DIR/orchestrator.pid"

TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
INPUT=$(cat)
TRANSCRIPT_PATH=$(echo "$INPUT" | jq -r '.transcript_path')
LATEST_RESULT=$(jq -r '.[-1].content // empty' "$TRANSCRIPT_PATH" 2>/dev/null | tail -100)

# Write results fragment
echo "# schema: results v2" > "$RESULTS_FILE"
echo "completed: $TIMESTAMP" >> "$RESULTS_FILE"
echo "output: |" >> "$RESULTS_FILE"
echo "$LATEST_RESULT" | sed 's/^/  /' >> "$RESULTS_FILE"

# RALPH LOOP LOGIC
if echo "$LATEST_RESULT" | grep -q "<promise>DONE</promise>"; then
    echo "[Ralph] Completion promise detected. Task finalized."
    sed -i "s/state:.*/state: TASK_COMPLETE/" "$SYSTEM_FILE"
else
    ITERATIONS=$(grep "ralph_iterations:" "$SYSTEM_FILE" | cut -d: -f2 | tr -d ' ')
    ITERATIONS=$((ITERATIONS + 1))
    
    if [[ $ITERATIONS -ge 20 ]]; then
        echo "[Ralph] Max iterations (20) reached. Escalating to human."
        sed -i "s/state:.*/state: BLOCKED_ON_QUESTION/" "$SYSTEM_FILE"
    else
        echo "[Ralph] Iteration $ITERATIONS. Requesting next cycle."
        sed -i "s/ralph_iterations:.*/ralph_iterations: $ITERATIONS/" "$SYSTEM_FILE"
        sed -i "s/state:.*/state: WAITING_FOR_PLAN/" "$SYSTEM_FILE"
    fi
fi

sed -i "s/last_agent:.*/last_agent: Claude/" "$SYSTEM_FILE"
sed -i "s/timestamp:.*/timestamp: $TIMESTAMP/" "$SYSTEM_FILE"

# Wake the orchestrator
if [[ -f "$PID_FILE" ]]; then
    PID=$(cat "$PID_FILE")
    kill -USR1 "$PID" 2>/dev/null
fi
