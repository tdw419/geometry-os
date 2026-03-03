/**
 * Geometry OS Visual Desktop
 *
 * A windowed desktop environment for Geometry OS.
 * Features:
 * - Multi-window support with drag, resize, minimize, maximize
 * - Window animations (open, close, minimize, maximize)
 * - Application launcher
 * - Keyboard shortcuts
 * - Taskbar with running applications
 * - Desktop icons
 */

// Window states
const WINDOW_STATE = {
    NORMAL: 'normal',
    MINIMIZED: 'minimized',
    MAXIMIZED: 'maximized'
};

// Animation durations (ms)
const ANIMATION = {
    OPEN: 200,
    CLOSE: 150,
    MINIMIZE: 200,
    MAXIMIZE: 200,
    RESTORE: 200
};

// Default window dimensions
const DEFAULT_WINDOW = {
    WIDTH: 600,
    HEIGHT: 400,
    MIN_WIDTH: 200,
    MIN_HEIGHT: 150
};

// Application registry
const APPS = {
    terminal: {
        name: 'Terminal',
        icon: '▣',
        color: '#00ff88',
        singleton: false
    },
    fileBrowser: {
        name: 'Files',
        icon: '📁',
        color: '#ffaa00',
        singleton: true
    },
    processManager: {
        name: 'Processes',
        icon: '☰',
        color: '#00aaff',
        singleton: true
    },
    editor: {
        name: 'Editor',
        icon: '✎',
        color: '#ff88ff',
        singleton: false
    },
    settings: {
        name: 'Settings',
        icon: '⚙',
        color: '#888888',
        singleton: true
    },
    networkMonitor: {
        name: 'Network',
        icon: '🌐',
        color: '#88ffff',
        singleton: true
    },
    memoryBrowser: {
        name: 'Memory',
        icon: '🧠',
        color: '#ff8888',
        singleton: true
    },
    help: {
        name: 'Help',
        icon: '?',
        color: '#ffff88',
        singleton: true
    }
};

/**
 * Desktop Window class
 */
class DesktopWindow {
    constructor(id, options = {}) {
        this.id = id;
        this.appId = options.appId || 'terminal';
        this.title = options.title || 'Window';
        this.state = WINDOW_STATE.NORMAL;
        
        // Dimensions
        this.x = options.x || 100 + (id * 30) % 200;
        this.y = options.y || 100 + (id * 30) % 150;
        this.width = options.width || DEFAULT_WINDOW.WIDTH;
        this.height = options.height || DEFAULT_WINDOW.HEIGHT;
        
        // Saved dimensions for restore
        this.savedX = this.x;
        this.savedY = this.y;
        this.savedWidth = this.width;
        this.savedHeight = this.height;
        
        // Z-index (managed by desktop)
        this.zIndex = options.zIndex || 10;
        
        // Animation state
        this.animating = false;
        this.animationProgress = 1;
        
        // Content
        this.content = options.content || '';
        this.onClose = options.onClose || null;
        
        // DOM element (created when rendered)
        this.element = null;
        this.headerEl = null;
        this.contentEl = null;
    }
    
