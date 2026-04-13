#!/usr/bin/env python3
"""
chain_dev.py -- Geometry OS self-chaining development loop.

Picks the next unchecked roadmap item, builds a self-contained prompt,
runs hermes chat, verifies tests, auto-reverts on failure.

Usage:
    python3 chain_dev.py                  # run until stopped or done
    python3 chain_dev.py --once           # run exactly one cycle
    python3 chain_dev.py --dry-run        # show what would be done
"""

import subprocess
import sys
import os
import re
import json
import time
import signal
from pathlib import Path

PROJECT_DIR = Path.home() / "zion" / "projects" / "geometry_os" / "geometry_os"
ROADMAP_PATH = PROJECT_DIR / "ROADMAP.md"
CARRY_FORWARD = Path.home() / "zion" / "projects" / "carry_forward" / "carry_forward" / "carry_forward.py"
SESSION_CHAIN = Path.home() / "zion" / "projects" / "session_relay" / "session_relay" / "session_chain.py"
LOG_FILE = Path("/tmp/geometry_os_chain.log")
MAX_CYCLES = 20
CYCLE_TIMEOUT = 1200  # 20 minutes per Hermes session
CONSECUTIVE_FAILURE_LIMIT = 2

_running = True


def log(msg):
    ts = time.strftime("%Y-%m-%d %H:%M:%S")
    line = f"[{ts}] {msg}"
    print(line, flush=True)
    with open(LOG_FILE, "a") as f:
        f.write(line + "\n")


def handle_sigterm(signum, frame):
    global _running
    log("SIGTERM received, stopping after current cycle")
    _running = False


signal.signal(signal.SIGTERM, handle_sigterm)


def pick_next_task():
    """Parse ROADMAP.md for the first unchecked checkbox in Priority Order section."""
    text = ROADMAP_PATH.read_text()
    
    # Find the Priority Order section
    in_priority = False
    for line in text.splitlines():
        if "Priority Order for Automated Development" in line:
            in_priority = True
            continue
        if in_priority and line.startswith("##"):
            break
        if in_priority and line.strip().startswith("- [ ]"):
            # Extract task description
            task = line.strip()
            # Remove the checkbox prefix
            task = re.sub(r'^- \[ \]\s*', '', task)
            return task
    
    return None


def get_carry_forward_context():
    """Get carry_forward context for the project."""
    try:
        result = subprocess.run(
            ["python3", str(CARRY_FORWARD), "context", "--project", str(PROJECT_DIR)],
            capture_output=True, text=True, timeout=30
        )
        if result.returncode == 0:
            # Truncate to keep prompt reasonable
            output = result.stdout
            if len(output) > 3000:
                output = output[:3000] + "\n... (truncated)"
            return output
    except Exception as e:
        log(f"carry_forward context failed: {e}")
    return ""


def get_git_state():
    """Get current git state."""
    try:
        head = subprocess.run(
            ["git", "rev-parse", "--short", "HEAD"],
            capture_output=True, text=True, cwd=PROJECT_DIR
        ).stdout.strip()
        
        dirty = subprocess.run(
            ["git", "status", "--short"],
            capture_output=True, text=True, cwd=PROJECT_DIR
        ).stdout.strip()
        
        test_count = subprocess.run(
            ["cargo", "test", "2>&1", "|", "grep", "-c", "^test "],
            capture_output=True, text=True, cwd=PROJECT_DIR, shell=True
        ).stdout.strip()
        
        return f"Git HEAD: {head}\nDirty files: {dirty or 'none'}\n"
    except Exception as e:
        return f"Git state unavailable: {e}"


def run_tests():
    """Run cargo test and return (success, output)."""
    result = subprocess.run(
        ["cargo", "test"],
        capture_output=True, text=True, cwd=PROJECT_DIR, timeout=120
    )
    output = result.stdout + result.stderr
    # Count passing tests
    passed = 0
    for line in output.splitlines():
        m = re.match(r"test result: ok\. (\d+) passed", line)
        if m:
            passed += int(m.group(1))
    return result.returncode == 0, passed, output


def auto_revert():
    """Revert uncommitted changes if tests fail."""
    log("Auto-reverting uncommitted changes...")
    subprocess.run(["git", "checkout", "--", "."], cwd=PROJECT_DIR, capture_output=True)
    subprocess.run(["git", "clean", "-fd"], cwd=PROJECT_DIR, capture_output=True)
    # Verify revert worked
    ok, count, _ = run_tests()
    if ok:
        log(f"Revert successful, tests green ({count} passing)")
        return True
    else:
        log("Revert failed -- last commit is broken!")
        return False


def build_prompt(task, context, git_state):
    """Build the self-contained prompt for Hermes."""
    north_star = (PROJECT_DIR / "docs" / "NORTH_STAR.md").read_text()
    ai_guide = (PROJECT_DIR / "AI_GUIDE.md").read_text()
    
    # Get current roadmap phase details
    roadmap_yaml = (PROJECT_DIR / "roadmap.yaml").read_text()
    
    prompt = f"""## NORTH STAR -- READ THIS FIRST
{north_star}

## TASK
{task}

## PROJECT CONTEXT
{ai_guide}

## CURRENT STATE
{git_state}

## CARRY FORWARD CONTEXT
{context}

## INSTRUCTIONS
1. Read docs/NORTH_STAR.md first
2. Read the relevant phase in ROADMAP.md for the full spec
3. Read AI_GUIDE.md for file layout and conventions
4. Implement the deliverables for this task
5. Write tests for every new behavior
6. Run `cargo test` to verify all tests pass
7. Run `cargo build` to check for warnings
8. Commit with a descriptive message
9. Update roadmap.yaml deliverable statuses to "done"
10. Do NOT push to git (the chain script handles that)

## RULES
- Every commit must leave `cargo test` green
- Every new opcode/feature gets a test
- Follow the opcode addition checklist in AI_GUIDE.md
- Use existing patterns (look at how SPAWN, PEEK, MOV were added)
- Don't add speculative features -- build exactly what the roadmap says
- Keep the North Star in mind: we're building an OS, not polishing a VM
"""
    return prompt


