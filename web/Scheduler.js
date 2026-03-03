/**
 * Priority Scheduler for Geometry OS processes.
 *
 * Implements priority-based scheduling with:
 * - Priority boost for starving processes (aging)
 * - Priority decay for GPU-heavy processes
 * - Linux-style nice values (lower = higher priority)
 * - Dynamic priority adjustment based on CPU usage
 */

// Priority constants (lower value = higher priority)
export const PRIORITY = {
    MIN: 1,      // Highest priority (real-time)
    MAX: 40,     // Lowest priority (background)
    DEFAULT: 20, // Default priority (middle)
    RT_BASE: 0,  // Real-time range: 0-99 (reserved)
};

// Decay/boost parameters
const AGING_THRESHOLD = 5;      // Ticks before aging boost kicks in
const AGING_BOOST = 2;          // Priority boost amount per aging tick
const DECAY_THRESHOLD = 1000;   // Cycles before decay kicks in
const DECAY_AMOUNT = 1;         // Priority decay amount
const CPU_USAGE_WINDOW = 10;    // Window size for CPU usage calculation

export class Scheduler {
    constructor(options = {}) {
        this.quantum = options.quantum || 100;
        this.maxProcesses = options.maxProcesses || 16;
        this.tickCount = 0;

        // Track last execution time for aging
        this.lastRunTick = new Map(); // pid -> tick

        // CPU usage history for each process
        this.cpuHistory = new Map(); // pid -> [cycles per tick]
    }

    /**
     * Get the next process to run based on priority.
     * Lower priority value = higher scheduling priority.
     * @param {Array<Process>} processes
     * @returns {Process|null} Next process to run, or null if none runnable
     */
    getNextProcess(processes) {
        const runnable = processes
            .filter(p => p && this.isRunnable(p));

        if (runnable.length === 0) {
            return null;
        }

        // Sort by priority (ascending - lower is better)
        // Tie-breaker: oldest lastRunTick (starving processes first)
        runnable.sort((a, b) => {
            if (a.dynamicPriority !== b.dynamicPriority) {
                return a.dynamicPriority - b.dynamicPriority;
            }
            const aLastRun = this.lastRunTick.get(a.pid) || 0;
            const bLastRun = this.lastRunTick.get(b.pid) || 0;
            return aLastRun - bLastRun; // Older = earlier in queue
        });

        const next = runnable[0];
        this.lastRunTick.set(next.pid, this.tickCount);
        return next;
    }

    /**
     * Check if a process is runnable.
     * @param {Process} process
     */
    isRunnable(process) {
        return process.status === 'running' ||
               process.status === 'idle' ||
               process.status === 'ready';
    }

    /**
     * Called each scheduler tick to update priorities.
     * Implements aging (boost starving) and decay (penalize heavy).
     * @param {Array<Process>} processes
     */
    tick(processes) {
        this.tickCount++;

        for (const process of processes) {
            if (!process || process.status === 'exit' || process.status === 'error') {
                continue;
            }

            // Update CPU usage history
            this._updateCpuHistory(process);

            // Apply aging boost for starving processes
            this._applyAging(process);

            // Apply decay for CPU-heavy processes
            this._applyDecay(process);
        }

        // Normalize priorities to prevent runaway values
        this._normalizePriorities(processes);
    }

    /**
     * Update CPU usage history for a process.
     * @param {Process} process
     */
    _updateCpuHistory(process) {
        if (!this.cpuHistory.has(process.pid)) {
            this.cpuHistory.set(process.pid, []);
        }

        const history = this.cpuHistory.get(process.pid);
        history.push(process.totalCycles);

        // Keep only recent history
        if (history.length > CPU_USAGE_WINDOW) {
            history.shift();
        }
    }

    /**
     * Calculate average CPU usage for a process.
     * @param {number} pid
     * @returns {number} Cycles per tick average
     */
    getCpuUsage(pid) {
        const history = this.cpuHistory.get(pid);
        if (!history || history.length < 2) {
            return 0;
        }
        // Calculate delta cycles
        const delta = history[history.length - 1] - history[0];
        return delta / (history.length - 1);
    }

    /**
     * Apply aging boost to processes that haven't run in a while.
     * This prevents starvation.
     * @param {Process} process
     */
    _applyAging(process) {
        const lastRun = this.lastRunTick.get(process.pid) || 0;
        const waitingTicks = this.tickCount - lastRun;

        if (waitingTicks >= AGING_THRESHOLD && this.isRunnable(process)) {
            // Boost priority (decrease value)
            process.dynamicPriority = Math.max(
                PRIORITY.MIN,
                process.dynamicPriority - AGING_BOOST
            );
            // Reset timer to prevent continuous boosting
            this.lastRunTick.set(process.pid, this.tickCount - AGING_THRESHOLD + 1);
        }
    }

