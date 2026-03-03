/**
 * Geometry OS Shell
 *
 * A glyph-based command interpreter for the Geometry OS.
 * Commands are typed as text but can be rendered as morphological glyphs.
 *
 * Built-in Commands:
 * - ls     (▣) List files
 * - cd     (◈) Change directory
 * - cat    (◆) Display file
 * - run    (▶) Execute program
 * - edit   (✎) Edit file
 * - help   (?) Show help
 * - clear  (✕) Clear screen
 * - ps     (☰) List processes
 * - kill   (✖) Terminate process
 */

// Command glyphs for visual rendering
const GLYPHS = {
    ls: '▣',
    cd: '◈',
    cat: '◆',
    run: '▶',
    edit: '✎',
    help: '?',
    clear: '✕',
    ps: '☰',
    kill: '✖',
    mkdir: '📁',
    touch: '📄',
    rm: '🗑️',
    pwd: '📍',
    echo: '💬',
    exit: '🚪'
};

// Command descriptions
const COMMAND_HELP = {
    ls: 'List files in current directory',
    cd: 'Change directory (cd <path>)',
    cat: 'Display file contents (cat <file>)',
    run: 'Execute a program (run <file.spv>)',
    edit: 'Edit a file (edit <file>)',
    help: 'Show this help message',
    clear: 'Clear the terminal screen',
    ps: 'List running processes',
    kill: 'Terminate process (kill <pid>)',
    mkdir: 'Create directory',
    touch: 'Create empty file',
    rm: 'Remove file',
    pwd: 'Print working directory',
    echo: 'Print text to terminal',
    exit: 'Exit the shell'
};

export class Shell {
    constructor(kernel, filesystem) {
        this.kernel = kernel;
        this.filesystem = filesystem;

        // Shell state
        this.cwd = '/';
        this.env = {
            USER: 'geometer',
            HOME: '/home/geometer',
            PATH: '/bin:/usr/bin',
            SHELL: '/bin/gsh',
            PS1: '\\u@geometry:\\w\\$ '
        };

        // History
        this.history = [];
        this.historyIndex = 0;

        // Output buffer
        this.output = [];

        // Command aliases
        this.aliases = new Map();

        // Redirection support
        this.stdout = null;
        this.stderr = null;
    }

    /**
     * Execute a command string.
     */
    async execute(input) {
        // Add to history
        this.history.push(input);
        this.historyIndex = this.history.length;

        // Parse command
        const trimmed = input.trim();
        if (!trimmed) return '';

        // Handle pipes and redirections (basic)
        const parts = trimmed.split('|').map(p => p.trim());
        let output = '';

        for (let i = 0; i < parts.length; i++) {
            const part = parts[i];
            const { command, args, redirections } = this._parseCommand(part);

            if (!command) continue;

            // Execute command
            const result = await this._executeCommand(command, args, output);
            output = result;

            // Handle redirections
            if (redirections.stdout) {
                await this._writeToFile(redirections.stdout, output);
            }
        }

        return output;
    }

    /**
     * Parse a command string into command, args, and redirections.
     */
    _parseCommand(input) {
        const tokens = this._tokenize(input);
        if (tokens.length === 0) return { command: null, args: [], redirections: {} };

        const command = tokens[0];
        const args = [];
        const redirections = {};

        for (let i = 1; i < tokens.length; i++) {
            const token = tokens[i];

            if (token === '>' && i + 1 < tokens.length) {
                redirections.stdout = { file: tokens[++i], append: false };
            } else if (token === '>>' && i + 1 < tokens.length) {
                redirections.stdout = { file: tokens[++i], append: true };
            } else if (token === '2>' && i + 1 < tokens.length) {
                redirections.stderr = { file: tokens[++i], append: false };
            } else if (token === '<' && i + 1 < tokens.length) {
                redirections.stdin = tokens[++i];
            } else {
                args.push(token);
            }
        }

        return { command, args, redirections };
    }

