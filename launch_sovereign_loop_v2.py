#!/usr/bin/env python3
"""
Sovereign Shell Autonomous Loop v2.5 - Stable Triple-Brain 🐍🧠🛡️✨

Hierarchy:
1. ZAI (Primary) - GLM-5 with Thinking
2. Gemini (Secondary) - Gemini 1.5 Pro
3. LM Studio (Fallback) - Qwen 2.5 Coder
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
from datetime import datetime

# Add Ouroboros to path
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
        
        super().__init__(**kwargs)
        self.adapter.config.base_url = self.PXOS_URL
        self.adapter.client = httpx.Client(base_url=self.PXOS_URL, timeout=30.0)
        self.lm_client = httpx.Client(timeout=180.0)
        
    def _read_target_content(self) -> str:
        target_path = Path(self.target)
        if target_path.exists():
            return target_path.read_text()
        return ""

    def generate_hypothesis(self) -> dict:
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

        # 1. ZAI Primary
        zai_key = os.environ.get("ZAI_API_KEY")
        zai_url = os.environ.get("ZAI_BASE_URL", "https://api.z.ai/api/coding/paas/v4")
        if zai_key:
            print("\n[1] 🧠 Generating patch using ZAI (Primary: GLM-5)...")
            try:
                resp = self.lm_client.post(f"{zai_url}/chat/completions", headers={
                    "Authorization": f"Bearer {zai_key}",
                    "Content-Type": "application/json"
                }, json={
                    "model": "glm-5",
                    "messages": [
                        {"role": "system", "content": system_prompt},
                        {"role": "user", "content": user_prompt}
                    ],
                    "thinking": { "type": "enabled" },
                    "response_format": { "type": "json_object" },
                    "temperature": 0.2,
                    "max_tokens": 4096,
                    "stream": False
                }, timeout=180.0)
                
                if resp.status_code == 200:
                    data = resp.json()
                    content_resp = data["choices"][0]["message"]["content"]
                    # If thinking is returned in content, try to strip it
                    if "<thought>" in content_resp:
                        content_resp = content_resp.split("</thought>")[-1]
                    
                    if "```json" in content_resp:
                        content_resp = content_resp.split("```json")[1].split("```")[0]
                    elif "```" in content_resp:
                        content_resp = content_resp.split("```")[1].split("```")[0]
                    return json.loads(content_resp.strip())
                else:
                    print(f"⚠️ ZAI Failed ({resp.status_code}). Trying Gemini...")
            except Exception as e:
                print(f"⚠️ ZAI Exception: {e}. Trying Gemini...")

        # 2. Gemini Secondary
        gemini_key = os.environ.get("GEMINI_API_KEY")
        if gemini_key:
            print("\n[2] 🧠 Generating patch using Gemini (Secondary: 1.5 Pro)...")
            try:
                # Use stable Gemini 1.5 Pro to avoid 404s
                url = f"https://generativelanguage.googleapis.com/v1beta/models/gemini-1.5-pro:generateContent?key={gemini_key}"
                resp = self.lm_client.post(url, json={
                    "systemInstruction": {"parts": [{"text": system_prompt}]},
                    "contents": [{"parts": [{"text": user_prompt}]}],
                    "generationConfig": {
                        "temperature": 0.2,
                        "maxOutputTokens": 4096,
                        "responseMimeType": "application/json"
                    }
                }, timeout=180.0)
                
                if resp.status_code == 200:
                    data = resp.json()
                    content_resp = data["candidates"][0]["content"]["parts"][0]["text"]
                    return json.loads(content_resp.strip())
                else:
                    print(f"⚠️ Gemini Failed ({resp.status_code}). Trying LM Studio...")
                    # print(resp.text)
            except Exception as e:
                print(f"⚠️ Gemini Exception: {e}. Trying LM Studio...")

        # 3. LM Studio Fallback
        print(f"\n[3] 🧠 Generating patch using LM Studio ({self.MODEL})...")
        try:
            resp = self.lm_client.post(self.LM_STUDIO_URL, json={
                "model": self.MODEL,
                "messages": [
                    {"role": "system", "content": system_prompt},
                    {"role": "user", "content": user_prompt}
                ],
                "temperature": 0.2,
                "max_tokens": 2048
            })
            if resp.status_code == 200:
                data = resp.json()
                content_resp = data["choices"][0]["message"]["content"]
                if "```json" in content_resp:
                    content_resp = content_resp.split("```json")[1].split("```")[0]
                elif "```" in content_resp:
                    content_resp = content_resp.split("```")[1].split("```")[0]
                return json.loads(content_resp.strip())
            return {"error": f"LM Studio Error: {resp.status_code}"}
        except Exception as e:
            return {"error": str(e)}

    def apply_patch(self, old_str: str, new_str: str):
        target_path = Path(self.target)
        content = target_path.read_text()
        if old_str not in content:
            print(f"❌ Error: old_string not found in {self.target}. Match failed.")
            return False

        version = 1
        while target_path.with_suffix(f".v{version}.bak").exists():
            version += 1
        backup_path = target_path.with_suffix(f".v{version}.bak")
        target_path.rename(backup_path)
        
        print(f"💾 Created backup: {backup_path.name}")
        new_content = content.replace(old_str, new_str, 1)
        target_path.write_text(new_content)
        return True

    def revert_patch(self):
        target_path = Path(self.target)
        backups = sorted(target_path.parent.glob(f"{target_path.name}.v*.bak"))
        if backups:
            latest_bak = backups[-1]
            print(f"⏪ Reverting to {latest_bak.name}...")
            if target_path.exists(): target_path.unlink()
            latest_bak.rename(target_path)

    def check_criteria(self, current: float) -> bool:
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
        delay_seconds=15.0
    )

    print(f"🚀 Launching v2.5 Stable Triple-Brain Ouroboros Loop...")
    try: loop.run()
    except KeyboardInterrupt: print("\n🛑 Stopped.")
    finally: loop.lm_client.close()

if __name__ == "__main__": main()
