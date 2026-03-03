/**
 * Geometry OS Syscall Manager
 *
 * Handles system calls from GPU processes to browser I/O.
 * Syncs mouse/keyboard state to shared memory before each kernel step.
 */

export const IO = {
    MOUSE_X: 50,
    MOUSE_Y: 51,
    MOUSE_BTN: 52,
    KEY_CODE: 53,
    KEY_STATE: 54,
    SYSCALL_ID: 100,
    SYSCALL_ARG1: 101,
    SYSCALL_ARG2: 102,
    SYSCALL_ARG3: 103,
    SYSCALL_RESULT: 104,
    SYSCALL_STATUS: 105
};

export const SYS = {
    GET_MOUSE: 1,
    GET_KEY: 2,
    WRITE_LOG: 3,
    GET_TIME: 4
};

export class SyscallManager {
    constructor(kernel) {
        this.kernel = kernel;
        this.mouseX = 0;
        this.mouseY = 0;
        this.mouseButtons = 0;
        this.keys = new Map();
        this.canvas = null;

        this._onMouseMove = this._handleMouseMove.bind(this);
        this._onMouseDown = this._handleMouseDown.bind(this);
        this._onMouseUp = this._handleMouseUp.bind(this);
        this._onKeyDown = this._handleKeyDown.bind(this);
        this._onKeyUp = this._handleKeyUp.bind(this);
    }

    attach(canvas) {
        this.canvas = canvas;
        canvas.addEventListener('mousemove', this._onMouseMove);
        canvas.addEventListener('mousedown', this._onMouseDown);
        canvas.addEventListener('mouseup', this._onMouseUp);
        window.addEventListener('keydown', this._onKeyDown);
        window.addEventListener('keyup', this._onKeyUp);
        console.log('[SyscallManager] I/O listeners attached');
    }

    detach() {
        if (this.canvas) {
            this.canvas.removeEventListener('mousemove', this._onMouseMove);
            this.canvas.removeEventListener('mousedown', this._onMouseDown);
            this.canvas.removeEventListener('mouseup', this._onMouseUp);
        }
        window.removeEventListener('keydown', this._onKeyDown);
        window.removeEventListener('keyup', this._onKeyUp);
    }

    _handleMouseMove(e) {
        const rect = this.canvas.getBoundingClientRect();
        this.mouseX = e.clientX - rect.left;
        this.mouseY = e.clientY - rect.top;
    }

    _handleMouseDown(e) {
        this.mouseButtons |= (1 << e.button);
    }

    _handleMouseUp(e) {
        this.mouseButtons &= ~(1 << e.button);
    }

    _handleKeyDown(e) {
        this.keys.set(e.keyCode, 1);
    }

    _handleKeyUp(e) {
        this.keys.set(e.keyCode, 0);
    }

    async syncToGPU() {
        const ioData = new Float32Array([
            this.mouseX,
            this.mouseY,
            this.mouseButtons,
            0,
            0
        ]);
        this.kernel.writeSharedMemory(IO.MOUSE_X, ioData);
    }

    async processSyscalls() {
        const syscallData = await this.kernel.readSharedMemory(IO.SYSCALL_ID, 6);
        const status = syscallData[5];

        if (status === 0) {
            const syscallId = syscallData[0];
            const result = await this._executeSyscall(syscallId, syscallData);
            const resultBuf = new Float32Array([result, 1]);
            this.kernel.writeSharedMemory(IO.SYSCALL_RESULT, resultBuf);
        }
    }

    async _executeSyscall(id, args) {
        switch (id) {
            case SYS.GET_MOUSE:
                return (Math.floor(this.mouseX) << 16) | Math.floor(this.mouseY);
            case SYS.GET_KEY:
                const keyCode = args[1];
                return this.keys.get(keyCode) || 0;
            case SYS.WRITE_LOG:
                const value = args[1];
                console.log(`[GPU LOG] ${value}`);
                return 0;
            case SYS.GET_TIME:
                return Date.now() / 1000;
            default:
                console.warn(`[SyscallManager] Unknown syscall: ${id}`);
                return -1;
        }
    }
}