    /**
     * Tokenize input string (handles quotes).
     */
    _tokenize(input) {
        const tokens = [];
        let current = '';
        let inQuote = null;

        for (let i = 0; i < input.length; i++) {
            const char = input[i];

            if (inQuote) {
                if (char === inQuote) {
                    inQuote = null;
                } else {
                    current += char;
                }
            } else if (char === '"' || char === "'") {
                inQuote = char;
            } else if (char === ' ' || char === '\t') {
                if (current) {
                    tokens.push(current);
                    current = '';
                }
            } else {
                current += char;
            }
        }

        if (current) tokens.push(current);
        return tokens;
    }

    /**
     * Execute a single command.
     */
    async _executeCommand(command, args, stdin) {
        // Check aliases
        const actualCommand = this.aliases.get(command) || command;

        // Built-in commands
        switch (actualCommand) {
            case 'ls':
            case 'list':
            case GLYPHS.ls:
                return this._cmd_ls(args);

            case 'cd':
            case GLYPHS.cd:
                return this._cmd_cd(args);

            case 'cat':
            case GLYPHS.cat:
                return this._cmd_cat(args);

            case 'pwd':
            case GLYPHS.pwd:
                return this._cmd_pwd(args);

            case 'echo':
            case GLYPHS.echo:
                return this._cmd_echo(args);

            case 'ps':
            case GLYPHS.ps:
                return this._cmd_ps(args);

            case 'kill':
            case GLYPHS.kill:
                return this._cmd_kill(args);

            case 'run':
            case GLYPHS.run:
                return await this._cmd_run(args);

            case 'help':
            case GLYPHS.help:
            case '?':
                return this._cmd_help(args);

            case 'clear':
            case GLYPHS.clear:
                return '\x1b[2J\x1b[H'; // ANSI clear screen

            case 'touch':
            case GLYPHS.touch:
                return this._cmd_touch(args);

            case 'rm':
            case GLYPHS.rm:
                return this._cmd_rm(args);

            case 'mkdir':
            case GLYPHS.mkdir:
                return this._cmd_mkdir(args);

            case 'env':
                return this._cmd_env(args);

            case 'export':
                return this._cmd_export(args);

            case 'alias':
                return this._cmd_alias(args);

            case 'history':
                return this._cmd_history(args);

            case 'exit':
            case GLYPHS.exit:
                return 'exit';

            default:
                // Try to execute as program
                return await this._executeProgram(actualCommand, args);
        }
    }

    // --- Built-in Commands ---

    _cmd_ls(args) {
        const showAll = args.includes('-a') || args.includes('--all');
        const longFormat = args.includes('-l');

        const pathArg = args.find(a => !a.startsWith('-'));
        const path = this._resolvePath(pathArg || this.cwd);

        if (!this.filesystem) {
            return 'Error: Filesystem not available';
        }

        const files = this.filesystem.listdir(path);
        if (files.length === 0) {
            return '';
        }

        if (longFormat) {
            let output = 'total ' + files.length + '\n';
            for (const file of files) {
                const stat = this.filesystem.stat(file.name);
                const perms = file.isDirectory ? 'drwxr-xr-x' : '-rw-r--r--';
                const size = (stat?.size || 0).toString().padStart(8);
                output += `${perms} 1 geometer geometer ${size} ${file.name}\n`;
            }
            return output.trim();
        }

        // Simple listing
        return files.map(f => {
            const glyph = f.isDirectory ? '📁' : '📄';
            return `${glyph} ${f.name}`;
        }).join('\n');
    }

    _cmd_cd(args) {
        if (args.length === 0) {
            this.cwd = this.env.HOME;
            return '';
        }

        const newPath = this._resolvePath(args[0]);

        // Check if directory exists
        if (this.filesystem) {
            const stat = this.filesystem.stat(newPath);
            if (!stat) {
                return `cd: ${args[0]}: No such file or directory`;
            }
            if (!stat.isDirectory) {
                return `cd: ${args[0]}: Not a directory`;
            }
        }

        this.cwd = newPath;
        return '';
    }

