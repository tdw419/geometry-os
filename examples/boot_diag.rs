fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    use geometry_os::riscv::RiscvVm;
    use geometry_os::riscv::cpu::{Privilege, StepResult};
    use geometry_os::riscv::csr;

    let (mut vm, boot_result) = RiscvVm::boot_linux(
        &kernel_image, initramfs.as_deref(), 256, 20_000_000, bootargs
    ).unwrap();

    println!("=== Boot Result ===");
    println!("Instructions: {}", boot_result.instructions);
    println!("PC: 0x{:08X}", vm.cpu.pc);
    println!("Privilege: {:?}", vm.cpu.privilege);
    println!("SP: 0x{:08X}", vm.cpu.x[2]);
    println!("SATP: 0x{:08X}", vm.cpu.csr.read(csr::SATP));
    println!("SSTATUS: 0x{:08X}", vm.cpu.csr.read(csr::SSTATUS));
    println!("STVEC: 0x{:08X}", vm.cpu.csr.read(csr::STVEC));
    println!("SIE: {}", (vm.cpu.csr.read(csr::SSTATUS) >> 1) & 1);
    println!("SBI output: {} chars", vm.bus.sbi.console_output.len());
    println!("UART tx: {} chars", vm.bus.uart.tx_buf.len());
    
    // Check page table PTE at index 770 (the one that gets corrupted)
    let satp = vm.cpu.csr.read(csr::SATP);
    let ppn = satp & 0x3FFFFF;
    let pt_base = (ppn as u64) << 12;
    let l1_addr = pt_base + (770u64) * 4;
    let l1_pte = vm.bus.read_word(l1_addr).unwrap_or(0);
    println!("L1[770] at 0x{:08X} = 0x{:08X} V={}", l1_addr, l1_pte, l1_pte & 1);
    
    // Check if handle_exception (0xC08EFF1C) is in a mapped page
    let he_vpn = (0xC08EFF1Cu32) >> 12;
    let he_pte_idx = (he_vpn >> 10) as u64;
    let he_pte_addr = pt_base + he_pte_idx * 4;
    let he_pte = vm.bus.read_word(he_pte_addr).unwrap_or(0);
    println!("handle_exception page VPN={} L1 idx={} PTE at 0x{:X} = 0x{:08X} V={}",
        he_vpn, he_pte_idx, he_pte_addr, he_pte, he_pte & 1);
    
    // Print first 200 chars of any output
    if !vm.bus.uart.tx_buf.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.uart.tx_buf);
        println!("UART: {}", &s[..s.len().min(200)]);
    }
    if !vm.bus.sbi.console_output.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.sbi.console_output);
        println!("SBI console: {}", &s[..s.len().min(200)]);
    }
}
