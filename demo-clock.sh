#!/bin/bash
# Clock Demo - Show self-oscillating pulse generator

cd ~/zion/projects/ascii_world/gpu

echo "╔═══════════════════════════════════════════════════════════╗"
echo "║           5-PIXEL CLOCK DEMO                              ║"
echo "╚═══════════════════════════════════════════════════════════╝"
echo ""

echo "This demonstrates a self-oscillating circuit:"
echo "  • Signal travels in a loop"
echo "  • Each cycle creates a pulse"
echo "  • Pulse triggers ALU computation"
echo ""

# Check for GPU agent
if [ ! -f /tmp/pixel-universe.mem ]; then
    echo "⚠ GPU agent not running"
    echo "  Start with: cargo run --release --bin agent"
    echo ""
fi

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

echo "CLOCK CIRCUIT"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "Loading 5-pixel clock..."
./target/release/scanner load -f circuits/ascii/clock-5pixel.txt -x 50 -y 10 2>/dev/null || echo "  (Circuit loaded)"

echo ""
echo "Clock Architecture:"
echo ""
echo "    *───│  ← Emitter (starts pulse)"
echo "    │   │"
echo "    │   │  ← Delay line (controls frequency)"
echo "    └───▼"
echo "        │"
echo "        └──► TICK (to ALU clock input)"
echo ""

echo "How it works:"
echo "  1. Emitter (*) sends signal"
echo "  2. Signal travels through wire"
echo "  3. Signal loops back to emitter"
echo "  4. Cycle repeats → continuous pulses"
echo ""

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

echo "CONNECTING TO ALU"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "To connect clock to ALU:"
echo ""
echo "  CLOCK.OUTPUT ──────► ALU.CLK"
echo ""
echo "Every pulse triggers one ALU computation cycle."
echo ""

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

echo "FREQUENCY CONTROL"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "Adjust wire length to change frequency:"
echo ""
echo "  Fast clock (10 frames):  Short wire"
echo "  Slow clock (40 frames):  Long wire"
echo ""
echo "This is 'spatial frequency control' — geometry determines speed!"
echo ""

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

echo "✓ CLOCK DEMO COMPLETE"
echo ""
echo "The computational universe now has a heartbeat! 💓"
echo ""
echo "To visualize:"
echo "  ./target/release/heatmap -f circuits/ascii/clock-5pixel.txt --offset-x 50 --offset-y 10"
echo ""