    render() {
        const app = APPS[this.appId] || APPS.terminal;
        
        const win = document.createElement('div');
        win.className = 'desktop-window';
        win.id = `window-${this.id}`;
        win.style.cssText = `
            position: absolute;
            left: ${this.x}px;
            top: ${this.y}px;
            width: ${this.width}px;
            height: ${this.height}px;
            z-index: ${this.zIndex};
            background: rgba(10, 20, 30, 0.95);
            border: 1px solid ${app.color};
            border-radius: 8px;
            box-shadow: 0 4px 20px rgba(0, 0, 0, 0.5);
            display: flex;
            flex-direction: column;
            overflow: hidden;
            transform: scale(0.8);
            opacity: 0;
            transition: transform ${ANIMATION.OPEN}ms ease-out, opacity ${ANIMATION.OPEN}ms ease-out;
        `;
        
        // Header
        const header = document.createElement('div');
        header.className = 'window-header';
        header.style.cssText = `
            display: flex;
            align-items: center;
            padding: 8px 12px;
            background: linear-gradient(180deg, rgba(40, 50, 60, 0.9) 0%, rgba(20, 30, 40, 0.9) 100%);
            border-bottom: 1px solid rgba(255, 255, 255, 0.1);
            cursor: move;
            user-select: none;
        `;
        
        // Window icon
        const icon = document.createElement('span');
        icon.className = 'window-icon';
        icon.textContent = app.icon;
        icon.style.cssText = `font-size: 16px; margin-right: 8px;`;
        
        // Title
        const title = document.createElement('span');
        title.className = 'window-title';
        title.textContent = this.title;
        title.style.cssText = `flex: 1; color: ${app.color}; font-family: monospace; font-size: 13px;`;
        
        // Window controls
        const controls = document.createElement('div');
        controls.className = 'window-controls';
        controls.style.cssText = `display: flex; gap: 8px;`;
        
        // Minimize button
        const minBtn = document.createElement('button');
        minBtn.className = 'win-btn minimize';
        minBtn.innerHTML = '−';
        minBtn.title = 'Minimize';
        minBtn.style.cssText = `
            width: 20px; height: 20px; border: none; border-radius: 50%;
            background: #ffaa00; color: #000; cursor: pointer;
            font-size: 14px; line-height: 18px; text-align: center;
        `;
        
        // Maximize button
        const maxBtn = document.createElement('button');
        maxBtn.className = 'win-btn maximize';
        maxBtn.innerHTML = '□';
        maxBtn.title = 'Maximize';
        maxBtn.style.cssText = `
            width: 20px; height: 20px; border: none; border-radius: 50%;
            background: #00ff88; color: #000; cursor: pointer;
            font-size: 12px; line-height: 18px; text-align: center;
        `;
        
        // Close button
        const closeBtn = document.createElement('button');
        closeBtn.className = 'win-btn close';
        closeBtn.innerHTML = '×';
        closeBtn.title = 'Close';
        closeBtn.style.cssText = `
            width: 20px; height: 20px; border: none; border-radius: 50%;
            background: #ff4444; color: #fff; cursor: pointer;
            font-size: 16px; line-height: 18px; text-align: center;
        `;
        
        controls.appendChild(minBtn);
        controls.appendChild(maxBtn);
        controls.appendChild(closeBtn);
        
        header.appendChild(icon);
        header.appendChild(title);
        header.appendChild(controls);
        
        // Content area
        const content = document.createElement('div');
        content.className = 'window-content';
        content.style.cssText = `
            flex: 1;
            overflow: auto;
            padding: 12px;
            color: #ccc;
            font-family: monospace;
            font-size: 12px;
        `;
        content.innerHTML = this.content;
        
        // Resize handle
        const resizeHandle = document.createElement('div');
        resizeHandle.className = 'resize-handle';
        resizeHandle.style.cssText = `
            position: absolute;
            bottom: 0;
            right: 0;
            width: 16px;
            height: 16px;
            cursor: se-resize;
            background: linear-gradient(135deg, transparent 50%, ${app.color}40 50%);
        `;
        
        win.appendChild(header);
        win.appendChild(content);
        win.appendChild(resizeHandle);
        
        this.element = win;
        this.headerEl = header;
        this.contentEl = content;
        
        // Trigger open animation
        requestAnimationFrame(() => {
            win.style.transform = 'scale(1)';
            win.style.opacity = '1';
        });
        
        return win;
    }
    
    minimize() {
        if (this.state === WINDOW_STATE.MINIMIZED) return;
        
        this.savedX = this.x;
        this.savedY = this.y;
        this.state = WINDOW_STATE.MINIMIZED;
        
        this.element.style.transition = `transform ${ANIMATION.MINIMIZE}ms ease-in, opacity ${ANIMATION.MINIMIZE}ms ease-in`;
        this.element.style.transform = 'scale(0.1) translateY(100vh)';
        this.element.style.opacity = '0';
        
        setTimeout(() => {
            this.element.style.display = 'none';
            this.element.style.transition = '';
        }, ANIMATION.MINIMIZE);
    }
    
    restore() {
        if (this.state === WINDOW_STATE.NORMAL) return;
        
        this.state = WINDOW_STATE.NORMAL;
        this.element.style.display = 'flex';
        this.element.style.transition = `transform ${ANIMATION.RESTORE}ms ease-out, opacity ${ANIMATION.RESTORE}ms ease-out`;
        
        requestAnimationFrame(() => {
            this.element.style.transform = 'scale(1)';
            this.element.style.opacity = '1';
        });
    }
    
