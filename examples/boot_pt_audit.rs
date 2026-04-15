/// Dump the first 20 L1 page table entries to verify PPN ranges.
/// With ram_base=0xC0000000, valid PPNs should start with 0xC0xxxx.
/// If any PPN starts with 0x00-0xBF, it's below ram_base and reads return 0.
use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::StepResult;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    let (mut vm, fw_addr, _, _) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();
    let fw_addr_u32 = fw_addr as u32;

    // Run to 16,999,000 instructions with trap forwarding
    let target = 16_999_000u64;
    let mut count: u64 = 0;

    while count < target {
        if vm.bus.sbi.shutdown_requested { break; }
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine {
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
                let mpp = (vm.cpu.csr.mstatus & 0x300) >> 8;
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
                        vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
                        vm.cpu.tlb.flush_all();
                        count += 1;
                        continue;
                    }
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }
        let _ = vm.step();
        count += 1;
    }

    let satp = vm.cpu.csr.satp;
    let pt_root_ppn = (satp & 0x3FFFFF) as u64;
    let pt_root_phys = pt_root_ppn << 12;
    let ram_base: u64 = 0xC0000000;

    eprintln!("SATP=0x{:08X}", satp);
    eprintln!("Page table root: PPN=0x{:06X} phys=0x{:08X}", pt_root_ppn, pt_root_phys);
    eprintln!("RAM base: 0x{:08X}", ram_base);
    eprintln!();

    // Check if the root is in RAM
    let root_in_ram = pt_root_phys >= ram_base;
    eprintln!("Root in RAM: {}", root_in_ram);
    eprintln!();

    // Dump first 20 L1 entries
    eprintln!("=== L1 Page Table (first 20 entries) ===");
    for i in 0..20u32 {
        let addr = pt_root_phys + (i as u64 * 4);
        match vm.bus.read_word(addr) {
            Ok(pte) => {
                let v = pte & 1;
                let rwx = (pte >> 1) & 0x7;
                let ppn = (pte >> 10) & 0x3FFFFF;
                let phys = (ppn as u64) << 12;
                let in_ram = phys >= ram_base;
                let entry_type = if v == 0 { "INVALID" } else if rwx != 0 { "megapage" } else { "L2-pointer" };
                let warn = if v == 1 && !in_ram { " *** BELOW RAM ***" } else { "" };
                let ram_status = if in_ram { "OK" } else { "ERR" };
                eprintln!("  L1[{:4}] 0x{:08X}: PTE=0x{:08X} V={} RWX={} PPN=0x{:06X} phys=0x{:08X} {} [{}]{}",
                    i, addr as u32, pte, v, rwx, ppn, phys as u32, ram_status, entry_type, warn);
            }
            Err(e) => eprintln!("  L1[{:4}] 0x{:08X}: ERR {:?}", i, addr as u32, e),
        }
    }

    // Dump entries around index 768 (0xC0000000 >> 22 = 768)
    eprintln!("\n=== L1 Page Table (entries 765-780) ===");
    for i in 765..=780u32 {
        let addr = pt_root_phys + (i as u64 * 4);
        match vm.bus.read_word(addr) {
            Ok(pte) => {
                let v = pte & 1;
                let rwx = (pte >> 1) & 0x7;
                let ppn = (pte >> 10) & 0x3FFFFF;
                let phys = (ppn as u64) << 12;
                let in_ram = phys >= ram_base;
                let entry_type = if v == 0 { "INVALID" } else if rwx != 0 { "megapage" } else { "L2-pointer" };
                let warn = if v == 1 && !in_ram { " *** BELOW RAM ***" } else { "" };
                let vaddr_base = (i as u64) << 22;
                let ram_status = if in_ram { "OK" } else { "ERR" };
                eprintln!("  L1[{:4}] 0x{:08X}: PTE=0x{:08X} V={} RWX={} PPN=0x{:06X} phys=0x{:08X} {} [vaddr 0x{:08X}+][{}]{}",
                    i, addr as u32, pte, v, rwx, ppn, phys as u32, ram_status, vaddr_base as u32, entry_type, warn);
            }
            Err(e) => eprintln!("  L1[{:4}] 0x{:08X}: ERR {:?}", i, addr as u32, e),
        }
    }

    // Count entries by type
    eprintln!("\n=== L1 Summary ===");
    let mut megapage_ok = 0u32;
    let mut megapage_err = 0u32;
    let mut l2ptr_ok = 0u32;
    let mut l2ptr_err = 0u32;
    let mut invalid = 0u32;
    for i in 0..1024u32 {
        let addr = pt_root_phys + (i as u64 * 4);
        if let Ok(pte) = vm.bus.read_word(addr) {
            let v = pte & 1;
            let rwx = (pte >> 1) & 0x7;
            let ppn = (pte >> 10) & 0x3FFFFF;
            let phys = (ppn as u64) << 12;
            let in_ram = phys >= ram_base;
            if v == 0 { invalid += 1; }
            else if rwx != 0 { if in_ram { megapage_ok += 1; } else { megapage_err += 1; } }
            else { if in_ram { l2ptr_ok += 1; } else { l2ptr_err += 1; } }
        }
    }
    eprintln!("  Megapages in RAM: {}", megapage_ok);
    eprintln!("  Megapages below RAM: {}", megapage_err);
    eprintln!("  L2 pointers in RAM: {}", l2ptr_ok);
    eprintln!("  L2 pointers below RAM: {}", l2ptr_err);
    eprintln!("  Invalid entries: {}", invalid);
}
