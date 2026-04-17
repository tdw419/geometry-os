use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    // Use boot_linux_setup to get the VM without running, then we control the loop
    let (mut vm, _fw_addr, _entry, _dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        256,
        "console=ttyS0 loglevel=8",
    ).unwrap();

    let mut count: u64 = 0;
    let max = 180_000u64;
    let trace_start = 177_000u64;
    
    // Capture last N instructions as ring buffer
    let buf_size = 200;
    let mut trace_buf: Vec<(u64, u32, u8, u32, [u32; 32])> = Vec::new();
    let mut fault_seen = false;

    while count < max && !fault_seen {
        let pc = vm.cpu.pc;
        let priv_level = match vm.cpu.privilege {
            geometry_os::riscv::cpu::Privilege::Machine => 0u8,
            geometry_os::riscv::cpu::Privilege::Supervisor => 1u8,
            geometry_os::riscv::cpu::Privilege::User => 2u8,
        };
        
        // Save state before step
        let saved_pc = pc;
        let saved_priv = priv_level;
        let saved_satp = vm.cpu.csr.satp;
        let saved_regs = vm.cpu.x;
        
        let result = vm.step();
        count += 1;
        
        if count >= trace_start {
            // Log instruction
            if let Some(inst) = vm.bus.read_word(saved_pc as u64).ok() {
                if trace_buf.len() >= buf_size {
                    trace_buf.remove(0);
                }
                trace_buf.push((count, saved_pc, saved_priv, saved_satp, saved_regs));
            }
        }
        
        // Check if we landed at a fault handler
        if count > 177_500 {
            let scause = vm.cpu.csr.scause;
            let sepc = vm.cpu.csr.sepc;
            if (scause & 0x1F) == 12 && sepc >= 0x3FFF0000 {
                println!("FAULT detected at count={}: scause=0x{:08X} sepc=0x{:08X}", 
                    count, scause, sepc);
                fault_seen = true;
            }
        }
        
        // Log SATP changes
        let cur_satp = vm.cpu.csr.satp;
        if count == 1 || cur_satp != saved_satp {
            println!("[{}] SATP=0x{:08X} PC=0x{:08X} priv={}", count, cur_satp, saved_pc, saved_priv);
        }
    }
    
    // Dump the trace
    println!("\n=== Last {} instructions before fault ===", trace_buf.len());
    for (c, pc, priv_lvl, satp, regs) in &trace_buf {
        let priv_name = match priv_lvl {
            0 => "M",
            1 => "S",
            2 => "U",
            _ => "?",
        };
        println!("[{}] PC=0x{:08X} {} SATP=0x{:08X} ra=0x{:08X} sp=0x{:08X}", 
            c, pc, priv_name, satp, regs[1], regs[2]);
    }
}
