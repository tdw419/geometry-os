#!/usr/bin/env node
/**
 * Queue Manager Dashboard - Real-time visualization
 * 
 * Usage: node queue_dashboard.js [--watch]
 */

import { readFileSync, existsSync, watchFile, unwatchFile } from 'fs';
import { join } from 'path';

const STATE_DIR = join(process.env.PROJECT_ROOT || '.', '.ouroboros', 'queue');
const QUEUE_FILE = join(STATE_DIR, 'prompt_queue.json');
const RATE_LIMIT_FILE = join(STATE_DIR, 'rate_limits.json');

// ANSI escape codes
const ANSI = {
    clear: '\x1b[2J\x1b[H',
    hide: '\x1b[?25l',
    show: '\x1b[?25h',
    reset: '\x1b[0m',
    bold: '\x1b[1m',
    dim: '\x1b[2m',
    red: '\x1b[31m',
    green: '\x1b[32m',
    yellow: '\x1b[33m',
    blue: '\x1b[34m',
    magenta: '\x1b[35m',
    cyan: '\x1b[36m',
    white: '\x1b[37m',
    bg: {
        red: '\x1b[41m',
        green: '\x1b[42m',
        yellow: '\x1b[43m',
        blue: '\x1b[44m',
    }
};

function loadQueue() {
    if (!existsSync(QUEUE_FILE)) {
        return { pending: [], completed: [], failed: [] };
    }
    try {
        return JSON.parse(readFileSync(QUEUE_FILE, 'utf-8'));
    } catch {
        return { pending: [], completed: [], failed: [] };
    }
}

function loadRateLimits() {
    if (!existsSync(RATE_LIMIT_FILE)) {
        return {};
    }
    try {
        return JSON.parse(readFileSync(RATE_LIMIT_FILE, 'utf-8'));
    } catch {
        return {};
    }
}

function truncate(str, len) {
    if (!str) return '';
    return str.length > len ? str.slice(0, len - 3) + '...' : str;
}

function formatTime(isoString) {
    if (!isoString) return '';
    const date = new Date(isoString);
    return date.toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit', second: '2-digit' });
}

function formatDuration(ms) {
    if (!ms || ms <= 0) return '';
    const seconds = Math.floor(ms / 1000);
    if (seconds < 60) return `${seconds}s`;
    const minutes = Math.floor(seconds / 60);
    if (minutes < 60) return `${minutes}m ${seconds % 60}s`;
    const hours = Math.floor(minutes / 60);
    return `${hours}h ${minutes % 60}m`;
}

function progressBar(filled, total, width = 20) {
    const pct = total ? filled / total : 0;
    const filledLen = Math.round(pct * width);
    const emptyLen = width - filledLen;
    return '█'.repeat(filledLen) + '░'.repeat(emptyLen);
}

