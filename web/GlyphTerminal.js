/**
 * Geometry OS - Glyph Terminal
 *
 * An interactive 3D console where typing triggers real-time VisualCompiler streams.
 * Allows piping glyph data between processes with visual eye-beam connections.
 *
 * Example:
 *   $ process_a | process_b
 *   (visually connects their eye-beams)
 */

import { GeometryKernel } from './GeometryKernel.js';

// Terminal colors
const COLORS = {
    prompt: '#00ff88',
    input: '#ffffff',
    output: '#888888',
    error: '#ff4444',
    highlight: '#00ffcc',
    pipe: '#ff00ff'
};

// Built-in commands
const COMMANDS = {
    help: 'Show available commands',
    ps: 'List running processes',
    spawn: 'Spawn a new process from .spv file',
    kill: 'Kill a process by PID',
    ipc: 'Show IPC message queues',
    cat: 'Display memory/process output',
    pipe: 'Connect two processes (a | b)',
    mem: 'Show memory usage',
    clear: 'Clear terminal',
    run: 'Execute a .spv program',
    compile: 'Compile and run code',
    agents: 'Show agent status',
    sectors: 'Show Hilbert sector map',
    tether: 'Create visual tether between entities',
    exit: 'Shutdown terminal'
};

export class GlyphTerminal {
    constructor(container, options = {}) {
        this.container = container;
        this.kernel = options.kernel || new GeometryKernel();

        // Terminal state
        this.history = [];
        this.historyIndex = 0;
        this.inputBuffer = '';
        this.cursorPosition = 0;

        // Process registry
        this.processes = new Map();
        this.pipes = [];  // Active pipe connections

        // Output buffer
        this.outputLines = [];
        this.maxLines = 1000;

        // UI elements
        this.element = null;
        this.outputElement = null;
        this.inputElement = null;

        // Callbacks
        this.onPipe = options.onPipe || (() => {});
        this.onCommand = options.onCommand || (() => {});
    }

    /**
     * Initialize the terminal
     */
    async init() {
        // Create terminal UI
        this._createUI();

        // Initialize kernel if needed
        if (!this.kernel.device) {
            await this.kernel.init();
        }

        // Show welcome message
        this._printWelcome();

        // Wire events
        this._wireEvents();

        console.log('[GlyphTerminal] Terminal ready');
        return this;
    }

    _createUI() {
        this.element = document.createElement('div');
        this.element.className = 'glyph-terminal';
        this.element.innerHTML = `
            <div class="terminal-header">
                <span class="terminal-title">Glyph Terminal</span>
                <span class="terminal-controls">
                    <button class="term-btn" id="term-clear">Clear</button>
                    <button class="term-btn" id="term-export">Export</button>
                </span>
            </div>
            <div class="terminal-output" id="terminal-output"></div>
            <div class="terminal-input-line">
                <span class="terminal-prompt">$</span>
                <input type="text" class="terminal-input" id="terminal-input"
                       autofocus autocomplete="off" spellcheck="false">
            </div>
            <div class="terminal-status">
                <span id="term-cursor">1:1</span>
                <span id="term-mode">INSERT</span>
                <span id="term-processes">0 processes</span>
            </div>
        `;

        // Apply styles
        this.element.style.cssText = `
            position: absolute;
            bottom: 60px;
            left: 50%;
            transform: translateX(-50%);
            width: 600px;
            height: 300px;
            background: rgba(0, 10, 10, 0.95);
            border: 1px solid ${COLORS.highlight};
            border-radius: 8px;
            font-family: 'Courier New', monospace;
            font-size: 12px;
            display: flex;
            flex-direction: column;
            z-index: 200;
            box-shadow: 0 0 20px rgba(0, 255, 204, 0.3);
        `;

        // Add embedded styles
        const style = document.createElement('style');
        style.textContent = `
            .glyph-terminal * { box-sizing: border-box; }
            .terminal-header {
                display: flex;
                justify-content: space-between;
                padding: 8px 12px;
                border-bottom: 1px solid ${COLORS.highlight}44;
                background: rgba(0, 50, 50, 0.3);
            }
            .terminal-title { color: ${COLORS.highlight}; font-weight: bold; }
            .terminal-controls { display: flex; gap: 8px; }
            .term-btn {
                background: transparent;
                border: 1px solid ${COLORS.highlight}66;
                color: ${COLORS.highlight};
                padding: 2px 8px;
                cursor: pointer;
                font-size: 10px;
                border-radius: 3px;
            }
            .term-btn:hover { background: ${COLORS.highlight}33; }
            .terminal-output {
                flex: 1;
                padding: 10px;
                overflow-y: auto;
                background: rgba(0, 0, 0, 0.3);
            }
            .terminal-line { margin: 2px 0; white-space: pre-wrap; }
            .terminal-line.prompt { color: ${COLORS.prompt}; }
            .terminal-line.input { color: ${COLORS.input}; }
            .terminal-line.output { color: ${COLORS.output}; }
            .terminal-line.error { color: ${COLORS.error}; }
            .terminal-line.highlight { color: ${COLORS.highlight}; }
            .terminal-line.pipe { color: ${COLORS.pipe}; font-weight: bold; }
            .terminal-input-line {
                display: flex;
                align-items: center;
                padding: 8px 12px;
                border-top: 1px solid ${COLORS.highlight}44;
                background: rgba(0, 30, 30, 0.3);
            }
            .terminal-prompt { color: ${COLORS.prompt}; margin-right: 8px; }
            .terminal-input {
                flex: 1;
                background: transparent;
                border: none;
                color: ${COLORS.input};
                font-family: inherit;
                font-size: inherit;
                outline: none;
            }
            .terminal-status {
                display: flex;
                justify-content: space-between;
                padding: 4px 12px;
                font-size: 10px;
                color: #666;
                border-top: 1px solid ${COLORS.highlight}22;
            }
        `;
        this.element.appendChild(style);

        this.container.appendChild(this.element);

        // Cache elements
        this.outputElement = document.getElementById('terminal-output');
        this.inputElement = document.getElementById('terminal-input');
    }

