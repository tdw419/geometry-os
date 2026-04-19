// Trace what calls udelay repeatedly
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = std::path::Path::new(".geometry_os/build/linux-6.14/vmlinux");
    let kernel = std::fs::read(kernel_path).expect("kernel");
    let initramfs_path = std::path::Path::new(".geometry_os/fs/linux/rv32/initramfs.cpio.gz");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, _fw_addr, _entry, _dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel,
        initramfs.as_deref(),
        128,
        "console=ttyS0 earlyethcon=sbi",
    )
    .unwrap();

    let udelay_start: u32 = 0xC020B0D0;
    let udelay_end: u32 = 0xC020B170;

    let mut in_udelay = false;
    let mut udelay_enter_count = 0u32;
    let mut count: u64 = 0;
    let max: u64 = 2_000_000;
    // Track RA when entering udelay
    let mut ra_on_enter: Vec<u32> = Vec::new();
    // Track last few PCs before entering udelay
    let mut pc_history: Vec<u32> = Vec::new();

    while count < max {
        let pc = vm.cpu.pc;
        pc_history.push(pc);
        if pc_history.len() > 20 {
            pc_history.remove(0);
        }

        let _result = vm.step();
        count += 1;

        let now_in = vm.cpu.pc >= udelay_start && vm.cpu.pc < udelay_end;
        if now_in && !in_udelay {
            // Just entered udelay
            in_udelay = true;
            udelay_enter_count += 1;
            let ra = vm.cpu.x[1]; // return address
            ra_on_enter.push(ra);
            if udelay_enter_count <= 10 {
                println!(
                    "[trace] udelay #{} at count={}, PC=0x{:08X}, RA=0x{:08X}, a0={}",
                    udelay_enter_count, count, vm.cpu.pc, ra, vm.cpu.x[10]
                );
                // Show last 5 PCs before entry
                let start = pc_history.len().saturating_sub(6);
                for (i, hpc) in pc_history[start..].iter().enumerate() {
                    println!("  prev[{}] = 0x{:08X}", i, hpc);
                }
            }
        } else if !now_in && in_udelay {
            in_udelay = false;
        }

        // Check if stuck
        if udelay_enter_count > 100 {
            println!(
                "[trace] udelay called {} times in {} instructions",
                udelay_enter_count, count
            );
            println!(
                "[trace] Last 5 RAs: {:X?}",
                &ra_on_enter[ra_on_enter.len().saturating_sub(5)..]
            );
            println!("[trace] Current PC = 0x{:08X}", vm.cpu.pc);
            break;
        }
    }

    if udelay_enter_count <= 100 {
        println!(
            "[trace] Total instructions: {}, udelay calls: {}",
            count, udelay_enter_count
        );
        println!("[trace] Final PC = 0x{:08X}", vm.cpu.pc);
    }
}
