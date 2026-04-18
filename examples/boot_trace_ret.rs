/// Focused diagnostic: trace the ret-to-zero transition and what happens after.
use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";

    use geometry_os::riscv::cpu::{Privilege, StepResult};
    use geometry_os::riscv::csr;
    use geometry_os::riscv::RiscvVm;

    let (mut vm, fw_addr, _entry, _dtb) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();
    let fw_addr_u32 = fw_addr as u32;

    let max_count = 300_000u64;
    let mut count: u64 = 0;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut sbi_count: u64 = 0;
    let mut forward_count: u64 = 0;
    let mut smode_faults: u64 = 0;
    let mut entered_kernel_va = false;
    let mut returned_to_zero = false;
    let mut trace_mode = false; // Enable detailed tracing after return to zero
    let mut trace_count: u64 = 0;

    while count < max_count {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        // Track SATP changes + inject identity mappings
        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            eprintln!(
                "[{}] SATP: 0x{:08X} -> 0x{:08X} PC=0x{:08X}",
                count, last_satp, cur_satp, vm.cpu.pc
            );
            last_satp = cur_satp;
            let mode = (cur_satp >> 31) & 1;
            if mode == 1 {
                let ppn = cur_satp & 0x3FFFFF;
                let pg_dir_phys = (ppn as u64) * 4096;
                let identity_pte: u32 = 0x0000_00CF;
                let l1_entries: &[u32] = &[
                    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 16, 32, 48, 64, 80, 96, 112, 127,
                ];
                let l1_0 = vm.bus.read_word(pg_dir_phys).unwrap_or(0);
                if (l1_0 & 0xCF) != 0xCF || ((l1_0 >> 20) & 0xFFF) != 0 {
                    for &l1_idx in l1_entries {
                        let pte = identity_pte | (l1_idx << 20);
                        vm.bus
                            .write_word(pg_dir_phys + (l1_idx * 4) as u64, pte)
                            .ok();
                    }
                    vm.cpu.tlb.flush_all();
                    eprintln!("[{}] Injected identity at PA 0x{:08X}", count, pg_dir_phys);
                }
            }
        }

        // M-mode trap handler
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code == csr::CAUSE_ECALL_M || cause_code == csr::CAUSE_ECALL_S {
                sbi_count += 1;
                let r = vm.bus.sbi.handle_ecall(
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
                if let Some((a0, a1)) = r {
                    vm.cpu.x[10] = a0;
                    vm.cpu.x[11] = a1;
                }
                eprintln!(
                    "[{}] SBI call #{}: ext=0x{:02X} fn={}",
                    count, sbi_count, vm.cpu.x[17], vm.cpu.x[16]
                );
            } else {
                let mpp = (vm.cpu.csr.mstatus & csr::MSTATUS_MPP_MASK) >> csr::MSTATUS_MPP_LSB;
                if mpp != 3 && cause_code != 0 {
                    let stvec = vm.cpu.csr.stvec & !0x3u32;
                    if stvec != 0 {
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
                        vm.cpu.pc = stvec;
                        vm.cpu.privilege = Privilege::Supervisor;
                        vm.cpu.tlb.flush_all();
                        forward_count += 1;
                        eprintln!(
                            "[{}] FWD: cause={} mepc=0x{:08X} stvec=0x{:08X}",
                            count, cause_code, vm.cpu.csr.sepc, stvec
                        );
                        count += 1;
                        continue;
                    }
                }
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            }
            count += 1;
            continue;
        }

        // Detect entry to kernel VA space
        if !entered_kernel_va && vm.cpu.pc >= 0xC0000000 {
            entered_kernel_va = true;
            eprintln!(
                "[{}] *** ENTERED KERNEL VA SPACE: PC=0x{:08X} ***",
                count, vm.cpu.pc
            );
        }

        // Detect return to zero
        if entered_kernel_va && !returned_to_zero && vm.cpu.pc < 0x100 {
            returned_to_zero = true;
            trace_mode = true;
            eprintln!(
                "[{}] *** RETURNED TO LOW ADDR: PC=0x{:08X} RA=0x{:08X} SP=0x{:08X} ***",
                count, vm.cpu.pc, vm.cpu.x[1], vm.cpu.x[2]
            );
            eprintln!(
                "[{}]   satp=0x{:08X} stvec=0x{:08X} sscratch=0x{:08X}",
                count, vm.cpu.csr.satp, vm.cpu.csr.stvec, vm.cpu.csr.sscratch
            );
        }

        // Detailed trace after return to zero (first 200 instructions)
        if trace_mode && trace_count < 200 {
            let inst = vm.bus.read_word(vm.cpu.pc as u64).unwrap_or(0);
            // Also read as 16-bit
            let inst16 = vm.bus.read_half(vm.cpu.pc as u64).unwrap_or(0);
            eprintln!("[{} T{}] PC=0x{:08X} inst32=0x{:08X} inst16=0x{:04X} priv={:?} ra=0x{:08X} sp=0x{:08X}",
                count, trace_count, vm.cpu.pc, inst, inst16, vm.cpu.privilege, vm.cpu.x[1], vm.cpu.x[2]);
            trace_count += 1;
        }

        let result = vm.step();

        // Check for UART
        let prev_uart = vm.bus.sbi.console_output.len();
        let result2 = (); // already stepped
        if vm.bus.sbi.console_output.len() > prev_uart {
            let s = String::from_utf8_lossy(&vm.bus.sbi.console_output[prev_uart..]);
            eprintln!("[{}] UART: {:?}", count, s);
        }

        match result {
            StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault => {
                if vm.cpu.privilege == Privilege::Supervisor {
                    smode_faults += 1;
                    if smode_faults <= 50 {
                        let ft = match result {
                            StepResult::FetchFault => "fetch",
                            StepResult::LoadFault => "load",
                            StepResult::StoreFault => "store",
                            _ => "",
                        };
                        eprintln!(
                            "[{}] S-mode {} fault: PC=0x{:08X} sepc=0x{:08X} stval=0x{:08X}",
                            count, ft, vm.cpu.pc, vm.cpu.csr.sepc, vm.cpu.csr.stval
                        );
                    }
                }
            }
            StepResult::Ebreak => {
                eprintln!("[{}] EBREAK", count);
                break;
            }
            _ => {}
        }

        count += 1;
    }

    eprintln!("\n=== DONE at {} ===", count);
    eprintln!(
        "PC=0x{:08X} priv={:?} satp=0x{:08X}",
        vm.cpu.pc, vm.cpu.privilege, vm.cpu.csr.satp
    );
}
