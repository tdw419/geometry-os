use geometry_os::riscv::{RiscvVm, cpu, csr, mmu};

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    // Run to 178500 to catch right at the crash
    let (mut vm, _br) = RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        256,
        178_500,
        "console=ttyS0 loglevel=8",
    ).unwrap();

    let sp = vm.cpu.x[2];
    let satp = vm.cpu.csr.satp;
    let sum = (vm.cpu.csr.mstatus >> csr::MSTATUS_SUM) & 1 != 0;
    let mxr = (vm.cpu.csr.mstatus >> csr::MSTATUS_MXR) & 1 != 0;
    
    println!("PC=0x{:08X} SP=0x{:08X}", vm.cpu.pc, sp);
    
    // Translate SP and dump a large chunk of the stack
    match mmu::translate(sp, mmu::AccessType::Load, cpu::Privilege::Supervisor, sum, mxr, satp, &mut vm.bus, &mut vm.cpu.tlb) {
        mmu::TranslateResult::Ok(sp_pa) => {
            println!("SP -> PA 0x{:08X}", sp_pa);
            println!("\nStack dump (SP+0 to SP+256):");
            for off in (0..256).step_by(4) {
                if let Ok(val) = vm.bus.read_word(sp_pa + off as u64) {
                    let mut label = "";
                    if val == 0x3FFFF000 { label = " <-- BAD RA!"; }
                    if val == sp { label = " <-- SP"; }
                        println!("  [SP+{:>3}] PA 0x{:08X} = 0x{:08X}{}", off, sp_pa + off, val, label);
                }
            }
        }
        _ => println!("SP -> FAULT!"),
    }
    
    // Also check a few frames up the stack
    println!("\n=== Return address chain ===");
    let mut frame_sp = sp;
    for frame in 0..8 {
        match mmu::translate(frame_sp, mmu::AccessType::Load, cpu::Privilege::Supervisor, sum, mxr, satp, &mut vm.bus, &mut vm.cpu.tlb) {
            mmu::TranslateResult::Ok(pa) => {
                // Try to read s0 (saved frame pointer) and ra (saved return address)
                // In RISC-V calling convention, s0/fp is saved at SP+0, ra at SP+4 in the callee's frame
                // But the actual offset depends on the function. Let's look for plausible return addresses
                if let Ok(s0_val) = vm.bus.read_word(pa) {
                    if let Ok(ra_val) = vm.bus.read_word(pa + 4) {
                        let ra_ok = if ra_val >= 0xC0000000 && ra_val < 0xC1500000 { "OK" } else { "BAD" };
                        println!("  Frame {}: SP=0x{:08X} s0(fp)=0x{:08X} ra=0x{:08X} [{}]", 
                            frame, frame_sp, s0_val, ra_val, ra_ok);
                        if s0_val < frame_sp || s0_val > frame_sp + 0x10000 {
                            break;
                        }
                        frame_sp = s0_val;
                    }
                }
            }
            _ => break,
        }
    }
}
