#!/usr/bin/env bash
# .bridge/tests/test_components.sh
set -e

echo "=== COMPONENT TESTS ==="

# Test 1.1: Orchestrator writes heartbeat
test_heartbeat() {
  python3 -c "
import sys
import os
sys.path.append(os.path.join(os.getcwd(), '.bridge'))
import orchestrator
orchestrator.update_watchdog_heartbeat()
"

  if grep -q "heartbeat:" .bridge/fragments/watchdog.ascii; then
    echo "✓ Heartbeat written correctly"
  else
    echo "✗ Heartbeat failed"
    exit 1
  fi
}

# Test 1.2: State transitions work
test_state_transition() {
  python3 -c "
import sys
import os
sys.path.append(os.path.join(os.getcwd(), '.bridge'))
import orchestrator
orchestrator.update_system_state('WAITING_FOR_PLAN', 'TestAgent')
"

  if grep -q "state: WAITING_FOR_PLAN" .bridge/fragments/system.ascii; then
    echo "✓ State transition works"
  else
    echo "✗ State transition failed"
    exit 1
  fi
}

# Test 1.3: Hook script syntax
test_hook_syntax() {
  bash -n .bridge/hooks/claude_hook.sh && echo "✓ claude_hook.sh syntax OK"
  bash -n .bridge/hooks/gemini_hook.sh && echo "✓ gemini_hook.sh syntax OK"
}

test_heartbeat
test_state_transition
test_hook_syntax

echo "All component tests passed!"
