/// Diagnostic: verify RAM read/write at page table addresses.
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 panic=1";

    let (mut vm, _fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();

    // Test 1: Write and read back at page table addresses
    println!("=== RAM read/write test ===");

    // Write a test value at trampoline_pg_dir
    let test_addr: u64 = 0x01484000;
    vm.bus.write_word(test_addr, 0xDEADBEEF).ok();
    let readback = vm.bus.read_word(test_addr).unwrap_or(0);
    println!("Write 0xDEADBEEF at PA 0x{:08X}, readback: 0x{:08X} {}",
        test_addr, readback, if readback == 0xDEADBEEF { "OK" } else { "FAIL" });

    // Write at early_pg_dir
    let test_addr2: u64 = 0x00802000;
    vm.bus.write_word(test_addr2, 0xCAFEBABE).ok();
    let readback2 = vm.bus.read_word(test_addr2).unwrap_or(0);
    println!("Write 0xCAFEBABE at PA 0x{:08X}, readback: 0x{:08X} {}",
        test_addr2, readback2, if readback2 == 0xCAFEBABE { "OK" } else { "FAIL" });

    // Test 2: Check if our boot page table entries are readable
    let boot_pt_ppn = (vm.cpu.csr.satp & 0x3FFFFF) as u64; // PPN from initial SATP
    let boot_pt_addr = (boot_pt_ppn as u64) * 4096;
    println!("\nBoot page table at PA 0x{:08X}:", boot_pt_addr);
    for i in 0..8 {
        let pte = vm.bus.read_word(boot_pt_addr + (i as u64) * 4).unwrap_or(0);
        if pte != 0 {
            println!("  L1[{}] = 0x{:08X}", i, pte);
        }
    }
    for i in 768..776 {
        let pte = vm.bus.read_word(boot_pt_addr + (i as u64) * 4).unwrap_or(0);
        if pte != 0 {
            println!("  L1[{}] = 0x{:08X}", i, pte);
        }
    }

    // Test 3: Run kernel through setup_vm and check page tables
    println!("\n=== Running kernel through setup_vm ===");
    let fw_addr_u32 = _fw_addr as u32;
    let mut count: u64 = 0;
    let max: u64 = 200_000;
    let mut setup_vm_done = false;
    let mut relocate_done = false;

    while count < max {
        if vm.bus.sbi.shutdown_requested { break; }

        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine {
            let cause_code = vm.cpu.csr.mcause & !(1u32 << 31);
            if cause_code == 9 || cause_code == 11 {
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16],
                    vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13],
                    vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0, a1)) = result {
                    vm.cpu.x[10] = a0; vm.cpu.x[11] = a1;
                }
            } else {
                let mpp = (vm.cpu.csr.mstatus & 0x1800) >> 11;
                if mpp != 3 {
                    let stvec = vm.cpu.csr.stvec & !0x3u32;
                    if stvec != 0 {
                        vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                        vm.cpu.csr.scause = vm.cpu.csr.mcause;
                        vm.cpu.csr.stval = vm.cpu.csr.mtval;
                        vm.cpu.pc = stvec;
                        vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
                        vm.cpu.tlb.flush_all();
                        count += 1;
                        continue;
                    }
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        // Detect setup_vm return (PC near 0xC00010F6)
        if !setup_vm_done && vm.cpu.pc >= 0xC00010F0 && vm.cpu.pc <= 0xC0001100 {
            setup_vm_done = true;
            println!("[setup_vm return] count={} PC=0x{:08X} SATP=0x{:08X}",
                count, vm.cpu.pc, vm.cpu.csr.satp);
        }

        // Detect relocate_enable_mmu return (PC near 0xC00010FA)
        if setup_vm_done && !relocate_done && vm.cpu.pc >= 0xC00010F8 && vm.cpu.pc <= 0xC0001100 {
            relocate_done = true;
            println!("[relocate_enable_mmu return] count={} PC=0x{:08X} SATP=0x{:08X}",
                count, vm.cpu.pc, vm.cpu.csr.satp);
        }

        let _ = vm.step();
        count += 1;
    }

    // After setup_vm, check trampoline and early page tables
    println!("\n=== After setup_vm ({} instructions) ===", count);

    // Check trampoline_pg_dir
    let tramp_addr: u64 = 0x01484000;
    println!("Trampoline pg_dir at PA 0x{:08X}:", tramp_addr);
    for i in 0..16 {
        let pte = vm.bus.read_word(tramp_addr + (i as u64) * 4).unwrap_or(0);
        if pte != 0 {
            println!("  L1[{:>3}] = 0x{:08X}", i, pte);
        }
    }

    // Check early_pg_dir
    let early_addr: u64 = 0x00802000;
    println!("\nEarly pg_dir at PA 0x{:08X}:", early_addr);
    for i in 0..16 {
        let pte = vm.bus.read_word(early_addr + (i as u64) * 4).unwrap_or(0);
        if pte != 0 {
            println!("  L1[{:>3}] = 0x{:08X}", i, pte);
        }
    }

    println!("\nPC=0x{:08X} SATP=0x{:08X} priv={:?}", vm.cpu.pc, vm.cpu.csr.satp, vm.cpu.privilege);
}
