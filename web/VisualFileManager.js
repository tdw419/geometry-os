/**
 * Geometry OS Visual File Manager
 *
 * 3D filesystem browser with code editing capabilities.
 * Files are visualized as nodes in semantic space.
 */

import { MemoryBrowser } from './MemoryBrowser.js';

// File type icons and colors
const FILE_TYPES = {
    js: { color: '#f7df1e', icon: 'JS' },
    ts: { color: '#3178c6', icon: 'TS' },
    wgsl: { color: '#00ffcc', icon: 'GPU' },
    py: { color: '#3776ab', icon: 'PY' },
    md: { color: '#ffffff', icon: 'MD' },
    html: { color: '#e34f26', icon: 'HTML' },
    css: { color: '#264de4', icon: 'CSS' },
    json: { color: '#000000', icon: '{}' },
    spv: { color: '#ff00ff', icon: 'SPV' },
    folder: { color: '#ffaa00', icon: 'D' },
    unknown: { color: '#888888', icon: '?' }
};

export class VisualFileManager extends MemoryBrowser {
    constructor(canvas, options = {}) {
        super(canvas);

        // File system state
        this.files = new Map();  // path -> FileNode
        this.activeFile = null;
        this.currentDirectory = '/';
        this.searchQuery = '';

        // Editor state
        this.editorContent = '';
        this.editorModified = false;
        this.editorPanel = null;
        this.fileTreePanel = null;

        // Options
        this.rootPath = options.rootPath || './';
        this.onFileOpen = options.onFileOpen || (() => {});
        this.onFileSave = options.onFileSave || (() => {});
    }

    async init() {
        await super.init();

        this._initFileUI();
        await this._loadFileTree();

        console.log('[FileManager] 3D File Manager ready');
    }

    _initFileUI() {
        // File tree panel (left sidebar)
        this.fileTreePanel = document.createElement('div');
        this.fileTreePanel.id = 'file-tree-panel';
        this.fileTreePanel.innerHTML = `
            <div class="panel-header">File Browser</div>
            <div class="search-box">
                <input type="text" id="file-search" placeholder="Search files...">
            </div>
            <div class="file-tree" id="file-tree">
                <div class="loading">Loading files...</div>
            </div>
            <div class="file-actions">
                <button class="btn" id="btn-new-file">New File</button>
                <button class="btn" id="btn-new-folder">New Folder</button>
            </div>
        `;
        this.fileTreePanel.style.cssText = `
            position: absolute; top: 80px; left: 20px; width: 280px;
            background: rgba(0, 20, 20, 0.9); border: 1px solid #00ffcc;
            padding: 15px; color: #00ffcc; font-family: monospace;
        `;

        // Editor panel (right sidebar)
        this.editorPanel = document.createElement('div');
        this.editorPanel.id = 'editor-panel';
        this.editorPanel.innerHTML = `
            <div class="panel-header">
                <span id="editor-filename">No file selected</span>
                <span id="editor-modified" style="color: #ffaa00; display: none;">[Modified]</span>
            </div>
            <div class="editor-toolbar">
                <button class="btn-sm" id="btn-save">Save</button>
                <button class="btn-sm" id="btn-compile">Compile</button>
                <button class="btn-sm" id="btn-run">Run</button>
            </div>
            <div class="editor-container">
                <div class="line-numbers" id="line-numbers"></div>
                <textarea id="code-editor" spellcheck="false"></textarea>
            </div>
            <div class="editor-status">
                <span id="cursor-pos">Ln 1, Col 1</span>
                <span id="file-size">0 bytes</span>
            </div>
        `;
        this.editorPanel.style.cssText = `
            position: absolute; top: 80px; right: 20px; width: 500px;
            background: rgba(0, 20, 20, 0.9); border: 1px solid #00ffcc;
            padding: 15px; color: #00ffcc; font-family: monospace;
        `;

        // Append panels
        this.canvas.parentElement.appendChild(this.fileTreePanel);
        this.canvas.parentElement.appendChild(this.editorPanel);

        // Wire events
        this._wireFileEvents();
    }

    _wireFileEvents() {
        // Search
        const searchInput = document.getElementById('file-search');
        if (searchInput) {
            searchInput.addEventListener('input', (e) => {
                this.searchQuery = e.target.value.toLowerCase();
                this._renderFileTree();
            });
        }

        // Editor
        const editor = document.getElementById('code-editor');
        if (editor) {
            editor.addEventListener('input', () => {
                this.editorModified = true;
                document.getElementById('editor-modified').style.display = 'inline';
                this._updateLineNumbers();
            });

            editor.addEventListener('keyup', () => this._updateCursorPosition());
            editor.addEventListener('click', () => this._updateCursorPosition());
        }

        // Buttons
        const saveBtn = document.getElementById('btn-save');
        if (saveBtn) {
            saveBtn.addEventListener('click', () => this._saveCurrentFile());
        }

        const compileBtn = document.getElementById('btn-compile');
        if (compileBtn) {
            compileBtn.addEventListener('click', () => this._compileCurrentFile());
        }

        const runBtn = document.getElementById('btn-run');
        if (runBtn) {
            runBtn.addEventListener('click', () => this._runCurrentFile());
        }

        const newFileBtn = document.getElementById('btn-new-file');
        if (newFileBtn) {
            newFileBtn.addEventListener('click', () => this._createNewFile());
        }

        // File selection from 3D view
        this.on('select', (memory) => {
            // Check if it's a file node
            const file = this.files.get(memory.id);
            if (file) {
                this._openFile(file.path);
            }
        });
    }