    maximize() {
        if (this.state === WINDOW_STATE.MAXIMIZED) {
            // Restore from maximized
            this.state = WINDOW_STATE.NORMAL;
            this.element.style.transition = `all ${ANIMATION.MAXIMIZE}ms ease-out`;
            this.element.style.left = `${this.savedX}px`;
            this.element.style.top = `${this.savedY}px`;
            this.element.style.width = `${this.savedWidth}px`;
            this.element.style.height = `${this.savedHeight}px`;
        } else {
            // Save current position
            this.savedX = this.x;
            this.savedY = this.y;
            this.savedWidth = this.width;
            this.savedHeight = this.height;
            
            // Maximize
            this.state = WINDOW_STATE.MAXIMIZED;
            this.element.style.transition = `all ${ANIMATION.MAXIMIZE}ms ease-out`;
            this.element.style.left = '0px';
            this.element.style.top = '40px';
            this.element.style.width = '100%';
            this.element.style.height = 'calc(100% - 80px)';
        }
    }
    
    close() {
        this.element.style.transition = `transform ${ANIMATION.CLOSE}ms ease-in, opacity ${ANIMATION.CLOSE}ms ease-in`;
        this.element.style.transform = 'scale(0.8)';
        this.element.style.opacity = '0';
        
        setTimeout(() => {
            this.element.remove();
            if (this.onClose) this.onClose();
        }, ANIMATION.CLOSE);
    }
    
    focus() {
        this.element.style.boxShadow = '0 4px 30px rgba(0, 255, 200, 0.3)';
    }
    
    blur() {
        const app = APPS[this.appId] || APPS.terminal;
        this.element.style.boxShadow = '0 4px 20px rgba(0, 0, 0, 0.5)';
    }
    
    setPosition(x, y) {
        this.x = x;
        this.y = y;
        this.element.style.left = `${x}px`;
        this.element.style.top = `${y}px`;
    }
    
    setSize(width, height) {
        this.width = Math.max(DEFAULT_WINDOW.MIN_WIDTH, width);
        this.height = Math.max(DEFAULT_WINDOW.MIN_HEIGHT, height);
        this.element.style.width = `${this.width}px`;
        this.element.style.height = `${this.height}px`;
    }
    
    setContent(html) {
        this.content = html;
        if (this.contentEl) {
            this.contentEl.innerHTML = html;
        }
    }
}

/**
 * Visual Desktop Manager
 */
export class VisualDesktop {
    constructor(container, options = {}) {
        this.container = container;
        this.options = options;
        
        // Window management
        this.windows = new Map();
        this.windowIdCounter = 0;
        this.activeWindowId = null;
        this.topZIndex = 10;
        
        // Desktop state
        this.icons = [];
        this.taskbarItems = new Map();
        
        // Drag state
        this.dragging = null;
        this.dragOffset = { x: 0, y: 0 };
        this.resizing = null;
        
        // Callbacks
        this.onAppLaunch = options.onAppLaunch || null;
        
        // Keyboard shortcuts
        this.shortcuts = new Map();
        this._registerDefaultShortcuts();
    }
    
    init() {
        this._createDesktop();
        this._createTaskbar();
        this._createLauncher();
        this._setupEventListeners();
        this._createDesktopIcons();
        
        console.log('[VisualDesktop] Desktop initialized');
    }
    
    _createDesktop() {
        this.desktopEl = document.createElement('div');
        this.desktopEl.className = 'visual-desktop';
        this.desktopEl.style.cssText = `
            position: absolute;
            top: 0;
            left: 0;
            right: 0;
            bottom: 40px;
            background: linear-gradient(135deg, #0a0a0f 0%, #1a1a2f 50%, #0a0a0f 100%);
            overflow: hidden;
        `;
        
        // Add grid pattern
        const gridPattern = document.createElement('div');
        gridPattern.style.cssText = `
            position: absolute;
            top: 0;
            left: 0;
            right: 0;
            bottom: 0;
            background-image: 
                linear-gradient(rgba(0, 255, 200, 0.03) 1px, transparent 1px),
                linear-gradient(90deg, rgba(0, 255, 200, 0.03) 1px, transparent 1px);
            background-size: 40px 40px;
            pointer-events: none;
        `;
        this.desktopEl.appendChild(gridPattern);
        
        this.container.appendChild(this.desktopEl);
    }
    
