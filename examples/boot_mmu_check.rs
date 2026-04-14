// Check MMU translation right after satp write
use std::fs;
use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::StepResult;
use geometry_os::riscv::mmu::{translate, AccessType};

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel = fs::read(kernel_path).unwrap();
    let initramfs = fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    let (mut vm, result) = RiscvVm::boot_linux(
        &kernel, initramfs.as_deref(), 512, 0, bootargs,
    ).unwrap();

    println!("Entry: 0x{:08X}, DTB: 0x{:08X}", result.entry, result.dtb_addr);

    // Run until satp changes
    let max_instr = 300_000u64;
    let mut count = 0u64;
    let mut prev_satap = vm.cpu.csr.satp;

    while count < max_instr {
        let satp_before = vm.cpu.csr.satp;
        let pc_before = vm.cpu.pc;
        let step_result = vm.step();
        count += 1;

        if vm.cpu.csr.satp != prev_satap {
            println!("[{}] satp changed: 0x{:08X} -> 0x{:08X}", count, prev_satap, vm.cpu.csr.satp);
            
            // Check translation of next instruction PC
            let next_pc = vm.cpu.pc;
            println!("  Next PC to fetch: 0x{:08X}", next_pc);
            
            // Try MMU translation
            let sum = vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::User;
            match translate(next_pc, AccessType::Fetch, sum, vm.cpu.csr.satp, &mut vm.bus, &mut vm.cpu.tlb) {

                geometry_os::riscv::mmu::TranslateResult::Ok(pa) => {
                    println!("  Translate: 0x{:08X} -> PA 0x{:08X}", next_pc, pa);
                    // Read instruction at PA
                    match vm.bus.read_word(pa) {
                        Ok(word) => println!("  Instruction at PA: 0x{:08X}", word),
                        Err(e) => println!("  Read failed: {:?}", e),
                    }
                    // Also read directly from VA (pre-MMU) for comparison
                    match vm.bus.mem.read_word(next_pc as u64) {
                        Ok(word) => println!("  Instruction at VA (raw): 0x{:08X}", word),
                        Err(e) => println!("  Raw read failed: {:?}", e),
                    }
                }
                geometry_os::riscv::mmu::TranslateResult::FetchFault => {
                    println!("  Translate: FETCH FAULT");
                }
                geometry_os::riscv::mmu::TranslateResult::LoadFault => {
                    println!("  Translate: LOAD FAULT");
                }
                geometry_os::riscv::mmu::TranslateResult::StoreFault => {
                    println!("  Translate: STORE FAULT");
                }
            }
            
            // Dump satp PPN and first few page table entries
            let root_ppn = vm.cpu.csr.satp & 0x003F_FFFF;
            let root_addr = (root_ppn as u64) << 12;
            println!("  Root PT at PA: 0x{:08X}", root_addr);
            for i in 0..8 {
                let pte_addr = root_addr + (i as u64) * 4;
                if let Ok(pte) = vm.bus.read_word(pte_addr) {
                    if pte != 0 {
                        println!("  PTE[{}] at PA 0x{:08X} = 0x{:08X}", i, pte_addr, pte);
                    }
                }
            }
            
            // Walk the page table for the specific VA
            let vpn1 = (next_pc >> 22) & 0x3FF;
            let vpn0 = (next_pc >> 12) & 0x3FF;
            println!("  VPN1={}, VPN0={}", vpn1, vpn0);
            
            prev_satap = vm.cpu.csr.satp;
            
            // Now run a few more steps to see what happens
            for j in 0..10 {
                let pc = vm.cpu.pc;
                let sr = vm.step();
                count += 1;
                let mcause = vm.cpu.csr.mcause;
                println!("  [{}] PC=0x{:08X} -> 0x{:08X} mcause={} result={:?}", 
                         count, pc, vm.cpu.pc, mcause, sr);
                if mcause != 0 || matches!(sr, StepResult::FetchFault) {
                    break;
                }
            }
            break;
        }
    }

    println!("\n=== Final: {} steps, PC=0x{:08X}, mcause=0x{:08X} ===", 
             count, vm.cpu.pc, vm.cpu.csr.mcause);
}
