use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";

    let (mut vm, fw_addr, _entry, _dtb_addr) = geometry_os::riscv::RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        512,
        bootargs,
    )
    .unwrap();

    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let max_instructions: u64 = 178_000u64;

    // Watch for writes to page table area (0x00802000-0x00803000)
    let mut pt_writes: Vec<(u64, u32)> = Vec::new();
    let pt_area_start: u64 = 0x00800000;
    let pt_area_end: u64 = 0x00810000;

    while count < max_instructions {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        if vm.cpu.pc == fw_addr_u32
            && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine
        {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            let mpp = (vm.cpu.csr.mstatus & 0x3000) >> 12;

            if cause_code == 11 || cause_code == 9 {
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
                if let Some((a0_val, a1_val)) = result {
                    vm.cpu.x[10] = a0_val;
                    vm.cpu.x[11] = a1_val;
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
                    count += 1;
                    continue;
                }
            }

            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        // Check SATP changes
        let cur_satap = vm.cpu.csr.satp;
        if cur_satap != 0 && count < 177270 {
            // About to enable MMU - dump PT area
            eprintln!(
                "[diag] SATAP becoming non-zero at count={}: 0x{:08X}",
                count, cur_satap
            );
            // Check what's at 0x00802000
            let mut nonzero_in_pt = 0;
            for addr in (0x00802000u64..0x00804000).step_by(4) {
                if let Ok(val) = vm.bus.read_word(addr) {
                    if val != 0 {
                        nonzero_in_pt += 1;
                        if nonzero_in_pt <= 20 {
                            eprintln!("  0x{:08X}: 0x{:08X}", addr, val);
                        }
                    }
                }
            }
            eprintln!(
                "[diag] Non-zero words in early_pg_dir (0x802000-0x803000): {}",
                nonzero_in_pt
            );

            // Also check trampoline area
            let mut nonzero_in_tramp = 0;
            for addr in (0x01484000u64..0x01486000).step_by(4) {
                if let Ok(val) = vm.bus.read_word(addr) {
                    if val != 0 {
                        nonzero_in_tramp += 1;
                        if nonzero_in_tramp <= 20 {
                            eprintln!("  0x{:08X}: 0x{:08X}", addr, val);
                        }
                    }
                }
            }
            eprintln!(
                "[diag] Non-zero words in trampoline_pg_dir (0x1484000-0x1485000): {}",
                nonzero_in_tramp
            );
            break;
        }

        let step_result = vm.step();
        if matches!(step_result, geometry_os::riscv::cpu::StepResult::Ebreak) {
            break;
        }

        count += 1;
    }

    if count >= 177270 {
        eprintln!(
            "[diag] Reached max count without SATAP change. PC=0x{:08X}",
            vm.cpu.pc
        );
    }
}