function renderDashboard() {
    const queue = loadQueue();
    const rateLimits = loadRateLimits();
    
    const lines = [];
    
    // Header
    lines.push(`${ANSI.bold}${ANSI.cyan}═══════════════════════════════════════════════════════════════════${ANSI.reset}`);
    lines.push(`${ANSI.bold}${ANSI.cyan}  🐍 OUROBOROS QUEUE DASHBOARD${ANSI.reset}  ${ANSI.dim}${new Date().toLocaleTimeString()}${ANSI.reset}`);
    lines.push(`${ANSI.bold}${ANSI.cyan}═══════════════════════════════════════════════════════════════════${ANSI.reset}`);
    lines.push('');
    
    // Queue Summary
    const total = queue.pending.length + queue.completed.length + queue.failed.length;
    lines.push(`${ANSI.bold}📊 Queue Summary${ANSI.reset}`);
    lines.push(`   Total: ${total}  │  ${ANSI.yellow}Pending: ${queue.pending.length}${ANSI.reset}  │  ${ANSI.green}Completed: ${queue.completed.length}${ANSI.reset}  │  ${ANSI.red}Failed: ${queue.failed.length}${ANSI.reset}`);
    lines.push('');
    
    // Rate Limits
    lines.push(`${ANSI.bold}⏱️  Rate Limits${ANSI.reset}`);
    const now = Date.now();
    for (const [provider, data] of Object.entries(rateLimits)) {
        if (data.rateLimited?.until) {
            const waitMs = data.rateLimited.until - now;
            if (waitMs > 0) {
                lines.push(`   ${ANSI.yellow}⚠️  ${provider}${ANSI.reset}: Rate limited for ${formatDuration(waitMs)}`);
            }
        }
    }
    if (Object.keys(rateLimits).filter(p => rateLimits[p].rateLimited?.until > now).length === 0) {
        lines.push(`   ${ANSI.green}✓${ANSI.reset} No active rate limits`);
    }
    lines.push('');
    
    // Pending Prompts
    lines.push(`${ANSI.bold}${ANSI.yellow}📥 Pending (${queue.pending.length})${ANSI.reset}`);
    if (queue.pending.length === 0) {
        lines.push(`   ${ANSI.dim}(empty)${ANSI.reset}`);
    } else {
        lines.push(`   ${ANSI.dim}ID                   Priority  Status       Prompt${ANSI.reset}`);
        lines.push(`   ${ANSI.dim}─────────────────────────────────────────────────────────${ANSI.reset}`);
        for (const item of queue.pending.slice(0, 10)) {
            const status = item.status === 'rate_limited' ? `${ANSI.yellow}rate_limited${ANSI.reset}` : 
                           item.status === 'processing' ? `${ANSI.blue}processing${ANSI.reset}` : 
                           `${item.status}`;
            lines.push(`   ${item.id.slice(-20)}  ${item.priority.toString().padStart(2)}        ${status.padEnd(12)} ${truncate(item.prompt, 35)}`);
        }
        if (queue.pending.length > 10) {
            lines.push(`   ${ANSI.dim}... and ${queue.pending.length - 10} more${ANSI.reset}`);
        }
    }
    lines.push('');
    
    // Completed Prompts
    lines.push(`${ANSI.bold}${ANSI.green}✅ Completed (${queue.completed.length})${ANSI.reset}`);
    if (queue.completed.length === 0) {
        lines.push(`   ${ANSI.dim}(empty)${ANSI.reset}`);
    } else {
        lines.push(`   ${ANSI.dim}ID                   Provider   Time        Prompt${ANSI.reset}`);
        lines.push(`   ${ANSI.dim}─────────────────────────────────────────────────────────${ANSI.reset}`);
        for (const item of queue.completed.slice(-5).reverse()) {
            lines.push(`   ${item.id.slice(-20)}  ${(item.provider || 'unknown').padEnd(8)}  ${formatTime(item.completedAt)}  ${truncate(item.prompt, 30)}`);
        }
        if (queue.completed.length > 5) {
            lines.push(`   ${ANSI.dim}... showing last 5 of ${queue.completed.length}${ANSI.reset}`);
        }
    }
    lines.push('');
    
    // Failed Prompts
    lines.push(`${ANSI.bold}${ANSI.red}❌ Failed (${queue.failed.length})${ANSI.reset}`);
    if (queue.failed.length === 0) {
        lines.push(`   ${ANSI.dim}(empty)${ANSI.reset}`);
    } else {
        lines.push(`   ${ANSI.dim}ID                   Attempts  Error${ANSI.reset}`);
        lines.push(`   ${ANSI.dim}─────────────────────────────────────────────────────────${ANSI.reset}`);
        for (const item of queue.failed.slice(-5).reverse()) {
            lines.push(`   ${item.id.slice(-20)}  ${item.attempts}/${item.maxAttempts}     ${truncate(item.lastError, 40)}`);
        }
        if (queue.failed.length > 5) {
            lines.push(`   ${ANSI.dim}... showing last 5 of ${queue.failed.length}${ANSI.reset}`);
        }
    }
    lines.push('');
    
    // Footer
    lines.push(`${ANSI.dim}───────────────────────────────────────────────────────────────────${ANSI.reset}`);
    lines.push(`${ANSI.dim}  Press Ctrl+C to exit${ANSI.reset}`);
    lines.push('');
    
    // Render
    process.stdout.write(ANSI.clear + lines.join('\n'));
}

// Watch mode
const watchMode = process.argv.includes('--watch') || process.argv.includes('-w');

if (watchMode) {
    // Hide cursor
    process.stdout.write(ANSI.hide);
    
    // Initial render
    renderDashboard();
    
    // Watch for changes
    if (existsSync(QUEUE_FILE)) {
        watchFile(QUEUE_FILE, { interval: 500 }, renderDashboard);
    }
    if (existsSync(RATE_LIMIT_FILE)) {
        watchFile(RATE_LIMIT_FILE, { interval: 500 }, renderDashboard);
    }
    
    // Refresh every 5 seconds
    setInterval(renderDashboard, 5000);
    
    // Cleanup on exit
    process.on('SIGINT', () => {
        process.stdout.write(ANSI.show);
        unwatchFile(QUEUE_FILE);
        unwatchFile(RATE_LIMIT_FILE);
        process.exit(0);
    });
} else {
    // Single render
    renderDashboard();
}
