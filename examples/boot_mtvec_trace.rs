use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel = std::fs::read(".geometry_os/build/linux-6.14/vmlinux").unwrap();
    let initramfs = std::fs::read(".geometry_os/fs/linux/rv32/initramfs.cpio.gz").unwrap();
    
    // Use boot_linux_setup to get the VM before boot
    let (mut vm, fw_addr, entry, _dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel,
        Some(&initramfs),
        64,
        "console=ttyS0 earlycon=sbi",
    ).unwrap();
    
    let fw_addr_u32 = fw_addr as u32;
    println!("fw_addr = 0x{:08X}", fw_addr_u32);
    println!("Initial privilege: {:?}", vm.cpu.privilege);
    println!("Initial mtvec: 0x{:08X}", vm.cpu.csr.mtvec);
    
    // Run 200K instructions with detailed mtvec/priv tracking
    for count in 0..200_000 {
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine {
            // At firmware trap handler - log and skip
            let mcause = vm.cpu.csr.mcause;
            let mpp = (vm.cpu.csr.mstatus & 0x1800) >> 11;
            if mpp != 3 {
                println!("count={}: Trap at fw_addr, mcause=0x{:08X}, mpp={}, privilege after trap_enter={:?}", 
                    count, mcause, mpp, vm.cpu.privilege);
            }
            // Skip: mepc += 4, MRET will execute next step
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }
        
        vm.step();
        
        // Check for mtvec change
        // (The eprintln in csr/bank.rs will show us)
    }
    
    println!("\nFinal mtvec: 0x{:08X}", vm.cpu.csr.mtvec);
    println!("Final privilege: {:?}", vm.cpu.privilege);
}
