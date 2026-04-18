// Targeted diagnostic: capture register state around the pcpu_block_update_hint_alloc crash
use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::{StepResult, Privilege};
use geometry_os::riscv::csr;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, fw_addr, _entry, _dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        256,
        "console=ttyS0 earlycon=sbi panic=1",
    )
    .unwrap();

    // Enter S-mode
    let entry = _entry as u32;
    vm.cpu.csr.mepc = entry;
    vm.cpu.csr.mstatus = 1u32 << csr::MSTATUS_MPP_LSB;
    vm.cpu.csr.mstatus |= 1 << csr::MSTATUS_MPIE;
    let restored = vm.cpu.csr.trap_return(Privilege::Machine);
    vm.cpu.pc = vm.cpu.csr.mepc;
    vm.cpu.privilege = restored;

    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let max_instructions: u64 = 20_000_000;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut trap_count: u64 = 0;

    // Watch for the crash region: 14.5M - 15.5M
    let watch_start: u64 = 14_500_000;
    let watch_end: u64 = 15_500_000;
    let mut in_watch_zone = false;
    let mut last_pcs: Vec<u32> = Vec::new();

    while count < max_instructions {
        let prev_pc = vm.cpu.pc;
        let step_result = vm.step();

        // Track PCs in the watch zone
        if count >= watch_start && count < watch_end {
            if !in_watch_zone {
                in_watch_zone = true;
                eprintln!("[watch] Entered watch zone at count={}", count);
            }
            last_pcs.push(prev_pc);
            if last_pcs.len() > 200 {
                last_pcs.remove(0);
            }
        }

        // Check for M-mode trap
        if vm.cpu.pc == fw_addr_u32 {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = (mcause & 0xFF) as u32;
            let mpp = (vm.cpu.csr.mstatus >> csr::MSTATUS_MPP_LSB) & 0x3;

            trap_count += 1;

            if cause_code == 11 {
                // ECALL_M = SBI call
                let a7 = vm.cpu.x[17];
                let a6 = vm.cpu.x[16];
                let a0 = vm.cpu.x[10];
                if trap_count <= 5 {
                    eprintln!(
                        "[watch] SBI ECALL at count={}: a7={:#x} a6={:#x} a0={:#x}",
                        count, a7, a6, a0
                    );
                }
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            } else if mpp != 3 {
                // Forward to S-mode
                let stvec = vm.cpu.csr.stvec & !0x3u32;
                if stvec != 0 && cause_code == 9 {
                    // ECALL_S = SBI call
                    eprintln!(
                        "[watch] ECALL_S at count={}: a7={:#x} a6={:#x} a0={:#x}",
                        count, vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10]
                    );
                    vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
                } else if stvec != 0 {
                    vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                    vm.cpu.csr.scause = mcause;
                    vm.cpu.csr.stval = vm.cpu.csr.mtval;
                    let spp = if mpp == 1 { 1u32 } else { 0u32 };
                    vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << csr::MSTATUS_SPP))
                        | (spp << csr::MSTATUS_SPP);
                    let sie = (vm.cpu.csr.mstatus >> csr::MSTATUS_SIE) & 1;
                    vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << csr::MSTATUS_SPIE))
                        | (sie << csr::MSTATUS_SPIE);
                    vm.cpu.csr.mstatus &= !(1 << csr::MSTATUS_SIE);
                    if cause_code == csr::INT_MTI {
                        vm.bus.clint.mtimecmp = vm.bus.clint.mtime + 100_000;
                    }
                    vm.cpu.pc = stvec;
                    vm.cpu.privilege = Privilege::Supervisor;
                    vm.cpu.tlb.flush_all();
                    count += 1;
                    continue;
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        // Detect the crash: S-mode load fault
        if in_watch_zone {
            if matches!(step_result, StepResult::LoadFault | StepResult::FetchFault | StepResult::StoreFault) {
                if vm.cpu.privilege == Privilege::Supervisor {
                    eprintln!("\n[CRASH] S-mode fault at count={}", count);
                    eprintln!("  PC (fault handler): 0x{:08X}", vm.cpu.pc);
                    eprintln!("  Previous PC: 0x{:08X}", prev_pc);
                    eprintln!("  Privilege: {:?}", vm.cpu.privilege);
                    eprintln!("  scause: 0x{:08X}", vm.cpu.csr.scause);
                    eprintln!("  sepc: 0x{:08X}", vm.cpu.csr.sepc);
                    eprintln!("  stval: 0x{:08X}", vm.cpu.csr.stval);
                    eprintln!("  SATP: 0x{:08X}", vm.cpu.csr.satp);
                    eprintln!("\n  Register dump:");
                    let names = [
                        "zero", "ra", "sp", "gp", "tp",
                        "t0", "t1", "t2", "s0", "s1",
                        "a0", "a1", "a2", "a3", "a4", "a5",
                        "a6", "a7", "s2", "s3", "s4", "s5",
                        "s6", "s7", "t3", "t4", "t5", "t6",
                        "t7", "??", "??", "??",
                    ];
                    for i in 0..32 {
                        eprintln!("    x{:2} ({:>4}): 0x{:08X}", i, names[i], vm.cpu.x[i]);
                    }
                    eprintln!("\n  Last 20 PCs before crash:");
                    for pc in last_pcs.iter().rev().take(20) {
                        eprintln!("    0x{:08X}", pc);
                    }
                    eprintln!("\n  UART output: {} chars", vm.bus.uart.tx_buf.len());
                    if !vm.bus.uart.tx_buf.is_empty() {
                        let s = String::from_utf8_lossy(&vm.bus.uart.tx_buf);
                        let preview: String = s.chars().take(500).collect();
                        eprintln!("UART:\n{}", preview);
                    }
                    break;
                }
            }
        }

        vm.bus.tick_clint();

        // Check for SATP changes
        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            eprintln!("[watch] SATP changed: 0x{:08X} -> 0x{:08X} at count={}", last_satp, cur_satp, count);
            last_satp = cur_satp;
        }

        count += 1;

        // Progress in watch zone
        if in_watch_zone && count % 100_000 == 0 {
            eprintln!("[watch] count={} PC=0x{:08X}", count, vm.cpu.pc);
        }
    }

    eprintln!("\nDone: {} instructions, {} traps", count, trap_count);
    eprintln!("UART: {} chars", vm.bus.uart.tx_buf.len());
}