    _createTaskbar() {
        this.taskbarEl = document.createElement('div');
        this.taskbarEl.className = 'taskbar';
        this.taskbarEl.style.cssText = `
            position: absolute;
            bottom: 0;
            left: 0;
            right: 0;
            height: 40px;
            background: rgba(10, 20, 30, 0.95);
            border-top: 1px solid rgba(0, 255, 200, 0.3);
            display: flex;
            align-items: center;
            padding: 0 10px;
            z-index: 9999;
        `;
        
        // Start button
        this.startBtn = document.createElement('button');
        this.startBtn.className = 'start-button';
        this.startBtn.innerHTML = '◈';
        this.startBtn.title = 'Application Launcher';
        this.startBtn.style.cssText = `
            width: 36px;
            height: 36px;
            border: 1px solid #00ffcc;
            border-radius: 6px;
            background: linear-gradient(180deg, #1a2a3a 0%, #0a1520 100%);
            color: #00ffcc;
            font-size: 18px;
            cursor: pointer;
            transition: all 0.2s;
        `;
        
        // Window list
        this.windowListEl = document.createElement('div');
        this.windowListEl.className = 'window-list';
        this.windowListEl.style.cssText = `
            display: flex;
            flex: 1;
            margin-left: 10px;
            gap: 4px;
            overflow-x: auto;
        `;
        
        // Clock
        this.clockEl = document.createElement('div');
        this.clockEl.className = 'taskbar-clock';
        this.clockEl.style.cssText = `
            color: #00ffcc;
            font-family: monospace;
            font-size: 12px;
            padding: 0 10px;
        `;
        
        this.taskbarEl.appendChild(this.startBtn);
        this.taskbarEl.appendChild(this.windowListEl);
        this.taskbarEl.appendChild(this.clockEl);
        
        this.container.appendChild(this.taskbarEl);
        
        // Start clock
        this._updateClock();
        setInterval(() => this._updateClock(), 1000);
    }
    
    _createLauncher() {
        this.launcherEl = document.createElement('div');
        this.launcherEl.className = 'app-launcher';
        this.launcherEl.style.cssText = `
            position: absolute;
            bottom: 50px;
            left: 10px;
            width: 280px;
            background: rgba(10, 20, 30, 0.98);
            border: 1px solid rgba(0, 255, 200, 0.3);
            border-radius: 8px;
            padding: 10px;
            display: none;
            z-index: 10000;
            box-shadow: 0 4px 20px rgba(0, 0, 0, 0.5);
        `;
        
        // Search box
        const searchBox = document.createElement('input');
        searchBox.type = 'text';
        searchBox.placeholder = 'Search applications...';
        searchBox.className = 'launcher-search';
        searchBox.style.cssText = `
            width: 100%;
            padding: 8px 12px;
            border: 1px solid rgba(0, 255, 200, 0.3);
            border-radius: 4px;
            background: rgba(0, 0, 0, 0.3);
            color: #00ffcc;
            font-family: monospace;
            font-size: 12px;
            margin-bottom: 10px;
            outline: none;
        `;
        
        // App list
        const appList = document.createElement('div');
        appList.className = 'launcher-app-list';
        appList.style.cssText = `
            display: flex;
            flex-direction: column;
            gap: 4px;
            max-height: 300px;
            overflow-y: auto;
        `;
        
        // Populate apps
        for (const [appId, app] of Object.entries(APPS)) {
            const appItem = document.createElement('div');
            appItem.className = 'launcher-app-item';
            appItem.dataset.appId = appId;
            appItem.style.cssText = `
                display: flex;
                align-items: center;
                padding: 8px 10px;
                border-radius: 4px;
                cursor: pointer;
                transition: background 0.15s;
            `;
            appItem.innerHTML = `
                <span style="font-size: 20px; margin-right: 10px;">${app.icon}</span>
                <span style="color: ${app.color}; font-family: monospace;">${app.name}</span>
            `;
            
            appItem.addEventListener('mouseenter', () => {
                appItem.style.background = 'rgba(0, 255, 200, 0.1)';
            });
            appItem.addEventListener('mouseleave', () => {
                appItem.style.background = 'transparent';
            });
            appItem.addEventListener('click', () => {
                this.launchApp(appId);
                this.toggleLauncher(false);
            });
            
            appList.appendChild(appItem);
        }
        
        this.launcherEl.appendChild(searchBox);
        this.launcherEl.appendChild(appList);
        
        // Search filtering
        searchBox.addEventListener('input', (e) => {
            const query = e.target.value.toLowerCase();
            appList.querySelectorAll('.launcher-app-item').forEach(item => {
                const name = item.textContent.toLowerCase();
                item.style.display = name.includes(query) ? 'flex' : 'none';
            });
        });
        
        this.container.appendChild(this.launcherEl);
    }
    
