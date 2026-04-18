// Find the full call chain to make_task_dead / panic
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = std::path::Path::new(".geometry_os/build/linux-6.14/vmlinux");
    let kernel = std::fs::read(kernel_path).expect("kernel");
    let initramfs_path = std::path::Path::new(".geometry_os/fs/linux/rv32/initramfs.cpio.gz");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, _fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel, initramfs.as_deref(), 128, "console=ttyS0 earlycon=sbi loglevel=8").unwrap();

    let panic_fn: u32 = 0xC000252E;
    let mut count: u64 = 0;
    let max: u64 = 400_000;

    while count < max {
        let pc = vm.cpu.pc;
        
        // Catch when we enter make_task_dead (before it calls panic)
        // make_task_dead is at 0xC0012DA0
        if pc == 0xC0012DA0 {
            println!("[make_task_dead] count={}, PC=0x{:08X}", count, pc);
            println!("[regs] a0=0x{:08X} a1=0x{:08X} ra=0x{:08X} sp=0x{:08X}",
                vm.cpu.x[10], vm.cpu.x[11], vm.cpu.x[1], vm.cpu.x[2]);
            
            // Walk the stack to find callers
            let sp = vm.cpu.x[2];
            println!("\n[stack-walk] Finding return chain:");
            let mut frame_sp = sp;
            for depth in 0..10 {
                if let Ok(ra) = vm.bus.read_word(frame_sp as u64) {
                    if ra >= 0xC0000000 && ra < 0xD0000000 && ra != 0 {
                        println!("  frame[{}]: RA=0x{:08X} (sp=0x{:08X})", depth, ra, frame_sp);
                        frame_sp = ra; // not right, but let's see the values
                    }
                }
                frame_sp += 4;
            }
            
            // Try proper frame pointer walk (fp = s0 = x8)
            let mut fp = vm.cpu.x[8]; // frame pointer
            for depth in 0..10 {
                if fp < 0xC0000000 || fp > 0xD0000000 || fp < sp {
                    break;
                }
                // In RISC-V, saved RA is at fp+4, saved FP is at fp+0
                if let Ok(saved_fp) = vm.bus.read_word(fp as u64) {
                    if let Ok(saved_ra) = vm.bus.read_word((fp as u64) + 4) {
                        if saved_ra >= 0xC0000000 && saved_ra < 0xD0000000 && saved_ra != 0 {
                            println!("  fp[{}]: RA=0x{:08X} FP=0x{:08X}", depth, saved_ra, saved_fp);
                        }
                        fp = saved_fp;
                        if fp == 0 { break; }
                    }
                }
            }
            break;
        }
        
        let _result = vm.step();
        count += 1;
    }
    
    if count >= max {
        println!("[info] No make_task_dead in {} instructions. PC=0x{:08X}", max, vm.cpu.pc);
    }
}
