import os
import time
from datetime import datetime, timedelta
from pathlib import Path
import signal

# Paths to fragments
BRIDGE_DIR = Path(".bridge")
FRAGMENTS_DIR = BRIDGE_DIR / "fragments"
WATCHDOG_FILE = FRAGMENTS_DIR / "watchdog.ascii"
SYSTEM_FILE = FRAGMENTS_DIR / "system.ascii"

STUCK_THRESHOLD = 30  # seconds

def get_watchdog_state():
    """Reads the current watchdog state."""
    try:
        content = WATCHDOG_FILE.read_text()
        data = {}
        for line in content.splitlines():
            if ":" in line:
                key, value = line.split(":", 1)
                data[key.strip()] = value.strip()
        return data
    except Exception as e:
        print(f"[Watchdog] Error reading watchdog file: {e}")
        return None

def monitor():
    """Monitor loop for the watchdog."""
    print("[Watchdog] Starting...")
    
    while True:
        state = get_watchdog_state()
        if state:
            heartbeat_str = state.get("heartbeat")
            if heartbeat_str:
                try:
                    heartbeat_dt = datetime.fromisoformat(heartbeat_str.replace("Z", ""))
                    now = datetime.utcnow()
                    
                    if now - heartbeat_dt > timedelta(seconds=STUCK_THRESHOLD):
                        print(f"[Watchdog] HEARTBEAT STALE: {heartbeat_str}")
                        handle_stuck_system(state)
                except Exception as e:
                    print(f"[Watchdog] Error parsing heartbeat: {e}")
        
        time.sleep(10)

def handle_stuck_system(state):
    """Initiates recovery for a stuck system."""
    pid_str = state.get("active_pid")
    if pid_str and pid_str != "none":
        pid = int(pid_str)
        print(f"[Watchdog] Attempting to kill stuck process PID: {pid}")
        try:
            os.kill(pid, signal.SIGTERM)
            print(f"[Watchdog] Process {pid} terminated.")
        except ProcessLookupError:
            print(f"[Watchdog] Process {pid} already dead.")
        except Exception as e:
            print(f"[Watchdog] Failed to kill process {pid}: {e}")
    
    # Update system state to indicate error/recovery
    try:
        content = SYSTEM_FILE.read_text()
        # Increment stuck count
        new_content = []
        for line in content.splitlines():
            if line.startswith("stuck_count:"):
                count = int(line.split(":")[1].strip()) + 1
                new_content.append(f"stuck_count: {count}")
            elif line.startswith("state:"):
                new_content.append("state: RECOVERY_NEEDED")
            else:
                new_content.append(line)
        SYSTEM_FILE.write_text("\n".join(new_content))
    except Exception as e:
        print(f"[Watchdog] Failed to update system state: {e}")

if __name__ == "__main__":
    monitor()