    _cmd_cat(args) {
        if (args.length === 0) {
            return 'cat: missing file operand';
        }

        const path = this._resolvePath(args[0]);

        if (!this.filesystem) {
            return 'Error: Filesystem not available';
        }

        const stat = this.filesystem.stat(path);
        if (!stat) {
            return `cat: ${args[0]}: No such file or directory`;
        }

        // Open and read file
        const fd = this.filesystem.open(path, 0); // READ mode
        if (fd < 0) {
            return `cat: ${args[0]}: Cannot open file`;
        }

        const buffer = new Uint8Array(stat.size);
        this.filesystem.read(fd, buffer, 0, stat.size);
        this.filesystem.close(fd);

        return new TextDecoder().decode(buffer);
    }

    _cmd_pwd(args) {
        return this.cwd;
    }

    _cmd_echo(args) {
        return args.join(' ');
    }

    _cmd_ps(args) {
        if (!this.kernel) {
            return 'Error: Kernel not available';
        }

        let output = '  PID  STATE   NAME\n';
        output += '───── ─────── ────────────\n';

        // Get processes from kernel
        const processes = this.kernel.processes || [];
        for (const proc of processes) {
            const stateMap = {
                'running': 'RUN',
                'waiting': 'WAIT',
                'terminated': 'EXIT',
                'idle': 'IDLE'
            };
            const state = stateMap[proc.status] || 'UNK';
            const name = proc.name || 'unnamed';
            output += `${proc.pid.toString().padStart(5)} ${state.padEnd(7)} ${name}\n`;
        }

        return output.trim();
    }

    _cmd_kill(args) {
        if (args.length === 0) {
            return 'kill: missing pid';
        }

        const pid = parseInt(args[0], 10);
        if (isNaN(pid)) {
            return `kill: ${args[0]}: invalid pid`;
        }

        if (!this.kernel) {
            return 'Error: Kernel not available';
        }

        if (this.kernel.killProcess(pid)) {
            return `Killed process ${pid}`;
        } else {
            return `kill: ${pid}: no such process`;
        }
    }

    async _cmd_run(args) {
        if (args.length === 0) {
            return 'run: missing program';
        }

        const path = this._resolvePath(args[0]);

        if (!this.kernel) {
            return 'Error: Kernel not available';
        }

        // TODO: Load and execute SPIR-V program
        return `run: ${path}: Program execution not yet implemented`;
    }

    _cmd_help(args) {
        let output = 'Geometry OS Shell - Available Commands:\n\n';

        for (const [cmd, desc] of Object.entries(COMMAND_HELP)) {
            const glyph = GLYPHS[cmd] || '  ';
            output += `  ${glyph} ${cmd.padEnd(8)} - ${desc}\n`;
        }

        output += '\nUse | for pipes, > for redirection';
        return output;
    }

    _cmd_touch(args) {
        if (args.length === 0) {
            return 'touch: missing file operand';
        }

        const path = this._resolvePath(args[0]);

        if (!this.filesystem) {
            return 'Error: Filesystem not available';
        }

        // Check if file exists
        let stat = this.filesystem.stat(path);
        if (!stat) {
            // Create file
            const fd = this.filesystem.open(path, 4); // CREATE mode
            if (fd >= 0) {
                this.filesystem.close(fd);
            }
        }

        return '';
    }

    _cmd_rm(args) {
        if (args.length === 0) {
            return 'rm: missing operand';
        }

        const path = this._resolvePath(args[0]);

        if (!this.filesystem) {
            return 'Error: Filesystem not available';
        }

        if (this.filesystem.unlink(path)) {
            return '';
        } else {
            return `rm: cannot remove '${args[0]}': No such file`;
        }
    }

    _cmd_mkdir(args) {
        if (args.length === 0) {
            return 'mkdir: missing operand';
        }
        // TODO: Implement directory creation in filesystem
        return `mkdir: created directory '${args[0]}'`;
    }

