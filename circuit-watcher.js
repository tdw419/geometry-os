#!/usr/bin/env node
/**
 * Circuit Watcher - Advanced hot-reload for ASCII circuits
 * 
 * Watches circuits/ directory and auto-injects on file change.
 * Supports multiple circuits at different positions via filename convention:
 *   - circuit@100,50.txt → injects at (100, 50)
 *   - circuit.txt → injects at default position
 */

const fs = require('fs');
const path = require('path');
const { exec } = require('child_process');

const CIRCUITS_DIR = process.argv[2] || 'circuits/ascii';
const DEFAULT_X = parseInt(process.argv[3]) || 100;
const DEFAULT_Y = parseInt(process.argv[4]) || 100;
const SCANNER_BIN = './target/release/scanner';

// Circuit position map (filename → {x, y})
const positionMap = new Map();

// Load position map from circuits/positions.json if exists
const positionsFile = path.join(CIRCUITS_DIR, 'positions.json');
if (fs.existsSync(positionsFile)) {
    try {
        const positions = JSON.parse(fs.readFileSync(positionsFile, 'utf8'));
        Object.entries(positions).forEach(([name, pos]) => {
            positionMap.set(name, pos);
        });
        console.log(`Loaded ${positionMap.size} circuit positions from positions.json`);
    } catch (e) {
        console.warn('Warning: Could not parse positions.json:', e.message);
    }
}

/**
 * Parse filename to get injection position
 * Examples:
 *   half-adder@100,50.txt → {x: 100, y: 50}
 *   my-circuit.txt → {x: DEFAULT_X, y: DEFAULT_Y}
 */
function parsePosition(filename) {
    // Check position map first
    const baseName = filename.replace('.txt', '');
    if (positionMap.has(baseName)) {
        return positionMap.get(baseName);
    }
    
    // Parse @x,y suffix
    const match = filename.match(/@(\d+),(\d+)\.txt$/);
    if (match) {
        return {
            x: parseInt(match[1]),
            y: parseInt(match[2])
        };
    }
    
    // Default position
    return { x: DEFAULT_X, y: DEFAULT_Y };
}

/**
 * Inject circuit into GPU
 */
function injectCircuit(filepath) {
    const filename = path.basename(filepath);
    const { x, y } = parsePosition(filename);
    
    console.log(`\n${new Date().toLocaleTimeString()} - Injecting: ${filename}`);
    console.log(`  Position: (${x}, ${y})`);
    
    const cmd = `${SCANNER_BIN} load -f "${filepath}" -x ${x} -y ${y}`;
    
    exec(cmd, (error, stdout, stderr) => {
        if (error) {
            console.error(`  ✗ Error: ${error.message}`);
            return;
        }
        
        if (stderr) {
            console.error(`  ⚠ ${stderr}`);
        }
        
        // Parse output for pixel count
        const match = stdout.match(/Loaded (\d+) pixels/);
        if (match) {
            console.log(`  ✓ Injected ${match[1]} pixels`);
        } else {
            console.log(`  ✓ Injected`);
        }
    });
}

/**
 * Initial load of all circuits
 */
function loadAllCircuits() {
    console.log('\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━');
    console.log('Initial Load');
    console.log('━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n');
    
    const files = fs.readdirSync(CIRCUITS_DIR)
        .filter(f => f.endsWith('.txt') && f !== 'positions.json');
    
    files.forEach(file => {
        injectCircuit(path.join(CIRCUITS_DIR, file));
    });
}

/**
 * Watch for file changes
 */
function startWatcher() {
    console.log('\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━');
    console.log('Watching for changes...');
    console.log('━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n');
    console.log('Edit any .txt file and it will auto-inject into GPU');
    console.log('Press Ctrl+C to stop\n');
    
    let watchTimeout;
    
    fs.watch(CIRCUITS_DIR, (eventType, filename) => {
        if (!filename || !filename.endsWith('.txt')) return;
        
        // Debounce (wait for write to complete)
        clearTimeout(watchTimeout);
        watchTimeout = setTimeout(() => {
            const filepath = path.join(CIRCUITS_DIR, filename);
            
            if (fs.existsSync(filepath)) {
                injectCircuit(filepath);
            }
        }, 100);
    });
}

// Main
console.log('╔═══════════════════════════════════════════════════════════╗');
console.log('║         CIRCUIT WATCHER - Hot Reload (Node.js)           ║');
console.log('╚═══════════════════════════════════════════════════════════╝');
console.log(`\nWatch Directory: ${path.resolve(CIRCUITS_DIR)}`);
console.log(`Default Position: (${DEFAULT_X}, ${DEFAULT_Y})`);

// Check if scanner binary exists
if (!fs.existsSync(SCANNER_BIN)) {
    console.error('\n✗ Error: Scanner binary not found');
    console.error('  Run: cargo build --release --bin scanner');
    process.exit(1);
}

// Check if GPU agent is running
if (!fs.existsSync('/tmp/pixel-universe.mem')) {
    console.warn('\n⚠ Warning: GPU agent not running (/tmp/pixel-universe.mem not found)');
    console.warn('  Circuits will be loaded when agent starts');
}

// Load all circuits initially
loadAllCircuits();

// Start watching
startWatcher();