    _createDesktopIcons() {
        const defaultIcons = [
            { appId: 'terminal', x: 20, y: 20 },
            { appId: 'fileBrowser', x: 20, y: 100 },
            { appId: 'editor', x: 20, y: 180 },
            { appId: 'processManager', x: 20, y: 260 }
        ];
        
        defaultIcons.forEach(icon => {
            this._createDesktopIcon(icon.appId, icon.x, icon.y);
        });
    }
    
    _createDesktopIcon(appId, x, y) {
        const app = APPS[appId];
        if (!app) return;
        
        const icon = document.createElement('div');
        icon.className = 'desktop-icon';
        icon.dataset.appId = appId;
        icon.style.cssText = `
            position: absolute;
            left: ${x}px;
            top: ${y}px;
            width: 70px;
            display: flex;
            flex-direction: column;
            align-items: center;
            padding: 8px;
            border-radius: 6px;
            cursor: pointer;
            transition: background 0.15s;
        `;
        icon.innerHTML = `
            <div style="font-size: 32px; margin-bottom: 4px;">${app.icon}</div>
            <div style="color: ${app.color}; font-family: monospace; font-size: 10px; text-align: center;">${app.name}</div>
        `;
        
        icon.addEventListener('dblclick', () => {
            this.launchApp(appId);
        });
        
        icon.addEventListener('mouseenter', () => {
            icon.style.background = 'rgba(0, 255, 200, 0.1)';
        });
        icon.addEventListener('mouseleave', () => {
            icon.style.background = 'transparent';
        });
        
        this.desktopEl.appendChild(icon);
        this.icons.push(icon);
    }
    
    _setupEventListeners() {
        // Start button
        this.startBtn.addEventListener('click', () => {
            this.toggleLauncher();
        });
        
        // Close launcher when clicking outside
        document.addEventListener('click', (e) => {
            if (!this.launcherEl.contains(e.target) && !this.startBtn.contains(e.target)) {
                this.toggleLauncher(false);
            }
        });
        
        // Desktop click to focus
        this.desktopEl.addEventListener('mousedown', (e) => {
            if (e.target === this.desktopEl || e.target.style.pointerEvents === 'none') {
                this._focusWindow(null);
            }
        });
        
        // Mouse move for dragging/resizing
        document.addEventListener('mousemove', (e) => {
            if (this.dragging) {
                const newX = e.clientX - this.dragOffset.x;
                const newY = Math.max(0, e.clientY - this.dragOffset.y);
                this.dragging.setPosition(newX, newY);
            }
            if (this.resizing) {
                const rect = this.resizing.element.getBoundingClientRect();
                const newWidth = e.clientX - rect.left;
                const newHeight = e.clientY - rect.top;
                this.resizing.setSize(newWidth, newHeight);
            }
        });
        
        // Mouse up to stop drag/resize
        document.addEventListener('mouseup', () => {
            this.dragging = null;
            this.resizing = null;
        });
        
        // Keyboard shortcuts
        document.addEventListener('keydown', (e) => {
            this._handleKeyboard(e);
        });
    }
    
    _registerDefaultShortcuts() {
        // Alt+F4 - Close window
        this.shortcuts.set('Alt+F4', () => {
            if (this.activeWindowId !== null) {
                this.closeWindow(this.activeWindowId);
            }
        });
        
        // Alt+Tab - Next window
        this.shortcuts.set('Alt+Tab', () => {
            this._cycleWindow(1);
        });
        
        // Alt+Shift+Tab - Previous window
        this.shortcuts.set('Alt+Shift+Tab', () => {
            this._cycleWindow(-1);
        });
        
        // Super/Win - Toggle launcher
        this.shortcuts.set('Meta', () => {
            this.toggleLauncher();
        });
        
        // Escape - Close launcher
        this.shortcuts.set('Escape', () => {
            this.toggleLauncher(false);
        });
    }
    
    _handleKeyboard(e) {
        // Build shortcut string
        const parts = [];
        if (e.altKey) parts.push('Alt');
        if (e.ctrlKey) parts.push('Ctrl');
        if (e.shiftKey) parts.push('Shift');
        if (e.metaKey) parts.push('Meta');
        if (!['Alt', 'Control', 'Shift', 'Meta'].includes(e.key)) {
            parts.push(e.key);
        }
        
        const shortcut = parts.join('+');
        
        if (this.shortcuts.has(shortcut)) {
            e.preventDefault();
            this.shortcuts.get(shortcut)();
        }
    }
    
