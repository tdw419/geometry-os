#!/bin/bash
# Bus System Demo - Show connected circuits communicating via bus

cd ~/zion/projects/ascii_world/gpu

echo "╔═══════════════════════════════════════════════════════════╗"
echo "║           UNIVERSAL BUS SYSTEM DEMO                       ║"
echo "╚═══════════════════════════════════════════════════════════╝"
echo ""

echo "1. Checking for circuit collisions..."
echo ""
node circuit-check.js circuits/ascii
echo ""

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

echo "2. Loading bus system..."
echo ""
echo "The Universal Bus connects multiple circuits:"
echo ""
echo "  SLOT A          SLOT B          SLOT C          SLOT D"
echo "     │               │               │               │"
echo "     │               │               │               │"
echo "─────┼───────────────┼───────────────┼───────────────┼────"
echo "     │               │               │               │"
echo "     │               │               │               │"
echo ""
echo "Circuits can:"
echo "  • Read data from bus (tap)"
echo "  • Process data (logic gates)"
echo "  • Write results back to bus"
echo "  • Communicate with other circuits"
echo ""

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

echo "3. Bus Protocol:"
echo ""
echo "  BUS 0 (DATA)  — Primary data channel"
echo "  BUS 1 (ADDR)  — Circuit selection"
echo "  BUS 2 (CTRL)  — Control signals"
echo "  BUS 3 (CLK)   — Global clock"
echo ""
echo "Signal levels:"
echo "  0-50    = LOW (0)"
echo "  50-150  = MEDIUM (control)"
echo "  150-255 = HIGH (1)"
echo ""

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

echo "4. Example: 4-Bit Adder on Bus"
echo ""
echo "  Each bit circuit:"
echo "    - Reads inputs from bus"
echo "    - Performs 1-bit addition"
echo "    - Writes sum to bus"
echo "    - Passes carry to next bit"
echo ""
echo "  Position mapping:"
cat << 'EOF'
    {
      "bus-main": { "x": 0, "y": 100, "width": 480, "height": 4 },
      "bit0": { "x": 0, "y": 50, "width": 100, "height": 50 },
      "bit1": { "x": 120, "y": 50, "width": 100, "height": 50 },
      "bit2": { "x": 240, "y": 50, "width": 100, "height": 50 },
      "bit3": { "x": 360, "y": 50, "width": 100, "height": 50 }
    }
EOF
echo ""

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

echo "5. Network Bridge for Distributed Bus"
echo ""
echo "  Instance A (GPU 1)       Instance B (GPU 2)"
echo "  ┌────────────────┐       ┌────────────────┐"
echo "  │ BUS ───────────┼───────┼─────────────BUS│"
echo "  │ 0-3            │       │ 0-3            │"
echo "  └────────────────┘       └────────────────┘"
echo ""
echo "  Start bridge:"
echo "    Terminal 1: ./target/release/bridge server --port 7890 --offset-x 400"
echo "    Terminal 2: ./target/release/bridge connect --server other-host:7890"
echo ""

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

echo "✓ Universal Bus system ready!"
echo ""
echo "Next steps:"
echo "  • Design circuits that use the bus"
echo "  • Connect multiple circuits via bus taps"
echo "  • Monitor bus traffic with heat-map"
echo "  • Scale across multiple GPUs with bridge"