    _wireEvents() {
        // Input handling
        this.inputElement.addEventListener('keydown', (e) => this._onKeyDown(e));
        this.inputElement.addEventListener('input', () => this._updateCursor());

        // Control buttons
        document.getElementById('term-clear')?.addEventListener('click', () => this.clear());
        document.getElementById('term-export')?.addEventListener('click', () => this._exportLog());

        // Focus on click
        this.element.addEventListener('click', () => this.inputElement.focus());
    }

    _printWelcome() {
        this._print(`
╔══════════════════════════════════════════════════════════════╗
║                    GEOMETRY OS TERMINAL                       ║
║                    Glyph Interface v1.0                       ║
╚══════════════════════════════════════════════════════════════╝

Type 'help' for available commands.
Type 'process_a | process_b' to visually pipe between processes.

`, 'highlight');
    }

    _onKeyDown(e) {
        switch (e.key) {
            case 'Enter':
                this._executeCommand();
                break;
            case 'ArrowUp':
                e.preventDefault();
                this._historyPrev();
                break;
            case 'ArrowDown':
                e.preventDefault();
                this._historyNext();
                break;
            case 'Tab':
                e.preventDefault();
                this._autocomplete();
                break;
            case 'Escape':
                this.inputElement.value = '';
                break;
        }
    }

    _executeCommand() {
        const input = this.inputElement.value.trim();
        this.inputElement.value = '';

        if (!input) return;

        // Add to history
        this.history.push(input);
        this.historyIndex = this.history.length;

        // Echo input
        this._print(`$ ${input}`, 'prompt');

        // Parse and execute
        this._parseCommand(input);
    }

    _parseCommand(input) {
        const parts = input.split(/\s+/);
        const cmd = parts[0].toLowerCase();

        // Check for pipe operator
        if (input.includes('|')) {
            this._handlePipe(input);
            return;
        }

        switch (cmd) {
            case 'help':
                this._cmdHelp();
                break;
            case 'ps':
                this._cmdPs();
                break;
            case 'spawn':
                this._cmdSpawn(parts.slice(1));
                break;
            case 'kill':
                this._cmdKill(parts[1]);
                break;
            case 'ipc':
                this._cmdIpc();
                break;
            case 'cat':
                this._cmdCat(parts[1]);
                break;
            case 'mem':
                this._cmdMem();
                break;
            case 'clear':
                this.clear();
                break;
            case 'run':
                this._cmdRun(parts[1]);
                break;
            case 'compile':
                this._cmdCompile(parts.slice(1).join(' '));
                break;
            case 'agents':
                this._cmdAgents();
                break;
            case 'sectors':
                this._cmdSectors();
                break;
            case 'tether':
                this._cmdTether(parts[1], parts[2]);
                break;
            case 'exit':
                this._cmdExit();
                break;
            default:
                this._print(`Unknown command: ${cmd}. Type 'help' for available commands.`, 'error');
        }

        this.onCommand(cmd, parts.slice(1));
    }

