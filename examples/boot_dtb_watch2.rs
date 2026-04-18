// Narrow down exactly when _dtb_early_va at PA 0x00801008 gets overwritten
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, _fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(
            &kernel_image,
            initramfs.as_deref(),
            256,
            "console=ttyS0 loglevel=8",
        )
        .unwrap();

    let target_pa: u64 = 0x00801008;
    let mut last_val = vm.bus.read_word(target_pa).unwrap_or(0);

    // Fast-forward to 177000, then check every instruction
    for count in 0..200_000 {
        vm.step();
        if count >= 177_000 {
            let cur = vm.bus.read_word(target_pa).unwrap_or(0);
            if cur != last_val {
                eprintln!(
                    "[{}] PA 0x{:08X}: 0x{:08X} -> 0x{:08X} (PC=0x{:08X}, SP=0x{:08X})",
                    count, target_pa, last_val, cur, vm.cpu.pc, vm.cpu.x[2]
                );
                // Disassemble the instruction at PC
                eprintln!("  a0=0x{:08X} a1=0x{:08X} s0=0x{:08X} s1=0x{:08X}",
                    vm.cpu.x[10], vm.cpu.x[11], vm.cpu.x[8], vm.cpu.x[9]);
                break;
            }
        }
    }
}
