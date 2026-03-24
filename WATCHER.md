# Circuit Watcher - Hot Reload

Edit ASCII circuits in your text editor and see them update live on the GPU.

## Quick Start

```bash
# Terminal 1: Start GPU agent
cd ~/zion/projects/ascii_world/gpu
cargo run --release --bin agent

# Terminal 2: Start watcher
./circuit-watcher.js circuits/ascii 100 100

# Terminal 3: Edit circuits
vim circuits/ascii/half-adder.txt
# Save and watch it auto-inject!
```

## Two Versions

### Shell Version (simple)

```bash
./circuit-watcher.sh [watch-dir] [offset-x] [offset-y]
```

**Requires:** `inotify-tools` (install: `sudo apt install inotify-tools`)

**Limitations:**
- Single position for all circuits
- Only watches .txt files

### Node.js Version (advanced)

```bash
./circuit-watcher.js [watch-dir] [default-x] [default-y]
```

**Features:**
- Multiple circuit positions via filename or positions.json
- Debounced file watching
- Initial load of all circuits
- Status messages

## Position Mapping

### Method 1: Filename Convention

Include position in filename:
```
circuits/ascii/half-adder@100,50.txt    → injects at (100, 50)
circuits/ascii/clock@300,100.txt        → injects at (300, 100)
```

### Method 2: positions.json

Create `circuits/ascii/positions.json`:
```json
{
  "half-adder": { "x": 100, "y": 50 },
  "replicator-field": { "x": 200, "y": 100 },
  "clock": { "x": 300, "y": 50 },
  "memory-cell": { "x": 400, "y": 100 }
}
```

Then name files without position suffix:
```
circuits/ascii/half-adder.txt
circuits/ascii/replicator-field.txt
```

### Method 3: Default Position

Files without position suffix or positions.json entry use default position:
```bash
./circuit-watcher.js circuits/ascii 100 100
# All unmapped files inject at (100, 100)
```

## Workflow

### Design → Test → Iterate

```bash
# 1. Start watcher
./circuit-watcher.js circuits/ascii 100 100

# 2. Create new circuit
vim circuits/ascii/my-circuit.txt

# 3. Type your circuit
--&--
   |
--X--

# 4. Save - auto-injects into GPU!

# 5. Watch it run
./target/release/scanner watch -x 100 -y 100 --width 20 --height 10

# 6. Iterate - edit, save, repeat
```

### Multi-Circuit Layout

```bash
# Create position map
cat > circuits/ascii/positions.json << 'EOF'
{
  "input-stage": { "x": 50, "y": 50 },
  "processing": { "x": 150, "y": 50 },
  "output-stage": { "x": 250, "y": 50 }
}
EOF

# Create circuits
vim circuits/ascii/input-stage.txt
vim circuits/ascii/processing.txt
vim circuits/ascii/output-stage.txt

# Start watcher - all circuits load at their positions
./circuit-watcher.js
```

## Integration with VS Code

### Workspace Setup

1. Open `circuits/ascii/` folder in VS Code
2. Install "File Watcher" extension (optional)
3. Configure auto-save:

```json
// .vscode/settings.json
{
  "files.autoSave": "afterDelay",
  "files.autoSaveDelay": 500
}
```

4. Edit circuits with auto-inject on save!

### Live Preview

Keep a terminal open with watch mode:
```bash
# Terminal 1: Watcher (auto-injects)
./circuit-watcher.js

# Terminal 2: Live preview
watch -n 0.5 './target/release/scanner scan -x 0 -y 0 --width 80 --height 30'
```

## Tips

### Debug Slow Updates

If circuits don't update immediately:
1. Check GPU agent is running: `ps aux | grep agent`
2. Check shared memory exists: `ls -l /tmp/pixel-universe.mem`
3. Try manual inject: `./target/release/scanner load -f circuit.txt`

### Multiple Circuits Overlap

If circuits collide:
1. Use positions.json to space them out
2. Scan the region first to see what's there
3. Use larger offsets

### Circuit Too Large

If circuit extends beyond screen:
1. Keep circuits < 40x20 for 480x240 resolution
2. Or run agent at higher resolution
3. Or split into multiple sub-circuits

## Advanced: Build Pipeline

Create a build script that:
1. Scans reference circuits
2. Optimizes layout
3. Generates positions.json
4. Loads into GPU

```bash
#!/bin/bash
# build-circuits.sh

echo "Building circuit layout..."

# Scan existing GPU state
./target/release/scanner scan -x 0 -y 0 --width 480 --height 240 -o backup.txt

# Generate positions
node generate-layout.js circuits/ascii/ > circuits/ascii/positions.json

# Load all circuits
./circuit-watcher.js --initial-only

echo "✓ Circuit layout complete"
```

## Troubleshooting

**"Scanner binary not found"**
```bash
cargo build --release --bin scanner
```

**"GPU agent not running"**
```bash
cargo run --release --bin agent
```

**"Position already occupied"**
```bash
# Scan region first
./target/release/scanner scan -x 100 -y 100 --width 20 --height 15

# Use different position
./circuit-watcher.js circuits/ascii 200 200
```

**Circuits not updating**
1. Check watcher is running
2. Check file is in correct directory
3. Check file extension is .txt
4. Try manual load: `./target/release/scanner load -f circuit.txt`

## Example Session

```bash
$ ./circuit-watcher.js

╔═══════════════════════════════════════════════════════════╗
║         CIRCUIT WATCHER - Hot Reload (Node.js)           ║
╚═══════════════════════════════════════════════════════════╝

Watch Directory: /home/jericho/zion/projects/ascii_world/gpu/circuits/ascii
Default Position: (100, 100)

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Initial Load
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

15:32:01 - Injecting: half-adder.txt
  Position: (100, 50)
  ✓ Injected 14 pixels

15:32:01 - Injecting: replicator-field.txt
  Position: (200, 100)
  ✓ Injected 34 pixels

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Watching for changes...
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Edit any .txt file and it will auto-inject into GPU
Press Ctrl+C to stop

15:33:45 - Injecting: half-adder.txt
  Position: (100, 50)
  ✓ Injected 16 pixels

15:34:12 - Injecting: my-new-circuit.txt
  Position: (100, 100)
  ✓ Injected 23 pixels

^C
```

## Files

```
gpu/
├── circuit-watcher.sh     — Shell version (simple)
├── circuit-watcher.js     — Node.js version (advanced)
├── circuits/ascii/
│   ├── positions.json     — Position map
│   ├── *.txt              — Circuit files
│   └── ...
└── target/release/
    └── scanner            — Scanner binary
```

## See Also

- `SCANNER.md` — Scanner tool documentation
- `INJECTOR.md` — Manual signal injection
- `CIRCUITS.md` — Circuit templates
