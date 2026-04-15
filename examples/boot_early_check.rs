fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    use geometry_os::riscv::RiscvVm;
    use geometry_os::riscv::cpu::{Privilege, StepResult};
    use geometry_os::riscv::csr;

    // Boot with a small limit to see early behavior
    let (mut vm, boot_result) = RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        256,
        1_000_000,  // 1M instructions - early boot
        bootargs,
    )
    .unwrap();

    println!("After 1M instr: PC=0x{:08X} priv={:?}", vm.cpu.pc, vm.cpu.privilege);
    println!("SBI console: {} chars", vm.bus.sbi.console_output.len());
    println!("UART tx: {} chars", vm.bus.uart.tx_buf.len());
    println!("mstatus: 0x{:08X}", vm.cpu.csr.mstatus);
    println!("SIE: {}", (vm.cpu.csr.mstatus >> csr::MSTATUS_SIE) & 1);
    println!("sstatus (mstatus): 0x{:08X}", vm.cpu.csr.mstatus);
    println!("satp: 0x{:08X}", vm.cpu.csr.satp);
    println!("stvec: 0x{:08X}", vm.cpu.csr.stvec);
    println!("sstatus.SIE: {}", (vm.cpu.csr.mstatus >> csr::MSTATUS_SIE) & 1);

    // Check if SIE bit is being blocked
    let sie = (vm.cpu.csr.mstatus >> csr::MSTATUS_SIE) & 1;
    let spp = (vm.cpu.csr.mstatus >> csr::MSTATUS_SPP) & 1;
    println!("SPP: {} (1=S-mode)", spp);

    // Now try with 5M
    let (mut vm2, _) = RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        256,
        5_000_000,
        bootargs,
    )
    .unwrap();

    println!("\nAfter 5M instr: PC=0x{:08X} priv={:?}", vm2.cpu.pc, vm2.cpu.privilege);
    println!("SBI console: {} chars", vm2.bus.sbi.console_output.len());
    println!("UART tx: {} chars", vm2.bus.uart.tx_buf.len());
    println!("satp: 0x{:08X}", vm2.cpu.csr.satp);
    println!("stvec: 0x{:08X}", vm2.cpu.csr.stvec);

    // Now try with 10M
    let (mut vm3, _) = RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        256,
        10_000_000,
        bootargs,
    )
    .unwrap();

    println!("\nAfter 10M instr: PC=0x{:08X} priv={:?}", vm3.cpu.pc, vm3.cpu.privilege);
    println!("SBI console: {} chars", vm3.bus.sbi.console_output.len());
    println!("UART tx: {} chars", vm3.bus.uart.tx_buf.len());

    // Check MMU log for first few events
    if !vm3.bus.mmu_log.is_empty() {
        println!("\nFirst 5 MMU events:");
        for event in vm3.bus.mmu_log.iter().take(5) {
            println!("  {:?}", event);
        }
        println!("Last 5 MMU events:");
        for event in vm3.bus.mmu_log.iter().rev().take(5) {
            println!("  {:?}", event);
        }
    }
}
