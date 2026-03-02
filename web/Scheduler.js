/**
 * Round-Robin Scheduler for Geometry OS processes.
 * 
 * Manages CPU-side lifecycle, priority boosting, and starvation prevention.
 */
export class Scheduler {
    constructor(options = {}) {
        this.quantum = options.quantum || 100;
        this.maxProcesses = options.maxProcesses || 16;
    }

    /**
     * Boost priorities for processes to prevent starvation.
     * This logic will eventually be synced back to the GPU's pcb_table.
     * @param {Map<number, Process>} processes 
     */
    boostPriorities(processes) {
        for (const process of processes.values()) {
            if (process.priority > 1 && process.status !== 'exit') {
                // Lower priority value means higher priority (standard linux-style)
                // If it's 1-10 scale, boost by decreasing the value
                process.priority = Math.max(1, process.priority - 1);
            }
        }
    }

    /**
     * Get summary of current system load
     * @param {Map<number, Process>} processes 
     */
    getSystemLoad(processes) {
        const stats = {
            total: processes.size,
            running: 0,
            waiting: 0,
            exit: 0,
            idle: 0
        };

        for (const p of processes.values()) {
            if (stats.hasOwnProperty(p.status)) {
                stats[p.status]++;
            }
        }
        return stats;
    }

    /**
     * Filter processes that have exited for cleanup
     * @param {Map<number, Process>} processes 
     */
    getTerminated(processes) {
        return Array.from(processes.values()).filter(p => p.status === 'exit');
    }
}
