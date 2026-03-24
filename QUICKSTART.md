# Geometry OS — 30-Second Quick Start

## 1. Build

```bash
./build.sh
```

## 2. Run (Simulation)

```bash
./target/release/agent
```

Opens PNG frames in `output/` directory.

## 3. Run (Live Framebuffer)

```bash
sudo ./target/release/agent
```

Switch to TTY (Ctrl+Alt+F2) to see live output.

## 4. Desktop Siphon

```bash
sudo ./target/release/siphon-demo
```

Watch your desktop pixels become agent data.

## What You'll See

- **Foundry (left):** Dense pixel activity
- **Architect (right):** Large CPU blocks
- **Portal wire (y=15):** Signal crossing zones

## Next Steps

- Read `README.md` for full documentation
- Check `circuits/ascii/` for example circuits
- See `ALU.md` for ALU design

---

*That's it. You're running a pixel-native OS on your GPU.*