    /**
     * Apply decay to processes using too much CPU.
     * This prevents CPU hogs from monopolizing the system.
     * @param {Process} process
     */
    _applyDecay(process) {
        const avgCpu = this.getCpuUsage(process.pid);

        if (avgCpu > DECAY_THRESHOLD) {
            // Decay priority (increase value)
            process.dynamicPriority = Math.min(
                PRIORITY.MAX,
                process.dynamicPriority + DECAY_AMOUNT
            );
        }
    }

    /**
     * Normalize priorities to keep them in reasonable range.
     * Prevents extreme values from accumulating over time.
     * @param {Array<Process>} processes
     */
    _normalizePriorities(processes) {
        // Calculate average priority
        const priorities = processes
            .filter(p => p && p.status !== 'exit')
            .map(p => p.dynamicPriority);

        if (priorities.length === 0) return;

        const avgPriority = priorities.reduce((a, b) => a + b, 0) / priorities.length;

        // Gradually pull extreme values toward average
        for (const process of processes) {
            if (!process || process.status === 'exit') continue;

            const diff = process.dynamicPriority - avgPriority;
            if (Math.abs(diff) > 10) {
                // Move 10% toward average
                process.dynamicPriority = Math.round(process.dynamicPriority - diff * 0.1);
                // Clamp to valid range
                process.dynamicPriority = Math.max(PRIORITY.MIN, Math.min(PRIORITY.MAX, process.dynamicPriority));
            }
        }
    }

    /**
     * Boost priorities for all processes (legacy API).
     * @param {Map<number, Process>} processes
     */
    boostPriorities(processes) {
        for (const process of processes.values()) {
            if (process.priority > PRIORITY.MIN && process.status !== 'exit') {
                process.priority = Math.max(PRIORITY.MIN, process.priority - 1);
            }
        }
    }

    /**
     * Set a process's nice value (priority adjustment).
     * Positive = lower priority, Negative = higher priority.
     * @param {Process} process
     * @param {number} nice - Value from -20 to +19
     */
    setNice(process, nice) {
        // Clamp nice to valid range
        nice = Math.max(-20, Math.min(19, nice));
        // Convert nice to priority (20 + nice)
        process.staticPriority = PRIORITY.DEFAULT + nice;
        process.staticPriority = Math.max(PRIORITY.MIN, Math.min(PRIORITY.MAX, process.staticPriority));
        process.dynamicPriority = process.staticPriority;
    }

    /**
     * Get system load summary.
     * @param {Array<Process>} processes
     */
    getSystemLoad(processes) {
        const stats = {
            total: processes.filter(p => p).length,
            running: 0,
            waiting: 0,
            exit: 0,
            idle: 0,
            error: 0,
            avgCpu: 0,
            tick: this.tickCount
        };

        let totalCpu = 0;
        let activeCount = 0;

        for (const p of processes) {
            if (!p) continue;
            if (stats.hasOwnProperty(p.status)) {
                stats[p.status]++;
            }
            if (p.status !== 'exit' && p.status !== 'error') {
                totalCpu += this.getCpuUsage(p.pid);
                activeCount++;
            }
        }

        stats.avgCpu = activeCount > 0 ? totalCpu / activeCount : 0;
        return stats;
    }

    /**
     * Get processes that have terminated.
     * @param {Array<Process>} processes
     */
    getTerminated(processes) {
        return processes.filter(p => p && p.status === 'exit');
    }

    /**
     * Get scheduling diagnostics for a process.
     * @param {Process} process
     */
    getDiagnostics(process) {
        return {
            pid: process.pid,
            name: process.name,
            status: process.status,
            priority: process.dynamicPriority,
            nice: process.staticPriority - PRIORITY.DEFAULT,
            cpuUsage: this.getCpuUsage(process.pid).toFixed(2),
            lastRun: this.lastRunTick.get(process.pid) || 0,
            waitingTicks: this.tickCount - (this.lastRunTick.get(process.pid) || 0)
        };
    }

    /**
     * Get ordered list of processes by scheduling priority.
     * @param {Array<Process>} processes
     */
    getRunQueue(processes) {
        return processes
            .filter(p => p && this.isRunnable(p))
            .sort((a, b) => a.dynamicPriority - b.dynamicPriority)
            .map(p => ({
                pid: p.pid,
                name: p.name,
                priority: p.dynamicPriority,
                status: p.status
            }));
    }
}
