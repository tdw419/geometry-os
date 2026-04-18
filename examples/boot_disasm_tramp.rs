use geometry_os::riscv::decode;
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, fw_addr, _, _) = RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        256,
        "console=ttyS0 earlycon=sbi panic=1 quiet",
    )
    .unwrap();

    println!("fw_addr = 0x{:X}", fw_addr);
    println!();

    // Disassemble around the stuck PC addresses
    for base in [0x00001000u64, 0x00001040, 0x00001080, 0x000010C0] {
        println!("--- Disassembly @ 0x{:X} ---", base);
        for offset in (0..64).step_by(2) {
            let addr = base + offset as u64;
            let half = vm.bus.read_half(addr).unwrap_or(0);
            let is_compressed = half & 0x3 != 0x3;
            let word = if is_compressed {
                half as u32
            } else {
                vm.bus.read_word(addr).unwrap_or(0)
            };
            let op = decode::decode(word);
            let op_str = format!("{:?}", op);
            let label = if addr == 0x00001048
                || addr == 0x00001060
                || addr == 0x000010B2
                || addr == 0x000010B8
            {
                " <-- STUCK"
            } else {
                ""
            };
            let display = format!("{}{}", &op_str[..op_str.len().min(60)], label);
            if is_compressed {
                println!("  0x{:08X}: {:04X}       {}", addr, half, display);
            } else {
                println!("  0x{:08X}: {:08X}    {}", addr, word, display);
            }
        }
        println!();
    }
}
