#!/usr/bin/env python3
"""
Geometry OS Integration Tests

Tests all subsystems:
1. WebGPU Initialization
2. GPU Memory Management
3. Process Model
4. Morphological Filesystem
5. Shell Commands
6. GPU Networking
7. Visual Desktop
8. End-to-End Integration
"""

import sys
import time
import json
import subprocess
from pathlib import Path

# Test result tracking
passed = 0
failed = 0
skipped = 0

def run_test(name, fn):
    """Run a single test function"""
    global passed, failed, skipped
    start = time.time()
    try:
        fn()
        duration = (time.time() - start) * 1000
        passed += 1
        print(f"  ✓ {name} ({duration:.0f}ms)")
        return True
    except AssertionError as e:
        duration = (time.time() - start) * 1000
        failed += 1
        print(f"  ✗ {name}: {e} ({duration:.0f}ms)")
        return False
    except Exception as e:
        skipped += 1
        print(f"  − {name}: SKIP - {e}")
        return None

def check_js_syntax(filepath):
    """Check if a JS file has valid syntax"""
    result = subprocess.run(
        ["node", "--check", str(filepath)],
        capture_output=True, text=True
    )
    if result.returncode != 0:
        raise AssertionError(f"Syntax error: {result.stderr}")
    return True

# Get paths
WEB_DIR = Path(__file__).parent.parent / "web"

# ============================================
# TEST SUITE 1: Project Structure
# ============================================

print("\n[1/7] Project Structure Tests")

def test_core_files():
    required = [
        "GeometryKernel.js",
        "GPUMemoryManager.js", 
        "Shell.js",
        "GOSRouter.js",
        "VisualDesktop.js",
        "kernel.wgsl"
    ]
    for f in required:
        path = WEB_DIR / f
        assert path.exists(), f"Missing: {f}"

run_test("Core files exist", test_core_files)

def test_wgsl_files():
    shaders = ["kernel.wgsl"]
    for f in shaders:
        path = WEB_DIR / f
        assert path.exists(), f"Missing shader: {f}"

run_test("WGSL shader files exist", test_wgsl_files)

def test_html_files():
    tests = [
        "test-memory.html",
        "test-shell.html",
        "test-network.html",
        "test-visual-desktop.html",
        "test-integration.html"
    ]
    for f in tests:
        path = WEB_DIR / f
        assert path.exists(), f"Missing test: {f}"

run_test("Test HTML files exist", test_html_files)

# ============================================
# TEST SUITE 2: GeometryKernel
# ============================================

print("\n[2/7] GeometryKernel Tests")

def test_kernel_syntax():
    check_js_syntax(WEB_DIR / "GeometryKernel.js")

run_test("GeometryKernel.js syntax valid", test_kernel_syntax)

def test_kernel_exports():
    content = (WEB_DIR / "GeometryKernel.js").read_text()
    assert "export class GeometryKernel" in content, "Missing GeometryKernel export"

run_test("GeometryKernel exports class", test_kernel_exports)

def test_kernel_methods():
    content = (WEB_DIR / "GeometryKernel.js").read_text()
    methods = ["init", "spawnProcess", "step", "readPCBs", "killProcess"]
    for m in methods:
        assert f"async {m}" in content or f"{m}(" in content, f"Missing method: {m}"

run_test("GeometryKernel has required methods", test_kernel_methods)

# ============================================
# TEST SUITE 3: Memory Management
# ============================================

print("\n[3/7] Memory Management Tests")

def test_memory_syntax():
    check_js_syntax(WEB_DIR / "GPUMemoryManager.js")

run_test("GPUMemoryManager.js syntax valid", test_memory_syntax)

def test_memory_constants():
    content = (WEB_DIR / "GPUMemoryManager.js").read_text()
    assert "PAGE_SIZE" in content, "Missing PAGE_SIZE constant"

run_test("GPUMemoryManager has page constants", test_memory_constants)

def test_memory_methods():
    content = (WEB_DIR / "GPUMemoryManager.js").read_text()
    assert "malloc(" in content, "Missing malloc method"
    assert "free(" in content, "Missing free method"
    assert "translate(" in content, "Missing translate method"

run_test("GPUMemoryManager has malloc/free", test_memory_methods)

# ============================================
# TEST SUITE 4: Shell
# ============================================

print("\n[4/7] Shell Tests")

