/// Read the exception handler table at 0xC1400AB4 base to understand
/// why the entry at offset 0x34 (cause 13) is 0.
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

    // Now read the handler table
    // Table base = 0xC1400AB4
    // Entry for cause 13 = base + (13 * 4) = base + 52 = 0xC1400AE8
    let table_base = 0xC1400AB4u64;
    eprintln!("=== Exception handler table at 0x{:08X} ===", table_base);

    // Read 32 entries (one per exception cause code)
    for i in 0..32 {
        let addr = table_base + (i as u64 * 4);
        match vm.bus.read_word(addr) {
            Ok(val) => {
                let marker = if i == 13 { " <-- cause 13 (load page fault)" } else { "" };
                eprintln!("  [{}] 0x{:08X} = 0x{:08X}{}", i, addr as u32, val, marker);
            }
            Err(e) => eprintln!("  [{}] 0x{:08X} = ERR: {:?}", i, addr as u32, e),
        }
    }

    // Also check if the PTE corruption happened
    // From the skill doc: L1 PTE at index 770 changes from 0x302000EF to 0x2097C64E
    // Let's read the page table root to see if it's still valid
    let satp = vm.cpu.csr.satp;
    let pt_root = (satp & 0x003FFFFF) << 12;
    eprintln!("\n=== Page table root at 0x{:08X} (SATP=0x{:08X}) ===", pt_root, satp);

    // Check the specific L1 PTE at index 770
    let pte_770_addr = pt_root as u64 + (770u64 * 4);
    match vm.bus.read_word(pte_770_addr) {
        Ok(val) => eprintln!("  L1[770] at 0x{:08X} = 0x{:08X}", pte_770_addr as u32, val),
        Err(e) => eprintln!("  L1[770] at 0x{:08X} = ERR: {:?}", pte_770_addr as u32, e),
    }

    // Check a few entries around the handler table address range
    // The handler table is at 0xC1400AB4. Let's check which L2 page table
    // maps this virtual address
    let vaddr = 0xC1400AB4u32;
    let vpn1 = ((vaddr >> 22) & 0x3FF) as usize;
    let vpn0 = ((vaddr >> 12) & 0x3FF) as usize;
    eprintln!("\n=== Page table walk for handler table at 0x{:08X} ===", vaddr);
    eprintln!("  VPN1={} VPN0={}", vpn1, vpn0);

    let l1_addr = pt_root as u64 + (vpn1 as u64 * 4);
    match vm.bus.read_word(l1_addr) {
        Ok(l1_pte) => {
            eprintln!("  L1[{}] at 0x{:08X} = 0x{:08X}", vpn1, l1_addr as u32, l1_pte);
            let l1_valid = l1_pte & 1;
            let l1_rwx = (l1_pte >> 1) & 0x7;
            let l1_ppn = (l1_pte >> 10) & 0xFFFFF;
            eprintln!("  V={} RWX={} PPN=0x{:05X}", l1_valid, l1_rwx, l1_ppn);

            if l1_valid == 1 && l1_rwx != 0 {
                eprintln!("  -> Megapage! Physical range: 0x{:08X} - 0x{:08X}",
                    l1_ppn << 12, ((l1_ppn << 12) + 0x400000).wrapping_sub(1));
            } else if l1_valid == 1 {
                // L2 lookup
                let l2_base = (l1_pte >> 10) << 12;
                let l2_addr = l2_base as u64 + (vpn0 as u64 * 4);
                match vm.bus.read_word(l2_addr) {
                    Ok(l2_pte) => {
                        eprintln!("  L2[{}] at 0x{:08X} = 0x{:08X}", vpn0, l2_addr as u32, l2_pte);
                        let l2_valid = l2_pte & 1;
                        let l2_rwx = (l2_pte >> 1) & 0x7;
                        let l2_ppn = (l2_pte >> 10) & 0xFFFFF;
                        eprintln!("  V={} RWX={} PPN=0x{:05X}", l2_valid, l2_rwx, l2_ppn);
                        eprintln!("  -> Physical: 0x{:08X}", (l2_ppn << 12) + (vaddr & 0xFFF));
                    }
                    Err(e) => eprintln!("  L2[{}] read ERR: {:?}", vpn0, e),
                }
            }
        }
        Err(e) => eprintln!("  L1[{}] read ERR: {:?}", vpn1, e),
    }

    // Check if the faulting address 0xC08BDFFC is accessible
    let fault_vaddr = 0xC08BDFFCu32;
    let fvpn1 = ((fault_vaddr >> 22) & 0x3FF) as usize;
    let fvpn0 = ((fault_vaddr >> 12) & 0x3FF) as usize;
    eprintln!("\n=== Page table walk for faulting address 0x{:08X} ===", fault_vaddr);
    eprintln!("  VPN1={} VPN0={}", fvpn1, fvpn0);
    let fl1_addr = pt_root as u64 + (fvpn1 as u64 * 4);
    match vm.bus.read_word(fl1_addr) {
        Ok(l1_pte) => {
            eprintln!("  L1[{}] at 0x{:08X} = 0x{:08X}", fvpn1, fl1_addr as u32, l1_pte);
            let l1_valid = l1_pte & 1;
            let l1_rwx = (l1_pte >> 1) & 0x7;
            let l1_ppn = (l1_pte >> 10) & 0xFFFFF;
            eprintln!("  V={} RWX={} PPN=0x{:05X}", l1_valid, l1_rwx, l1_ppn);
            if l1_valid == 1 && l1_rwx != 0 {
                eprintln!("  -> Megapage");
            } else if l1_valid == 1 {
                let l2_base = (l1_pte >> 10) << 12;
                let l2_addr = l2_base as u64 + (fvpn0 as u64 * 4);
                match vm.bus.read_word(l2_addr) {
                    Ok(l2_pte) => {
                        eprintln!("  L2[{}] at 0x{:08X} = 0x{:08X}", fvpn0, l2_addr as u32, l2_pte);
                        let l2_valid = l2_pte & 1;
                        eprintln!("  V={}", l2_valid);
                    }
                    Err(e) => eprintln!("  L2[{}] read ERR: {:?}", fvpn0, e),
                }
            }
        }
        Err(e) => eprintln!("  L1[{}] read ERR: {:?}", fvpn1, e),
    }
}
