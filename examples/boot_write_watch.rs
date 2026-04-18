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
    let mut in_setup_vm = false;
    let mut pt_write_count = 0;

    // Track writes to 0x802000 area
    let mut writes_to_pt: Vec<(u64, u64, u32)> = Vec::new(); // (count, addr, val)

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

        // Track entry into setup_vm
        if vm.cpu.pc == 0x10EA && !in_setup_vm {
            in_setup_vm = true;
            eprintln!("[diag] Entering setup_vm at count={}", count);
            // Set write watchpoint for page table area
            vm.bus.write_watch_addr = Some(0x00800000);
        }

        // Check write watchpoint
        if in_setup_vm && vm.bus.write_watch_hit {
            pt_write_count += 1;
            writes_to_pt.push((
                count,
                vm.bus.write_watch_addr.unwrap(),
                vm.bus.write_watch_val,
            ));
            if pt_write_count <= 5 {
                eprintln!(
                    "[diag] PT WRITE #{} at count={}: addr=0x{:08X} val=0x{:08X}",
                    pt_write_count,
                    count,
                    vm.bus.write_watch_addr.unwrap(),
                    vm.bus.write_watch_val
                );
            }
            vm.bus.write_watch_hit = false;
            // Reset watchpoint to catch next write
            vm.bus.write_watch_addr = Some(0x00800000);
        }

        // Check if we've returned from setup_vm
        if in_setup_vm && vm.cpu.pc == 0x10EE {
            eprintln!(
                "[diag] Returned from setup_vm at count={}, PT writes={}",
                count, pt_write_count
            );
            // Show all writes
            for (c, a, v) in &writes_to_pt {
                eprintln!("  count={} addr=0x{:08X} val=0x{:08X}", c, a, v);
            }
            break;
        }

        let step_result = vm.step();
        if matches!(step_result, geometry_os::riscv::cpu::StepResult::Ebreak) {
            break;
        }
        count += 1;
    }
}
