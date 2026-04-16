use super::Vm;
use super::types::*;

impl Vm {
    /// Boot the OS: load init.asm as PID 1, create boot.cfg if missing.
    /// Returns Ok(pid) on success, Err(msg) on failure.
    /// After booting, the VM is in kernel mode with the init process running
    /// as the first child process. The host should call step_all_processes().
    pub fn boot(&mut self) -> Result<u32, String> {
        if self.booted {
            return Err("already booted".into());
        }

        // Ensure boot.cfg exists in the VFS
        self.ensure_boot_config();

        // Assemble and load init.asm as PID 1
        let init_path = std::path::Path::new("programs/init.asm");
        let source = match std::fs::read_to_string(init_path) {
            Ok(s) => s,
            Err(e) => return Err(format!("cannot read init.asm: {}", e)),
        };

        let asm_result = match crate::assembler::assemble(&source, 0) {
            Ok(r) => r,
            Err(e) => return Err(format!("init.asm assembly error: {}", e)),
        };

        let page_dir = match self.create_process_page_dir() {
            Some(pd) => pd,
            None => return Err("no memory for init process".into()),
        };

        let phys_base = (page_dir[0] as usize) * PAGE_SIZE;
        for (i, &word) in asm_result.pixels.iter().enumerate() {
            let addr = phys_base + i;
            if addr >= self.ram.len() {
                break;
            }
            self.ram[addr] = word;
        }

        let pid = (self.processes.len() + 1) as u32;
        self.processes.push(SpawnedProcess {
            pc: 0,
            regs: [0; NUM_REGS],
            state: ProcessState::Ready,
            pid,
            mode: CpuMode::User,
            page_dir: Some(page_dir),
            segfaulted: false,
            priority: 2, // init gets higher priority than normal processes
            slice_remaining: 0,
            sleep_until: 0,
            yielded: false,
            kernel_stack: Vec::new(),
            msg_queue: Vec::new(),
            exit_code: 0,
            parent_pid: 0, // init has no parent
            pending_signals: Vec::new(),
            signal_handlers: [0; 4],
            vmas: Process::default_vmas_for_process(),
            brk_pos: PAGE_SIZE as u32,
        });

        // Set default environment
        self.env_vars
            .insert("SHELL".into(), "shell".into());
        self.env_vars.insert("HOME".into(), "/".into());
        self.env_vars.insert("CWD".into(), "/".into());
        self.env_vars.insert("USER".into(), "root".into());

        self.booted = true;
        Ok(pid)
    }

    /// Create default boot.cfg in the VFS if it doesn't exist.
    /// Format: one directive per line, "key=value".
    /// Keys: init, shell, services (comma-separated program names).
    pub(super) fn ensure_boot_config(&mut self) {
        // Check if boot.cfg exists by trying to open it
        let boot_cfg_path = self.vfs.base_dir.join("boot.cfg");
        if !boot_cfg_path.exists() {
            let default_cfg = "init=init\nshell=shell\nservices=\n";
            let _ = std::fs::write(&boot_cfg_path, default_cfg);
        }
    }

    /// Read a configuration value from boot.cfg in the VFS.
    /// Returns the value for the given key, or None if not found.
    #[allow(dead_code)]
    pub fn read_boot_config(&self, key: &str) -> Option<String> {
        let boot_cfg_path = self.vfs.base_dir.join("boot.cfg");
        let content = std::fs::read_to_string(&boot_cfg_path).ok()?;
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                if k.trim() == key {
                    return Some(v.trim().to_string());
                }
            }
        }
        None
    }
}
