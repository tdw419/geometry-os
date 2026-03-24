#!/bin/bash
# Circuit Scanner Demo - Show bidirectional ASCII ↔ GPU bridge

cd ~/zion/projects/ascii_world/gpu

echo "╔═══════════════════════════════════════════════════════════╗"
echo "║         CIRCUIT SCANNER - Bidirectional Bridge           ║"
echo "║         ASCII ↔ GPU Pixel Agents                         ║"
echo "╚═══════════════════════════════════════════════════════════╝"
echo ""

echo "1. Scanning existing GPU state..."
echo "   (Reading from /tmp/pixel-universe.mem)"
echo ""
./target/release/scanner scan -x 50 -y 30 --width 40 --height 15
echo ""

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

echo "2. Loading ASCII circuit..."
echo "   File: circuits/ascii/replicator-field.txt"
cat circuits/ascii/replicator-field.txt
echo ""
./target/release/scanner load -f circuits/ascii/replicator-field.txt -x 200 -y 100
echo ""

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

echo "3. Verifying load by scanning back..."
echo ""
./target/release/scanner scan -x 200 -y 100 --width 12 --height 10
echo ""

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

echo "✓ Bidirectional bridge working!"
echo ""
echo "What this enables:"
echo "  • Design circuits visually (ASCII art)"
echo "  • Load them into GPU for execution"
echo "  • Scan running circuits back to ASCII"
echo "  • Save, share, and version control circuits"
echo ""
echo "Try it yourself:"
echo "  ./target/release/scanner watch -x 200 -y 100 --width 30 --height 20"
