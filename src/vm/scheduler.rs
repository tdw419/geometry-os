use super::types::*;
use super::Vm;

impl Vm {
    /// Run preemptive scheduler for all child processes.
    /// Each process gets a time slice proportional to its priority level.
    /// Sleeping processes (sleep_until > sched_tick) are skipped.
    /// Yielded processes lose their remaining slice.
    /// When a process's slice is exhausted, it waits until ALL runnable
    /// processes have also exhausted their slices (a new round), then
    /// everyone gets a fresh allocation based on current priority.
    pub fn step_all_processes(&mut self) {
        self.sched_tick += 1;

        let mut procs = std::mem::take(&mut self.processes);

        let saved_pc = self.pc;
        let saved_regs = self.regs;
        let saved_halted = self.halted;
        let saved_mode = self.mode;
        let saved_kernel_stack = std::mem::take(&mut self.kernel_stack);
        let saved_page_dir = self.current_page_dir.take();
        let saved_vmas = std::mem::take(&mut self.current_vmas);
        let saved_segfault = self.segfault;
        let saved_segfault_pid = self.segfault_pid;
        let saved_current_pid = self.current_pid;

        // Check if all runnable (non-halted, non-sleeping) processes have
        // exhausted their slices. If so, start a new scheduling round.
        let all_exhausted = procs.iter().all(|p| {
            p.is_halted()
                || (p.sleep_until > 0 && self.sched_tick < p.sleep_until)
                || p.slice_remaining == 0
        });
        if all_exhausted {
            for proc in &mut procs {
                if proc.is_halted() {
                    continue;
                }
                if proc.sleep_until > 0 && self.sched_tick < proc.sleep_until {
                    continue;
                }
                let multiplier = 1u32 << proc.priority;
                proc.slice_remaining = self.default_time_slice * multiplier;
                proc.yielded = false;
            }
        }

        // Sort by priority descending (highest priority runs first)
        let mut indices: Vec<usize> = (0..procs.len()).collect();
        indices.sort_by_key(|&i| std::cmp::Reverse(procs[i].priority));

        for idx in indices {
            let proc = &mut procs[idx];
            if proc.is_halted() {
                continue;
            }

            // Skip blocked processes (waiting for pipe data or message)
            if proc.state == ProcessState::Blocked {
                continue;
            }

            // Skip sleeping processes whose sleep hasnt expired
            if proc.sleep_until > 0 && self.sched_tick < proc.sleep_until {
                continue;
            }
            // Wake up: clear sleep flag
            if proc.sleep_until > 0 && self.sched_tick >= proc.sleep_until {
                proc.sleep_until = 0;
                proc.slice_remaining = 0;
            }

            // Skip processes whose time slice is exhausted (wait for next round)
            if proc.slice_remaining == 0 {
                continue;
            }

            self.pc = proc.pc;
            self.regs = proc.regs;
            self.halted = false;
            self.mode = proc.mode;
            self.kernel_stack.clear();
            self.current_page_dir = proc.page_dir.take();
            self.current_vmas = std::mem::take(&mut proc.vmas);
            self.segfault = false;
            self.current_pid = proc.pid;

            // Reset per-step scheduler flags
            self.yielded = false;
            self.sleep_frames = 0;
            self.new_priority = proc.priority;
            self.step_exit_code = None;
            self.step_zombie = false;

            // Execute one instruction within the time slice
            let still_running = self.step();
            self.sched_tick += 1;

            // Save process state back
            proc.pc = self.pc;
            proc.regs = self.regs;
            proc.state = if !still_running || self.halted || self.segfault {
                ProcessState::Zombie
            } else {
                ProcessState::Ready
            };
            proc.mode = self.mode;
            proc.page_dir = self.current_page_dir.take();
            proc.vmas = std::mem::take(&mut self.current_vmas);
            proc.segfaulted = self.segfault;
            // Propagate EXIT opcode's exit code and zombie status
            if let Some(code) = self.step_exit_code {
                proc.exit_code = code;
            }
            if self.step_zombie {
                proc.state = ProcessState::Zombie;
            }
            if self.segfault {
                self.segfault_pid = proc.pid;
                self.ram[0xFF9] = proc.pid;
            }

            // Apply SETPRIORITY if requested
            if self.new_priority != proc.priority && self.new_priority <= 3 {
                proc.priority = self.new_priority;
            }

            // Handle YIELD: forfeit remaining time slice
            if self.yielded {
                proc.slice_remaining = 0;
                proc.yielded = true;
            } else if proc.slice_remaining > 0 {
                proc.slice_remaining -= 1;
            }

            // Handle SLEEP: mark process as sleeping
            if self.sleep_frames > 0 {
                proc.sleep_until = self.sched_tick.wrapping_add(self.sleep_frames as u64);
                proc.slice_remaining = 0;
            }
        }

        self.pc = saved_pc;
        self.regs = saved_regs;
        self.halted = saved_halted;
        self.mode = saved_mode;
        self.kernel_stack = saved_kernel_stack;
        self.current_page_dir = saved_page_dir;
        self.current_vmas = saved_vmas;
        self.segfault = saved_segfault;
        self.segfault_pid = saved_segfault_pid;
        self.current_pid = saved_current_pid;
        self.yielded = false;
        self.sleep_frames = 0;
        self.new_priority = 0;
        self.step_exit_code = None;
        self.step_zombie = false;

        procs.extend(std::mem::take(&mut self.processes));
        self.processes = procs;
    }

    /// Count active (non-halted) child processes
    #[allow(dead_code)]
    pub fn active_process_count(&self) -> usize {
        self.processes.iter().filter(|p| !p.is_halted()).count()
    }
}