    toggleLauncher(show = null) {
        const shouldShow = show !== null ? show : this.launcherEl.style.display === 'none';
        this.launcherEl.style.display = shouldShow ? 'block' : 'none';
        
        if (shouldShow) {
            const searchBox = this.launcherEl.querySelector('.launcher-search');
            searchBox.value = '';
            searchBox.focus();
            
            // Show all apps
            this.launcherEl.querySelectorAll('.launcher-app-item').forEach(item => {
                item.style.display = 'flex';
            });
        }
    }
    
    launchApp(appId, options = {}) {
        const app = APPS[appId];
        if (!app) {
            console.warn(`[VisualDesktop] Unknown app: ${appId}`);
            return null;
        }
        
        // Check singleton
        if (app.singleton) {
            for (const [winId, win] of this.windows) {
                if (win.appId === appId) {
                    this._focusWindow(winId);
                    return win;
                }
            }
        }
        
        // Create window
        const win = this.createWindow({
            appId,
            title: options.title || app.name,
            content: options.content || this._getAppContent(appId),
            ...options
        });
        
        // Callback
        if (this.onAppLaunch) {
            this.onAppLaunch(appId, win);
        }
        
        console.log(`[VisualDesktop] Launched app: ${app.name}`);
        return win;
    }
    
    _getAppContent(appId) {
        const contents = {
            terminal: `
                <div style="color: #00ff88;">
                    <div>Geometry OS Shell v1.0</div>
                    <div>Type 'help' for commands.</div>
                    <div style="margin-top: 10px;">
                        <span style="color: #00aaff;">geometer@geometry</span>:<span style="color: #ffaa00;">~</span>$ <span id="terminal-input" contenteditable="true" style="outline: none; caret-color: #00ff88;"></span>
                    </div>
                </div>
            `,
            fileBrowser: `
                <div style="display: grid; grid-template-columns: repeat(4, 1fr); gap: 10px;">
                    <div class="file-item" style="text-align: center; padding: 10px;">
                        <div style="font-size: 32px;">📁</div>
                        <div style="font-size: 10px; color: #ffaa00;">home/</div>
                    </div>
                    <div class="file-item" style="text-align: center; padding: 10px;">
                        <div style="font-size: 32px;">📁</div>
                        <div style="font-size: 10px; color: #ffaa00;">bin/</div>
                    </div>
                    <div class="file-item" style="text-align: center; padding: 10px;">
                        <div style="font-size: 32px;">📄</div>
                        <div style="font-size: 10px; color: #00aaff;">readme.txt</div>
                    </div>
                    <div class="file-item" style="text-align: center; padding: 10px;">
                        <div style="font-size: 32px;">⚙</div>
                        <div style="font-size: 10px; color: #ff88ff;">program.spv</div>
                    </div>
                </div>
            `,
            processManager: `
                <div>
                    <div style="display: flex; justify-content: space-between; padding: 4px; background: rgba(0,255,200,0.1); border-radius: 4px; margin-bottom: 8px;">
                        <span>PID</span><span>NAME</span><span>STATE</span><span>CPU</span>
                    </div>
                    <div id="process-list">
                        <div style="display: flex; justify-content: space-between; padding: 4px;">
                            <span style="color: #00aaff;">#1</span>
                            <span>kernel</span>
                            <span style="color: #00ff88;">RUN</span>
                            <span>12%</span>
                        </div>
                        <div style="display: flex; justify-content: space-between; padding: 4px;">
                            <span style="color: #00aaff;">#2</span>
                            <span>shell</span>
                            <span style="color: #ffaa00;">WAIT</span>
                            <span>2%</span>
                        </div>
                    </div>
                </div>
            `,
            editor: `
                <div style="height: 100%; display: flex; flex-direction: column;">
                    <div style="display: flex; gap: 10px; padding: 4px; background: rgba(0,0,0,0.3); border-radius: 4px; margin-bottom: 8px;">
                        <button style="background: #1a2a3a; border: 1px solid #00ffcc; color: #00ffcc; padding: 4px 8px; border-radius: 4px; cursor: pointer;">New</button>
                        <button style="background: #1a2a3a; border: 1px solid #00ffcc; color: #00ffcc; padding: 4px 8px; border-radius: 4px; cursor: pointer;">Save</button>
                        <button style="background: #1a2a3a; border: 1px solid #00ffcc; color: #00ffcc; padding: 4px 8px; border-radius: 4px; cursor: pointer;">Run</button>
                    </div>
                    <textarea style="flex: 1; background: rgba(0,0,0,0.3); border: 1px solid rgba(0,255,200,0.2); border-radius: 4px; color: #00ff88; font-family: monospace; padding: 8px; resize: none; outline: none;" placeholder="// Write your GeoASM code here..."></textarea>
                </div>
            `,
            settings: `
                <div>
                    <h3 style="color: #888; margin: 0 0 10px 0;">System Settings</h3>
                    <div style="margin-bottom: 10px;">
                        <label style="display: flex; align-items: center; gap: 10px;">
                            <span>Dark Mode</span>
                            <input type="checkbox" checked style="accent-color: #00ffcc;">
                        </label>
                    </div>
                    <div style="margin-bottom: 10px;">
                        <label style="display: flex; align-items: center; gap: 10px;">
                            <span>Animations</span>
                            <input type="checkbox" checked style="accent-color: #00ffcc;">
                        </label>
                    </div>
                    <div style="margin-bottom: 10px;">
                        <span>Kernel Priority: </span>
                        <input type="range" min="0" max="100" value="50" style="accent-color: #00ffcc;">
                    </div>
                </div>
            `,
            networkMonitor: `
                <div>
                    <div style="display: flex; justify-content: space-between; margin-bottom: 10px;">
                        <span>Packets In: <span style="color: #00ff88;">1,234</span></span>
                        <span>Packets Out: <span style="color: #ffaa00;">567</span></span>
                    </div>
                    <div style="background: rgba(0,0,0,0.3); padding: 10px; border-radius: 4px; font-size: 10px;">
                        <div style="color: #888;">Active Connections:</div>
                        <div>→ 192.168.1.1:5000 (KERNEL)</div>
                        <div>→ 192.168.1.2:5001 (FS)</div>
                        <div>→ 192.168.1.3:5003 (SHELL)</div>
                    </div>
                </div>
            `,
            memoryBrowser: `
                <div>
                    <div style="background: rgba(0,0,0,0.3); padding: 8px; border-radius: 4px; margin-bottom: 10px;">
                        <div style="display: flex; justify-content: space-between;">
                            <span>Used: <span style="color: #00ff88;">32MB</span></span>
                            <span>Free: <span style="color: #ffaa00;">224MB</span></span>
                        </div>
                        <div style="background: #333; height: 8px; border-radius: 4px; margin-top: 4px;">
                            <div style="background: linear-gradient(90deg, #00ff88, #00aaff); width: 12.5%; height: 100%; border-radius: 4px;"></div>
                        </div>
                    </div>
                    <div style="font-size: 10px; color: #888;">
                        Pages: 65,536 total | 8,192 used
                    </div>
                </div>
            `,
            help: `
                <div style="line-height: 1.6;">
                    <h3 style="color: #ffff88; margin: 0 0 10px 0;">Geometry OS Help</h3>
                    <div style="margin-bottom: 8px;">
                        <strong style="color: #00ff88;">Keyboard Shortcuts:</strong>
                        <div style="margin-left: 10px;">
                            <div>Alt+F4 - Close window</div>
                            <div>Alt+Tab - Cycle windows</div>
                            <div>Win/Meta - Application launcher</div>
                        </div>
                    </div>
                    <div style="margin-bottom: 8px;">
                        <strong style="color: #00ff88;">Mouse:</strong>
                        <div style="margin-left: 10px;">
                            <div>Drag header - Move window</div>
                            <div>Drag corner - Resize window</div>
                            <div>Double-click icon - Launch app</div>
                        </div>
                    </div>
                </div>
            `
        };
        
        return contents[appId] || '<div style="color: #888;">No content</div>';
    }
    
