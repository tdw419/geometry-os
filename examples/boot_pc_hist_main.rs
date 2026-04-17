use geometry_os::riscv::RiscvVm;
use std::collections::HashMap;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, _, _, _) = RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        256,
        "console=ttyS0 loglevel=8",
    ).unwrap();
    
    let mut pc_hist: HashMap<u32, u64> = HashMap::new();
    let mut count = 0u64;
    
    // Sample PCs for 5M instructions (starting after 5M warmup)
    while count < 10_000_000 {
        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        let _ = vm.step();
        
        if count >= 5_000_000 {
            // Sample every 10th instruction
            if count % 10 == 0 {
                *pc_hist.entry(vm.cpu.pc).or_insert(0) += 1;
            }
        }
        count += 1;
    }
    
    // Print top 30 most-executed PCs
    let mut sorted: Vec<_> = pc_hist.iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(a.1));
    eprintln!("Top 30 most-executed PCs (5M-10M range, sampled every 10th):");
    for (i, (pc, count)) in sorted.iter().take(30).enumerate() {
        let inst = vm.bus.read_word(**pc as u64).unwrap_or(0);
        let half = vm.bus.read_half(**pc as u64).unwrap_or(0);
        eprintln!("{:3}. PC=0x{:08X}: {} hits  inst=0x{:08X}", i+1, pc, count, inst);
    }
}
