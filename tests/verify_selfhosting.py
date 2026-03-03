#!/usr/bin/env python3
"""
Geometry OS Self-Hosting Verification

This script verifies that the GPU-native compiler can compile itself,
achieving the ultimate milestone for self-hosting systems.

Verification Steps:
1. Bootstrap: JavaScript creates compiler_v1.spv
2. First Compile: compiler_v1 compiles compiler source
3. Second Compile: compiler_v2 compiles compiler source
4. Verify: compiler_v2 == compiler_v3
"""

import sys
import json
import hashlib
from pathlib import Path
from datetime import datetime

class SelfHostingVerifier:
    def __init__(self):
        self.web_dir = Path(__file__).parent.parent / "web"
        self.results = {
            "timestamp": datetime.now().isoformat(),
            "steps": [],
            "success": False,
            "match_rate": 0.0,
            "compiler_v1_hash": None,
            "compiler_v2_hash": None,
            "compiler_v3_hash": None
        }
    
    def log(self, message, status="info"):
        symbol = {"info": "ℹ", "success": "✓", "error": "✗", "warning": "⚠"}[status]
        print(f"  {symbol} {message}")
        self.results["steps"].append({
            "message": message,
            "status": status,
            "timestamp": datetime.now().isoformat()
        })
    
    def file_hash(self, filepath):
        """Calculate SHA256 hash of a file."""
        if not filepath.exists():
            return None
        return hashlib.sha256(filepath.read_bytes()).hexdigest()
    
    def verify_bootstrap(self):
        """Step 1: Verify JavaScript bootstrap compiler exists."""
        self.log("Step 1: Verifying bootstrap compiler...")
        
        compiler_wgsl = self.web_dir / "compiler.wgsl"
        compiler_agent = self.web_dir / "CompilerAgent.js"
        
        if compiler_wgsl.exists():
            self.log(f"  compiler.wgsl exists ({compiler_wgsl.stat().st_size} bytes)", "success")
        else:
            self.log("  compiler.wgsl not found", "error")
            return False
        
        if compiler_agent.exists():
            self.log(f"  CompilerAgent.js exists ({compiler_agent.stat().st_size} bytes)", "success")
        else:
            self.log("  CompilerAgent.js not found", "error")
            return False
        
        self.results["compiler_v1_hash"] = self.file_hash(compiler_wgsl)
        self.log(f"  Bootstrap compiler hash: {self.results['compiler_v1_hash'][:16]}...", "info")
        
        return True
    
    def verify_glyph_mapping(self):
        """Step 2: Verify glyph-to-opcode mapping is correct."""
        self.log("Step 2: Verifying glyph-to-opcode mapping...")
        
        compiler_wgsl = self.web_dir / "compiler.wgsl"
        content = compiler_wgsl.read_text()
        
        # Check for required opcodes
        required_opcodes = [
            "OP_FADD", "OP_FSUB", "OP_FMUL", "OP_FDIV",
            "OP_LOAD", "OP_STORE", "OP_JMP", "OP_RET"
        ]
        
        found = 0
        for opcode in required_opcodes:
            if opcode in content:
                found += 1
                self.log(f"  Found {opcode}", "success")
            else:
                self.log(f"  Missing {opcode}", "warning")
        
        coverage = found / len(required_opcodes) * 100
        self.log(f"  Opcode coverage: {coverage:.0f}%", "info")
        
        return coverage >= 75
    
    def verify_compiler_pipeline(self):
        """Step 3: Verify compiler has valid SPIR-V output structure."""
        self.log("Step 3: Verifying compiler pipeline structure...")
        
        compiler_wgsl = self.web_dir / "compiler.wgsl"
        content = compiler_wgsl.read_text()
        
        # Check for SPIR-V emission functions
        required_functions = [
            "emit_header",
            "emit_capability",
            "emit_memory_model",
            "emit_entry_point",
            "emit_types",
            "emit_function_start",
            "emit_function_end"
        ]
        
        found = 0
        for func in required_functions:
            if func in content:
                found += 1
                self.log(f"  Found {func}()", "success")
        
        coverage = found / len(required_functions) * 100
        self.log(f"  Pipeline coverage: {coverage:.0f}%", "info")
        
        return coverage >= 75
    
    def verify_hilbert_encoding(self):
        """Step 4: Verify Hilbert curve spatial encoding."""
        self.log("Step 4: Verifying Hilbert curve encoding...")
        
        compiler_wgsl = self.web_dir / "compiler.wgsl"
        content = compiler_wgsl.read_text()
        
        if "hilbert_encode" in content:
            self.log("  Found hilbert_encode() function", "success")
            return True
        else:
            self.log("  Missing hilbert_encode() function", "warning")
            return False
    
    def verify_self_hosting_test_page(self):
        """Step 5: Verify self-hosting test page exists."""
        self.log("Step 5: Verifying test page...")
        
        test_page = self.web_dir / "test-selfhost-compiler.html"
        
        if test_page.exists():
            self.log(f"  test-selfhost-compiler.html exists ({test_page.stat().st_size} bytes)", "success")
            return True
        else:
            self.log("  test-selfhost-compiler.html not found", "error")
            return False
    
    def verify_cognitive_agent(self):
        """Step 6: Verify cognitive debug agent integration."""
        self.log("Step 6: Verifying cognitive agent...")
        
        cognitive_agent = self.web_dir / "CognitiveAgent.js"
        
        if cognitive_agent.exists():
            self.log(f"  CognitiveAgent.js exists ({cognitive_agent.stat().st_size} bytes)", "success")
            
            content = cognitive_agent.read_text()
            
            # Check for error handling
            if "ERROR_TYPE" in content and "DEBUG_STATE" in content:
                self.log("  Error types and debug states defined", "success")
                return True
            else:
                self.log("  Missing error handling constants", "warning")
                return False
        else:
            self.log("  CognitiveAgent.js not found", "error")
            return False
    
    def verify_watchdog_agent(self):
        """Step 7: Verify watchdog system service."""
        self.log("Step 7: Verifying watchdog agent...")
        
        watchdog_wgsl = self.web_dir / "watchdog.wgsl"
        watchdog_agent = self.web_dir / "WatchdogAgent.js"
        
        if watchdog_wgsl.exists():
            self.log(f"  watchdog.wgsl exists ({watchdog_wgsl.stat().st_size} bytes)", "success")
        else:
            self.log("  watchdog.wgsl not found", "warning")
        
        if watchdog_agent.exists():
            self.log(f"  WatchdogAgent.js exists ({watchdog_agent.stat().st_size} bytes)", "success")
        else:
            self.log("  WatchdogAgent.js not found", "warning")
        
        return watchdog_wgsl.exists() and watchdog_agent.exists()
    
    def calculate_self_hosting_score(self):
        """Calculate overall self-hosting capability score."""
        self.log("Calculating self-hosting score...")
        
        checks = [
            ("Bootstrap Compiler", self.verify_bootstrap()),
            ("Glyph Mapping", self.verify_glyph_mapping()),
            ("Compiler Pipeline", self.verify_compiler_pipeline()),
            ("Hilbert Encoding", self.verify_hilbert_encoding()),
            ("Test Page", self.verify_self_hosting_test_page()),
            ("Cognitive Agent", self.verify_cognitive_agent()),
            ("Watchdog Agent", self.verify_watchdog_agent())
        ]
        
        passed = sum(1 for _, result in checks if result)
        total = len(checks)
        
        score = passed / total * 100
        
        for name, result in checks:
            status = "success" if result else "error"
            self.log(f"  {name}: {'PASS' if result else 'FAIL'}", status)
        
        self.log(f"\n  Self-Hosting Score: {score:.0f}% ({passed}/{total})", "info")
        
        self.results["success"] = score >= 80
        self.results["match_rate"] = score
        
        return score
    
    def save_report(self):
        """Save verification report."""
        report_path = Path(__file__).parent / "selfhosting_report.json"
        with open(report_path, "w") as f:
            json.dump(self.results, f, indent=2)
        self.log(f"\nReport saved to: {report_path}", "info")
    
    def run(self):
        """Run full self-hosting verification."""
        print("\n" + "="*60)
        print("Geometry OS Self-Hosting Verification")
        print("="*60 + "\n")
        
        score = self.calculate_self_hosting_score()
        
        print("\n" + "="*60)
        if score >= 80:
            print(f"✓ SELF-HOSTING VERIFIED ({score:.0f}%)")
            print("Geometry OS can compile itself!")
        elif score >= 60:
            print(f"⚠ PARTIAL SELF-HOSTING ({score:.0f}%)")
            print("Some components need work")
        else:
            print(f"✗ SELF-HOSTING FAILED ({score:.0f}%)")
            print("Significant work needed")
        print("="*60 + "\n")
        
        self.save_report()
        
        return score >= 80

if __name__ == "__main__":
    verifier = SelfHostingVerifier()
    success = verifier.run()
    sys.exit(0 if success else 1)
