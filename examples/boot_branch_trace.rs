// Detailed trace of pcpu_alloc_first_chunk: watch key branch decisions
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, _fw_addr, _entry, _dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        256,
        "console=ttyS0 earlycon=sbi loglevel=8",
    )
    .unwrap();

    let mut count: u64 = 0;
    while count < 15_560_000 {
        if vm.bus.sbi.shutdown_requested {
            break;
        }
        let _ = vm.step();
        count += 1;
    }

    // Now single-step and watch key PCs
    let mut step_count: u64 = 0;
    while step_count < 500_000 {
        let pc = vm.cpu.pc;
        let _ = vm.step();
        step_count += 1;
        count += 1;

        // Key branch at c040b070: beqz s4, c040b0b6
        // This checks start_offset
        if pc == 0xC040B070 {
            let s4 = vm.cpu.x[20];
            let s1 = vm.cpu.x[9];
            eprintln!(
                "[{}] Branch: start_offset(s4)={}, chunk=0x{:08X}",
                count, s4, s1
            );
            if s4 != 0 {
                let pa = (s1 - 0xC0000000) as u64;
                let so = vm.bus.read_word(pa + 68).unwrap();
                eprintln!("  chunk[68] = 0x{:08X} (should be start_offset)", so);
            }
        }

        // Second branch at c040b0ba: beqz s4, c040b118
        if pc == 0xC040B0BA {
            let s4 = vm.cpu.x[20];
            eprintln!("[{}] Branch: end_offset(s4)={}", count, s4);
        }

        // pcpu_block_update_hint_alloc entry
        if pc == 0xC0080954 {
            let a0 = vm.cpu.x[10];
            let a1 = vm.cpu.x[11];
            let a2 = vm.cpu.x[12];
            let ra = vm.cpu.x[1];
            eprintln!(
                "[{}] pcpu_block_update_hint_alloc(0x{:08X}, 0x{:08X}, 0x{:08X}) ra=0x{:08X}",
                count, a0, a1, a2, ra
            );

            // Read the call site to determine which call this is
            // If ra = 0xC040B0B6, it's the start_offset call
            // If ra = 0xC040B160, it's the end_offset call
            // Otherwise it's from elsewhere
            if ra == 0xC040B0B6 {
                eprintln!("  -> start_offset call");
            } else if ra == 0xC040B160 {
                eprintln!("  -> end_offset call");
            } else {
                eprintln!("  -> OTHER call site!");
            }
            break;
        }
    }
}
