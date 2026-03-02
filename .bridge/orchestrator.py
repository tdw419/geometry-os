import os
import time
import subprocess
import signal
import threading
import requests
import json
import sys
from datetime import datetime, timezone
from pathlib import Path

# Neural Loopback: Import Open Brain integration
PROJECT_ROOT = Path(__file__).parent.parent
sys.path.insert(0, str(PROJECT_ROOT))
try:
    from open_brain.agent_bridge import store_bridge_fragment, refresh_substrate
    OPEN_BRAIN_ENABLED = True
except ImportError as e:
    print(f"[Orchestrator] Open Brain integration disabled: {e}")
    OPEN_BRAIN_ENABLED = False

# ═════════════════════════════════════════════════════════════════
# PATHS AND CONFIGURATION
# ═════════════════════════════════════════════════════════════════
BRIDGE_DIR = Path(".bridge")
FRAGMENTS_DIR = BRIDGE_DIR / "fragments"
HOOKS_DIR = BRIDGE_DIR / "hooks"

SYSTEM_FILE = FRAGMENTS_DIR / "system.ascii"
WATCHDOG_FILE = FRAGMENTS_DIR / "watchdog.ascii"
PLAN_FILE = FRAGMENTS_DIR / "plan.ascii"
RESULTS_FILE = FRAGMENTS_DIR / "results.ascii"
QUESTION_FILE = FRAGMENTS_DIR / "question.ascii"
TASK_FILE = FRAGMENTS_DIR / "task.ascii"
ANSWER_FILE = FRAGMENTS_DIR / "answer.ascii"
PLANNER_INSTRUCTIONS = FRAGMENTS_DIR / "planner-instructions.ascii"
PLUGINS_DOC = Path("docs/PLUGINS.md")
ORCHESTRATOR_PID_FILE = BRIDGE_DIR / "orchestrator.pid"
PROMPT_TEMP_FILE = BRIDGE_DIR / "current_prompt.txt"
WORKER_TEMP_FILE = BRIDGE_DIR / "worker_prompt.txt"
PROGRESS_FILE = FRAGMENTS_DIR / "worker_progress.ascii"

wake_event = threading.Event()


def get_clean_env(for_claude=False):
    """Get environment dict, optionally cleaned for Claude CLI spawning."""
    env = os.environ.copy()
    if for_claude:
        env.pop("CLAUDECODE", None)
    return env


# State machine states
STATES = {
    "INITIALIZING": "Initializing the system.",
    "WAITING_FOR_PLAN": "Waiting for the Planner (Gemini) to generate a plan.",
    "WAITING_FOR_EXECUTION": "Waiting for the Worker (Claude) to execute the plan.",
    "BLOCKED_ON_QUESTION": "Blocked on a question from one of the agents.",
    "RECOVERY_NEEDED": "Watchdog detected a failure and intervention is required.",
    "TASK_COMPLETE": "Task has been successfully completed.",
    "ERROR": "An error occurred during orchestration."
}

# ═════════════════════════════════════════════════════════════════
# LM STUDIO FALLBACK CONFIGURATION
# ═════════════════════════════════════════════════════════════════
LM_STUDIO_CONFIG_FILE = FRAGMENTS_DIR / "lm-studio-config.ascii"


