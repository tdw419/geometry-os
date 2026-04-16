/// Trace every instruction between count 178300 and the first S-mode fault.
/// Goal: find the exact instruction that sets RA to 0x3FFFF000.
use geometry_os::riscv::{RiscvVm, cpu::StepResult, cpu::Privilege};

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
    let mut tracing = false;
    let mut smode_faults: u64 = 0;

    while count < max_count {
        // SATP change handling
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

        // M-mode trap handler forwarding
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code != 11 {
                let mpp = (vm.cpu.csr.mstatus & 0x1800) >> 11;
                if cause_code == 9 {
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
                        vm.cpu.privilege = Privilege::Supervisor;
                        vm.cpu.tlb.flush_all();
                        count += 1;
                        continue;
                    }
                }
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            } else {
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
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            }
        }

        // Start tracing at count 178300
        if count >= 178300 && !tracing {
            tracing = true;
            println!("=== Starting per-instruction trace at count={} ===", count);
        }

        if tracing {
            // Log every instruction
            println!(
                "{:6} PC=0x{:08X} RA=0x{:08X} SP=0x{:08X} A0=0x{:08X} A3=0x{:08X} A7=0x{:08X}",
                count, vm.cpu.pc, vm.cpu.x[1], vm.cpu.x[2], vm.cpu.x[10], vm.cpu.x[13], vm.cpu.x[17]
            );
        }

        let step_result = vm.step();
        match step_result {
            StepResult::Ok => {}
            StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault => {
                if vm.cpu.privilege == Privilege::Supervisor {
                    smode_faults += 1;
                    println!("\n=== S-mode fault #{} at count={} ===", smode_faults, count);
                    println!("  PC=0x{:08X} sepc=0x{:08X} stval=0x{:08X} stvec=0x{:08X}",
                        vm.cpu.pc, vm.cpu.csr.sepc, vm.cpu.csr.stval, vm.cpu.csr.stvec);
                    println!("  scause=0x{:08X} satp=0x{:08X}", vm.cpu.csr.scause, vm.cpu.csr.satp);
                    println!("  RA=0x{:08X} SP=0x{:08X} A0-A7: {:08X} {:08X} {:08X} {:08X} {:08X} {:08X} {:08X} {:08X}",
                        vm.cpu.x[1], vm.cpu.x[2],
                        vm.cpu.x[10], vm.cpu.x[11], vm.cpu.x[12], vm.cpu.x[13],
                        vm.cpu.x[14], vm.cpu.x[15], vm.cpu.x[16], vm.cpu.x[17]);
                    if smode_faults >= 2 { break; }
                }
            }
            StepResult::Ebreak => break,
            StepResult::Ecall => {}
        }
        count += 1;
    }
    println!("\n=== Done at count={}, faults={} ===", count, smode_faults);
}