    _cmd_env(args) {
        let output = '';
        for (const [key, value] of Object.entries(this.env)) {
            output += `${key}=${value}\n`;
        }
        return output.trim();
    }

    _cmd_export(args) {
        if (args.length === 0) {
            return this._cmd_env([]);
        }

        const assignment = args[0];
        const eqIdx = assignment.indexOf('=');
        if (eqIdx === -1) {
            return `export: ${assignment}: not a valid identifier`;
        }

        const key = assignment.slice(0, eqIdx);
        const value = assignment.slice(eqIdx + 1);
        this.env[key] = value;

        return '';
    }

    _cmd_alias(args) {
        if (args.length === 0) {
            let output = '';
            for (const [name, cmd] of this.aliases) {
                output += `alias ${name}='${cmd}'\n`;
            }
            return output.trim();
        }

        const assignment = args[0];
        const eqIdx = assignment.indexOf('=');
        if (eqIdx === -1) {
            const existing = this.aliases.get(assignment);
            return existing ? `alias ${assignment}='${existing}'` : `alias: ${assignment}: not found`;
        }

        const name = assignment.slice(0, eqIdx);
        const cmd = assignment.slice(eqIdx + 1).replace(/^['"]|['"]$/g, '');
        this.aliases.set(name, cmd);

        return '';
    }

    _cmd_history(args) {
        let output = '';
        for (let i = 0; i < this.history.length; i++) {
            output += `  ${(i + 1).toString().padStart(4)}  ${this.history[i]}\n`;
        }
        return output.trim();
    }

    // --- Helper Methods ---

    _resolvePath(path) {
        if (path.startsWith('/')) {
            return path;
        }
        if (path === '~') {
            return this.env.HOME;
        }
        if (path.startsWith('~/')) {
            return this.env.HOME + path.slice(1);
        }
        if (path === '..') {
            const parts = this.cwd.split('/').filter(p => p);
            parts.pop();
            return '/' + parts.join('/');
        }
        if (path === '.') {
            return this.cwd;
        }
        return this.cwd + (this.cwd.endsWith('/') ? '' : '/') + path;
    }

    async _executeProgram(name, args) {
        // Search in PATH
        const paths = this.env.PATH.split(':');
        for (const dir of paths) {
            const programPath = `${dir}/${name}`;

            // Check if program exists
            if (this.filesystem) {
                const stat = this.filesystem.stat(programPath);
                if (stat && !stat.isDirectory) {
                    // TODO: Load and execute program
                    return `${name}: Program execution not yet implemented`;
                }
            }
        }

        return `gsh: ${name}: command not found`;
    }

    async _writeToFile(filename, content) {
        if (!this.filesystem) return;

        const path = this._resolvePath(filename);
        const fd = this.filesystem.open(path, 1); // WRITE mode
        if (fd >= 0) {
            const data = new TextEncoder().encode(content);
            this.filesystem.write(fd, data, 0, data.length);
            this.filesystem.close(fd);
        }
    }

    /**
     * Get the prompt string.
     */
    getPrompt() {
        let prompt = this.env.PS1;

        // Expand escape sequences
        prompt = prompt.replace(/\\u/g, this.env.USER);
        prompt = prompt.replace(/\\h/g, 'geometry');
        prompt = prompt.replace(/\\w/g, this.cwd);
        prompt = prompt.replace(/\\W/g, this.cwd.split('/').pop() || '/');
        prompt = prompt.replace(/\\$/g, '$');
        prompt = prompt.replace(/\\n/g, '\n');

        return prompt;
    }

    /**
     * Get command glyph for visual rendering.
     */
    static getGlyph(command) {
        return GLYPHS[command] || '▶';
    }

    /**
     * Get all available commands.
     */
    static getCommands() {
        return Object.keys(GLYPHS);
    }
}

export { GLYPHS, COMMAND_HELP };
