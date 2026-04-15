// Test boot with cgroup disabled
use std::fs;
use geometry_os::riscv::RiscvVm;

fn run_boot(max_instr: u64, bootargs: &str) {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel = fs::read(kernel_path).unwrap();
    let initramfs = fs::read(initramfs_path).ok();

    let (mut vm, _) = RiscvVm::boot_linux(
        &kernel, initramfs.as_deref(), 256, max_instr, bootargs,
    ).unwrap();

    let sbi_len = vm.bus.sbi.console_output.len();
    
    println!("{:>8} instrs: PC=0x{:08X} priv={:?} | SBI={}B",
             max_instr, vm.cpu.pc, vm.cpu.privilege, sbi_len);
    
    if sbi_len > 0 {
        let s = String::from_utf8_lossy(&vm.bus.sbi.console_output);
        let lines: Vec<&str> = s.lines().collect();
        println!("  SBI output ({} lines):", lines.len());
        for line in lines.iter().take(10) {
            println!("    {}", line);
        }
        if lines.len() > 10 {
            println!("    ... and {} more lines", lines.len() - 10);
        }
    }
}

fn main() {
    println!("=== Boot with cgroup disabled ===\n");
    
    let configs = [
        "console=ttyS0 earlycon=sbi panic=1 cgroup_disable=memory",
        "console=ttyS0 earlycon=sbi panic=1 cgroup_no_v1=all",
        "console=ttyS0 earlycon=sbi panic=1 rdinit=/bin/sh",
    ];
    
    for args in &configs {
        println!("Bootargs: {}", args);
        run_boot(5_000_000, args);
        println!();
    }
}
