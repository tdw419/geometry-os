// Check page tables after Linux boot setup
use std::fs;
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel = fs::read(kernel_path).unwrap();
    let initramfs = fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=1";
    let (vm, _) = RiscvVm::boot_linux(
        &kernel, initramfs.as_deref(), 256, 1_000_000, bootargs,
    ).unwrap();

    let satp = vm.cpu.csr.satp;
    let asid = (satp >> 22) & 0x1FF;
    let ppn = satp & 0x003FFFFF;
    let pt_root = (ppn as u64) << 12;
    let sv32 = (satp >> 31) & 1;
    
    println!("SATP: 0x{:08X}", satp);
    println!("  Mode: {} ({})", sv32, if sv32 == 1 { "SV32" } else { "Bare" });
    println!("  ASID: {}", asid);
    println!("  PPN: 0x{:06X}", ppn);
    println!("  Page table root: 0x{:08X}", pt_root);
    
    // Dump first-level page table (1024 entries)
    println!("\n=== L1 Page Table at 0x{:08X} ===", pt_root);
    let mem = &vm.bus.mem;
    let mut mapped_count = 0;
    for i in 0..1024 {
        let pte = mem.read_word(pt_root + (i as u64) * 4).unwrap_or(0);
        if pte != 0 {
            let v = pte & 0xFFFFFFF0; // PPN
            let r = (pte >> 1) & 1;
            let w = (pte >> 2) & 1;
            let x = (pte >> 3) & 1;
            let u = (pte >> 4) & 1;
            let g = (pte >> 5) & 1;
            let a = (pte >> 6) & 1;
            let d = (pte >> 7) & 1;
            let leaf = r == 1 || w == 1 || x == 1;
            let va_base = (i as u64) << 22;
            println!("  [{:>4}] PTE=0x{:08X} PPN=0x{:07X} R{}W{}X{}U{}G{}A{}D{} {} VA=0x{:08X}-0x{:08X}",
                     i, pte, v, r, w, x, u, g, a, d,
                     if leaf { "LEAF" } else { "PTR " },
                     va_base, va_base + 0x3FFFFF);
            mapped_count += 1;
        }
    }
    println!("\nMapped L1 entries: {}/1024", mapped_count);
    
    // Check a specific L2 table
    // Find a non-leaf entry (pointer to L2)
    for i in 0..1024 {
        let pte = mem.read_word(pt_root + (i as u64) * 4).unwrap_or(0);
        if pte != 0 {
            let r = (pte >> 1) & 1;
            let w = (pte >> 2) & 1;
            let x = (pte >> 3) & 1;
            let leaf = r == 1 || w == 1 || x == 1;
            if !leaf {
                let l2_addr = (pte & 0xFFFFFFF0) << 2;
                println!("\n=== L2 Page Table at 0x{:08X} (from L1[{}]) ===", l2_addr, i);
                let mut l2_mapped = 0;
                for j in 0..1024 {
                    let l2_pte = mem.read_word(l2_addr as u64 + (j as u64) * 4).unwrap_or(0);
                    if l2_pte != 0 {
                        if l2_mapped < 5 {
                            let va_base = ((i as u64) << 22) | ((j as u64) << 12);
                            println!("  [{:>4}] PTE=0x{:08X} VA=0x{:08X}", j, l2_pte, va_base);
                        }
                        l2_mapped += 1;
                    }
                }
                println!("  Mapped L2 entries: {}/1024", l2_mapped);
                break;
            }
        }
    }
}