    createWindow(options = {}) {
        const id = ++this.windowIdCounter;
        const win = new DesktopWindow(id, options);
        
        win.onClose = () => {
            this.windows.delete(id);
            this._removeTaskbarItem(id);
            
            if (this.activeWindowId === id) {
                this.activeWindowId = null;
            }
        };
        
        const element = win.render();
        this.desktopEl.appendChild(element);
        
        this.windows.set(id, win);
        this._addTaskbarItem(win);
        this._focusWindow(id);
        
        // Setup window interactions
        this._setupWindowInteractions(win);
        
        return win;
    }
    
    _setupWindowInteractions(win) {
        const element = win.element;
        const header = win.headerEl;
        const resizeHandle = element.querySelector('.resize-handle');
        
        // Focus on click
        element.addEventListener('mousedown', () => {
            this._focusWindow(win.id);
        });
        
        // Drag handling
        header.addEventListener('mousedown', (e) => {
            if (e.target.classList.contains('win-btn')) return;
            
            this.dragging = win;
            this.dragOffset = {
                x: e.clientX - win.x,
                y: e.clientY - win.y
            };
            
            // If maximized, restore first
            if (win.state === WINDOW_STATE.MAXIMIZED) {
                win.maximize(); // Toggle off
            }
        });
        
        // Resize handling
        resizeHandle.addEventListener('mousedown', (e) => {
            e.stopPropagation();
            this.resizing = win;
        });
        
        // Window controls
        const minBtn = element.querySelector('.win-btn.minimize');
        const maxBtn = element.querySelector('.win-btn.maximize');
        const closeBtn = element.querySelector('.win-btn.close');
        
        minBtn.addEventListener('click', () => win.minimize());
        maxBtn.addEventListener('click', () => win.maximize());
        closeBtn.addEventListener('click', () => win.close());
    }
    
