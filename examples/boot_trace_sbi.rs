// Trace SBI calls during boot
use geometry_os::riscv::RiscvVm;
use std::collections::HashMap;
use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel = fs::read(kernel_path).unwrap();
    let initramfs = fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=1 loglevel=8";
    let (mut vm, _) =
        RiscvVm::boot_linux(&kernel, initramfs.as_deref(), 256, 3_000_000, bootargs).unwrap();

    println!(
        "After boot_linux: PC=0x{:08X} priv={:?}",
        vm.cpu.pc, vm.cpu.privilege
    );
    println!("SBI output: {} bytes", vm.bus.sbi.console_output.len());
    println!("mtimecmp: {}", vm.bus.clint.mtimecmp);
    println!("mtime: {}", vm.bus.clint.mtime);
    println!();

    // Now step manually to trace what happens
    let mut prev_pc = vm.cpu.pc;
    let mut stall = 0u64;
    let mut sbi_count_before = vm.bus.sbi.console_output.len();
    let mut last_progress = 0u64;

    for i in 0u64..2_000_000 {
        let pc = vm.cpu.pc;

        if pc == prev_pc {
            stall += 1;
        } else {
            if stall > 100 {
                // Report stall ended
                if stall > 10000 {
                    println!("Stall ended: {} instructions at 0x{:08X}", stall, prev_pc);
                }
            }
            stall = 0;
            prev_pc = pc;
        }

        vm.step();

        // Check for new SBI output
        let new_len = vm.bus.sbi.console_output.len();
        if new_len > sbi_count_before {
            let s = String::from_utf8_lossy(&vm.bus.sbi.console_output[sbi_count_before..]);
            sbi_count_before = new_len;
            println!("[{}] SBI output: {:?}", i, s);
        }

        // Progress report every 500K
        if i - last_progress >= 500_000 {
            last_progress = i;
            println!(
                "[{}] PC=0x{:08X} priv={:?} SBI={}B stall={}",
                i, vm.cpu.pc, vm.cpu.privilege, new_len, stall
            );
        }

        // Hard stop if stalling too long
        if stall > 1_000_000 {
            println!(
                "[{}] Hard stall at 0x{:08X} for {} instructions",
                i, vm.cpu.pc, stall
            );
            break;
        }
    }

    println!(
        "\nFinal: PC=0x{:08X} priv={:?}",
        vm.cpu.pc, vm.cpu.privilege
    );
    println!("SBI output: {} bytes", vm.bus.sbi.console_output.len());
    if !vm.bus.sbi.console_output.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.sbi.console_output);
        println!("{}", s);
    }
}
