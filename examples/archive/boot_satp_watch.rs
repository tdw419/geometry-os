fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    use geometry_os::riscv::cpu::{Privilege, StepResult};
    use geometry_os::riscv::csr;
    use geometry_os::riscv::mmu;
    use geometry_os::riscv::RiscvVm;

    let (mut vm, boot_result) = RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        256,
        17_500_000,
        bootargs,
    )
    .unwrap();

    println!(
        "Boot result: {} instructions, entry=0x{:08X}, dtb=0x{:08X}",
        boot_result.instructions, boot_result.entry, boot_result.dtb_addr
    );
    println!(
        "PC=0x{:08X} priv={:?} satp=0x{:08X}",
        vm.cpu.pc,
        vm.cpu.privilege,
        vm.cpu.csr.read(csr::SATP)
    );
    println!(
        "stvec=0x{:08X} scause=0x{:08X}",
        vm.cpu.csr.stvec, vm.cpu.csr.scause
    );

    // Check SATP write history from mmu_log
    let satp_changes: Vec<_> = vm
        .bus
        .mmu_log
        .iter()
        .filter_map(|e| {
            if let mmu::MmuEvent::SatpWrite { old, new } = e {
                Some((*old, *new))
            } else {
                None
            }
        })
        .collect();
    println!("\n=== SATP changes ({} total) ===", satp_changes.len());
    for (i, (old, new)) in satp_changes.iter().enumerate() {
        println!("  [{}] 0x{:08X} -> 0x{:08X}", i, old, new);
    }

    // Check last few PageFault events
    let faults: Vec<_> = vm
        .bus
        .mmu_log
        .iter()
        .filter_map(|e| {
            if let mmu::MmuEvent::PageFault { va, access, .. } = e {
                Some((*va, *access))
            } else {
                None
            }
        })
        .collect();
    println!("\n=== Last 20 page faults ({} total) ===", faults.len());
    for (va, access) in faults.iter().rev().take(20) {
        let a = match access {
            mmu::AccessType::Fetch => "F",
            mmu::AccessType::Load => "L",
            mmu::AccessType::Store => "S",
        };
        println!("  0x{:08X} [{}]", va, a);
    }

    // Check the PTE for the faulting page at various points
    // First, check what SATP was when the kernel was running fine
    // (before the first fault on this page)
    let satp = vm.cpu.csr.read(csr::SATP);
    let ppn = satp & 0x3FFFFF;
    let pt_base = (ppn as u64) << 12;

    // Check L1[770] (the one with V=0)
    let l1_idx: u32 = 770;
    let l1_addr = pt_base + (l1_idx as u64) * 4;
    let l1_pte = vm.bus.read_word(l1_addr).unwrap_or(0);
    println!("\nL1[770] at PA=0x{:08X}: 0x{:08X}", l1_addr, l1_pte);
    println!(
        "  V={} R={} W={} X={} U={} A={} D={}",
        l1_pte & 1,
        (l1_pte >> 1) & 1,
        (l1_pte >> 2) & 1,
        (l1_pte >> 3) & 1,
        (l1_pte >> 4) & 1,
        (l1_pte >> 6) & 1,
        (l1_pte >> 7) & 1
    );

    // Now try booting again but stop just BEFORE the fault (~17M instructions)
    // and check the PTE at that point
    println!("\n=== Re-booting with 17M instruction limit ===");
    let (mut vm2, _) = RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        256,
        17_000_000,
        bootargs,
    )
    .unwrap();
    let satp2 = vm2.cpu.csr.read(csr::SATP);
    let ppn2 = satp2 & 0x3FFFFF;
    let pt_base2 = (ppn2 as u64) << 12;
    let l1_addr2 = pt_base2 + (l1_idx as u64) * 4;
    let l1_pte2 = vm2.bus.read_word(l1_addr2).unwrap_or(0);
    println!("At 17M: PC=0x{:08X} satp=0x{:08X}", vm2.cpu.pc, satp2);
    println!("L1[770] at PA=0x{:08X}: 0x{:08X}", l1_addr2, l1_pte2);
    println!(
        "  V={} R={} W={} X={} U={} A={} D={}",
        l1_pte2 & 1,
        (l1_pte2 >> 1) & 1,
        (l1_pte2 >> 2) & 1,
        (l1_pte2 >> 3) & 1,
        (l1_pte2 >> 4) & 1,
        (l1_pte2 >> 6) & 1,
        (l1_pte2 >> 7) & 1
    );

    // Try even earlier: 16M
    println!("\n=== Re-booting with 16M instruction limit ===");
    let (mut vm3, _) = RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        256,
        16_000_000,
        bootargs,
    )
    .unwrap();
    let satp3 = vm3.cpu.csr.read(csr::SATP);
    let ppn3 = satp3 & 0x3FFFFF;
    let pt_base3 = (ppn3 as u64) << 12;
    let l1_addr3 = pt_base3 + (l1_idx as u64) * 4;
    let l1_pte3 = vm3.bus.read_word(l1_addr3).unwrap_or(0);
    println!("At 16M: PC=0x{:08X} satp=0x{:08X}", vm3.cpu.pc, satp3);
    println!("L1[770] at PA=0x{:08X}: 0x{:08X}", l1_addr3, l1_pte3);
    println!(
        "  V={} R={} W={} X={} U={} A={} D={}",
        l1_pte3 & 1,
        (l1_pte3 >> 1) & 1,
        (l1_pte3 >> 2) & 1,
        (l1_pte3 >> 3) & 1,
        (l1_pte3 >> 4) & 1,
        (l1_pte3 >> 6) & 1,
        (l1_pte3 >> 7) & 1
    );

    // Check if SATP changed between 16M and 17M
    let satp_changes_16m: Vec<_> = vm3
        .bus
        .mmu_log
        .iter()
        .filter_map(|e| {
            if let mmu::MmuEvent::SatpWrite { old, new } = e {
                Some((*old, *new))
            } else {
                None
            }
        })
        .collect();
    println!("SATP changes at 16M: {}", satp_changes_16m.len());

    let satp_changes_17m: Vec<_> = vm2
        .bus
        .mmu_log
        .iter()
        .filter_map(|e| {
            if let mmu::MmuEvent::SatpWrite { old, new } = e {
                Some((*old, *new))
            } else {
                None
            }
        })
        .collect();
    println!("SATP changes at 17M: {}", satp_changes_17m.len());
}
