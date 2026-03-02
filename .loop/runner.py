#!/usr/bin/env python3
"""
Geometry OS Auto-Prompting Loop Runner

Reads STATE.md, invokes Claude with state context, Claude updates STATE.md, repeat.
Uses RGB encoding: R=context, G=action, B=target
"""
import subprocess
import sys
import os
import signal
import tempfile
import re
from datetime import datetime
from pathlib import Path

# Neural Loopback: Import Open Brain integration
PROJECT_ROOT = Path(__file__).parent.parent
sys.path.insert(0, str(PROJECT_ROOT))
try:
    from open_brain.agent_bridge import store_loop_iteration, refresh_substrate
    OPEN_BRAIN_ENABLED = True
except ImportError as e:
    print(f"[Loop] Open Brain integration disabled: {e}")
    OPEN_BRAIN_ENABLED = False

LOOP_DIR = Path(__file__).parent
STATE_FILE = LOOP_DIR / "STATE.md"
TASK_FILE = LOOP_DIR / "TASK.md"
SYSTEM_FILE = LOOP_DIR / "SYSTEM.md"
MAX_ITERATIONS = 100
TIMEOUT = 600  # 10 minutes per iteration


def get_clean_env():
    """Get environment without CLAUDECODE for nested Claude sessions."""
    env = os.environ.copy()
    env.pop("CLAUDECODE", None)
    return env


def read_state():
    """Read current state from STATE.md."""
    if not STATE_FILE.exists():
        raise FileNotFoundError(
            f"STATE.md not found at {STATE_FILE}. "
            "Run 'python .loop/start.py \"<task>\"' to initialize a task first."
        )
    return STATE_FILE.read_text()


def is_done(state_content):
    """Check if state indicates DONE action in G: Action section."""
    if not state_content:
        return False
    # Look for DONE: in the G: Action section specifically
    # Pattern: ## G: Action followed by DONE: on the same or next line
    pattern = r'##\s*G:\s*Action\s*\n\s*DONE:'
    return bool(re.search(pattern, state_content))


def run_iteration(iteration):
    """Run one iteration of the auto-prompting loop.

    Returns:
        tuple: (should_stop: bool, is_done: bool)
            - should_stop=True means exit the loop immediately (error or done)
            - is_done=True means task completed successfully
    """
    timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    print(f"\n{'='*50}")
    print(f"[{timestamp}] Iteration {iteration}")
    print(f"{'='*50}")

    # Read current state
    try:
        state = read_state()
    except FileNotFoundError as e:
        print(f"[Loop] ERROR: {e}", file=sys.stderr)
        return (True, False)  # Stop loop, not done (error)

    print(f"[Loop] Current state:\n{state[:500]}...")

    # Check if done
    if is_done(state):
        print("[Loop] DONE detected in state, stopping loop")
        return (True, True)  # Stop loop, task done

    # Read system instructions if available
    system_instructions = ""
    if SYSTEM_FILE.exists():
        system_instructions = SYSTEM_FILE.read_text()

    # Build prompt for Claude
    prompt = f"""You are in an AUTO-PROMPTING LOOP. Your job is to:

1. Read the current STATE.md below
2. Perform the action specified in G: Action
3. Update STATE.md with your progress and next action

## STATE ENCODING (Geometry OS Style)
- **R (Context)**: File state, progress, what's been done
- **G (Action)**: What to do next: READ, WRITE, EDIT, RUN, ANALYZE, or DONE
- **B (Target)**: File path, command, or content for the action

## SYSTEM INSTRUCTIONS:
{system_instructions}

## CURRENT STATE:
{state}

## YOUR TASK:
1. Execute the action in G: Action
2. Update STATE.md:
   - Update R: Context with what you did
   - Set G: Action to the NEXT action needed
   - Set B: Target for that next action
3. If the overall task is complete, set G: Action to: DONE: [summary]

Use the Write tool to update .loop/STATE.md
Use Read/Edit/Write/Bash tools as needed for the action.

DO NOT ask questions. Execute autonomously. Update state and continue.
"""

    # Write prompt to temp file atomically
    prompt_file = LOOP_DIR / "current_prompt.txt"
    atomic_write(prompt_file, prompt)

    # Invoke Claude
    print(f"[Loop] Invoking Claude...")
    try:
        result = subprocess.run(
            ["claude", "--dangerously-skip-permissions", "-p", f"@{prompt_file}"],
            capture_output=True,
            text=True,
            env=get_clean_env(),
            timeout=TIMEOUT
        )
        print(f"[Loop] Claude returned: {result.returncode}")
        if result.stdout:
            print(f"[Loop] Output: {result.stdout[:1000]}...")
        if result.stderr:
            print(f"[Loop] Stderr: {result.stderr}", file=sys.stderr)
        if result.returncode != 0:
            print(f"[Loop] WARNING: Claude exited with non-zero code: {result.returncode}", file=sys.stderr)
    except subprocess.TimeoutExpired:
        print(f"[Loop] ERROR: Claude iteration timed out after {TIMEOUT}s", file=sys.stderr)
        print("[Loop] Consider increasing TIMEOUT or simplifying the task", file=sys.stderr)
    except FileNotFoundError:
        print("[Loop] ERROR: 'claude' CLI not found in PATH", file=sys.stderr)
        print("[Loop] Ensure Claude CLI is installed and accessible", file=sys.stderr)
    except PermissionError as e:
        print(f"[Loop] ERROR: Permission denied: {e}", file=sys.stderr)
    except OSError as e:
        print(f"[Loop] ERROR: OS error invoking Claude: {e}", file=sys.stderr)
    except Exception as e:
        print(f"[Loop] ERROR: Unexpected error: {type(e).__name__}: {e}", file=sys.stderr)

    # Neural Loopback: Store iteration in Open Brain
    if OPEN_BRAIN_ENABLED:
        try:
            # Extract action and target from state
            action_match = re.search(r'##\s*G:\s*Action\s*\n\s*(\w+):', state)
            target_match = re.search(r'##\s*B:\s*Target\s*\n\s*target:\s*(.+)', state)
            action = action_match.group(1) if action_match else "UNKNOWN"
            target = target_match.group(1).strip() if target_match else "unknown"

            context_match = re.search(r'##\s*R:\s*Context\s*\n(.+?)(?=##\s*[GB]:)', state, re.DOTALL)
            context = context_match.group(1).strip()[:500] if context_match else ""

            store_loop_iteration(
                iteration=iteration,
                action=action,
                target=target,
                context=context
            )
            print(f"[Loop] Stored iteration {iteration} in Open Brain")
        except Exception as e:
            print(f"[Loop] Warning: Failed to store in Open Brain: {e}", file=sys.stderr)

    return (False, False)  # Continue loop, not done