    _handlePipe(input) {
        const [left, right] = input.split('|').map(s => s.trim());

        this._print(`Creating pipe: ${left} → ${right}`, 'pipe');

        // Parse source and destination
        const sourcePid = this._resolveProcess(left);
        const destPid = this._resolveProcess(right);

        if (sourcePid !== null && destPid !== null) {
            // Create visual pipe connection
            this.pipes.push({
                from: sourcePid,
                to: destPid,
                created: Date.now()
            });

            // Emit pipe event for visualization
            this.onPipe({
                from: sourcePid,
                to: destPid,
                visual: true
            });

            this._print(`Pipe established: process ${sourcePid} → process ${destPid}`, 'highlight');
        } else {
            this._print('Could not resolve process names', 'error');
        }
    }

    _resolveProcess(name) {
        // Try parsing as PID
        const pid = parseInt(name);
        if (!isNaN(pid)) return pid;

        // Try looking up by name
        for (const [pid, proc] of this.processes) {
            if (proc.name?.includes(name)) return pid;
        }

        return null;
    }

    _cmdHelp() {
        this._print('\nAvailable commands:', 'highlight');
        for (const [cmd, desc] of Object.entries(COMMANDS)) {
            this._print(`  ${cmd.padEnd(12)} - ${desc}`);
        }
        this._print('\nPiping:', 'highlight');
        this._print('  process_a | process_b  - Connect processes visually');
        this._print('');
    }

    _cmdPs() {
        this._print('\nPID   STATUS    NAME                 CYCLES', 'highlight');
        this._print('─'.repeat(45));

        for (const [pid, proc] of this.processes) {
            const status = ['IDLE', 'RUNNING', 'WAITING', 'DONE', 'ERROR'][proc.state || 0];
            this._print(`${String(pid).padEnd(6)}${status.padEnd(10)}${(proc.name || 'unnamed').padEnd(21)}${proc.cycles || 0}`);
        }

        if (this.processes.size === 0) {
            this._print('No running processes');
        }
        this._print('');
    }

    async _cmdSpawn(args) {
        if (args.length === 0) {
            this._print('Usage: spawn <file.spv>', 'error');
            return;
        }

        const file = args[0];
        this._print(`Spawning process from ${file}...`);

        try {
            const response = await fetch(file);
            if (!response.ok) throw new Error(`File not found: ${file}`);

            const buffer = await response.arrayBuffer();
            const pid = await this.kernel.spawnProcess(buffer, file);

            this.processes.set(pid, {
                name: file,
                state: 1,  // Running
                cycles: 0
            });

            this._print(`Process spawned with PID ${pid}`, 'highlight');
            this._updateStatus();
        } catch (e) {
            this._print(`Error: ${e.message}`, 'error');
        }
    }

    _cmdKill(pidStr) {
        const pid = parseInt(pidStr);
        if (isNaN(pid)) {
            this._print('Usage: kill <pid>', 'error');
            return;
        }

        if (this.processes.has(pid)) {
            const proc = this.processes.get(pid);
            proc.state = 3;  // Done
            this._print(`Process ${pid} killed`, 'highlight');
            this._updateStatus();
        } else {
            this._print(`Process ${pid} not found`, 'error');
        }
    }

    _cmdIpc() {
        this._print('\nIPC Message Queues:', 'highlight');
        this._print('─'.repeat(30));

        // Show mailbox status
        for (let i = 0; i < 16; i++) {
            const base = i * 32;
            this._print(`Mailbox[${i}]: Offset ${base}`);
        }
        this._print('');
    }

    _cmdCat(target) {
        if (!target) {
            this._print('Usage: cat <pid|memory>', 'error');
            return;
        }
        this._print(`Output from ${target}:`, 'highlight');
        this._print('(Process output would appear here)');
    }