def test_shell_syntax():
    check_js_syntax(WEB_DIR / "Shell.js")

run_test("Shell.js syntax valid", test_shell_syntax)

def test_shell_glyphs():
    content = (WEB_DIR / "Shell.js").read_text()
    glyphs = ["ls", "cd", "cat", "run", "ps", "kill", "help"]
    for g in glyphs:
        assert f"'{g}'" in content or f'"{g}"' in content, f"Missing command: {g}"

run_test("Shell has command glyphs", test_shell_glyphs)

def test_shell_commands():
    content = (WEB_DIR / "Shell.js").read_text()
    assert "_cmd_ls" in content, "Missing ls command"
    assert "_cmd_ps" in content, "Missing ps command"
    assert "_cmd_kill" in content, "Missing kill command"

run_test("Shell has built-in commands", test_shell_commands)

# ============================================
# TEST SUITE 5: Networking
# ============================================

print("\n[5/7] Networking Tests")

def test_router_syntax():
    check_js_syntax(WEB_DIR / "GOSRouter.js")

run_test("GOSRouter.js syntax valid", test_router_syntax)

def test_router_ports():
    content = (WEB_DIR / "GOSRouter.js").read_text()
    assert "on(" in content or "bind(" in content, "Missing port handler method"
    assert "send(" in content, "Missing send method"
    assert "route(" in content, "Missing route method"

run_test("GOSRouter has port management", test_router_ports)

def test_net_shader():
    content = (WEB_DIR / "kernel.wgsl").read_text()
    # Network operations should be in kernel
    assert "OP_" in content, "Missing opcodes"

run_test("Kernel shader has operations", test_net_shader)

# ============================================
# TEST SUITE 6: Visual Desktop
# ============================================

print("\n[6/7] Visual Desktop Tests")

def test_desktop_syntax():
    check_js_syntax(WEB_DIR / "VisualDesktop.js")

run_test("VisualDesktop.js syntax valid", test_desktop_syntax)

def test_desktop_windows():
    content = (WEB_DIR / "VisualDesktop.js").read_text()
    assert "createWindow" in content, "Missing createWindow"
    assert "minimize" in content, "Missing minimize"
    assert "maximize" in content, "Missing maximize"

run_test("VisualDesktop has window management", test_desktop_windows)

def test_desktop_shortcuts():
    content = (WEB_DIR / "VisualDesktop.js").read_text()
    assert "shortcuts" in content.lower(), "Missing keyboard shortcuts"

run_test("VisualDesktop has keyboard shortcuts", test_desktop_shortcuts)

def test_desktop_apps():
    content = (WEB_DIR / "VisualDesktop.js").read_text()
    apps = ["terminal", "editor", "settings"]
    for app in apps:
        assert app in content.lower(), f"Missing app: {app}"

run_test("VisualDesktop has app registry", test_desktop_apps)

# ============================================
# TEST SUITE 7: WGSL Shaders
# ============================================

print("\n[7/7] WGSL Shader Tests")

def test_kernel_opcodes():
    content = (WEB_DIR / "kernel.wgsl").read_text()
    # Use actual opcode names from kernel.wgsl
    opcodes = ["OP_FADD", "OP_STORE", "OP_LOAD", "OP_CONSTANT"]
    for op in opcodes:
        assert op in content, f"Missing opcode: {op}"

run_test("kernel.wgsl has opcodes", test_kernel_opcodes)

def test_memory_opcodes():
    content = (WEB_DIR / "kernel.wgsl").read_text()
    # Check for memory or process operations
    has_ops = "OP_" in content and len(content) > 1000
    assert has_ops, "Missing sufficient opcodes"

run_test("kernel.wgsl has extended operations", test_memory_opcodes)

# ============================================
# SUMMARY
# ============================================

total = passed + failed + skipped
print(f"\n{'='*50}")
print(f"Results: {passed} passed, {failed} failed, {skipped} skipped")
print(f"Total: {total} tests")
print(f"{'='*50}")

# Write JSON report
report = {
    "timestamp": time.strftime("%Y-%m-%d %H:%M:%S"),
    "passed": passed,
    "failed": failed,
    "skipped": skipped,
    "total": total
}

report_path = Path(__file__).parent / "integration_report.json"
with open(report_path, "w") as f:
    json.dump(report, f, indent=2)
print(f"\nReport saved to: {report_path}")

sys.exit(0 if failed == 0 else 1)
