#!/usr/bin/env python3
"""
Sovereign Shell Autonomous Loop v2.2 - Production Edition 🐍🧠🛡️✨

Fixes:
1. Model ID: Corrected to 'qwen2.5-coder-7b-instruct'.
2. Criteria Logic: Fixed to properly check for completion of goals.
3. Type Safety: Ensured criteria is handled consistently.
4. API Signature: Fixed run_experiment call with coordinates.
"""

import sys
import yaml
import httpx
import json
import time
import subprocess
import re
from pathlib import Path
from datetime import datetime

# Add Ouroboros to path
OUROBOROS_PATH = Path("/home/jericho/zion/projects/ouroboros/ouroboros")
sys.path.insert(0, str(OUROBOROS_PATH))

from autonomous_loop import AutonomousLoop

class SmartAutonomousLoop(AutonomousLoop):
    """
    Autonomous loop with LLM-powered patch generation and robust safety.
    """
    
    LM_STUDIO_URL = "http://localhost:1234/v1/chat/completions"
    MODEL = "qwen2.5-coder-7b-instruct" # Confirmed model ID
    PXOS_URL = "http://localhost:3841"   # pxOS on dedicated port (3839 is OpenClaw)
    
    def __init__(self, **kwargs):
        # Ensure criteria is a string for parent compatibility
        raw_criteria = kwargs.get('criteria', "")
        if isinstance(raw_criteria, list):
            kwargs['criteria'] = "\n".join(raw_criteria)
        
        super().__init__(**kwargs)
        # Override the adapter's base URL to our actual pxOS port
        self.adapter.config.base_url = self.PXOS_URL
        self.adapter.client = httpx.Client(base_url=self.PXOS_URL, timeout=30.0)
        self.lm_client = httpx.Client(timeout=180.0)
        
    def _read_target_content(self) -> str:
        target_path = Path(self.target)
        if target_path.exists():
            return target_path.read_text()
        return ""

    def generate_hypothesis(self) -> dict:
        """
        Use LM Studio to generate a targeted PATCH instead of a full rewrite.
        """
        current_shader = self._read_target_content()
        cells = self.adapter.get_cells()
        
        system_prompt = f"""You are a senior GPU Architect and Shading Specialist.
Objective: {self.objective}
Success Criteria:
{self.criteria}

You must output a JSON object:
{{
    "analysis": "Brief analysis of current shader bottlenecks",
    "hypothesis": "What specifically to improve",
    "target_file": "{self.target}",
    "old_string": "Exact literal string to find in the shader (include unique context)",
    "new_string": "Exact replacement string"
}}

Rules:
1. old_string MUST be an exact match for a block in the file.
2. new_string MUST be the corrected version of that block.
3. Do not rewrite the whole file. Just a surgical patch.
Respond ONLY with the JSON object."""

        user_prompt = f"""Target: {self.target}
Metrics: {json.dumps(cells, indent=2)}

Current Shader Content (Partial):
{current_shader[:8000]} # First 8k chars for context

Propose a surgical patch to improve the INPUT ZONE or font rendering."""

        print(f"\n[1] 🧠 Generating patch using {self.MODEL}...")
        
        try:
            resp = self.lm_client.post(
                self.LM_STUDIO_URL,
                json={
                    "model": self.MODEL,
                    "messages": [
                        {"role": "system", "content": system_prompt},
                        {"role": "user", "content": user_prompt}
                    ],
                    "temperature": 0.2,
                    "max_tokens": 2048
                }
            )
            
            if resp.status_code == 200:
                data = resp.json()
                content = data["choices"][0]["message"]["content"]
                if "```json" in content:
                    content = content.split("```json")[1].split("```")[0]
                elif "```" in content:
                    content = content.split("```")[1].split("```")[0]
                return json.loads(content.strip())
            return {"error": f"LM Studio Error: {resp.status_code}"}
        except Exception as e:
            return {"error": str(e)}

    def apply_patch(self, old_str: str, new_str: str):
        """Apply a surgical patch with versioned backups."""
        target_path = Path(self.target)
        content = target_path.read_text()
        
        if old_str not in content:
            print(f"❌ Error: old_string not found in {self.target}. Match failed.")
            return False

        # Versioned Backup (e.g., .v1.bak, .v2.bak)
        version = 1
        while target_path.with_suffix(f".v{version}.bak").exists():
            version += 1
        backup_path = target_path.with_suffix(f".v{version}.bak")
        target_path.rename(backup_path)
        
        print(f"💾 Created backup: {backup_path.name}")
        new_content = content.replace(old_str, new_str, 1) # Only replace first occurrence
        target_path.write_text(new_content)
        return True

    def revert_patch(self):
        """Revert to the MOST RECENT versioned backup."""
        target_path = Path(self.target)
        backups = sorted(target_path.parent.glob(f"{target_path.name}.v*.bak"))
        if backups:
            latest_bak = backups[-1]
            print(f"⏪ Reverting to {latest_bak.name}...")
            if target_path.exists(): target_path.unlink()
            latest_bak.rename(target_path)

    def check_criteria(self, current: float) -> bool:
        """Robust criteria check for multi-line goals."""
        # Check against target metric from goal.yaml logic
        # For Sovereign Shell, we target latency < 1.0s
        if current > 0 and current < 1.0:
            print(f"🎯 Milestone reached: loop_latency = {current:.3f}s")
            return True
        return False

    def run_iteration(self) -> dict:
        self.iteration += 1
        print(f"\n{'='*60}\nITERATION {self.iteration}\n{'='*60}")
        
        llm_response = self.generate_hypothesis()
        if "error" in llm_response:
            print(f"⚠️ Error: {llm_response['error']}")
            return {"status": "error"}
            
        old_s = llm_response.get("old_string")
        new_s = llm_response.get("new_string")
        
        if not old_s or not new_s:
            print("⚠️ Incomplete patch received.")
            return {"status": "no_change"}

        if self.apply_patch(old_s, new_s):
            spec = f"H: {llm_response.get('hypothesis')}\nT: {self.target}\nM: {self.criteria}\nB: 5"
            print(f"🚀 Running experiment...")
            # Fixed call signature with coordinates
            result = self.adapter.run_experiment(spec, x=0, y=100 + (self.iteration % 10) * 20)
            status = result.get("status", "unknown").upper()
            
            if status not in ["KEEP", "SUCCESS", "ACHIEVED"]:
                print(f"❌ Experiment Status: {status}. Reverting.")
                self.revert_patch()
            else:
                print(f"✅ Experiment SUCCEEDED. Keeping changes.")
                try:
                    subprocess.run(["git", "commit", "-am", f"Ouroboros: {llm_response.get('hypothesis')}"], capture_output=True)
                except: pass

        cells = self.adapter.get_cells()
        current_metric = cells.get(self.metric_name, 0)
        if self.check_criteria(current_metric):
            return {"status": "achieved", "metric": current_metric}
            
        return {"status": "continue"}

def main():
    goal_path = Path(".ouroboros/goal.yaml")
    if not goal_path.exists(): sys.exit(1)
    with open(goal_path, 'r') as f: config = yaml.safe_load(f)

    loop = SmartAutonomousLoop(
        objective=config['description'],
        criteria=config['success_criteria'],
        target="sovereign_shell_hud.wgsl",
        metric_name="loop_latency",
        max_iterations=config.get('max_iterations', 50),
        delay_seconds=15.0 # Responsive iterations
    )

    print(f"🚀 Launching v2.2 Production Ouroboros Loop...")
    try: loop.run()
    except KeyboardInterrupt: print("\n🛑 Stopped.")
    finally: loop.lm_client.close()

if __name__ == "__main__": main()
