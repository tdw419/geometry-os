// Check if DTB is readable at VA through the MMU after setup_vm
use geometry_os::riscv::mmu;
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, _fw_addr, _entry, dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        256,
        "console=ttyS0 loglevel=8",
    )
    .unwrap();

    let dtb_pa = dtb_addr as u32;
    let dtb_va = dtb_pa.wrapping_add(0xC0000000);

    eprintln!("DTB PA: 0x{:08X}, VA: 0x{:08X}", dtb_pa, dtb_va);

    // Check PA directly (should always work)
    let magic_pa = vm.bus.read_word(dtb_pa as u64).unwrap_or(0);
    eprintln!(
        "DTB magic at PA 0x{:08X}: 0x{:08X} (expect 0xD00DFEED)",
        dtb_pa, magic_pa
    );

    // Run until after 3rd SATP change (after all fixups), then check MMU translation
    let mut satp_count = 0u32;
    let max_count = 1_000_000;

    for count in 0..max_count {
        let _ = vm.step();
        let satp = vm.cpu.csr.read(0x180); // SATP
                                           // Use a simple change detection
        if count % 1000 == 0 && satp != 0 && satp & 0x80000000 != 0 {
            // MMU is on, check translation
            let result = mmu::translate(
                dtb_va,
                mmu::AccessType::Load,
                geometry_os::riscv::cpu::Privilege::Supervisor,
                false, // sum
                false, // mxr
                satp,
                &mut vm.bus,
                &mut vm.cpu.tlb,
            );
            match result {
                mmu::TranslateResult::Ok(pa) => {
                    let word = vm.bus.read_word(pa).unwrap_or(0);
                    eprintln!(
                        "[{}] DTB VA 0x{:08X} -> PA 0x{:08X}, word=0x{:08X} (expect 0xD00DFEED)",
                        count, dtb_va, pa, word
                    );
                    if word == 0xD00DFEED {
                        eprintln!("DTB IS READABLE THROUGH MMU!");
                        return;
                    }
                }
                _ => {
                    eprintln!(
                        "[{}] DTB VA 0x{:08X} -> FAULT (satp=0x{:08X})",
                        count, dtb_va, satp
                    );
                }
            }
        }
    }
}
