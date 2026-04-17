use geometry_os::riscv::{RiscvVm, cpu, csr, mmu};

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, _br) = RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        256,
        180_000,
        "console=ttyS0 loglevel=8",
    ).unwrap();

    println!("=== MMU Translation Check ===");
    
    // Use mmu::translate directly
    let satp = vm.cpu.csr.satp;
    let sum = (vm.cpu.csr.mstatus >> csr::MSTATUS_SUM) & 1 != 0;
    let mxr = (vm.cpu.csr.mstatus >> csr::MSTATUS_MXR) & 1 != 0;
    
    // Check handler address translation
    let handler_va = 0xC0210F14u32;
    match mmu::translate(handler_va, mmu::AccessType::Fetch, cpu::Privilege::Supervisor, sum, mxr, satp, &mut vm.bus, &mut vm.cpu.tlb) {
        mmu::TranslateResult::Ok(pa) => {
            println!("Handler VA 0x{:08X} -> PA 0x{:08X} OK", handler_va, pa);
            if let Ok(inst) = vm.bus.read_word(pa) {
                println!("  Instruction: 0x{:08X}", inst);
            }
        }
        mmu::TranslateResult::FetchFault => println!("Handler VA 0x{:08X} -> FETCH FAULT!", handler_va),
        mmu::TranslateResult::LoadFault => println!("Handler VA 0x{:08X} -> LOAD FAULT!", handler_va),
        mmu::TranslateResult::StoreFault => println!("Handler VA 0x{:08X} -> STORE FAULT!", handler_va),
    }
    
    // Check bad return address
    let bad_ra = 0x3FFFF000u32;
    match mmu::translate(bad_ra, mmu::AccessType::Fetch, cpu::Privilege::Supervisor, sum, mxr, satp, &mut vm.bus, &mut vm.cpu.tlb) {
        mmu::TranslateResult::Ok(pa) => println!("Bad RA VA 0x{:08X} -> PA 0x{:08X} OK (unexpected!)", bad_ra, pa),
        _ => println!("Bad RA VA 0x{:08X} -> FAULT (expected)", bad_ra),
    }
    
    // Check instructions around the crash
    println!("\nInstructions near crash point (VA 0xC003F9B0):");
    let base_va = 0xC003F9B0u32;
    for offset in 0..48 {
        let va = base_va + offset * 2; // compressed instructions are 2 bytes
        match mmu::translate(va, mmu::AccessType::Fetch, cpu::Privilege::Supervisor, sum, mxr, satp, &mut vm.bus, &mut vm.cpu.tlb) {
            mmu::TranslateResult::Ok(pa) => {
                if let Ok(hw) = vm.bus.read_half(pa) {
                    println!("  VA 0x{:08X} -> PA 0x{:08X}: 0x{:04X}", va, pa, hw);
                }
            }
            _ => println!("  VA 0x{:08X} -> FAULT", va),
        }
    }
    
    // Check page table for user space
    let pg_dir_phys = ((satp & 0x3FFFFF) as u64) * 4096;
    println!("\nPage table L1[255] (VA 0x3FC00000-0x3FFFFFFF):");
    let l1_255 = vm.bus.read_word(pg_dir_phys + 255 * 4).unwrap_or(0);
    println!("  PTE = 0x{:08X} V={}", l1_255, l1_255 & 1);
    
    // Check stack
    let sp = vm.cpu.x[2];
    println!("\nStack (SP=0x{:08X}):", sp);
    match mmu::translate(sp, mmu::AccessType::Load, cpu::Privilege::Supervisor, sum, mxr, satp, &mut vm.bus, &mut vm.cpu.tlb) {
        mmu::TranslateResult::Ok(sp_pa) => {
            println!("  SP -> PA 0x{:08X}", sp_pa);
            for off in [80, 84, 88, 92, 96, 100] as [u32; 6] {
                if let Ok(val) = vm.bus.read_word(sp_pa + off as u64) {
                    let label = match off {
                        88 => " (s0 save area)",
                        92 => " (ra save area!)",
                        _ => "",
                    };
                    println!("  [SP+{}] = 0x{:08X}{}", off, val, label);
                }
            }
        }
        _ => println!("  SP -> FAULT!"),
    }
}
