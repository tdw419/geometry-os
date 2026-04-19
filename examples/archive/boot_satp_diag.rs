use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, _br) = RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        256,
        200_000,
        "earlycon=sbi loglevel=8",
    )
    .unwrap();

    // Check what's at the two page table roots
    let satp = vm.cpu.csr.satp;
    let ppn = satp & 0x3FFFFF;
    let pg_dir = (ppn as u64) << 12;
    eprintln!(
        "Final SATP: 0x{:08X} ppn=0x{:08X} pg_dir_phys=0x{:08X}",
        satp, ppn, pg_dir
    );

    // Check L1 entries for the trampoline range
    // VA 0x3FFFF000: VPN1 = 0x3FFFF000 >> 22 = 0xFF, VPN0 = (0x3FFFF000 >> 12) & 0x3FF = 0xFF0
    eprintln!(
        "VA 0x3FFFF000: VPN1=0x{:03X} VPN0=0x{:03X}",
        0x3FFFF000 >> 22,
        (0x3FFFF000 >> 12) & 0x3FF
    );

    // Dump L1[0xFF] for both page tables
    for pt_name in &["0x00802000", "0x01484000"] {
        let pt_base: u64 = u32::from_str_radix(&pt_name[2..], 16).unwrap() as u64;
        eprintln!(
            "
Page table at {}:",
            pt_name
        );
        // Check L1[0xFF] (trampoline)
        let l1_ff = vm.bus.read_word(pt_base + 0xFF * 4).unwrap_or(0xDEAD);
        eprintln!("  L1[0xFF] = 0x{:08X} (VA 0x3FC00000-0x3FFFFFFF)", l1_ff);
        // Check L1[0x300] (kernel at 0xC0000000)
        let l1_300 = vm.bus.read_word(pt_base + 0x300 * 4).unwrap_or(0xDEAD);
        eprintln!("  L1[0x300] = 0x{:08X} (VA 0xC0000000)", l1_300);
        // Check L1[0x00] (identity low)
        let l1_0 = vm.bus.read_word(pt_base + 0x00 * 4).unwrap_or(0xDEAD);
        eprintln!("  L1[0x00] = 0x{:08X} (VA 0x00000000)", l1_0);
        // Check L1[0x01] (identity)
        let l1_1 = vm.bus.read_word(pt_base + 0x01 * 4).unwrap_or(0xDEAD);
        eprintln!("  L1[0x01] = 0x{:08X} (VA 0x00400000)", l1_1);
    }

    // Check what stvec=0xC0210F14 points to in memory
    eprintln!(
        "
stvec=0xC0210F14:"
    );
    // Read a few instructions at that VA (need to translate through current page table)
    for off in (0..16).step_by(4) {
        let addr = 0xC0210F14u32.wrapping_add(off) as u64;
        // Read from bus directly (physical address)
        // We'd need to translate VA->PA but just read the raw PA assuming kernel VA mapping
        let pa = addr - 0xC0000000; // phys = virt - PAGE_OFFSET
        let inst = vm.bus.read_word(pa).unwrap_or(0xDEAD);
        eprintln!("  PA 0x{:08X} (VA 0x{:08X}): inst=0x{:08X}", pa, addr, inst);
    }

    // Check the UART tx_buf and SBI console output
    eprintln!(
        "
UART tx_buf len: {}",
        vm.bus.uart.tx_buf.len()
    );
    eprintln!(
        "SBI console output len: {}",
        vm.bus.sbi.console_output.len()
    );
    if !vm.bus.sbi.console_output.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.sbi.console_output);
        eprintln!("SBI output: {}", s);
    }
    if !vm.bus.uart.tx_buf.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.uart.tx_buf);
        eprintln!("UART output: {}", s);
    }

    eprintln!(
        "
Final state: PC=0x{:08X} priv={:?} mstatus=0x{:08X}",
        vm.cpu.pc, vm.cpu.privilege, vm.cpu.csr.mstatus
    );
    eprintln!(
        "scause=0x{:08X} sepc=0x{:08X} stval=0x{:08X} stvec=0x{:08X}",
        vm.cpu.csr.scause, vm.cpu.csr.sepc, vm.cpu.csr.stval, vm.cpu.csr.stvec
    );
}