def load_lm_studio_config():
    """Load LM Studio configuration from ASCII file with env overrides."""
    config = {
        "enabled": True,
        "endpoint": "http://localhost:1234",
        "planner_model": "local-model",
        "worker_model": "local-model",
        "timeout": 120,
        "planner_timeout": 300,
        "worker_timeout": 7200,
        "fallback_on_cli_failure": True,
    }

    if LM_STUDIO_CONFIG_FILE.exists():
        try:
            content = LM_STUDIO_CONFIG_FILE.read_text()
            for line in content.splitlines():
                if line.strip().startswith("endpoint:"):
                    config["endpoint"] = line.split(":", 1)[1].strip()
                elif line.strip().startswith("planner_model:"):
                    config["planner_model"] = line.split(":", 1)[1].strip()
                elif line.strip().startswith("worker_model:"):
                    config["worker_model"] = line.split(":", 1)[1].strip()
                elif line.strip().startswith("timeout:"):
                    config["timeout"] = int(line.split(":", 1)[1].strip())
                elif line.strip().startswith("planner_timeout:"):
                    config["planner_timeout"] = int(line.split(":", 1)[1].strip())
                elif line.strip().startswith("worker_timeout:"):
                    config["worker_timeout"] = int(line.split(":", 1)[1].strip())
                elif line.strip().startswith("fallback_enabled:"):
                    config["fallback_on_cli_failure"] = line.split(":", 1)[1].strip().lower() == "true"
            print(f"[Orchestrator] Loaded LM Studio config from {LM_STUDIO_CONFIG_FILE}")
        except Exception as e:
            print(f"[Orchestrator] Error loading LM Studio config: {e}")

    # Environment variables override
    if os.environ.get("LM_STUDIO_ENDPOINT"):
        config["endpoint"] = os.environ["LM_STUDIO_ENDPOINT"]
    if os.environ.get("LM_STUDIO_PLANNER_MODEL"):
        config["planner_model"] = os.environ["LM_STUDIO_PLANNER_MODEL"]
    if os.environ.get("LM_STUDIO_WORKER_MODEL"):
        config["worker_model"] = os.environ["LM_STUDIO_WORKER_MODEL"]
    if os.environ.get("LM_STUDIO_TIMEOUT"):
        config["timeout"] = int(os.environ["LM_STUDIO_TIMEOUT"])
    if os.environ.get("BRIDGE_PLANNER_TIMEOUT"):
        config["planner_timeout"] = int(os.environ["BRIDGE_PLANNER_TIMEOUT"])
    if os.environ.get("BRIDGE_WORKER_TIMEOUT"):
        config["worker_timeout"] = int(os.environ["BRIDGE_WORKER_TIMEOUT"])

    return config


LM_STUDIO_CONFIG = load_lm_studio_config()


def update_system_state(state, last_agent=None):
    """Updates the system.ascii fragment with the new state."""
    timestamp = datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")
    content = f"# schema: system-state v1\nstate: {state}\n"
    if last_agent:
        content += f"last_agent: {last_agent}\n"
    content += f"timestamp: {timestamp}\nstuck_count: 0\n"

    if SYSTEM_FILE.exists():
        try:
            old_content = SYSTEM_FILE.read_text()
            for line in old_content.splitlines():
                if line.startswith("ralph_iterations:"):
                    content += f"{line}\n"
        except Exception:
            pass

    SYSTEM_FILE.write_text(content)
    print(f"[Orchestrator] Transitioned to state: {state}")

    # Neural Loopback: Store state transition in Open Brain
    if OPEN_BRAIN_ENABLED:
        try:
            store_bridge_fragment(
                fragment_type="system",
                agent=last_agent or "Orchestrator",
                content=f"State transition: {state}",
                state_transition=state
            )
        except Exception as e:
            print(f"[Orchestrator] Warning: Failed to store in Open Brain: {e}", file=sys.stderr)


def update_watchdog_heartbeat():
    """Updates watchdog.ascii fragment with a new heartbeat."""
    timestamp = datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")
    pid = os.getpid()
    content = f"# schema: watchdog v1\nheartbeat: {timestamp}\nactive_pid: {pid}\nhealth_status: GREEN\nrecovery_action: NONE\n"
    WATCHDOG_FILE.write_text(content)


def heartbeat_loop():
    """Continuously updates heartbeat every 5 seconds."""
    while True:
        update_watchdog_heartbeat()
        time.sleep(5)


def signal_handler(signum, frame):
    """Handler for SIGUSR1 to wake the orchestrator."""
    print("[Orchestrator] Waking up due to signal...")
    wake_event.set()


def check_lm_studio_available():
    """Check if LM Studio server is running and accessible."""
    try:
        response = requests.get(f"{LM_STUDIO_CONFIG['endpoint']}/v1/models", timeout=5)
        return response.status_code == 200
    except Exception:
        return False


