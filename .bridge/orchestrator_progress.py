"""
Progress tracking for the Claude-Gemini Bridge.
Provides real-time updates during worker execution.
"""
import os
import time
import threading
from datetime import datetime, timezone
from pathlib import Path

class ProgressTracker:
    """Tracks progress of worker agents and writes to ASCII file."""

    def __init__(self, progress_file: Path, role: str = "worker"):
        self.progress_file = progress_file
        self.role = role
        self.start_time = time.time()
        self._running = True
        self._thread = None

    def start(self):
        """Start progress tracking thread."""
        self._thread = threading.Thread(target=self._track, daemon=True)
        self._thread.start()

    def stop(self):
        """Stop progress tracking."""
        self._running = False
        if self._thread:
            self._thread.join(timeout=1)

    def _track(self):
        """Write progress updates periodically."""
        while self._running:
            elapsed = time.time() - self.start_time
            progress_content = f"""# schema: progress v1
role: {self.role}
status: running
elapsed_seconds: {int(elapsed)}
started: {datetime.fromtimestamp(self.start_time, timezone.utc).isoformat()}
last_heartbeat: {datetime.now(timezone.utc).isoformat()}
timeout_remaining: auto-calculated
"""
            try:
                self.progress_file.write_text(progress_content)
            except Exception:
                pass
            time.sleep(30)  # Update every 30 seconds

    def get_elapsed_str(self):
        """Get human-readable elapsed time."""
        elapsed = time.time() - self.start_time
        if elapsed < 60:
            return f"{int(elapsed)}s"
        elif elapsed < 3600:
            return f"{int(elapsed / 60)}m {int(elapsed % 60)}s"
        else:
            hours = int(elapsed / 3600)
            minutes = int((elapsed % 3600) / 60)
            return f"{hours}h {minutes}m"