    _cmdMem() {
        const used = this.processes.size * 512 * 4;
        const total = 256 * 1024;

        this._print('\nMemory Usage:', 'highlight');
        this._print(`  Processes: ${this.processes.size}`);
        this._print(`  Used: ${(used / 1024).toFixed(1)} KB`);
        this._print(`  Total: ${(total / 1024).toFixed(0)} KB`);
        this._print(`  Free: ${((total - used) / 1024).toFixed(1)} KB`);
        this._print('');
    }

    _cmdRun(file) {
        if (!file) {
            this._print('Usage: run <file.spv>', 'error');
            return;
        }
        this._print(`Running ${file}...`);
        // Would integrate with kernel execution
    }

    _cmdCompile(code) {
        if (!code) {
            this._print('Usage: compile <code>', 'error');
            return;
        }
        this._print(`Compiling: ${code}`, 'highlight');
        this._print('(VisualCompiler would process this)');
    }

    _cmdAgents() {
        this._print('\nAgent Status:', 'highlight');
        this._print('  ◇ Loop Orchestrator: Active');
        this._print('  △ Bridge Planner: Idle');
        this._print('  □ Bridge Worker: Active');
        this._print('  ○ Memory Agent: Reading');
        this._print('');
    }

    _cmdSectors() {
        this._print('\nHilbert Sector Map:', 'highlight');
        this._print('  KERNEL  │ 0-1023      │ PCBs, Scheduler');
        this._print('  MAILBOX │ 1024-1535   │ IPC Queues');
        this._print('  SHELL   │ 2048-6143   │ Process RAM');
        this._print('  FILES   │ 8192-24575  │ Filesystem');
        this._print('  MEMORY  │ 32768-65535 │ Substrate');
        this._print('  AGENTS  │ 65536-81919 │ Avatar Space');
        this._print('');
    }

    _cmdTether(from, to) {
        if (!from || !to) {
            this._print('Usage: tether <from> <to>', 'error');
            return;
        }
        this._print(`Creating tether: ${from} ↔ ${to}`, 'pipe');
        // Would create visual tether
    }

    _cmdExit() {
        this._print('Shutting down terminal...', 'highlight');
        this.element.style.display = 'none';
    }

    _historyPrev() {
        if (this.historyIndex > 0) {
            this.historyIndex--;
            this.inputElement.value = this.history[this.historyIndex];
        }
    }

    _historyNext() {
        if (this.historyIndex < this.history.length - 1) {
            this.historyIndex++;
            this.inputElement.value = this.history[this.historyIndex];
        } else {
            this.historyIndex = this.history.length;
            this.inputElement.value = '';
        }
    }

    _autocomplete() {
        const input = this.inputElement.value;
        const parts = input.split(/\s+/);
        const last = parts[parts.length - 1].toLowerCase();

        // Complete command
        for (const cmd of Object.keys(COMMANDS)) {
            if (cmd.startsWith(last)) {
                parts[parts.length - 1] = cmd;
                this.inputElement.value = parts.join(' ');
                break;
            }
        }
    }

    _updateCursor() {
        const pos = this.inputElement.selectionStart + 1;
        document.getElementById('term-cursor').textContent = `1:${pos}`;
    }

    _updateStatus() {
        document.getElementById('term-processes').textContent = `${this.processes.size} processes`;
    }

    _print(text, className = 'output') {
        const line = document.createElement('div');
        line.className = `terminal-line ${className}`;
        line.textContent = text;
        this.outputElement.appendChild(line);
        this.outputElement.scrollTop = this.outputElement.scrollHeight;
    }

    _exportLog() {
        const log = this.outputElement.innerText;
        const blob = new Blob([log], { type: 'text/plain' });
        const url = URL.createObjectURL(blob);

        const a = document.createElement('a');
        a.href = url;
        a.download = `terminal-${Date.now()}.log`;
        a.click();

        URL.revokeObjectURL(url);
        this._print('Log exported', 'highlight');
    }

    /**
     * Clear the terminal
     */
    clear() {
        this.outputElement.innerHTML = '';
        this._printWelcome();
    }

    /**
     * Show the terminal
     */
    show() {
        this.element.style.display = 'flex';
        this.inputElement.focus();
    }

    /**
     * Hide the terminal
     */
    hide() {
        this.element.style.display = 'none';
    }

    /**
     * Toggle terminal visibility
     */
    toggle() {
        if (this.element.style.display === 'none') {
            this.show();
        } else {
            this.hide();
        }
    }
}
