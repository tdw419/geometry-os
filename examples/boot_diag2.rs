use std::fs;
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).unwrap();
    let initramfs = fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1 quiet";
    let (mut vm, _r) = RiscvVm::boot_linux(
        &kernel_image, initramfs.as_deref(), 256, 200_000_000, bootargs,
    ).unwrap();

    // Read using PHYSICAL addresses (VA - 0xC0000000 = PA)
    let va_pa_offset = 0xC0000000u32;
    
    for &va_base in &[0xC00010B0u32, 0xC0002780u32, 0xC020B0D0u32, 0xC006ADD0u32] {
        let pa_base = va_base.wrapping_sub(va_pa_offset);
        println!("
--- Instructions at VA 0x{:08X} (PA 0x{:08X}) ---", va_base, pa_base);
        for off in (0u32..32).step_by(2) {
            let addr = (pa_base + off) as u64;
            if let Ok(hw) = vm.bus.read_half(addr) {
                let is_c = (hw & 0x3) != 0x3;
                if is_c {
                    println!("  {:08X}: {:04X}  (compressed)", va_base + off, hw);
                } else {
                    if let Ok(w) = vm.bus.read_word(addr) {
                        println!("  {:08X}: {:08X}", va_base + off, w);
                    }
                }
            }
        }
    }
    
    // Check stval address - what is VA 0x804046B4?
    // This is in the range 0x80000000-0x807FFFFF (L1[512])
    // VPN[1] = 0x201 = 513. Let's check L1[512] and nearby
    let satp = vm.cpu.csr.satp;
    let pgdir = ((satp & 0x3FFFFF) as u64) * 4096;
    println!("
--- L1 entries around index 512 ---");
    for idx in 510..520 {
        let addr = pgdir + (idx as u64) * 4;
        let pte = vm.bus.read_word(addr).unwrap_or(0);
        println!("L1[{}] @ PA 0x{:08X} = 0x{:08X} (VA 0x{:08X}-0x{:08X})", 
            idx, addr, pte, idx << 22, ((idx+1) << 22) - 1);
    }
    
    // Check the kernel's linear mapping: L1[768+] should map VA 0xC0000000+
    println!("
--- L1 entries for kernel linear map (VA 0xC0000000+) ---");
    for idx in 768..780 {
        let addr = pgdir + (idx as u64) * 4;
        let pte = vm.bus.read_word(addr).unwrap_or(0);
        if pte != 0 {
            println!("L1[{}] @ PA 0x{:08X} = 0x{:08X} (VA 0x{:08X})", 
                idx, addr, pte, idx << 22);
        }
    }
    
    // The stuck PC and sepc
    println!("
PC=0x{:08X} (PA 0x{:08X})", vm.cpu.pc, vm.cpu.pc - va_pa_offset);
    println!("sepc=0x{:08X} (PA 0x{:08X})", vm.cpu.csr.sepc, vm.cpu.csr.sepc - va_pa_offset);
    println!("stval=0x{:08X}", vm.cpu.csr.stval);
    println!("scause=0x{:08X}", vm.cpu.csr.scause);
}
