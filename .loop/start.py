#!/usr/bin/env python3
"""
Task initialization script for Geometry OS auto-prompting loop.
Creates STATE.md and TASK.md with initial state.
"""

import sys
import os
from pathlib import Path
from datetime import datetime, timezone


def get_timestamp() -> str:
    """Return ISO 8601 timestamp with Z suffix."""
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def create_task_file(task_description: str, loop_dir: Path) -> None:
    """Write TASK.md with timestamp and description."""
    task_file = loop_dir / "TASK.md"
    content = f"""# TASK

**Created**: {get_timestamp()}

## Description

{task_description}
"""
    task_file.write_text(content)


def create_state_file(task_description: str, loop_dir: Path) -> None:
    """Write STATE.md with initial RUNNING status and ANALYZE action."""
    state_file = loop_dir / "STATE.md"
    content = f"""# STATE: RUNNING

## R: Context
- **Goal**: {task_description}
- **Progress**: Task initialized
- **Files**: None yet
- **Blockers**: None

## G: Action
ANALYZE: Understand the task and plan approach

## B: Target
target: .
content: |
  Analyzing task requirements
"""
    state_file.write_text(content)


def check_existing_state(loop_dir: Path) -> None:
    """Error if STATE.md already exists with RUNNING status."""
    state_file = loop_dir / "STATE.md"
    if state_file.exists():
        content = state_file.read_text()
        if "STATE: RUNNING" in content:
            print("Error: A task is already running. Complete or clear it first.", file=sys.stderr)
            print("  Run: rm .loop/STATE.md .loop/TASK.md", file=sys.stderr)
            sys.exit(1)
        # If not RUNNING, allow overwriting (e.g., previous DONE state)
        print("Note: Overwriting previous completed task.")


def start_task(task_description: str) -> None:
    """Creates STATE.md and TASK.md with initial state."""
    # Get .loop directory (same directory as this script)
    loop_dir = Path(__file__).parent.resolve()

    # Create .loop/ directory if missing (should exist, but be safe)
    loop_dir.mkdir(exist_ok=True)

    # Check for existing RUNNING task
    check_existing_state(loop_dir)

    # Create TASK.md and STATE.md
    create_task_file(task_description, loop_dir)
    create_state_file(task_description, loop_dir)

    print(f"Task initialized: {task_description}")
    print(f"  STATE.md: {loop_dir / 'STATE.md'}")
    print(f"  TASK.md: {loop_dir / 'TASK.md'}")
    print("\nRun the loop with: python .loop/runner.py")


def main() -> None:
    """Parse CLI args and start task."""
    if len(sys.argv) < 2:
        print("Usage: python start.py <task-description>", file=sys.stderr)
        print("Example: python start.py \"Create hello.txt\"", file=sys.stderr)
        sys.exit(1)

    task_description = " ".join(sys.argv[1:])
    start_task(task_description)


if __name__ == "__main__":
    main()
