use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::mmu::{self, AccessType};
use std::fs;

fn main() {
    let kernel = fs::read(".geometry_os/build/linux-6.14/vmlinux").unwrap();
    let initramfs = fs::read(".geometry_os/fs/linux/rv32/initramfs.cpio.gz").ok();
    
    let (mut vm, _) = RiscvVm::boot_linux(
        &kernel,
        initramfs.as_deref(),
        256,
        500_000, // Short run to get past setup
        "console=ttyS0 earlycon",
    ).unwrap();

    let satp = vm.cpu.csr.satp;
    let priv_mode = vm.cpu.privilege;
    let sum = (vm.cpu.csr.mstatus >> 18) & 1 != 0;
    let mxr = (vm.cpu.csr.mstatus >> 19) & 1 != 0;
    
    eprintln!("SATP: 0x{:08X}, priv: {:?}", satp, priv_mode);
    eprintln!("SUM: {}, MXR: {}", sum, mxr);
    
    // Test MMU translation for key addresses
    let test_vas: &[u32] = &[
        0xC08E5D6Au32, // __memmove (was working)
        0xC08EFF1Cu32, // handle_exception (fetch fault)
        0xC08BDFFCu32, // load fault target
        0xC00010D0u32, // _start_kernel
    ];
    
    for &va in test_vas {
        let result = mmu::translate(
            va, AccessType::Fetch, priv_mode, sum, mxr, satp,
            &mut vm.bus, &mut vm.cpu.tlb,
        );
        match result {
            mmu::TranslateResult::Ok(pa) => {
                eprintln!("VA 0x{:08X} -> PA 0x{:08X} (fetch OK)", va, pa);
                // Try to read from PA
                match vm.bus.read_word(pa) {
                    Ok(word) => eprintln!("  bus read OK: 0x{:08X}", word),
                    Err(_) => eprintln!("  bus read FAILED!"),
                }
            }
            mmu::TranslateResult::FetchFault => eprintln!("VA 0x{:08X} -> FETCH FAULT", va),
            mmu::TranslateResult::LoadFault => eprintln!("VA 0x{:08X} -> LOAD FAULT", va),
            mmu::TranslateResult::StoreFault => eprintln!("VA 0x{:08X} -> STORE FAULT", va),
        }
        
        // Also test load
        let result2 = mmu::translate(
            va, AccessType::Load, priv_mode, sum, mxr, satp,
            &mut vm.bus, &mut vm.cpu.tlb,
        );
        match result2 {
            mmu::TranslateResult::Ok(pa) => eprintln!("VA 0x{:08X} -> PA 0x{:08X} (load OK)", va, pa),
            mmu::TranslateResult::FetchFault => eprintln!("VA 0x{:08X} -> FETCH FAULT (load)", va),
            mmu::TranslateResult::LoadFault => eprintln!("VA 0x{:08X} -> LOAD FAULT (load)", va),
            mmu::TranslateResult::StoreFault => eprintln!("VA 0x{:08X} -> STORE FAULT (load)", va),
        }
    }
    
    // Check last few MMU log entries
    let log = &vm.bus.mmu_log;
    eprintln!("\nMMU log: {} entries", log.len());
    // Show last 5
    for entry in log.iter().rev().take(5) {
        eprintln!("  {:?}", entry);
    }
}
