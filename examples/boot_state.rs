use std::fs;
use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::Privilege;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    
    let kernel_image = fs::read(kernel_path).unwrap();
    let initramfs = fs::read(initramfs_path).ok();
    
    let bootargs = "console=ttyS0 earlycon=sbi panic=1 quiet";
    let (mut vm, _r) = RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        256,
        200_000_000,
        bootargs,
    ).unwrap();
    
    println!("PC: 0x{:08X}", vm.cpu.pc);
    println!("Privilege: {:?}", vm.cpu.privilege);
    println!("satap: 0x{:08X}", vm.cpu.csr.satp);
    println!("mstatus: 0x{:08X}", vm.cpu.csr.mstatus);
    println!("mepc: 0x{:08X}", vm.cpu.csr.mepc);
    println!("sepc: 0x{:08X}", vm.cpu.csr.sepc);
    println!("mcause: 0x{:08X}", vm.cpu.csr.mcause);
    println!("stvec: 0x{:08X}", vm.cpu.csr.stvec);
    println!("SBI: {} bytes", vm.bus.sbi.console_output.len());
    println!("mtimecmp: {}", vm.bus.clint.mtimecmp);
    let mpp = (vm.cpu.csr.mstatus >> 11) & 3;
    let mpp_name = if mpp == 0 { "U" } else if mpp == 1 { "S" } else if mpp == 3 { "M" } else { "?" };
    println!("MPP: {} ({})", mpp, mpp_name);
}