    async _loadFileTree() {
        // For MVP, we'll simulate a file tree with known files
        // In production, this would fetch from a file server API
        const mockFiles = [
            { path: '/kernel.wgsl', type: 'wgsl', size: 4096 },
            { path: '/GeometryKernel.js', type: 'js', size: 8192 },
            { path: '/VisualShell.js', type: 'js', size: 12288 },
            { path: '/VisualFileManager.js', type: 'js', size: 6144 },
            { path: '/MemoryBrowser.js', type: 'js', size: 10240 },
            { path: '/memory_browser.wgsl', type: 'wgsl', size: 3072 },
            { path: '/assets/', type: 'folder', size: 0 },
            { path: '/assets/universal_font.spv', type: 'spv', size: 2048 },
            { path: '/agents/', type: 'folder', size: 0 },
            { path: '/agents/AgentGenerator.js', type: 'js', size: 5120 },
            { path: '/agents/index.js', type: 'js', size: 512 },
            { path: '/README.md', type: 'md', size: 1536 },
        ];

        for (const file of mockFiles) {
            this.files.set(file.path, {
                ...file,
                name: file.path.split('/').pop(),
                loaded: false,
                content: ''
            });
        }

        this._renderFileTree();
        this._updateMemoryNodes();
    }

    _renderFileTree() {
        const treeEl = document.getElementById('file-tree');
        if (!treeEl) return;

        // Filter by search
        let files = Array.from(this.files.values());
        if (this.searchQuery) {
            files = files.filter(f => f.name.toLowerCase().includes(this.searchQuery));
        }

        // Build tree structure
        const tree = this._buildTree(files);

        treeEl.innerHTML = this._renderTreeNode(tree, '');
    }

    _buildTree(files) {
        const root = {};

        for (const file of files) {
            const parts = file.path.split('/').filter(p => p);
            let current = root;

            for (let i = 0; i < parts.length; i++) {
                const part = parts[i];
                if (i === parts.length - 1) {
                    // File
                    current[part] = file;
                } else {
                    // Directory
                    if (!current[part]) {
                        current[part] = { __isDir: true };
                    }
                    current = current[part];
                }
            }
        }

        return root;
    }

    _renderTreeNode(node, path, depth = 0) {
        let html = '';
        const indent = '  '.repeat(depth);

        for (const [name, item] of Object.entries(node).sort((a, b) => {
            // Folders first, then alphabetically
            const aIsDir = item.__isDir || item.type === 'folder';
            const bIsDir = b[1].__isDir || b[1].type === 'folder';
            if (aIsDir && !bIsDir) return -1;
            if (!aIsDir && bIsDir) return 1;
            return a.localeCompare(b);
        })) {
            const fullPath = path + '/' + name;
            const isDir = item.__isDir || item.type === 'folder';

            if (isDir) {
                html += `<div class="tree-folder" style="padding-left: ${depth * 16}px">
                    <span class="folder-icon">📁</span> ${name}/
                </div>`;
                html += this._renderTreeNode(item, fullPath, depth + 1);
            } else {
                const typeInfo = FILE_TYPES[item.type] || FILE_TYPES.unknown;
                const isActive = this.activeFile === item.path;
                html += `<div class="tree-file ${isActive ? 'active' : ''}"
                             style="padding-left: ${depth * 16}px; border-left: 3px solid ${typeInfo.color}"
                             data-path="${item.path}">
                    <span class="file-icon" style="color: ${typeInfo.color}">${typeInfo.icon}</span>
                    ${name}
                </div>`;
            }
        }

        return html;
    }

    _updateMemoryNodes() {
        // Create memory nodes for 3D visualization
        const files = Array.from(this.files.values()).filter(f => f.type !== 'folder');

        this.memories = files.map((file, i) => ({
            id: file.path,
            type: this._getFileCategory(file.type),
            priority: file.size / 1024,  // Size determines visual prominence
            content: file.name,
            tags: [file.type, 'source'],
            embedding: this._generateFileEmbedding(file)
        }));

        // Emit update event
        this.emit('filesLoaded', { count: files.length });
    }

    _getFileCategory(type) {
        const categories = {
            js: 'code',
            ts: 'code',
            wgsl: 'code',
            py: 'code',
            md: 'note',
            html: 'code',
            css: 'code',
            spv: 'binary'
        };
        return categories[type] || 'unknown';
    }

