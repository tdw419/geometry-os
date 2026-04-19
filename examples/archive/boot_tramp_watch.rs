use geometry_os::riscv::cpu::StepResult;
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, _fw, _entry, _dtb) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, "loglevel=0 quiet")
            .unwrap();

    // Patch kernel
    let sw_a5_12 = vm.bus.read_half(0x0040495E).unwrap_or(0);
    let sw_a1_20 = vm.bus.read_half(0x00404968).unwrap_or(0);
    if sw_a5_12 == 0xC4DC && sw_a1_20 == 0xC8CC {
        vm.bus.write_half(0x0040495E, 0x0001).ok();
        vm.bus.write_half(0x00404968, 0x0001).ok();
        vm.bus.write_word(0x00C79E90 + 12, 0x00000000).ok();
        vm.bus.write_word(0x00C79E90 + 20, 0xC0000000).ok();
        vm.bus.auto_pte_fixup = false;
        vm.bus.known_pt_pages.clear();
        eprintln!("Patched");
    }

    let mut count: u64 = 0;
    let max: u64 = 180_000; // Stop around the SATP changes
    let mut last_satp = vm.cpu.csr.satp;

    // Watch addresses: trampoline_pg_dir at PA 0x01484000
    let tramp_pgdir = 0x01484000u64;
    let early_pgdir = 0x00802000u64;

    // Set write watchpoint on trampoline_pg_dir L1[0xFF]
    vm.bus.write_watch_addr = Some(tramp_pgdir + 0xFF * 4);

    while count < max {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        // Check SATP
        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            let ppn = cur_satp & 0x3FFFFF;
            let pgdir = (ppn as u64) * 4096;
            let l1_ff = vm.bus.read_word(pgdir + 0xFF * 4).unwrap_or(0xDEAD);
            let l1_300 = vm.bus.read_word(pgdir + 0x300 * 4).unwrap_or(0xDEAD);
            eprintln!(
                "SATP: 0x{:08X} -> 0x{:08X} at count={}",
                last_satp, cur_satp, count
            );
            eprintln!(
                "  pgdir=0x{:08X} L1[0xFF]=0x{:08X} L1[0x300]=0x{:08X}",
                pgdir, l1_ff, l1_300
            );

            // Also check the write watchpoint
            if vm.bus.write_watch_hit {
                eprintln!(
                    "  WATCHPOINT HIT: wrote 0x{:08X} from PC=0x{:08X}",
                    vm.bus.write_watch_val, vm.bus.write_watch_pc
                );
                vm.bus.write_watch_hit = false;
            }
            last_satp = cur_satp;
        }

        // Check watchpoint every step
        if vm.bus.write_watch_hit && !vm.bus.write_watch_hit { /* skip */ }
        if vm.bus.write_watch_hit {
            eprintln!(
                "  WP HIT at count={}: wrote 0x{:08X} from PC=0x{:08X} priv={:?}",
                count, vm.bus.write_watch_val, vm.bus.write_watch_pc, vm.cpu.privilege
            );
            vm.bus.write_watch_hit = false;
            // Move watchpoint to catch next interesting write
        }

        let result = vm.step();
        match result {
            StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault => {
                if count > 177_000 {
                    eprintln!(
                        "FAULT at count={}: PC=0x{:08X} result={:?} scause=0x{:08X} stval=0x{:08X}",
                        count, vm.cpu.pc, result, vm.cpu.csr.scause, vm.cpu.csr.stval
                    );
                }
            }
            StepResult::Ebreak => break,
            _ => {}
        }
        count += 1;
    }

    // Final check
    for &pgdir in &[tramp_pgdir, early_pgdir] {
        eprintln!(
            "
pgdir at PA 0x{:08X}:",
            pgdir
        );
        for i in [0u32, 1, 2, 0x300, 0x301, 0xFF] {
            let pte = vm.bus.read_word(pgdir + i as u64 * 4).unwrap_or(0xDEAD);
            eprintln!("  L1[0x{:03X}] = 0x{:08X}", i, pte);
        }
    }
}
