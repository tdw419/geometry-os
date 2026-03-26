#!/usr/bin/env python3
"""
Sovereign Shell Autonomous Loop v3.1 - Obsidian Vision 🧠🔳🛰️

Upgrades:
1. Deep Context: Sends the LAST 15,000 characters (The Frontier).
2. Pro Brain: Gemini 1.5 Pro as Primary (most stable for math).
3. Debug Pulse: Prints every interaction step.
"""

import sys
import os
import yaml
import httpx
import json
import time
import subprocess
import re
from pathlib import Path

OUROBOROS_PATH = Path("/home/jericho/zion/projects/ouroboros/ouroboros")
sys.path.insert(0, str(OUROBOROS_PATH))

from autonomous_loop import AutonomousLoop

class SmartAutonomousLoop(AutonomousLoop):
    LM_STUDIO_URL = "http://localhost:1234/v1/chat/completions"
    MODEL = "qwen2.5-coder-7b-instruct"
    PXOS_URL = "http://localhost:3841"
    
    def __init__(self, **kwargs):
        raw_criteria = kwargs.get('criteria', "")
        if isinstance(raw_criteria, list):
            kwargs['criteria'] = "\n".join(raw_criteria)
        kwargs['max_iterations'] = 10000
        super().__init__(**kwargs)
        self.adapter.config.base_url = self.PXOS_URL
        self.adapter.client = httpx.Client(base_url=self.PXOS_URL, timeout=30.0)
        self.lm_client = httpx.Client(timeout=180.0)
        
    def _read_target_content(self) -> str:
        target_path = Path(self.target)
        return target_path.read_text() if target_path.exists() else ""

    def generate_hypothesis(self) -> dict:
        full_content = self._read_target_content()
        # Take the tail end where active logic resides
        context_window = full_content[-15000:] if len(full_content) > 15000 else full_content
        cells = self.adapter.get_cells()
        
        system_prompt = f"""You are the Lead Architectural Weaver for Geometry OS.
Objective: {self.objective}
Philosophy: "Code is Geometry."
Note: You are seeing the TAIL END of sovereign_shell_hud.wgsl.

Rules:
1. Provide a JSON object with 'analysis', 'hypothesis', 'old_string', and 'new_string'.
2. old_string must exist EXACTLY in the provided tail.
3. new_string must be the improved code.
4. Surgical patches only. Respond ONLY with JSON."""

        user_prompt = f"Metrics: {json.dumps(cells)}\n\n--- SHADER TAIL ---\n{context_window}"

        # 1. Gemini Primary (Stable & Intelligent)
        gemini_key = os.environ.get("GEMINI_API_KEY")
        if gemini_key:
            print("\n[1] 🧠 Consulting Gemini 1.5 Pro (Primary)...")
            try:
                url = f"https://generativelanguage.googleapis.com/v1beta/models/gemini-1.5-pro:generateContent?key={gemini_key}"
                resp = self.lm_client.post(url, json={
                    "contents": [{"parts": [{"text": f"{system_prompt}\n\n{user_prompt}"}]}],
                    "generationConfig": { "temperature": 0.2, "responseMimeType": "application/json" }
                }, timeout=180.0)
                if resp.status_code == 200:
                    data = resp.json()
                    return json.loads(data["candidates"][0]["content"]["parts"][0]["text"])
                else: print(f"⚠️ Gemini error {resp.status_code}")
            except Exception as e: print(f"⚠️ Gemini Exception: {e}")

        # 2. ZAI Secondary
        zai_key = os.environ.get("ZAI_API_KEY")
        if zai_key:
            print("\n[2] 🧠 Consulting ZAI (GLM-5)...")
            try:
                resp = self.lm_client.post(f"https://api.z.ai/api/coding/paas/v4/chat/completions", headers={
                    "Authorization": f"Bearer {zai_key}", "Content-Type": "application/json"
                }, json={
                    "model": "glm-5", "messages": [{"role": "system", "content": system_prompt}, {"role": "user", "content": user_prompt}],
                    "thinking": { "type": "enabled" }, "response_format": { "type": "json_object" },
                    "temperature": 0.2, "max_tokens": 4096
                }, timeout=180.0)
                if resp.status_code == 200:
                    data = resp.json()
                    content = data["choices"][0]["message"]["content"]
                    if "<thought>" in content: content = content.split("</thought>")[-1]
                    if "```json" in content: content = content.split("```json")[1].split("```")[0]
                    return json.loads(content.strip())
            except Exception as e: print(f"⚠️ ZAI Exception: {e}")

        return {"error": "All brains offline"}

    def apply_patch(self, old_str: str, new_str: str):
        target_path = Path(self.target)
        content = target_path.read_text()
        if old_str not in content:
            print(f"❌ Match Failed. Old string not found in shader.")
            return False

        version = 1
        while target_path.with_suffix(f".v{version}.bak").exists(): version += 1
        backup_path = target_path.with_suffix(f".v{version}.bak")
        target_path.rename(backup_path)
        
        print(f"💾 Created backup: {backup_path.name}")
        target_path.write_text(content.replace(old_str, new_str, 1))
        return True

    def run_iteration(self) -> dict:
        self.iteration += 1
        print(f"\n{'='*60}\nITERATION {self.iteration}\n{'='*60}")
        
        llm = self.generate_hypothesis()
        if "error" in llm:
            print(f"⚠️ Error: {llm['error']}")
            return {"status": "error"}
        
        if isinstance(llm, list): llm = llm[0]
        old_s, new_s = llm.get("old_string"), llm.get("new_string")
        
        if old_s and new_s:
            if self.apply_patch(old_s, new_s):
                print(f"🚀 Breakthrough: {llm.get('hypothesis')}")
                result = self.adapter.run_experiment(f"H: {llm.get('hypothesis')}\nT: {self.target}\nM: {self.criteria}")
                if result.get("status", "").upper() in ["KEEP", "SUCCESS"]:
                    print("✅ Evolution SUCCESS. Committing logic.")
                    try: subprocess.run(["git", "commit", "-am", f"Evolution: {llm.get('hypothesis')}"], capture_output=True)
                    except: pass
                else:
                    print("❌ Rejected. Reverting.")
                    self.revert_patch()
        else:
            print("⚠️ Incomplete patch proposed by brain.")
            
        return {"status": "continue"}

def main():
    goal_path = Path(".ouroboros/goal.yaml")
    with open(goal_path, 'r') as f: config = yaml.safe_load(f)
    loop = SmartAutonomousLoop(
        objective=config['description'], criteria=config['success_criteria'],
        target="sovereign_shell_hud.wgsl", metric_name="loop_latency",
        delay_seconds=15.0
    )
    print("🚀 Launching v3.1 Obsidian Vision Loop...")
    try: loop.run()
    except KeyboardInterrupt: print("\n🛑 Stopped.")
    finally: loop.lm_client.close()

if __name__ == "__main__": main()
