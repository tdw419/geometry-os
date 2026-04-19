// Diagnostic: use MMU translate directly to check what PA the MMU returns
use std::fs;
use std::time::Instant;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    let start = Instant::now();
    let (mut vm, result) = geometry_os::riscv::RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        512,
        20_000_000u64,
        bootargs,
    )
    .unwrap();
    let elapsed = start.elapsed();
    println!(
        "Boot: {} instructions in {:?}",
        result.instructions, elapsed
    );
    println!("PC: 0x{:08X}, Privilege: {:?}", vm.cpu.pc, vm.cpu.privilege);
    println!("SATP: 0x{:08X}", vm.cpu.csr.satp);

    let ra = vm.cpu.x[1];
    let sp = vm.cpu.x[2];
    println!("\nRA=0x{:08X} SP=0x{:08X}", ra, sp);

    // Test: read through bus (which goes through MMU for S-mode)
    // and compare with direct physical read
    let ra_via_bus = vm.bus.read_word(ra as u64).unwrap_or(0xFFFFFFFF);
    println!("RA via bus.read_word(VA): 0x{:08X}", ra_via_bus);

    // Manually compute expected PA from L1 table
    let satp = vm.cpu.csr.satp;
    let pg_dir_phys = ((satp & 0x3FFFFF) as u64) * 4096;
    let ra_vpn1 = ((ra >> 22) & 0x3FF) as u64;
    let l1_entry = vm.bus.read_word(pg_dir_phys + ra_vpn1 * 4).unwrap_or(0);
    let l1_ppn = ((l1_entry >> 10) & 0x3FFFFF) as u64;
    let is_leaf = (l1_entry & 0xE) != 0;

    println!(
        "L1 entry for RA: 0x{:08X}, PPN=0x{:06X}, is_leaf={}",
        l1_entry, l1_ppn, is_leaf
    );

    if is_leaf {
        // Megapage
        let ppn_hi = (l1_ppn >> 10) & 0xFFF;
        let ra_vpn0 = ((ra >> 12) & 0x3FF) as u64;
        let ra_off = (ra & 0xFFF) as u64;
        let expected_pa = (ppn_hi << 22) | (ra_vpn0 << 12) | ra_off;
        println!("Expected PA (manual): 0x{:08X}", expected_pa);
        let direct_read = vm.bus.read_word(expected_pa).unwrap_or(0xFFFFFFFF);
        println!("Direct read at expected PA: 0x{:08X}", direct_read);
    } else {
        // L2 lookup
        let l2_base = l1_ppn * 4096;
        let ra_vpn0 = ((ra >> 12) & 0x3FF) as u64;
        let l2_entry = vm.bus.read_word(l2_base + ra_vpn0 * 4).unwrap_or(0);
        let l2_ppn = ((l2_entry >> 10) & 0x3FFFFF) as u64;
        let ra_off = (ra & 0xFFF) as u64;
        let expected_pa = (l2_ppn * 4096) + ra_off;
        println!("L2 entry: 0x{:08X}, PPN=0x{:06X}", l2_entry, l2_ppn);
        println!("Expected PA (manual): 0x{:08X}", expected_pa);
        let direct_read = vm.bus.read_word(expected_pa).unwrap_or(0xFFFFFFFF);
        println!("Direct read at expected PA: 0x{:08X}", direct_read);
    }

    // Now check: what does bus.read_word actually do for a VA?
    // In S-mode, bus.read_word should translate through MMU
    // Let's also check if auto_pte_fixup is affecting things
    println!("\nauto_pte_fixup: {}", vm.bus.auto_pte_fixup);

    // Check UART output
    let tx = vm.bus.uart.drain_tx();
    if !tx.is_empty() {
        let s = String::from_utf8_lossy(&tx);
        println!("\nUART output:\n{}", s);
    }

    let sbi_out = &vm.bus.sbi.console_output;
    if !sbi_out.is_empty() {
        let s = String::from_utf8_lossy(sbi_out);
        println!("\nSBI console output:\n{}", s);
    }
}
