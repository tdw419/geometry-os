/// Quick diagnostic: check SATP value and MMU state after boot attempt
use geometry_os::riscv::RiscvVm;
use std::fs;

fn main() {
    let kernel = fs::read(".geometry_os/build/linux-6.14/vmlinux").unwrap();
    let initramfs = fs::read(".geometry_os/fs/linux/rv32/initramfs.cpio.gz").ok();

    let (mut vm, _) = RiscvVm::boot_linux(
        &kernel,
        initramfs.as_deref(),
        256,
        10_000_000,
        "console=ttyS0 earlycon",
    )
    .unwrap();

    eprintln!("=== Post-boot state ===");
    eprintln!("PC: 0x{:08X}  Priv: {:?}", vm.cpu.pc, vm.cpu.privilege);
    eprintln!("SATP: 0x{:08X}", vm.cpu.csr.satp);
    eprintln!("mstatus: 0x{:08X}", vm.cpu.csr.mstatus);
    eprintln!("stvec: 0x{:08X}", vm.cpu.csr.stvec);
    eprintln!("mtvec: 0x{:08X}", vm.cpu.csr.mtvec);
    eprintln!("medeleg: 0x{:08X}", vm.cpu.csr.medeleg);
    eprintln!("mideleg: 0x{:08X}", vm.cpu.csr.mideleg);
    eprintln!("scause: 0x{:08X}", vm.cpu.csr.scause);
    eprintln!("mcause: 0x{:08X}", vm.cpu.csr.mcause);

    // Check if MMU is active
    let asid = vm.cpu.csr.satp & 0xFF;
    let ppn = vm.cpu.csr.satp >> 10;
    eprintln!(
        "\nSATP breakdown: mode={}, ASID={}, PPN=0x{:X}",
        (vm.cpu.csr.satp >> 31) & 1,
        asid,
        ppn
    );

    if vm.cpu.csr.satp != 0 {
        eprintln!("MMU IS ENABLED (SV32)");
    } else {
        eprintln!("MMU NOT ENABLED (bare mode)");
    }

    // Check UART
    let uart = vm.bus.uart.drain_tx();
    eprintln!("\nUART TX: {} bytes", uart.len());
    if !uart.is_empty() {
        let s = String::from_utf8_lossy(&uart);
        for line in s.lines().take(20) {
            eprintln!("  {}", line);
        }
    }

    // MMU log
    eprintln!("\nMMU events: {} entries", vm.bus.mmu_log.len());
    for ev in vm.bus.mmu_log.iter().take(10) {
        eprintln!("  {:?}", ev);
    }
}
