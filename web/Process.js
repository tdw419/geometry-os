/**
 * Geometry OS Process Control Block (PCB)
 * 
 * Represents a single visual program with isolated memory and lifecycle state.
 */
export class Process {
    constructor(pid, name, options = {}) {
        this.pid = pid;
        this.name = name;
        this.status = 'idle'; // idle, running, waiting, ready, exit, error
        this.staticPriority = options.priority || 20; // Default priority (Linux-style)
        this.dynamicPriority = this.staticPriority;
        
        // Performance metrics
        this.totalCycles = 0;
        this.faultCount = 0;
        this.pc = 0;
        this.sp = 0;
        this.lastRunTimestamp = 0;
        
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
        this.totalCycles = gpuState.totalCycles || 0;
        this.faultCount = gpuState.faultCount || 0;
        this.dynamicPriority = gpuState.dynamicPriority || this.staticPriority;
        this.lastRunTimestamp = gpuState.lastRunTimestamp || 0;
    }

    /**
     * Is the process currently runnable?
     */
    isRunnable() {
        return this.status === 'running' ||
               this.status === 'idle' ||
               this.status === 'ready';
    }
}
