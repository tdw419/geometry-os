/**
 * GPU-Native Health Watchdog (System Service)
 * 
 * This shader runs as PID 1 (System Service) and monitors all processes
 * in the Geometry OS kernel. It detects:
 * - Zombie processes (terminated but not cleaned)
 * - Hog processes (exceeding cycle quotas)
 * - Deadlock conditions
 * - Memory leaks
 * 
 * The watchdog can issue SYS_KILL or SYS_SIGNAL to remediate issues.
 * 
 * This moves core OS policy from the host (JavaScript) to the GPU substrate.
 */

// Process states (must match kernel.wgsl)
const PROC_IDLE: u32 = 0u;
const PROC_RUNNING: u32 = 1u;
const PROC_WAITING: u32 = 2u;
const PROC_EXIT: u32 = 3u;
const PROC_ERROR: u32 = 4u;

// Watchdog commands
const WDOG_IDLE: u32 = 0u;
const WDOG_SCAN: u32 = 1u;
const WDOG_REMEDIATE: u32 = 2u;
const WDOG_GET_STATS: u32 = 3u;

// Remediation actions
const ACTION_NONE: u32 = 0u;
const ACTION_WARN: u32 = 1u;
const ACTION_KILL: u32 = 2u;
const ACTION_RESTART: u32 = 3u;
const ACTION_SIGNAL: u32 = 4u;

// Signal types
const SIGTERM: u32 = 15u;
const SIGKILL: u32 = 9u;
const SIGSEGV: u32 = 11u;

// Configuration
const MAX_PROCESSES: u32 = 256u;
const CYCLE_QUOTA: u32 = 1000000u;     // Max cycles per time slice
const ZOMBIE_THRESHOLD: u32 = 1000u;    // Cycles before zombie detection
const MEMORY_LEAK_THRESHOLD: u32 = 1024u * 1024u;  // 1MB growth

// PCB structure (must match kernel.wgsl)
struct PCB {
    pid: u32,
    ppid: u32,
    state: u32,
    priority: u32,
    pc: u32,
    sp: u32,
    mem_base: u32,
    mem_size: u32,
    total_cycles: u32,
    fault_count: u32,
    signal_handler: u32,
    exit_code: u32,
    _reserved: array<u32, 4u>,
}

// Watchdog statistics
struct WatchdogStats {
    total_scans: u32,
    zombies_found: u32,
    hogs_found: u32,
    deadlocks_found: u32,
    processes_killed: u32,
    processes_restarted: u32,
    last_scan_time: u32,
    _padding: u32,
}

// Process health record
struct HealthRecord {
    pid: u32,
    health_score: u32,      // 0-100, lower is worse
    cycles_last_scan: u32,
    memory_last_scan: u32,
    issue_type: u32,        // Type of issue detected
    issue_count: u32,       // Number of times issue detected
    recommended_action: u32,
    _padding: array<u32, 2u>,
}

// Control block for watchdog
struct WatchdogControl {
    command: u32,
    interval_ms: u32,       // Scan interval
    auto_remediate: u32,    // 0=manual, 1=auto
    cycle_quota: u32,
    status: u32,
    active_processes: u32,
    unhealthy_processes: u32,
    _padding: array<u32, 2u>,
}

// Bindings
@group(0) @binding(0) var<storage, read> pcb_table: array<PCB>;
@group(0) @binding(1) var<storage, read_write> health_records: array<HealthRecord>;
@group(0) @binding(2) var<storage, read_write> stats: WatchdogStats;
@group(0) @binding(3) var<storage, read_write> control: WatchdogControl;
@group(0) @binding(4) var<storage, read_write> action_queue: array<u32>;  // Pending actions

// Calculate health score for a process
fn calculate_health_score(pcb: PCB, record: ptr<function, HealthRecord>) -> u32 {
    var score: u32 = 100u;
    
    // Check for zombie state
    if (pcb.state == PROC_EXIT && pcb.total_cycles > ZOMBIE_THRESHOLD) {
        score -= 50u;
        (*record).issue_type = 1u;  // Zombie
    }
    
    // Check for cycle hog
    if (pcb.total_cycles > (*record).cycles_last_scan + CYCLE_QUOTA) {
        score -= 30u;
        (*record).issue_type = 2u;  // Hog
    }
    
    // Check for errors
    if (pcb.fault_count > 0u) {
        score -= pcb.fault_count * 5u;
        if ((*record).issue_type == 0u) {
            (*record).issue_type = 3u;  // Errors
        }
    }
    
    // Check for memory growth (potential leak)
    if (pcb.mem_size > (*record).memory_last_scan + MEMORY_LEAK_THRESHOLD) {
        score -= 20u;
        (*record).issue_type = 4u;  // Memory leak
    }
    
    return max(0u, score);
}

