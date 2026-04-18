/// Trace instructions between setup_vm completion and first S-mode fault.
/// Goal: understand why the kernel tries to execute at VA 0x3FFFF000.
use geometry_os::riscv::{cpu::StepResult, RiscvVm};

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=uart8250,mmio,0x10000000 panic=1";

    let (mut vm, fw_addr, _entry, _dtb) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();

    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let max_count: u64 = 200_000;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut smode_fault_count: u64 = 0;
    let mut sbi_count: u64 = 0;
    let mut forward_count: u64 = 0;

    // Phase 1: Run until first S-mode fault after count 177000
    let mut tracing = false;
    let mut trace_buf: Vec<String> = Vec::new();

    while count < max_count {
        // SATP change handling (same as boot_linux)
        {
            let cur_satp = vm.cpu.csr.satp;
            if cur_satp != last_satp {
                let mode = (cur_satp >> 31) & 1;
                if mode == 1 {
                    let ppn = cur_satp & 0x3FFFFF;
                    let pg_dir_phys = (ppn as u64) * 4096;
                    let l1_0_val = vm.bus.read_word(pg_dir_phys).unwrap_or(0);
                    let already_patched =
                        (l1_0_val & 0xCF) == 0xCF && ((l1_0_val >> 20) & 0xFFF) == 0;
                    if !already_patched {
                        let l1_entries: &[u32] = &[
                            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 16, 32, 48, 64, 80, 96, 112, 127,
                        ];
                        for &l1_idx in l1_entries {
                            let pte = 0xCF | (l1_idx << 20);
                            vm.bus
                                .write_word(pg_dir_phys + (l1_idx * 4) as u64, pte)
                                .ok();
                        }
                        vm.cpu.tlb.flush_all();
                    }
                }
                last_satp = cur_satp;
            }
        }

        // M-mode trap handler forwarding (same as boot_linux)
        if vm.cpu.pc == fw_addr_u32
            && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine
        {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code != 11 {
                let mpp = (vm.cpu.csr.mstatus & 0x1800) >> 11;
                if cause_code == 9 {
                    sbi_count += 1;
                    let result = vm.bus.sbi.handle_ecall(
                        vm.cpu.x[17],
                        vm.cpu.x[16],
                        vm.cpu.x[10],
                        vm.cpu.x[11],
                        vm.cpu.x[12],
                        vm.cpu.x[13],
                        vm.cpu.x[14],
                        vm.cpu.x[15],
                        &mut vm.bus.uart,
                        &mut vm.bus.clint,
                    );
                    if let Some((a0, a1)) = result {
                        vm.cpu.x[10] = a0;
                        vm.cpu.x[11] = a1;
                    }
                } else if mpp != 3 {
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
                        vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
                        vm.cpu.tlb.flush_all();
                        forward_count += 1;
                        count += 1;
                        continue;
                    }
                }
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            } else {
                // ECALL_M = SBI
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17],
                    vm.cpu.x[16],
                    vm.cpu.x[10],
                    vm.cpu.x[11],
                    vm.cpu.x[12],
                    vm.cpu.x[13],
                    vm.cpu.x[14],
                    vm.cpu.x[15],
                    &mut vm.bus.uart,
                    &mut vm.bus.clint,
                );
                if let Some((a0, a1)) = result {
                    vm.cpu.x[10] = a0;
                    vm.cpu.x[11] = a1;
                }
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            }
        }

        // Start tracing when we approach setup_vm completion
        if count >= 176000 && !tracing {
            tracing = true;
            trace_buf.clear();
        }

        // Record trace entry every 100 instructions during tracing
        if tracing && count % 100 == 0 {
            trace_buf.push(format!(
                "count={:6} PC=0x{:08X} ra=0x{:08X} sp=0x{:08X} a0=0x{:08X} satp=0x{:08X}",
                count, vm.cpu.pc, vm.cpu.x[1], vm.cpu.x[2], vm.cpu.x[10], vm.cpu.csr.satp
            ));
        }

        let step_result = vm.step();
        match step_result {
            StepResult::Ok => {}
            StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault => {
                if vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Supervisor {
                    smode_fault_count += 1;
                    if smode_fault_count <= 5 {
                        let fault_type = match step_result {
                            StepResult::FetchFault => "fetch",
                            StepResult::LoadFault => "load",
                            StepResult::StoreFault => "store",
                            _ => unreachable!(),
                        };
                        // Dump register state at first fault
                        println!(
                            "=== FIRST S-mode {} fault #{} at count={} ===",
                            fault_type, smode_fault_count, count
                        );
                        println!(
                            "  PC=0x{:08X} sepc=0x{:08X} stval=0x{:08X} stvec=0x{:08X}",
                            vm.cpu.pc, vm.cpu.csr.sepc, vm.cpu.csr.stval, vm.cpu.csr.stvec
                        );
                        println!(
                            "  scause=0x{:08X} satp=0x{:08X}",
                            vm.cpu.csr.scause, vm.cpu.csr.satp
                        );
                        println!(
                            "  RA=0x{:08X} SP=0x{:08X} GP=0x{:08X} TP=0x{:08X}",
                            vm.cpu.x[1], vm.cpu.x[2], vm.cpu.x[3], vm.cpu.x[4]
                        );
                        println!(
                            "  A0=0x{:08X} A1=0x{:08X} A2=0x{:08X} A3=0x{:08X} A4=0x{:08X}",
                            vm.cpu.x[10], vm.cpu.x[11], vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14]
                        );
                        println!(
                            "  A5=0x{:08X} A6=0x{:08X} A7=0x{:08X}",
                            vm.cpu.x[15], vm.cpu.x[16], vm.cpu.x[17]
                        );
                        println!(
                            "  S0=0x{:08X} S1=0x{:08X} T0=0x{:08X} T1=0x{:08X} T2=0x{:08X}",
                            vm.cpu.x[8], vm.cpu.x[9], vm.cpu.x[5], vm.cpu.x[6], vm.cpu.x[7]
                        );

                        // What's at the faulting address?
                        let sepc = vm.cpu.csr.sepc;
                        let instr = vm.bus.read_word(sepc as u64).unwrap_or(0);
                        println!("  Instruction at sepc: 0x{:08X}", instr);

                        // Check if 0x3FFFF000 is in any page table
                        let satp = vm.cpu.csr.satp;
                        let pg_dir_phys = ((satp & 0x3FFFFF) as u64) * 4096;
                        let vpn1 = (sepc >> 22) & 0x3FF;
                        println!("  sepc VPN1={} page_dir_phys=0x{:08X}", vpn1, pg_dir_phys);
                        let l1_pte = vm
                            .bus
                            .read_word(pg_dir_phys + (vpn1 as u64) * 4)
                            .unwrap_or(0);
                        println!(
                            "  L1[{}] = 0x{:08X} (V={} R={} W={} X={})",
                            vpn1,
                            l1_pte,
                            l1_pte & 1,
                            (l1_pte >> 1) & 1,
                            (l1_pte >> 2) & 1,
                            (l1_pte >> 3) & 1
                        );

                        // Print last 20 trace entries
                        println!("\n--- Last 20 trace entries ---");
                        for entry in trace_buf.iter().rev().take(20) {
                            println!("  {}", entry);
                        }
                    }
                    if smode_fault_count == 3 {
                        break; // Stop after 3 faults
                    }
                }
            }
            StepResult::Ebreak => break,
            StepResult::Ecall => {}
        }
        count += 1;
    }

    println!("\n=== Summary ===");
    println!("Instructions: {}", count);
    println!("S-mode faults: {}", smode_fault_count);
    println!("SBI calls: {}", sbi_count);
    println!("M->S forwards: {}", forward_count);
    println!("UART output: {} bytes", vm.bus.uart.tx_buf.len());
    if !vm.bus.uart.tx_buf.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.uart.tx_buf);
        let preview: String = s.chars().take(2000).collect();
        println!("UART:\n{}", preview);
    }
}
