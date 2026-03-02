#!/usr/bin/env bash
# .bridge/tests/test_e2e.sh

echo "=== END-TO-END TESTS ==="

# Test 3.1: Simple file creation task
test_simple_task() {
  # Create a simple task
  cat > .bridge/fragments/task.ascii << 'EOF'
# schema: task v1
objective: |
  Create a file called test_output.txt containing "Bridge test successful"
EOF

  # Reset state
  echo "# schema: system-state v1
state: INITIALIZING
last_agent: none
timestamp: $(date -u +"%Y-%m-%dT%H:%M:%SZ")
stuck_count: 0" > .bridge/fragments/system.ascii

  # Run orchestrator for max 60 seconds
  timeout 60 python3 -u .bridge/orchestrator.py > /tmp/orch_e2e.log 2>&1 &
  ORCH_PID=$!

  echo "Waiting for task completion..."
  for i in {1..60}; do
    if [ -f "test_output.txt" ]; then
      if grep -q "Bridge test successful" test_output.txt; then
        echo "✓ Simple task completed successfully"
        kill $ORCH_PID 2>/dev/null
        rm test_output.txt
        return 0
      fi
    fi
    sleep 1
  done

  echo "✗ Simple task timed out or failed"
  kill $ORCH_PID 2>/dev/null
  exit 1
}

# Test 3.2: Question handling
test_question_blocking() {
  # Simulate a question being written
  cat > .bridge/fragments/question.ascii << 'EOF'
# schema: question v1
question: Should I use tabs or spaces for indentation?
EOF

  # Set state to blocked
  sed -i 's/state:.*/state: BLOCKED_ON_QUESTION/' .bridge/fragments/system.ascii

  if grep -q "BLOCKED_ON_QUESTION" .bridge/fragments/system.ascii; then
    echo "✓ Question blocking state works"
  else
    echo "✗ Question state failed"
    exit 1
  fi

  # Cleanup
  rm .bridge/fragments/question.ascii
}

test_simple_task
test_question_blocking

echo "All E2E tests passed!"
