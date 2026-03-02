/**
 * Geometry OS Process Control Block (PCB)
 * 
 * Represents a single visual program with isolated memory and lifecycle state.
 */
export class Process {
    constructor(pid, name, options = {}) {
        this.pid = pid;
        this.name = name;
        this.status = 'idle'; // idle, running, waiting, exit
        this.priority = options.priority || 5;
        
        // Memory metrics
        this.cycles = 0;
        this.pc = 0;
        this.sp = 0;
        
        // Configuration
        this.memBase = options.memBase || 0;
        this.memLimit = options.memLimit || 1024;
    }

    /**
     * Update process state from GPU PCB data
     * @param {Object} gpuState - Data read back from GPU pcb_table
     */
    update(gpuState) {
        this.pc = gpuState.pc;
        this.sp = gpuState.sp;
        this.status = gpuState.status;
        this.cycles = gpuState.cycles || 0;
    }

    /**
     * Is the process currently runnable?
     */
    isRunnable() {
        return this.status === 'running' || this.status === 'idle';
    }
}
