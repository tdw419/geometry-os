#!/usr/bin/env python3
"""
Geometry OS Font Toolkit Installer

Sets up the font system:
1. Checks dependencies (numpy, Pillow, fonttools)
2. Generates font atlas
3. Generates TTF font file
4. Creates preview
"""

import os
import sys
import subprocess
from pathlib import Path

TOOLKIT_ROOT = Path(__file__).parent

def print_header():
    print("\033[96m" + "=" * 60)
    print("      GEOMETRY OS FONT TOOLKIT INSTALLER")
    print("=" * 60 + "\033[0m")

def check_dependencies():
    print("\n[1/4] Checking Dependencies...")
    deps = {
        "numpy": "Numerical processing",
        "PIL": "Image processing (Pillow)",
        "fontTools": "Vector font generation"
    }

    missing = []
    for dep, purpose in deps.items():
        try:
            if dep == "PIL":
                import PIL
            elif dep == "fontTools":
                import fontTools
            else:
                __import__(dep)
            print(f"  [OK] {dep:12} - Found")
        except ImportError:
            print(f"  [--] {dep:12} - MISSING ({purpose})")
            missing.append(dep)

    if missing:
        print("\nInstall missing dependencies? (y/n)")
        choice = input("> ").lower()
        if choice == 'y':
            pkg_map = {"PIL": "Pillow", "fontTools": "fonttools"}
            to_install = [pkg_map.get(m, m) for m in missing]
            print(f"Installing: {' '.join(to_install)}...")
            subprocess.check_call([sys.executable, "-m", "pip", "install"] + to_install)
        else:
            print("Cannot proceed without dependencies.")
            sys.exit(1)

def generate_atlas():
    print("\n[2/4] Generating Font Atlas...")
    atlas_script = TOOLKIT_ROOT / "core" / "atlas_gen.py"
    if atlas_script.exists():
        subprocess.check_call([sys.executable, str(atlas_script)])
        print("  [OK] Atlas generated")
    else:
        print(f"  [ERROR] {atlas_script} not found")
        sys.exit(1)

def generate_ttf():
    print("\n[3/4] Generating TTF Font (optional)...")
    ttf_script = TOOLKIT_ROOT / "core" / "ttf_export.py"
    if ttf_script.exists():
        try:
            subprocess.check_call([sys.executable, str(ttf_script), "GeometryOS-Regular.ttf"])
            print("  [OK] TTF generated: GeometryOS-Regular.ttf")
        except subprocess.CalledProcessError:
            print("  [WARN] TTF generation failed - continuing without it")
            print("       (Atlas is the primary output, TTF is optional)")
    else:
        print(f"  [SKIP] {ttf_script} not found")

def show_next_steps():
    print("\n[4/4] Installation Complete!")
    print("\033[92m" + "=" * 60)
    print("  Geometry OS Font Toolkit is ready!")
    print("=" * 60 + "\033[0m")

    print("\nNext Steps:")
    print("  1. Web Demo: cd web && python3 -m http.server 8770")
    print("  2. Open: http://localhost:8770/demo.html")
    print("  3. Use TTF: Install GeometryOS-Regular.ttf to your system")
    print("\nFor AI onboarding, share AI_ONBOARDING.md with your assistant.")

if __name__ == "__main__":
    print_header()
    check_dependencies()
    generate_atlas()
    generate_ttf()
    show_next_steps()
