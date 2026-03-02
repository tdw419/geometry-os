#!/usr/bin/env bash
# .bridge/tests/test_integration.sh

echo "=== INTEGRATION TESTS ==="

# Test 2.1: Simulate Gemini hook → verify plan.ascii written
test_gemini_hook() {
  echo '{"response": "Test plan content"}' | .bridge/hooks/gemini_hook.sh

  if grep -q "Test plan content" .bridge/fragments/plan.ascii; then
    echo "✓ Gemini hook writes plan.ascii"
  else
    echo "✗ Gemini hook failed"
    exit 1
  fi
}

# Test 2.2: Simulate Claude hook → verify results.ascii written
test_claude_hook() {
  # Create a mock transcript
  echo '[{"content": "Test execution result"}]' > /tmp/test_transcript.json

  echo '{"transcript_path": "/tmp/test_transcript.json"}' | .bridge/hooks/claude_hook.sh

  if grep -q "Test execution result" .bridge/fragments/results.ascii; then
    echo "✓ Claude hook writes results.ascii"
  else
    echo "✗ Claude hook failed"
    exit 1
  fi
}

# Test 2.3: SIGUSR1 wakes orchestrator
test_signal_wake() {
  # Start orchestrator in background
  python3 -u .bridge/orchestrator.py > /tmp/orch_test.log 2>&1 &
  ORCH_PID=$!
  sleep 2

  # Send signal
  kill -USR1 $ORCH_PID
  sleep 1

  if grep -q "Waking up due to signal..." /tmp/orch_test.log; then
    echo "✓ SIGUSR1 wakes orchestrator"
  else
    echo "✗ Signal handling failed"
    kill $ORCH_PID 2>/dev/null
    exit 1
  fi

  kill $ORCH_PID 2>/dev/null
}

test_gemini_hook
test_claude_hook
test_signal_wake

echo "All integration tests passed!"
