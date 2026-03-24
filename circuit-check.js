#!/usr/bin/env node
/**
 * Circuit Collision Detector
 * 
 * Checks for overlapping circuits in positions.json and suggests safe placements.
 * Prevents opcode corruption when multiple circuits occupy the same pixels.
 */

const fs = require('fs');
const path = require('path');

const CIRCUITS_DIR = process.argv[2] || 'circuits/ascii';
const POSITIONS_FILE = path.join(CIRCUITS_DIR, 'positions.json');

// ANSI colors
const RED = '\x1B[31m';
const GREEN = '\x1B[32m';
const YELLOW = '\x1B[33m';
const CYAN = '\x1B[36m';
const RESET = '\x1B[0m';
const BOLD = '\x1B[1m';

/**
 * Load positions map
 */
function loadPositions() {
    if (!fs.existsSync(POSITIONS_FILE)) {
        return new Map();
    }
    
    try {
        const data = JSON.parse(fs.readFileSync(POSITIONS_FILE, 'utf8'));
        return new Map(Object.entries(data).map(([name, pos]) => [name, {
            x: pos.x,
            y: pos.y,
            width: pos.width || 0,
            height: pos.height || 0
        }]));
    } catch (e) {
        console.error(`${RED}Error parsing positions.json: ${e.message}${RESET}`);
        return new Map();
    }
}

/**
 * Get bounding box of ASCII circuit
 */
function getBoundingBox(filepath) {
    if (!fs.existsSync(filepath)) {
        return { width: 0, height: 0 };
    }
    
    const content = fs.readFileSync(filepath, 'utf8');
    const lines = content.split('\n');
    
    const height = lines.length;
    const width = Math.max(...lines.map(l => l.length));
    
    return { width, height };
}

/**
 * Check if two rectangles overlap
 */
function rectanglesOverlap(r1, r2) {
    return !(r1.x + r1.width <= r2.x ||  // r1 is left of r2
             r2.x + r2.width <= r1.x ||  // r2 is left of r1
             r1.y + r1.height <= r2.y || // r1 is above r2
             r2.y + r2.height <= r1.y);  // r2 is above r1
}

/**
 * Find safe position for a circuit
 */
function findSafePosition(width, height, positions, gridWidth = 480, gridHeight = 240) {
    // Try positions in a grid pattern
    for (let y = 0; y < gridHeight - height; y += 20) {
        for (let x = 0; x < gridWidth - width; x += 20) {
            let safe = true;
            
            for (const [name, pos] of positions) {
                if (rectanglesOverlap(
                    { x, y, width, height },
                    { x: pos.x, y: pos.y, width: pos.width, height: pos.height }
                )) {
                    safe = false;
                    break;
                }
            }
            
            if (safe) {
                return { x, y };
            }
        }
    }
    
    return null;  // No safe position found
}

/**
 * Main collision detection
 */
function checkCollisions() {
    console.log(`\n${BOLD}${CYAN}╔═══════════════════════════════════════════════════════════╗`);
    console.log(`║           CIRCUIT COLLISION DETECTOR                      ║`);
    console.log(`╚═══════════════════════════════════════════════════════════╝${RESET}\n`);
    
    console.log(`Circuits directory: ${CIRCUITS_DIR}`);
    console.log(`Positions file: ${POSITIONS_FILE}\n`);
    
    // Load positions
    const positions = loadPositions();
    
    if (positions.size === 0) {
        console.log(`${YELLOW}No positions defined in positions.json${RESET}`);
        console.log(`Run: ./circuit-watcher.js to auto-generate positions\n`);
        return;
    }
    
    console.log(`${BOLD}Analyzing ${positions.size} circuits...${RESET}\n`);
    
    // Update bounding boxes
    for (const [name, pos] of positions) {
        const filepath = path.join(CIRCUITS_DIR, `${name}.txt`);
        if (fs.existsSync(filepath)) {
            const box = getBoundingBox(filepath);
            pos.width = box.width;
            pos.height = box.height;
        }
    }
    
    // Check for collisions
    const collisions = [];
    const circuitArray = Array.from(positions.entries());
    
    for (let i = 0; i < circuitArray.length; i++) {
        for (let j = i + 1; j < circuitArray.length; j++) {
            const [name1, pos1] = circuitArray[i];
            const [name2, pos2] = circuitArray[j];
            
            if (rectanglesOverlap(
                { x: pos1.x, y: pos1.y, width: pos1.width, height: pos1.height },
                { x: pos2.x, y: pos2.y, width: pos2.width, height: pos2.height }
            )) {
                collisions.push({ name1, name2, pos1, pos2 });
            }
        }
    }
    
    // Report results
    if (collisions.length === 0) {
        console.log(`${GREEN}✓ No collisions detected!${RESET}\n`);
        console.log(`${BOLD}Circuit Layout:${RESET}`);
        
        for (const [name, pos] of positions) {
            console.log(`  ${name.padEnd(20)} (${pos.x}, ${pos.y}) ${pos.width}×${pos.height}`);
        }
        
        console.log(`\n${GREEN}All circuits have safe spacing.${RESET}\n`);
    } else {
        console.log(`${RED}✗ ${collisions.length} collision(s) detected!${RESET}\n`);
        
        for (const col of collisions) {
            console.log(`${BOLD}${RED}COLLISION:${RESET} ${col.name1} ↔ ${col.name2}`);
            console.log(`  ${col.name1}: (${col.pos1.x}, ${col.pos1.y}) ${col.pos1.width}×${col.pos1.height}`);
            console.log(`  ${col.name2}: (${col.pos2.x}, ${col.pos2.y}) ${col.pos2.width}×${col.pos2.height}`);
            
            // Suggest fix
            const safePos = findSafePosition(col.pos2.width, col.pos2.height, positions);
            if (safePos) {
                console.log(`  ${YELLOW}Suggestion: Move ${col.name2} to (${safePos.x}, ${safePos.y})${RESET}`);
            }
            
            console.log();
        }
        
        console.log(`${YELLOW}Fix by editing positions.json or renaming files with @x,y suffix${RESET}\n`);
    }
    
    // Grid usage statistics
    let totalPixels = 0;
    for (const [name, pos] of positions) {
        totalPixels += (pos.width || 0) * (pos.height || 0);
    }
    
    const gridPixels = 480 * 240;
    const usage = ((totalPixels / gridPixels) * 100).toFixed(2);
    
    console.log(`${BOLD}Grid Statistics:${RESET}`);
    console.log(`  Total circuits: ${positions.size}`);
    console.log(`  Pixels used: ${totalPixels.toLocaleString()} / ${gridPixels.toLocaleString()}`);
    console.log(`  Grid usage: ${usage}%`);
    console.log(`  Available: ${(gridPixels - totalPixels).toLocaleString()} pixels\n`);
}

// Run
checkCollisions();