// Determine recommended action
fn determine_action(score: u32, issue_type: u32, issue_count: u32) -> u32 {
    if (score < 20u) {
        return ACTION_KILL;
    }
    if (score < 40u) {
        if (issue_count > 3u) {
            return ACTION_KILL;
        }
        return ACTION_SIGNAL;  // Send warning signal
    }
    if (score < 60u) {
        return ACTION_WARN;
    }
    return ACTION_NONE;
}

// Check for deadlocks (processes waiting on each other)
fn detect_deadlock(pid: u32, pcb_table: array<PCB>) -> bool {
    // Simplified deadlock detection
    // Check if process is waiting and the process it's waiting for is also waiting
    let pcb = pcb_table[pid];
    
    if (pcb.state != PROC_WAITING) {
        return false;
    }
    
    // Check parent chain for circular wait
    var current = pcb.ppid;
    for (var i: u32 = 0u; i < 10u; i++) {
        if (current == 0u || current >= MAX_PROCESSES) {
            break;
        }
        
        let parent = pcb_table[current];
        if (parent.state == PROC_WAITING && parent.ppid == pid) {
            return true;  // Circular wait detected
        }
        current = parent.ppid;
    }
    
    return false;
}

// Main watchdog kernel
@compute @workgroup_size(1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let tid = global_id.x;
    
    // Only first thread runs watchdog
    if (tid != 0u) {
        return;
    }
    
    // Check command
    if (control.command == WDOG_IDLE) {
        return;
    }
    
    // Update scan count
    stats.total_scans++;
    
    var unhealthy_count: u32 = 0u;
    var active_count: u32 = 0u;
    var action_idx: u32 = 0u;
    
    // Scan all processes
    for (var pid: u32 = 0u; pid < MAX_PROCESSES; pid++) {
        let pcb = pcb_table[pid];
        
        // Skip idle/empty slots
        if (pcb.state == PROC_IDLE && pcb.pid == 0u) {
            continue;
        }
        
        active_count++;
        
        // Get or initialize health record
        var record = health_records[pid];
        if (record.pid != pid) {
            // Initialize new record
            record.pid = pid;
            record.health_score = 100u;
            record.cycles_last_scan = 0u;
            record.memory_last_scan = 0u;
            record.issue_type = 0u;
            record.issue_count = 0u;
            record.recommended_action = ACTION_NONE;
        }
        
        // Calculate health score
        let score = calculate_health_score(pcb, &record);
        record.health_score = score;
        
        // Track issues
        if (score < 100u) {
            record.issue_count++;
            unhealthy_count++;
            
            // Classify issue
            switch (record.issue_type) {
                case 1u: { stats.zombies_found++; }
                case 2u: { stats.hogs_found++; }
                case 3u: { /* errors counted elsewhere */ }
                case 4u: { /* memory leak */ }
                default: { }
            }
        } else {
            record.issue_count = 0u;
        }
        
        // Check for deadlock
        if (detect_deadlock(pid, pcb_table)) {
            stats.deadlocks_found++;
            record.health_score = max(0u, record.health_score - 40u);
            record.issue_type = 5u;  // Deadlock
            unhealthy_count++;
        }
        
        // Determine action
        record.recommended_action = determine_action(
            record.health_score,
            record.issue_type,
            record.issue_count
        );
        
        // Update record
        record.cycles_last_scan = pcb.total_cycles;
        record.memory_last_scan = pcb.mem_size;
        health_records[pid] = record;
        
        // Queue action if auto-remediate is enabled
        if (control.auto_remediate == 1u && record.recommended_action != ACTION_NONE) {
            // Action format: [action_type, pid, signal/exit_code, _]
            let action_base = action_idx * 4u;
            action_queue[action_base + 0u] = record.recommended_action;
            action_queue[action_base + 1u] = pid;
            action_queue[action_base + 2u] = if (record.recommended_action == ACTION_SIGNAL) { 
                SIGTERM 
            } else if (record.recommended_action == ACTION_KILL) { 
                SIGKILL 
            else { 
                0u 
            };
            action_queue[action_base + 3u] = 0u;  // Reserved
            
            if (record.recommended_action == ACTION_KILL) {
                stats.processes_killed++;
            }
            
            action_idx++;
        }
    }
    
    // Update control stats
    control.active_processes = active_count;
    control.unhealthy_processes = unhealthy_count;
    control.status = if (unhealthy_count > 0u) { 2u } else { 1u };  // 1=healthy, 2=issues
    
    // Mark end of action queue
    action_queue[action_idx * 4u] = 0xFFFFFFFFu;  // End marker
    
    // Clear command
    control.command = WDOG_IDLE;
}
