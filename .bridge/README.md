# Claude-Gemini Bridge: ASCII Exposed Watchdog

This system orchestrates a handoff loop between Claude CLI (Worker) and Gemini CLI (Planner) using plain-text ASCII state fragments for high resilience and observability.

## Directory Structure

- `.bridge/fragments/`: Contains the state of the system (`system.ascii`, `watchdog.ascii`, `plan.ascii`, `results.ascii`).
- `.bridge/hooks/`: Contains the event scripts triggered by the CLIs.
- `.bridge/orchestrator.py`: Main state machine logic.
- `.bridge/watchdog.py`: Health monitoring and deadlock recovery script.

## How it Works

1. **Planner (Gemini)**: Analyzes the task and writes a `plan.ascii`.
2. **Worker (Claude)**: Executes the steps in `plan.ascii` and writes `results.ascii`.
3. **Orchestrator**: Monitors `system.ascii` and triggers the next agent in the loop.
4. **Watchdog**: Ensures the orchestrator is still alive. If it detects a hang (stale heartbeat), it kills the stuck process and signals recovery.

## States

- `INITIALIZING`: System setup.
- `WAITING_FOR_PLAN`: Gemini's turn to act.
- `WAITING_FOR_EXECUTION`: Claude's turn to act.
- `BLOCKED_ON_QUESTION`: An agent is waiting for clarification.
- `RECOVERY_NEEDED`: Watchdog detected a failure and intervention is required.

## Fallback System

The bridge implements a **tiered fallback system** for resilience:

### Planner Role
1. **Primary**: Gemini CLI
2. **First Fallback**: Claude CLI (takes over planning if Gemini fails)
3. **Last Resort**: LM Studio (local inference server)

### Worker Role
1. **Primary**: Claude CLI
2. **Last Resort**: LM Studio

Configure LM Studio settings in `.bridge/fragments/lm-studio-config.ascii`.

## Usage

1. Start the orchestrator: `python3 .bridge/orchestrator.py`
2. Start the watchdog: `python3 .bridge/watchdog.py`
3. Configure Claude Code hooks in `~/.claude/settings.json`:
   ```json
   "hooks": {
     "Stop": [ { "hooks": [".bridge/hooks/claude_hook.sh"] } ]
   }
   ```
4. Configure Gemini CLI hooks in `~/.gemini/settings.json`:
   ```json
   "hooks": {
     "AfterAgent": [ { "command": ".bridge/hooks/gemini_hook.sh" } ]
   }
   ```
