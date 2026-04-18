// Check what udelay is waiting for - diagnostic
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = std::path::Path::new(".geometry_os/build/linux-6.14/vmlinux");
    let kernel = std::fs::read(kernel_path).expect("kernel");
    let initramfs_path = std::path::Path::new(".geometry_os/fs/linux/rv32/initramfs.cpio.gz");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, _fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel, initramfs.as_deref(), 128, "console=ttyS0 earlycon=sbi").unwrap();

    // Run until we hit udelay (PC in range 0xC020B0D0-0xC020B170)
    let mut count: u64 = 0;
    let max = 1_000_000;
    let udelay_start: u32 = 0xC020B0D0;
    let udelay_end: u32 = 0xC020B170;

    loop {
        let _result = vm.step();
        count += 1;
        if count > max { break; }
        if vm.cpu.pc >= udelay_start && vm.cpu.pc < udelay_end {
            // We hit udelay - dump registers and lpj_fine
            println!("[diag] Hit udelay at count={}, PC=0x{:08X}", count, vm.cpu.pc);
            println!("[diag] a0={}, a1={}, a2={}, a3={}, a4={}, a5={}", 
                vm.cpu.x[10], vm.cpu.x[11], vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15]);
            
            // Read lpj_fine from VA 0xC1482060 (PA = 0x01482060)
            let lpj_pa = 0x01482060;
            let lpj_fine = vm.bus.read_word(lpj_pa).unwrap_or(0);
            println!("[diag] lpj_fine = {} (0x{:08X})", lpj_fine, lpj_fine);
            
            // Read riscv_timebase from VA 0xC0C79E54 (PA = 0x00C79E54)
            let tb_pa = 0x00C79E54;
            let timebase = vm.bus.read_word(tb_pa).unwrap_or(0);
            println!("[diag] riscv_timebase = {} (0x{:08X})", timebase, timebase);
            
            // Check CLINT mtime
            println!("[diag] CLINT mtime = {}", vm.bus.clint.mtime);
            
            // Run a few more steps to see if udelay completes
            let start_mtime = vm.bus.clint.mtime;
            let mut udelay_count = 0u64;
            loop {
                let _r = vm.step();
                udelay_count += 1;
                count += 1;
                // Check if we left udelay
                if vm.cpu.pc < udelay_start || vm.cpu.pc >= udelay_end {
                    println!("[diag] udelay completed after {} instructions ({} mtime ticks)", 
                        udelay_count, vm.bus.clint.mtime - start_mtime);
                    break;
                }
                if udelay_count > 500_000_000 {
                    println!("[diag] udelay STILL running after 500M instructions!");
                    // Show what the loop is computing
                    println!("[diag] a3={}, a4={}, a5={}", 
                        vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15]);
                    println!("[diag] mtime = {}", vm.bus.clint.mtime);
                    break;
                }
            }
            break;
        }
    }
    
    if count <= max {
        println!("[diag] Total instructions: {}", count);
    } else {
        println!("[diag] Did not hit udelay in {} instructions. PC=0x{:08X}", max, vm.cpu.pc);
    }
}