def call_lm_studio(prompt, system_prompt=None, model=None, role="assistant"):
    """Call LM Studio's OpenAI-compatible API."""
    if model is None:
        if role == "planner":
            model = LM_STUDIO_CONFIG["planner_model"]
        else:
            model = LM_STUDIO_CONFIG["worker_model"]

    messages = []
    if system_prompt:
        messages.append({"role": "system", "content": system_prompt})
    messages.append({"role": "user", "content": prompt})

    payload = {
        "model": model,
        "messages": messages,
        "temperature": 0.7,
        "max_tokens": 4096,
    }

    try:
        print(f"[LM Studio] Calling {role} model: {model}")
        response = requests.post(
            f"{LM_STUDIO_CONFIG['endpoint']}/v1/chat/completions",
            json=payload,
            timeout=LM_STUDIO_CONFIG["timeout"]
        )

        if response.status_code == 200:
            result = response.json()
            content = result["choices"][0]["message"]["content"]
            print(f"[LM Studio] Response received ({len(content)} chars)")
            return content
        else:
            print(f"[LM Studio] Error: {response.status_code} - {response.text}")
            return None
    except requests.exceptions.Timeout:
        print("[LM Studio] Request timed out")
        return None
    except Exception as e:
        print(f"[LM Studio] Exception: {e}")
        return None


def run_agent(command, input_content=None, role="worker", fallback_prompt=None, fallback_system=None):
    """Runs an agent CLI with tiered fallback."""
    # Get timeout based on role
    if role == "planner":
        timeout = LM_STUDIO_CONFIG.get("planner_timeout", 300)
    else:
        timeout = LM_STUDIO_CONFIG.get("worker_timeout", 7200)

    print(f"[Orchestrator] Invoking: {' '.join(command)} (timeout: {timeout}s)")
    cli_success = False
    stdout = None

    # Progress tracking for worker
    start_time = time.time()

    # TIER 1: PRIMARY CLI
    is_claude_cmd = "claude" in command[0] if command else False
    try:
        process = subprocess.Popen(
            command,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            env=get_clean_env(for_claude=is_claude_cmd)
        )
        stdout, stderr = process.communicate(input=input_content, timeout=timeout)
        print(f"[Orchestrator] Agent finished with code {process.returncode}")
        if stdout:
            print(f"[Orchestrator] Stdout: {stdout[:500]}...")
        if stderr:
            print(f"[Orchestrator] Stderr: {stderr}")

        if process.returncode == 0 and stdout:
            cli_success = True
            return stdout
        else:
            print(f"[Orchestrator] Primary CLI failed with code {process.returncode}")

    except subprocess.TimeoutExpired:
        print("[Orchestrator] Primary CLI timed out")
        process.kill()
    except FileNotFoundError:
        print(f"[Orchestrator] Primary CLI not found: {command[0]}")
    except Exception as e:
        print(f"[Orchestrator] Error running primary agent: {e}")

    # TIER 2: CLAUDE CLI FALLBACK (for planner role)
    if not cli_success and role == "planner":
        print("[Orchestrator] Attempting Claude CLI as fallback planner...")
        try:
            if fallback_prompt:
                PROMPT_TEMP_FILE.write_text(fallback_prompt)
            else:
                PROMPT_TEMP_FILE.write_text(input_content or "Generate a plan.")

            claude_command = ["claude", "--dangerously-skip-permissions", "-p", f"@{PROMPT_TEMP_FILE}"]
            process = subprocess.Popen(
                claude_command,
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                env=get_clean_env(for_claude=True)
            )
            stdout, stderr = process.communicate(timeout=timeout)
            print(f"[Orchestrator] Claude fallback finished with code {process.returncode}")

            if process.returncode == 0 and stdout:
                print(f"[Orchestrator] Claude fallback succeeded!")
                _write_plan_from_cli(stdout, "Claude_Fallback")
                return stdout
            else:
                print(f"[Orchestrator] Claude fallback failed: {stderr}")

        except subprocess.TimeoutExpired:
            print("[Orchestrator] Claude fallback timed out")
            process.kill()
        except FileNotFoundError:
            print("[Orchestrator] Claude CLI not found for fallback")
        except Exception as e:
            print(f"[Orchestrator] Claude fallback error: {e}")

    # TIER 3: LM STUDIO FALLBACK
    if not cli_success and LM_STUDIO_CONFIG["fallback_on_cli_failure"]:
        if not fallback_prompt:
            fallback_prompt = input_content or "Continue the task."

        print(f"[Orchestrator] Attempting LM Studio fallback for {role}...")

        if check_lm_studio_available():
            result = call_lm_studio(
                prompt=fallback_prompt,
                system_prompt=fallback_system,
                role=role
            )
            if result:
                if role == "planner":
                    _write_plan_from_lm_studio(result)
                else:
                    _write_results_from_lm_studio(result)
                return result
        else:
            print("[Orchestrator] LM Studio not available for fallback")

    return stdout