def atomic_write(file_path, content):
    """Write file atomically using temp file + rename."""
    file_path = Path(file_path)
    # Create temp file in same directory for atomic rename
    fd, temp_path = tempfile.mkstemp(dir=file_path.parent, prefix=".tmp_")
    try:
        with os.fdopen(fd, 'w') as f:
            f.write(content)
        os.rename(temp_path, file_path)
    except Exception:
        # Clean up temp file on error
        if os.path.exists(temp_path):
            os.unlink(temp_path)
        raise


def handle_sigint(signum, frame):
    """Handle Ctrl+C for immediate exit."""
    print("\n[Loop] Interrupted by user, exiting immediately...")
    sys.exit(130)  # 128 + SIGINT(2)


def main():
    """Main loop entry point."""
    # Set up signal handler for Ctrl+C
    signal.signal(signal.SIGINT, handle_sigint)

    start_time = datetime.now()
    print(f"[Loop] Geometry OS Auto-Prompting Loop Starting... ({start_time.strftime('%Y-%m-%d %H:%M:%S')})")
    print(f"[Loop] State file: {STATE_FILE}")

    for i in range(1, MAX_ITERATIONS + 1):
        should_stop, is_done = run_iteration(i)
        if should_stop:
            end_time = datetime.now()
            duration = (end_time - start_time).total_seconds()
            if is_done:
                print(f"\n[Loop] Task completed after {i} iterations! ({duration:.1f}s total)")
                # Neural Loopback: Final substrate refresh
                if OPEN_BRAIN_ENABLED:
                    try:
                        refresh_substrate()
                        print("[Loop] Memory substrate refreshed")
                    except Exception as e:
                        print(f"[Loop] Warning: Failed to refresh substrate: {e}", file=sys.stderr)
                sys.exit(0)
            else:
                print(f"\n[Loop] Loop terminated due to error after {i} iterations ({duration:.1f}s)", file=sys.stderr)
                sys.exit(1)

    end_time = datetime.now()
    duration = (end_time - start_time).total_seconds()
    print(f"\n[Loop] ERROR: Reached max iterations ({MAX_ITERATIONS}) after {duration:.1f}s", file=sys.stderr)
    print("[Loop] Task may be too complex or stuck in a loop", file=sys.stderr)
    sys.exit(1)


if __name__ == "__main__":
    main()
