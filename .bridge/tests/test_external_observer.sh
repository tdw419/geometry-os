#!/usr/bin/env bash
# .bridge/tests/test_external_observer.sh

echo "=== EXTERNAL LLM VERIFICATION ==="

# Use Gemini to observe and verify the system
test_with_external_observer() {
  # Read all fragments
  TASK=$(cat .bridge/fragments/task.ascii 2>/dev/null || echo "missing")
  SYSTEM=$(cat .bridge/fragments/system.ascii 2>/dev/null || echo "missing")
  WATCHDOG=$(cat .bridge/fragments/watchdog.ascii 2>/dev/null || echo "missing")
  PLAN=$(cat .bridge/fragments/plan.ascii 2>/dev/null || echo "missing")
  RESULTS=$(cat .bridge/fragments/results.ascii 2>/dev/null || echo "missing")

  OBSERVER_INSTRUCTIONS=$(cat .bridge/fragments/observer-instructions.ascii)

  # Construct observation prompt
  OBSERVATION="
$OBSERVER_INSTRUCTIONS

CURRENT SYSTEM STATE:

=== task.ascii ===
$TASK

=== system.ascii ===
$SYSTEM

=== watchdog.ascii ===
$WATCHDOG

=== plan.ascii ===
$PLAN

=== results.ascii ===
$RESULTS

Please verify this system state and output a JSON verification report.
"

  # Send to observer LLM
  REPORT=$(echo "$OBSERVATION" | gemini -p "$(cat -)" 2>/dev/null)

  echo "Observer Report:"
  echo "$REPORT"

  # Parse report and determine pass/fail
  if echo "$REPORT" | grep -iq '"status": "PASS"'; then
    echo "✓ External observer verification PASSED"
    return 0
  else
    echo "✗ External observer found issues"
    return 1
  fi
}

test_with_external_observer