def _write_plan_from_cli(content, agent_name):
    """Write plan fragment from CLI fallback response."""
    timestamp = datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")
    plan_content = f"# schema: plan v3 (TOON)\n# Generated by {agent_name}\nobjective: |\n  {content[:200]}...\n\nraw_output: |\n"
    plan_content += "\n".join(f"  {line}" for line in content.split("\n"))
    PLAN_FILE.write_text(plan_content)
    update_system_state("WAITING_FOR_EXECUTION", agent_name)
    print(f"[Orchestrator] Plan written to plan.ascii by {agent_name}")

    # Neural Loopback: Store plan in Open Brain
    if OPEN_BRAIN_ENABLED:
        try:
            store_bridge_fragment("plan", agent_name, content[:2000])
        except Exception as e:
            print(f"[Orchestrator] Warning: Failed to store plan in Open Brain: {e}", file=sys.stderr)


def _write_plan_from_lm_studio(content):
    """Write plan fragment from LM Studio response."""
    timestamp = datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")
    plan_content = f"# schema: plan v3 (TOON)\n# Generated by LM Studio fallback\nobjective: |\n  {content[:200]}...\n\nraw_output: |\n"
    plan_content += "\n".join(f"  {line}" for line in content.split("\n"))
    PLAN_FILE.write_text(plan_content)
    _update_state_after_lm_studio("WAITING_FOR_EXECUTION", "LM_Studio_Planner")
    print("[LM Studio] Plan written to plan.ascii")


def _write_results_from_lm_studio(content):
    """Write results fragment from LM Studio response."""
    timestamp = datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")
    results_content = f"# schema: results v2\ncompleted: {timestamp}\noutput: |\n"
    results_content += "\n".join(f"  {line}" for line in content.split("\n"))
    RESULTS_FILE.write_text(results_content)
    if "<promise>DONE</promise>" in content:
        _update_state_after_lm_studio("TASK_COMPLETE", "LM_Studio_Worker")
        print("[LM Studio] Task complete signal detected")
    else:
        _update_state_after_lm_studio("WAITING_FOR_PLAN", "LM_Studio_Worker")
    print("[LM Studio] Results written to results.ascii")


def _update_state_after_lm_studio(state, agent):
    """Update system state after LM Studio operation."""
    timestamp = datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")
    content = f"# schema: system-state v1\nstate: {state}\nlast_agent: {agent}\n"
    content += f"timestamp: {timestamp}\nstuck_count: 0\n"

    if SYSTEM_FILE.exists():
        try:
            old_content = SYSTEM_FILE.read_text()
            for line in old_content.splitlines():
                if line.startswith("ralph_iterations:"):
                    content += f"{line}\n"
        except Exception:
            pass

    SYSTEM_FILE.write_text(content)
    print(f"[LM Studio] State updated to: {state}")


def invoke_planner(task_content):
    """Invokes Gemini with planner system instructions, results, and plugin docs."""
    if PLANNER_INSTRUCTIONS.exists():
        system_prompt = PLANNER_INSTRUCTIONS.read_text()
    else:
        system_prompt = "You are a PLANNING AGENT. You analyze tasks and produce executable plans."

    # Include results from previous iteration if available
    results_context = ""
    if RESULTS_FILE.exists():
        try:
            results_content = RESULTS_FILE.read_text()
            results_context = f"\n\nPREVIOUS EXECUTION RESULTS:\n{results_content}"
        except Exception:
            pass

    if PLUGINS_DOC.exists():
        plugin_context = PLUGINS_DOC.read_text()
        system_prompt += f"\n\nAVAILABLE PLUGINS REFERENCE:\n{plugin_context}"

    full_prompt = f"{system_prompt}{results_context}\n\nTASK TO PLAN:\n{task_content}"
    PROMPT_TEMP_FILE.write_text(full_prompt)

    run_agent(
        command=["gemini", "-p", f"@{PROMPT_TEMP_FILE}"],
        role="planner",
        fallback_prompt=full_prompt,
        fallback_system=system_prompt
    )


