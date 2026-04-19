use geometry_os::riscv::RiscvVm;
use std::fs;

fn read_string(vm: &mut RiscvVm, addr: u64, max_len: usize) -> String {
    let mut bytes = Vec::new();
    for i in 0..max_len {
        match vm.bus.read_byte(addr + i as u64) {
            Ok(b) if b == 0 => break,
            Ok(b) => bytes.push(b),
            Err(_) => break,
        }
    }
    String::from_utf8_lossy(&bytes).to_string()
}

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let initramfs = fs::read(initramfs_path).ok();

    let (mut vm, _fw, _entry, _dtb) = RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        256,
        "console=ttyS0 loglevel=8",
    )
    .unwrap();

    // Read the panic string at VA 0xC1408AC8 (from a1 in the panic)
    let va = 0xC1408AC8u64;
    let s = read_string(&mut vm, va, 300);
    println!("VA 0x{:08X}: \"{}\"", va, s);

    // Also check some well-known panic strings
    // The kernel's __warn_printk should have written the panic string
    // Let's search the kernel for panic-related strings
    let panic_candidates = [
        "Kernel panic",
        "not syncing",
        "Attempted to kill",
        "BUG: unable to",
        "out of memory",
        "No init found",
        "Failed to execute",
        "VFS: Cannot open",
        "init",
    ];

    // Read from the kernel .rodata section (PA 0x00D0xxxx range)
    for offset in (0x00C00000..0x01400000).step_by(0x1000) {
        let mut buf = [0u8; 256];
        let mut _found = false;
        for i in 0..256 {
            if let Ok(b) = vm.bus.read_byte(offset + i as u64) {
                buf[i] = b;
            }
        }
        let text = String::from_utf8_lossy(&buf);
        for &candidate in &panic_candidates {
            if text.contains(candidate) {
                println!("Found \"{}\" at PA 0x{:08X}", candidate, offset);
                // Show context
                let pos = text.find(candidate).unwrap();
                let start = pos.saturating_sub(20);
                let end = (pos + candidate.len() + 60).min(text.len());
                println!("  Context: \"{}\"", &text[start..end]);
                _found = true;
            }
        }
    }
}
