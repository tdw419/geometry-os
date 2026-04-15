
use std::fs;
use std::collections::HashSet;
use geometry_os::riscv::RiscvVm;

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
        50_000_000,
        bootargs,
    ).unwrap();
    
    // Run for 50M more steps, collect unique PCs
    let pc_start = vm.cpu.pc;
    let mut unique_pcs: HashSet<u32> = HashSet::new();
    let mut call_targets: HashSet<u32> = HashSet::new();
    
    for _ in 0..50_000_000 {
        let pc = vm.cpu.pc;
        unique_pcs.insert(pc);
        
        // Track function calls (JAL with rd != 0)
        if let Some(ref last) = vm.cpu.last_step {
            use geometry_os::riscv::decode::Operation;
            match &last.op {
                Operation::Jal { rd, imm } if *rd != 0 => {
                    let target = (pc as i64 + *imm as i64) as u32;
                    call_targets.insert(target);
                }
                _ => {}
            }
        }
        
        vm.step();
    }
    
    let pc_end = vm.cpu.pc;
    println!("Start PC: 0x{:08X}", pc_start);
    println!("End PC: 0x{:08X}", pc_end);
    println!("Unique PCs: {}", unique_pcs.len());
    println!("Function calls to {} unique targets", call_targets.len());
    
    // Find kernel symbols for call targets
    let nm = std::process::Command::new("riscv64-linux-gnu-nm")
        .args(["-n", ".geometry_os/build/linux-6.14/vmlinux"])
        .output().unwrap();
    let nm_out = String::from_utf8_lossy(&nm.stdout);
    
    for target in call_targets.iter().take(20) {
        let t = *target as u64;
        let mut best = String::new();
        let mut best_addr = 0u64;
        for line in nm_out.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                if let Ok(addr) = u64::from_str_radix(parts[0], 16) {
                    if addr <= t && addr > best_addr {
                        best_addr = addr;
                        best = parts[2].to_string();
                    }
                }
            }
        }
        println!("  call 0x{:08X} -> {} +{}", target, best, t - best_addr);
    }
    
    println!("
SBI: {} bytes", vm.bus.sbi.console_output.len());
    let uart: Vec<u8> = vm.bus.uart.drain_tx();
    println!("UART: {} bytes", uart.len());
}
