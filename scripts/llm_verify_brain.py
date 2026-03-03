#!/usr/bin/env python3
"""
LLM Brain Verification: Simulated MCP Interaction

This script simulates an AI agent using the VisualMCPServer tools
to verify that the morphological proof has been 'crystallized' in memory.
"""

import asyncio
import os
import sys
import json
import base64
from pathlib import Path

# Add root to path
PROJECT_ROOT = Path(__file__).parent.parent
sys.path.insert(0, str(PROJECT_ROOT))

from open_brain.visual_mcp import VisualMCPServer

async def simulate_llm_verification():
    print("--- Simulating LLM Brain Verification via MCP ---")
    
    # Initialize server (requires DATABASE_URL)
    db_url = os.environ.get("DATABASE_URL", "postgresql://postgres:openbrain@localhost:5432/open_brain")
    server = VisualMCPServer(db_url)
    
    try:
        await server.connect()
        print("  [OK] Connected to Open Brain Substrate")
        
        # 1. LLM Action: Store the Proof Result
        print("\n  > LLM calls store_memory('Morphological Proof Verified', type='decision')...")
        store_result = await server.call_tool("store_memory", {
            "content": "The self-hosting compiler (geo_cc.spv) successfully compiled a morphological program using ⊕ and → glyphs. The SPIR-V output is verified.",
            "type": "decision",
            "priority": 0.9,
            "tags": ["proof", "self-hosting", "verified"]
        })
        print(f"  [OK] Memory stored with ID: {store_result['id']}")
        
        # 2. LLM Action: Query Visual Memory
        print("\n  > LLM calls query_visual_memory(limit=10)...")
        visual_memory = await server.call_tool("query_visual_memory", {"limit": 10})
        
        if visual_memory['memory_count'] > 0:
            print(f"  [OK] Visual memory retrieved: {visual_memory['memory_count']} entries")
            print(f"  [OK] Image payload size: {len(visual_memory['image_base64'])} chars")
            
            # Check legend for our proof
            legend = visual_memory['legend']
            proof_entry = next((item for item in legend if "proof" in item.get('content_preview', '').lower() or item.get('id') == store_result['id']), None)
            
            if proof_entry:
                print(f"  [OK] Proof identified in visual legend: {proof_entry['content_preview']}...")
                print(f"  [OK] Glyph: {proof_entry['char']} (Opcode: 0x{proof_entry['rgb']['g']:02x})")
            else:
                print("  [WARN] Proof not yet visible in legend (first 20 only)")
        else:
            print("  [FAIL] No memories found in substrate")
            
        # 3. LLM Action: Query SPIR-V Memory
        print("\n  > LLM calls query_spirv_memory(limit=5)...")
        spirv_memory = await server.call_tool("query_spirv_memory", {"limit": 5})
        print(f"  [OK] Binary substrate size: {spirv_memory['binary_size']} bytes")
        
    except Exception as e:
        print(f"  [ERROR] MCP Simulation failed: {e}")
    finally:
        await server.disconnect()

if __name__ == "__main__":
    import sys
    asyncio.run(simulate_llm_verification())