    _generateFileEmbedding(file) {
        // Generate a pseudo-embedding based on file properties
        // In production, this would use actual semantic embeddings
        const embedding = new Float32Array(384);

        // Use file type as primary feature
        const typeIdx = Object.keys(FILE_TYPES).indexOf(file.type);
        embedding[typeIdx] = 1.0;

        // Use size as secondary feature
        const sizeFeature = Math.log(file.size + 1) / 10;
        embedding[10] = sizeFeature;

        // Use path components as features
        const pathParts = file.path.split('/').filter(p => p);
        for (let i = 0; i < pathParts.length && i < 10; i++) {
            const hash = this._hashString(pathParts[i]);
            embedding[20 + i] = (hash % 100) / 100;
        }

        return embedding;
    }

    _hashString(str) {
        let hash = 0;
        for (let i = 0; i < str.length; i++) {
            hash = ((hash << 5) - hash) + str.charCodeAt(i);
            hash |= 0;
        }
        return Math.abs(hash);
    }

    async _openFile(path) {
        const file = this.files.get(path);
        if (!file || file.type === 'folder') return;

        console.log(`[FileManager] Opening ${path}`);

        // Load file content (simulated for MVP)
        if (!file.loaded) {
            file.content = await this._fetchFileContent(path);
            file.loaded = true;
        }

        this.activeFile = path;
        this.editorContent = file.content;
        this.editorModified = false;

        // Update UI
        document.getElementById('editor-filename').textContent = file.name;
        document.getElementById('editor-modified').style.display = 'none';
        document.getElementById('code-editor').value = file.content;
        document.getElementById('file-size').textContent = `${file.content.length} bytes`;

        this._updateLineNumbers();
        this._renderFileTree();  // Highlight active file

        this.onFileOpen(file);
    }

    async _fetchFileContent(path) {
        // In production, fetch from server
        // For MVP, try to fetch the actual file
        try {
            const response = await fetch(path.slice(1));  // Remove leading /
            if (response.ok) {
                return await response.text();
            }
        } catch (e) {
            // File not found, return placeholder
        }

        return `// File: ${path}\n// Content not loaded\n`;
    }

    async _saveCurrentFile() {
        if (!this.activeFile) return;

        const content = document.getElementById('code-editor').value;
        const file = this.files.get(this.activeFile);

        if (file) {
            file.content = content;
            file.loaded = true;
            file.size = content.length;

            this.editorModified = false;
            document.getElementById('editor-modified').style.display = 'none';
            document.getElementById('file-size').textContent = `${content.length} bytes`;

            console.log(`[FileManager] Saved ${this.activeFile}`);
            this.onFileSave(file);
        }
    }

    _compileCurrentFile() {
        if (!this.activeFile) return;

        const file = this.files.get(this.activeFile);
        if (file && file.type === 'wgsl') {
            console.log(`[FileManager] Compiling ${this.activeFile}...`);
            // TODO: Integrate with VisualCompiler
            this.emit('compile', { path: this.activeFile, content: file.content });
        } else {
            console.log(`[FileManager] Cannot compile ${file?.type} files`);
        }
    }

    _runCurrentFile() {
        if (!this.activeFile) return;

        const file = this.files.get(this.activeFile);
        if (file && (file.type === 'wgsl' || file.type === 'spv')) {
            console.log(`[FileManager] Running ${this.activeFile}...`);
            this.emit('run', { path: this.activeFile, content: file.content });
        }
    }

    _createNewFile() {
        const name = prompt('Enter file name:');
        if (!name) return;

        const type = name.split('.').pop() || 'txt';
        const path = '/' + name;

        this.files.set(path, {
            path,
            name,
            type,
            size: 0,
            loaded: true,
            content: ''
        });

        this._renderFileTree();
        this._updateMemoryNodes();
        this._openFile(path);
    }

    _updateLineNumbers() {
        const editor = document.getElementById('code-editor');
        const lineNumbers = document.getElementById('line-numbers');
        if (!editor || !lineNumbers) return;

        const lines = editor.value.split('\n');
        lineNumbers.innerHTML = lines.map((_, i) => `<div>${i + 1}</div>`).join('');
    }

    _updateCursorPosition() {
        const editor = document.getElementById('code-editor');
        const cursorPos = document.getElementById('cursor-pos');
        if (!editor || !cursorPos) return;

        const text = editor.value.substring(0, editor.selectionStart);
        const lines = text.split('\n');
        const line = lines.length;
        const col = lines[lines.length - 1].length + 1;

        cursorPos.textContent = `Ln ${line}, Col ${col}`;
    }

    /**
     * Get all files of a specific type.
     * @param {string} type - File extension
     * @returns {Array} Matching files
     */
    getFilesByType(type) {
        return Array.from(this.files.values()).filter(f => f.type === type);
    }

    /**
     * Search files by content.
     * @param {string} query - Search query
     * @returns {Array} Matching files
     */
    searchContent(query) {
        const results = [];
        for (const file of this.files.values()) {
            if (file.content && file.content.toLowerCase().includes(query.toLowerCase())) {
                results.push(file);
            }
        }
        return results;
    }
}