def run_cycle(dry_run=False):
    """Run one development cycle. Returns exit code."""
    log("=" * 60)
    log("Starting new cycle")
    
    # 1. Pick next task
    task = pick_next_task()
    if not task:
        log("No tasks remaining! Roadmap complete.")
        return 0  # permanent stop
    
    log(f"Next task: {task[:80]}...")
    
    if dry_run:
        log("Dry run -- would execute this task")
        return 0
    
    # 2. Get context
    context = get_carry_forward_context()
    git_state = get_git_state()
    log(f"Git state: {git_state.strip().splitlines()[0]}")
    
    # 3. Check carry_forward gate
    try:
        gate = subprocess.run(
            ["python3", str(CARRY_FORWARD), "should-continue", "--project", str(PROJECT_DIR)],
            capture_output=True, text=True, timeout=30
        )
        if gate.returncode != 0:
            log(f"Carry Forward gate says stop: {gate.stdout.strip()}")
            return 2  # hard stop
    except Exception as e:
        log(f"Gate check failed: {e} (continuing anyway)")
    
    # 4. Build prompt
    prompt = build_prompt(task, context, git_state)
    
    # 5. Run Hermes
    log(f"Running Hermes session (timeout: {CYCLE_TIMEOUT}s)...")
    start = time.time()
    try:
        result = subprocess.run(
            ["hermes", "chat", "-q", prompt, "--yolo", "-Q",
             "-s", "rust-safe-edits"],
            capture_output=True, text=True, timeout=CYCLE_TIMEOUT,
            cwd=PROJECT_DIR
        )
        elapsed = time.time() - start
        log(f"Hermes finished in {elapsed:.0f}s (exit code {result.returncode})")
        
        # Log abbreviated output
        output = result.stdout
        if len(output) > 2000:
            output = output[:1000] + "\n... (truncated) ...\n" + output[-1000:]
        log(f"Hermes output:\n{output}")
        
    except subprocess.TimeoutExpired:
        elapsed = time.time() - start
        log(f"Hermes timed out after {elapsed:.0f}s")
    
    # 6. Run tests
    log("Running tests...")
    ok, count, test_output = run_tests()
    
    if ok:
        log(f"Tests PASS: {count} tests green")
        
        # Record git heads
        try:
            subprocess.run(
                ["python3", str(CARRY_FORWARD), "record-git-heads",
                 "--project", str(PROJECT_DIR)],
                capture_output=True, text=True, timeout=10
            )
        except:
            pass
        
        # Commit any uncommitted roadmap changes
        subprocess.run(["git", "add", "-A"], cwd=PROJECT_DIR, capture_output=True)
        subprocess.run(
            ["git", "commit", "-m", "chain: update roadmap after cycle", "--allow-empty"],
            cwd=PROJECT_DIR, capture_output=True
        )
        
        # Push
        subprocess.run(["git", "push"], cwd=PROJECT_DIR, capture_output=True)
        log("Pushed to GitHub")
        
        return 1  # continue
        
    else:
        log(f"Tests FAIL after Hermes session")
        log(f"Test output (last 20 lines):\n" + "\n".join(test_output.splitlines()[-20:]))
        
        # Auto-revert
        if auto_revert():
            return 1  # continue (revert succeeded)
        else:
            return 2  # hard stop (broken commit)


def main():
    global _running
    
    args = sys.argv[1:]
    once = "--once" in args
    dry_run = "--dry-run" in args
    
    log("Geometry OS Chain starting")
    log(f"Project: {PROJECT_DIR}")
    log(f"Roadmap: {ROADMAP_PATH}")
    
    consecutive_failures = 0
    cycle = 0
    
    while _running and cycle < MAX_CYCLES:
        cycle += 1
        log(f"Cycle {cycle}/{MAX_CYCLES}")
        
        exit_code = run_cycle(dry_run=dry_run)
        
        if exit_code == 0:
            log("Roadmap complete! Stopping permanently.")
            sys.exit(0)
        elif exit_code == 2:
            log("Hard stop (broken commit or gate). Stopping.")
            sys.exit(2)
        elif exit_code == 1:
            # Check if tests actually passed (exit_code 1 means continue)
            # Re-run tests to confirm
            ok, count, _ = run_tests()
            if ok:
                consecutive_failures = 0
            else:
                consecutive_failures += 1
        
        if consecutive_failures >= CONSECUTIVE_FAILURE_LIMIT:
            log(f"{CONSECUTIVE_FAILURE_LIMIT} consecutive failures. Circuit breaker.")
            sys.exit(2)
        
        if once:
            log("--once mode, stopping after 1 cycle")
            break
        
        log("Waiting 5s before next cycle...")
        time.sleep(5)
    
    if not _running:
        log("Stopped by signal")
        sys.exit(2)
    
    log(f"Reached max cycles ({MAX_CYCLES})")
    sys.exit(1)


if __name__ == "__main__":
    main()
