use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    match RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        256,
        1_000_000,
        "console=ttyS0 earlycon=sbi panic=1",
    ) {
        Ok((vm, br)) => {
            eprintln!(
                "instr={} ecall={} console={}",
                br.instructions,
                vm.cpu.ecall_count,
                vm.bus.sbi.console_output.len()
            );
            eprintln!("priv={:?} PC=0x{:08X}", vm.cpu.privilege, vm.cpu.pc);
            eprintln!(
                "sstatus=0x{:08X} satp=0x{:08X}",
                vm.cpu.csr.mstatus & 0x000001FF,
                vm.cpu.csr.satp
            );

            // Check if we're stuck at fw_addr (M-mode trap loop)
            let fw_addr = {
                let first_vaddr = 0xC0000000u64; // known from boot setup
                first_vaddr + 0x940_000
            };
            eprintln!("fw_addr=0x{:X}", fw_addr);
            eprintln!("at_fw_addr: {}", vm.cpu.pc == fw_addr as u32);
        }
        Err(e) => eprintln!("err: {:?}", e),
    }
}
