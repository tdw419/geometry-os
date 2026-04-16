/// Log the last 50 steps and the first 20 steps after SATP change.
use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::Privilege;
use geometry_os::riscv::decode;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    let (mut vm, fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();

    let max = 500_000u64;
    let mut count: u64 = 0;
    let fw_addr_u32 = fw_addr as u32;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut post_satp_steps: Vec<String> = Vec::new();
    let mut collecting_post_satp = false;
    let mut last_steps: Vec<String> = Vec::new();

    while count < max {
        if vm.bus.sbi.shutdown_requested { break; }

        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code == 11 {
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16],
                    vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13],
                    vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0, a1)) = result {
                    vm.cpu.x[10] = a0;
                    vm.cpu.x[11] = a1;
                }
            } else {
                let mpp = (vm.cpu.csr.mstatus & 0x1800) >> 11;
                if mpp != 3 {
                    let stvec = vm.cpu.csr.stvec & !0x3u32;
                    if stvec != 0 {
                        vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                        vm.cpu.csr.scause = mcause;
                        vm.cpu.csr.stval = vm.cpu.csr.mtval;
                        let spp = if mpp == 1 { 1u32 } else { 0u32 };
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 5)) | (spp << 5);
                        let sie = (vm.cpu.csr.mstatus >> 1) & 1;
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 5)) | (sie << 5);
                        vm.cpu.csr.mstatus &= !(1 << 1);
                        vm.cpu.pc = stvec;
                        vm.cpu.privilege = Privilege::Supervisor;
                        vm.cpu.tlb.flush_all();
                        count += 1;
                        continue;
                    }
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        vm.step();

        // Record step info from last_step
        if let Some(ref ls) = vm.cpu.last_step {
            let desc = format!("{}: PC=0x{:08X} word=0x{:08X} len={} {:?} => PC=0x{:08X}",
                count, ls.pc, ls.word, ls.inst_len, ls.op, ls.pc_after);
            
            // Keep last 50 steps
            if last_steps.len() >= 50 {
                last_steps.remove(0);
            }
            last_steps.push(desc);
        }

        // Collect 30 steps after last SATP change
        if vm.cpu.csr.satp != last_satp {
            println!("[trace] SATP: 0x{:08X} -> 0x{:08X} at count={}", last_satp, vm.cpu.csr.satp, count);
            last_satp = vm.cpu.csr.satp;
            collecting_post_satp = true;
            post_satp_steps.clear();
        }
        if collecting_post_satp && post_satp_steps.len() < 30 {
            if let Some(ref ls) = vm.cpu.last_step {
                let desc = format!("  {} PC=0x{:08X} word=0x{:08X} len={} {:?} => PC=0x{:08X}",
                    count, ls.pc, ls.word, ls.inst_len, ls.op, ls.pc_after);
                post_satp_steps.push(desc);
            }
        }
        if post_satp_steps.len() >= 30 {
            collecting_post_satp = false;
        }

        count += 1;
    }

    println!("\n[trace] Done: count={} ecall_count={}", count, vm.cpu.ecall_count);
    println!("[trace] UART tx_buf: {} chars", vm.bus.uart.tx_buf.len());
    println!("[trace] SBI console: {} chars", vm.bus.sbi.console_output.len());

    if !post_satp_steps.is_empty() {
        println!("\n[trace] Steps after final SATP change:");
        for s in &post_satp_steps {
            println!("{}", s);
        }
    }

    println!("\n[trace] Last 50 steps:");
    for s in &last_steps {
        println!("{}", s);
    }

    if !vm.bus.sbi.console_output.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.sbi.console_output);
        let preview: String = s.chars().take(500).collect();
        println!("\n[trace] SBI console output:\n{}", preview);
    }
    if !vm.bus.uart.tx_buf.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.uart.tx_buf);
        let preview: String = s.chars().take(500).collect();
        println!("\n[trace] UART output:\n{}", preview);
    }
}
