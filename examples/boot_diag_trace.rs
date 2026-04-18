use geometry_os::riscv::RiscvVm;
use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).unwrap();
    let initramfs = fs::read(initramfs_path).ok();

    let (mut vm, _fw_addr, _entry, dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        256,
        "console=ttyS0 earlycon=sbi panic=1 quiet",
    )
    .unwrap();

    // Dump DTB header
    println!("DTB at PA 0x{:08X}", dtb_addr);
    let magic = vm.bus.read_word(dtb_addr).unwrap_or(0);
    let totalsize = vm.bus.read_word(dtb_addr + 4).unwrap_or(0);
    let off_dt_struct = vm.bus.read_word(dtb_addr + 8).unwrap_or(0);
    let off_dt_strings = vm.bus.read_word(dtb_addr + 12).unwrap_or(0);
    let version = vm.bus.read_word(dtb_addr + 16).unwrap_or(0);
    let last_comp_version = vm.bus.read_word(dtb_addr + 20).unwrap_or(0);

    println!("magic:           0x{:08X} (expected 0xD00DFEED)", magic);
    println!("totalsize:       0x{:08X} ({} bytes)", totalsize, totalsize);
    println!("off_dt_struct:   0x{:08X}", off_dt_struct);
    println!("off_dt_strings:  0x{:08X}", off_dt_strings);
    println!("version:         0x{:08X}", version);
    println!("last_comp_ver:   0x{:08X}", last_comp_version);

    // Check DTB struct first bytes
    println!(
        "\nDTB struct at PA 0x{:08X}:",
        dtb_addr + off_dt_struct as u64
    );
    for i in 0..8 {
        let addr = dtb_addr + off_dt_struct as u64 + (i * 4) as u64;
        if let Ok(word) = vm.bus.read_word(addr) {
            println!("  +0x{:03X}: 0x{:08X}", i * 4, word);
        }
    }

    // Check a1 register (DTB address passed to kernel)
    println!("\nKernel registers at entry:");
    println!("  a0 (hartid):  0x{:08X}", vm.cpu.x[10]);
    println!("  a1 (dtb):     0x{:08X}", vm.cpu.x[11]);
    println!("  sp:           0x{:08X}", vm.cpu.x[2]);
    println!("  SATP:         0x{:08X}", vm.cpu.csr.satp);
    println!("  STVEC:        0x{:08X}", vm.cpu.csr.stvec);
}