    _focusWindow(id) {
        // Blur previous window
        if (this.activeWindowId !== null) {
            const prevWin = this.windows.get(this.activeWindowId);
            if (prevWin) {
                prevWin.blur();
            }
        }
        
        // Focus new window
        if (id !== null) {
            const win = this.windows.get(id);
            if (win) {
                win.focus();
                win.zIndex = ++this.topZIndex;
                win.element.style.zIndex = win.zIndex;
                this.activeWindowId = id;
                
                // Update taskbar
                this._updateTaskbarActive(id);
            }
        } else {
            this.activeWindowId = null;
        }
    }
    
    _cycleWindow(direction) {
        const windowIds = Array.from(this.windows.keys());
        if (windowIds.length === 0) return;
        
        const currentIndex = windowIds.indexOf(this.activeWindowId);
        let newIndex = currentIndex + direction;
        
        if (newIndex < 0) newIndex = windowIds.length - 1;
        if (newIndex >= windowIds.length) newIndex = 0;
        
        this._focusWindow(windowIds[newIndex]);
    }
    
    closeWindow(id) {
        const win = this.windows.get(id);
        if (win) {
            win.close();
        }
    }
    
    _addTaskbarItem(win) {
        const app = APPS[win.appId] || APPS.terminal;
        
        const item = document.createElement('div');
        item.className = 'taskbar-item';
        item.dataset.windowId = win.id;
        item.style.cssText = `
            display: flex;
            align-items: center;
            padding: 4px 12px;
            background: rgba(0, 255, 200, 0.1);
            border: 1px solid ${app.color}40;
            border-radius: 4px;
            cursor: pointer;
            min-width: 100px;
            max-width: 150px;
        `;
        item.innerHTML = `
            <span style="margin-right: 6px;">${app.icon}</span>
            <span style="color: ${app.color}; font-size: 11px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;">${win.title}</span>
        `;
        
        item.addEventListener('click', () => {
            if (win.state === WINDOW_STATE.MINIMIZED) {
                win.restore();
            }
            this._focusWindow(win.id);
        });
        
        this.windowListEl.appendChild(item);
        this.taskbarItems.set(win.id, item);
    }
    
    _removeTaskbarItem(id) {
        const item = this.taskbarItems.get(id);
        if (item) {
            item.remove();
            this.taskbarItems.delete(id);
        }
    }
    
    _updateTaskbarActive(activeId) {
        this.taskbarItems.forEach((item, id) => {
            if (id === activeId) {
                item.style.background = 'rgba(0, 255, 200, 0.2)';
                item.style.borderColor = '#00ffcc';
            } else {
                const win = this.windows.get(id);
                const app = win ? APPS[win.appId] : APPS.terminal;
                item.style.background = 'rgba(0, 255, 200, 0.1)';
                item.style.borderColor = `${app.color}40`;
            }
        });
    }
    
    _updateClock() {
        const now = new Date();
        const hours = now.getHours().toString().padStart(2, '0');
        const minutes = now.getMinutes().toString().padStart(2, '0');
        const seconds = now.getSeconds().toString().padStart(2, '0');
        this.clockEl.textContent = `${hours}:${minutes}:${seconds}`;
    }
    
    /**
     * Register a custom keyboard shortcut
     */
    registerShortcut(key, callback) {
        this.shortcuts.set(key, callback);
    }
    
    /**
     * Register a custom application
     */
    registerApp(appId, config) {
        APPS[appId] = config;
    }
    
    /**
     * Get window by ID
     */
    getWindow(id) {
        return this.windows.get(id);
    }
    
    /**
     * Get all windows
     */
    getAllWindows() {
        return Array.from(this.windows.values());
    }
}

export { DesktopWindow, WINDOW_STATE, ANIMATION, APPS };
