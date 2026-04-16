/// Diagnostic: count ALL writes in device MMIO ranges.
/// This tells us if the kernel is trying to access any devices.

use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    let (mut vm, fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();

    let max = 2_000_000u64;
    let mut count: u64 = 0;
    let fw_addr_u32 = fw_addr as u32;

    // Counters for different MMIO ranges
    let mut uart_writes: u64 = 0;
    let mut clint_writes: u64 = 0;
    let mut plic_writes: u64 = 0;
    let mut virtio_writes: u64 = 0;
    let mut high_writes: u64 = 0;
    let mut uart_reads: u64 = 0;
    let mut clint_reads: u64 = 0;
    let mut low_reads: u64 = 0;

    // First, read some instructions manually and check for store instructions to MMIO addresses
    // Instead, let's just watch the bus activity by checking if we can hook in.
    // Actually, let me just run and check the UART/clint state afterward.

    while count < max {
        if vm.bus.sbi.shutdown_requested { break; }

        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine {
            let cause_code = vm.cpu.csr.mcause & !(1u32 << 31);
            if cause_code == 9 || cause_code == 11 {
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16],
                    vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13],
                    vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0, a1)) = result {
                    vm.cpu.x[10] = a0; vm.cpu.x[11] = a1;
                }
            } else {
                let mpp = (vm.cpu.csr.mstatus & 0x1800) >> 11;
                if mpp != 3 {
                    let stvec = vm.cpu.csr.stvec & !0x3u32;
                    if stvec != 0 {
                        vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                        vm.cpu.csr.scause = vm.cpu.csr.mcause;
                        vm.cpu.csr.stval = vm.cpu.csr.mtval;
                        vm.cpu.pc = stvec;
                        vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
                        vm.cpu.tlb.flush_all();
                        count += 1;
                        continue;
                    }
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        let _ = vm.step();
        count += 1;
    }

    // Check device state
    println!("=== After {} instructions ===", count);
    println!("UART: ier={} lcr={} lsr=0x{:02X} mcr={} fcr={}",
        vm.bus.uart.ier, vm.bus.uart.lcr, vm.bus.uart.lsr,
        vm.bus.uart.mcr, vm.bus.uart.fcr);
    println!("UART tx_buf: {} chars", vm.bus.uart.tx_buf.len());
    println!("PC=0x{:08X} priv={:?} SATP=0x{:08X}", vm.cpu.pc, vm.cpu.privilege, vm.cpu.csr.satp);
    println!("SIE={} STVEC=0x{:08X} MEDELEG=0x{:08X}",
        (vm.cpu.csr.mstatus >> 1) & 1, vm.cpu.csr.stvec, vm.cpu.csr.medeleg);
}
