use geometry_os::riscv::{RiscvVm, cpu, csr};
use geometry_os::riscv::bus::Bus;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    // Use boot_linux_setup to get the VM BEFORE any instructions execute
    let (mut vm, _fw_addr, _entry, _dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        256,
        "console=ttyS0 loglevel=8",
    ).unwrap();

    let satp = vm.cpu.csr.satp;
    let pg_dir_phys = ((satp & 0x3FFFFF) as u64) * 4096;
    
    println!("=== BOOT PAGE TABLE (before kernel executes) ===");
    println!("SATP=0x{:08X} pg_dir at PA 0x{:08X}", satp, pg_dir_phys);
    
    println!("\nKernel linear mapping PTEs (L1[768..776]):");
    for i in 768..776 {
        let pte_addr = pg_dir_phys + (i as u64) * 4;
        let pte = vm.bus.read_word(pte_addr).unwrap_or(0);
        
        let full_ppn = (pte >> 10) & 0x3FFFFF;
        let ppn_hi = (full_ppn >> 10) & 0xFFF;
        let vpn0_start = ((i - 768) % 2) * 512;
        
        // Compute actual PA for start of this L1 entry's VA range
        let va_start = 0xC0000000u32 + ((i - 768) as u32) * 0x200000;
        let vpn0 = ((va_start >> 12) & 0x3FF) as u64;
        let pa_start = (ppn_hi as u64) << 22 | (vpn0 << 12);
        let expected_pa = ((i - 768) as u64) * 0x200000;
        
        let status = if pa_start == expected_pa { "OK" } else { "WRONG" };
        
        println!("  L1[{}] PTE=0x{:08X} ppn_hi={} VA=0x{:08X} PA=0x{:08X} expected=0x{:08X} {}",
            i, pte, ppn_hi, va_start, pa_start, expected_pa, status);
    }
    
    // Also check the first few low-address identity mappings
    println!("\nLow address identity mapping PTEs (L1[0..4]):");
    for i in 0..4 {
        let pte_addr = pg_dir_phys + (i as u64) * 4;
        let pte = vm.bus.read_word(pte_addr).unwrap_or(0);
        
        let full_ppn = (pte >> 10) & 0x3FFFFF;
        let ppn_hi = (full_ppn >> 10) & 0xFFF;
        
        let va_start = (i as u32) * 0x200000;
        let vpn0 = ((va_start >> 12) & 0x3FF) as u64;
        let pa_start = (ppn_hi as u64) << 22 | (vpn0 << 12);
        let expected_pa = (i as u64) * 0x200000;
        
        let status = if pa_start == expected_pa { "OK" } else { "WRONG" };
        
        println!("  L1[{}] PTE=0x{:08X} ppn_hi={} VA=0x{:08X} PA=0x{:08X} expected=0x{:08X} {}",
            i, pte, ppn_hi, va_start, pa_start, expected_pa, status);
    }
}