def prompt_human(question_content):
    """Prompts a human in the terminal for an answer."""
    print("\n" + "="*40)
    print("QUESTION DETECTED:")
    print(question_content)
    print("="*40)
    answer = input("Your Answer: ")
    timestamp = datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")
    ANSWER_FILE.write_text(f"# schema: answer v1\nanswered_at: {timestamp}\ncontent: |\n  {answer}\n")
    return answer


def main_loop():
    """Main orchestration loop."""
    print("[Orchestrator] Starting...")
    ORCHESTRATOR_PID_FILE.write_text(str(os.getpid()))
    signal.signal(signal.SIGUSR1, signal_handler)

    hb_thread = threading.Thread(target=heartbeat_loop, daemon=True)
    hb_thread.start()

    # Try to resume existing state, otherwise start with INITIALIZING
    if not SYSTEM_FILE.exists():
        update_system_state("INITIALIZING")
    else:
        print("[Orchestrator] Resuming from existing state in system.ascii")

    while True:
        try:
            system_content = SYSTEM_FILE.read_text()
            state_line = [l for l in system_content.splitlines() if l.startswith("state:")][0]
            current_state = state_line.split(":")[1].strip()
        except Exception as e:
            print(f"[Orchestrator] Error reading state: {e}")
            wake_event.wait(5)
            wake_event.clear()
            continue

        if current_state == "INITIALIZING":
            if TASK_FILE.exists():
                update_system_state("WAITING_FOR_PLAN")
            else:
                print("[Orchestrator] Waiting for task.ascii...")

        elif current_state == "WAITING_FOR_PLAN":
            try:
                task_content = TASK_FILE.read_text()
                invoke_planner(task_content)
            except Exception as e:
                print(f"[Orchestrator] Error invoking planner: {e}")

        elif current_state == "WAITING_FOR_EXECUTION":
            if PLAN_FILE.exists():
                try:
                    plan_content = PLAN_FILE.read_text()
                    worker_system = """You are a WORKER AGENT. Execute the plan provided below.
Read the plan carefully and perform each step using your tools (Read, Edit, Write, Bash).
When the task is fully complete and verified, output: <promise>DONE</promise>
If you encounter issues, describe them clearly before stopping.

PLAN TO EXECUTE:
"""
                    full_prompt = worker_system + plan_content
                    WORKER_TEMP_FILE.write_text(full_prompt)
                    run_agent(
                        command=["claude", "--dangerously-skip-permissions", "-p", f"@{WORKER_TEMP_FILE}"],
                        role="worker",
                        fallback_prompt=full_prompt,
                        fallback_system=worker_system
                    )
                except Exception as e:
                    print(f"[Orchestrator] Error invoking worker: {e}")

        elif current_state == "BLOCKED_ON_QUESTION":
            if QUESTION_FILE.exists():
                question_content = QUESTION_FILE.read_text()
                prompt_human(question_content)
                last_agent_line = [l for l in system_content.splitlines() if l.startswith("last_agent:")][0]
                last_agent = last_agent_line.split(":")[1].strip()
                if last_agent == "Gemini":
                    update_system_state("WAITING_FOR_PLAN")
                else:
                    update_system_state("WAITING_FOR_EXECUTION")

        elif current_state == "RECOVERY_NEEDED":
            print("[Orchestrator] Recovery needed, check fragments.")

        elif current_state == "TASK_COMPLETE":
            print("[Orchestrator] MISSION ACCOMPLISHED. Task finalized by Worker.")
            break

        print("[Orchestrator] Sleeping...")
        wake_event.wait(5)
        wake_event.clear()


if __name__ == "__main__":
    main_loop()
