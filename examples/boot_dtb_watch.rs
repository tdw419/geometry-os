// Watch when _dtb_early_va at PA 0x00801008 gets overwritten
use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::StepResult;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, _fw_addr, _entry, dtb_addr) =
        RiscvVm::boot_linux_setup(
            &kernel_image,
            initramfs.as_deref(),
            256,
            "console=ttyS0 loglevel=8",
        )
        .unwrap();

    let target_pa: u64 = 0x00801008; // _dtb_early_va
    let expected_va = dtb_addr.wrapping_add(0xC0000000) as u32;

    eprintln!("Watching PA 0x{:08X} for changes (expect 0x{:08X})", target_pa, expected_va);

    let mut last_val = vm.bus.read_word(target_pa).unwrap_or(0);
    let mut change_count = 0;
    let max_changes = 5;
    let max_count = 1_000_000;

    for count in 0..max_count {
        vm.step();

        if count % 1000 == 0 {
            let cur = vm.bus.read_word(target_pa).unwrap_or(0);
            if cur != last_val {
                change_count += 1;
                eprintln!(
                    "[{}] PA 0x{:08X}: 0x{:08X} -> 0x{:08X} (PC=0x{:08X})",
                    count, target_pa, last_val, cur, vm.cpu.pc
                );
                last_val = cur;
                if change_count >= max_changes {
                    break;
                }
            }
        }
    }

    // Also check after each SATP change
    eprintln!("\nFinal _dtb_early_va = 0x{:08X} (expect 0x{:08X})",
        vm.bus.read_word(target_pa).unwrap_or(0), expected_va);
}
