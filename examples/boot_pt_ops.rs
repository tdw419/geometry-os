use std::fs;
use std::time::Instant;
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    let (mut vm, fw_addr, _entry, _dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        512,
        bootargs,
    ).unwrap();

    // pt_ops is at VA 0xC0801000 (PA 0x00801000)
    // It contains 6 function pointers (24 bytes):
    //   [0] get_pgd (used for early page table during setup_vm)
    //   [4] set_pgd
    //   [8] get_pte
    //   [12] set_pte
    //   [16] alloc_pgtable  
    //   [20] free_pgtable
    let pt_ops_pa: u64 = 0x00801000;
    eprintln!("=== pt_ops (PA 0x{:08X}) ===", pt_ops_pa);
    for i in 0..6 {
        let val = vm.bus.read_word(pt_ops_pa + i * 4).unwrap_or(0);
        eprintln!("  pt_ops[{}] = 0x{:08X}", i, val);
    }

    // kernel_map at PA 0x00C7A098
    let km_pa: u64 = 0x00C7A098;
    eprintln!("\n=== kernel_map (PA 0x{:08X}) ===", km_pa);
    for i in 0..8 {
        let val = vm.bus.read_word(km_pa + i * 4).unwrap_or(0);
        eprintln!("  kernel_map[{}] = 0x{:08X}  (offset {})", i, val, i * 4);
    }

    // Also check what create_pgd_mapping's callback pointer would be
    // The function loads from s6 (pt_ops) offset 0: lw a5, 0(s6)
    // So a5 = pt_ops[0] = get_pgd/early_pg_dir pointer
    // This is called from setup_vm before it switches to final page tables
    
    // Check the current SATP page table
    let satp = vm.cpu.csr.satp;
    let pg_dir_ppn = (satp & 0x3FFFFF) as u64;
    let pg_dir_phys = pg_dir_ppn * 4096;
    eprintln!("\n=== Initial boot page table (SATP=0x{:08X}, pg_dir PA=0x{:08X}) ===", satp, pg_dir_phys);
    for idx in [768, 769, 770, 771, 772, 773, 774, 775, 776, 777, 0, 1, 2, 3, 4, 5, 8, 48, 64] {
        let pte = vm.bus.read_word(pg_dir_phys + idx as u64 * 4).unwrap_or(0);
        eprintln!("  L1[{}] = 0x{:08X}", idx, pte);
    }

    // Now run for 200K instructions to get past setup_vm's initial phase
    let fw_addr_u32 = fw_addr as u32;
    let max_count: u64 = 200_000;
    let mut count: u64 = 0;
    while count < max_count {
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code == 9 {
                // ECALL_S = SBI call
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0, a1)) = result {
                    vm.cpu.x[10] = a0;
                    vm.cpu.x[11] = a1;
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }
        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        vm.step();
        count += 1;
    }
    
    eprintln!("\n=== After 200K instructions ===");
    eprintln!("PC=0x{:08X} SP=0x{:08X} RA=0x{:08X}", vm.cpu.pc, vm.cpu.x[2], vm.cpu.x[1]);
    eprintln!("SATP=0x{:08X}", vm.cpu.csr.satp);
    
    // Check pt_ops again after kernel has run
    eprintln!("\n=== pt_ops after 200K instructions ===");
    for i in 0..6 {
        let val = vm.bus.read_word(pt_ops_pa + i * 4).unwrap_or(0);
        eprintln!("  pt_ops[{}] = 0x{:08X}", i, val);
    }
    
    // Check kernel_map again
    eprintln!("\n=== kernel_map after 200K instructions ===");
    for i in 0..8 {
        let val = vm.bus.read_word(km_pa + i * 4).unwrap_or(0);
        eprintln!("  kernel_map[{}] = 0x{:08X}  (offset {})", i, val, i * 4);
    }
    
    // Check phys_ram_base
    let prb = vm.bus.read_word(0x00C7A0B4).unwrap_or(0);
    eprintln!("\nphys_ram_base = 0x{:08X}", prb);
    
    let elapsed = Instant::now().elapsed();
    eprintln!("\nTime: {:?}", elapsed);
}
