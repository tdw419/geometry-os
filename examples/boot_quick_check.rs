fn main() {
    // Test: boot with A/D bit updates disabled to see if it's the cause
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    use geometry_os::riscv::cpu::{Privilege, StepResult};
    use geometry_os::riscv::csr;
    use geometry_os::riscv::mmu;
    use geometry_os::riscv::RiscvVm;

    let (mut vm, boot_result) = RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        256,
        20_000_000,
        bootargs,
    )
    .unwrap();

    println!(
        "Boot: {} instr, PC=0x{:08X}",
        boot_result.instructions, vm.cpu.pc
    );
    println!("UART output: {} chars", vm.bus.uart.tx_buf.len());
    if !vm.bus.uart.tx_buf.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.uart.tx_buf);
        let preview: String = s.chars().take(500).collect();
        println!("UART: {}", preview);
    }

    // Check final TLB size
    println!("TLB entries: {}", vm.cpu.tlb.valid_count());

    // Check the PTE
    let satp = vm.cpu.csr.read(csr::SATP);
    let ppn = satp & 0x3FFFFF;
    let pt_base = (ppn as u64) << 12;
    let l1_addr = pt_base + (770u64) * 4;
    let l1_pte = vm.bus.read_word(l1_addr).unwrap_or(0);
    println!("L1[770] = 0x{:08X} V={}", l1_pte, l1_pte & 1);
}
