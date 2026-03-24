#!/bin/bash
# Heat-Map Demo - Show live signal flow visualization

cd ~/zion/projects/ascii_world/gpu

echo "╔═══════════════════════════════════════════════════════════╗"
echo "║           CIRCUIT HEAT-MAP - Live Signal Flow             ║"
echo "╚═══════════════════════════════════════════════════════════╝"
echo ""

# Check if GPU agent is running
if [ ! -f /tmp/pixel-universe.mem ]; then
    echo "⚠ Warning: GPU agent not running"
    echo "  Start it first: cargo run --release --bin agent"
    echo ""
    echo "  Running in simulation mode (no real GPU data)"
fi

# Demo 1: Half-adder with full color
echo "Demo 1: Half-Adder (full color mode)"
echo "Press Ctrl+C to exit, then run next demo"
echo ""
./target/release/heatmap -f circuits/ascii/half-adder.txt --offset-x 100 --offset-y 50 --color full --interval 100

# Note: This will run continuously until Ctrl+C
# To test different modes, run separately:

# Full color (default):
# ./target/release/heatmap -f circuits/ascii/half-adder.txt --offset-x 100 --offset-y 50

# Monochrome:
# ./target/release/heatmap -f circuits/ascii/half-adder.txt --offset-x 100 --offset-y 50 --color mono

# ASCII only (for terminals without color):
# ./target/release/heatmap -f circuits/ascii/half-adder.txt --offset-x 100 --offset-y 50 --color ascii
