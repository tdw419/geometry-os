// Diagnostic: dump L1 page table at current SATP, check translation of RA
use std::fs;
use std::time::Instant;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    let start = Instant::now();
    let (mut vm, result) = geometry_os::riscv::RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        512,
        20_000_000u64,
        bootargs,
    )
    .unwrap();
    let elapsed = start.elapsed();
    println!(
        "Boot: {} instructions in {:?}",
        result.instructions, elapsed
    );
    println!("PC: 0x{:08X}, Privilege: {:?}", vm.cpu.pc, vm.cpu.privilege);
    println!("SATP: 0x{:08X}", vm.cpu.csr.satp);

    // UART output
    let tx = vm.bus.uart.drain_tx();
    println!("UART TX: {} bytes", tx.len());
    if !tx.is_empty() {
        let s = String::from_utf8_lossy(&tx);
        println!("{}", s);
    }

    let ra = vm.cpu.x[1];
    let sp = vm.cpu.x[2];
    println!("\nRA=0x{:08X} SP=0x{:08X}", ra, sp);

    // Get SATP PPN and read L1 page table
    let satp = vm.cpu.csr.satp;
    let pg_dir_phys = ((satp & 0x3FFFFF) as u64) * 4096;
    println!("Page directory PA: 0x{:08X}", pg_dir_phys);

    // Read L1 entry for RA (VA 0xC020B794 -> VPN1 = 0xC020B794 >> 22 = 0xC08)
    let ra_vpn1 = (ra >> 22) & 0x3FF;
    let ra_l1_addr = pg_dir_phys + (ra_vpn1 as u64) * 4;
    let ra_l1_entry = vm.bus.read_word(ra_l1_addr).unwrap_or(0);
    println!(
        "RA L1[0x{:03X}] at PA 0x{:08X} = 0x{:08X}",
        ra_vpn1, ra_l1_addr, ra_l1_entry
    );

    // Read L1 entry for SP (VA 0xC1401E30 -> VPN1 = 0xC1401E30 >> 22 = 0xC14)
    let sp_vpn1 = (sp >> 22) & 0x3FF;
    let sp_l1_addr = pg_dir_phys + (sp_vpn1 as u64) * 4;
    let sp_l1_entry = vm.bus.read_word(sp_l1_addr).unwrap_or(0);
    println!(
        "SP L1[0x{:03X}] at PA 0x{:08X} = 0x{:08X}",
        sp_vpn1, sp_l1_addr, sp_l1_entry
    );

    // If L1 for RA is non-leaf (R=0,W=0,X=0), read L2
    let l1_rwx = ra_l1_entry & 0xE;
    if l1_rwx == 0 && (ra_l1_entry & 1) != 0 {
        let l2_ppn = (ra_l1_entry >> 10) & 0x3FFFFF;
        let l2_base = (l2_ppn as u64) * 4096;
        let ra_vpn0 = (ra >> 12) & 0x3FF;
        let l2_addr = l2_base + (ra_vpn0 as u64) * 4;
        let l2_entry = vm.bus.read_word(l2_addr).unwrap_or(0);
        println!(
            "RA L2[0x{:03X}] at PA 0x{:08X} = 0x{:08X}",
            ra_vpn0, l2_addr, l2_entry
        );
        let l2_ppn_val = (l2_entry >> 10) & 0x3FFFFF;
        let final_pa = (l2_ppn_val as u64) * 4096 + ((ra & 0xFFF) as u64);
        println!("RA final PA: 0x{:08X}", final_pa);
        let word = vm.bus.read_word(final_pa).unwrap_or(0);
        println!("Word at final PA: 0x{:08X}", word);
    } else if l1_rwx != 0 {
        // Megapage
        let l1_ppn = (ra_l1_entry >> 10) & 0x3FFFFF;
        let final_pa = (l1_ppn as u64) * 0x200000 + ((ra & 0x3FFFFF) as u64);
        println!("RA megapage PA: 0x{:08X}", final_pa);
        let word = vm.bus.read_word(final_pa).unwrap_or(0);
        println!("Word at megapage PA: 0x{:08X}", word);
    }

    // Do the same for SP
    let sp_l1_rwx = sp_l1_entry & 0xE;
    if sp_l1_rwx == 0 && (sp_l1_entry & 1) != 0 {
        let l2_ppn = (sp_l1_entry >> 10) & 0x3FFFFF;
        let l2_base = (l2_ppn as u64) * 4096;
        let sp_vpn0 = (sp >> 12) & 0x3FF;
        let l2_addr = l2_base + (sp_vpn0 as u64) * 4;
        let l2_entry = vm.bus.read_word(l2_addr).unwrap_or(0);
        println!(
            "SP L2[0x{:03X}] at PA 0x{:08X} = 0x{:08X}",
            sp_vpn0, l2_addr, l2_entry
        );
        let l2_ppn_val = (l2_entry >> 10) & 0x3FFFFF;
        let final_pa = (l2_ppn_val as u64) * 4096 + ((sp & 0xFFF) as u64);
        println!("SP final PA: 0x{:08X}", final_pa);
        let word = vm.bus.read_word(final_pa).unwrap_or(0);
        println!("Word at final PA (SP): 0x{:08X}", word);
    } else if sp_l1_rwx != 0 {
        let l1_ppn = (sp_l1_entry >> 10) & 0x3FFFFF;
        let final_pa = (l1_ppn as u64) * 0x200000 + ((sp & 0x3FFFFF) as u64);
        println!("SP megapage PA: 0x{:08X}", final_pa);
        let word = vm.bus.read_word(final_pa).unwrap_or(0);
        println!("Word at megapage PA (SP): 0x{:08X}", word);
    }

    // Also dump a range of L1 entries around kernel space (768-864)
    println!("\nL1 entries 768-800:");
    for i in 768..800 {
        let addr = pg_dir_phys + (i as u64) * 4;
        let entry = vm.bus.read_word(addr).unwrap_or(0);
        if entry != 0 {
            let v = (entry & 1) != 0;
            let r = (entry & 2) != 0;
            let w = (entry & 4) != 0;
            let x = (entry & 8) != 0;
            let ppn = (entry >> 10) & 0x3FFFFF;
            let va = (i as u32) << 22;
            let pa = ppn * if r || w || x { 0x200000 } else { 4096 };
            let kind = if r || w || x { "mega" } else { "L2->" };
            println!(
                "  L1[{}] VA=0x{:08X} V={} R={} W={} X={} PPN=0x{:05X} {} PA=0x{:08X} raw=0x{:08X}",
                i, va, v, r, w, x, ppn, kind, pa, entry
            );
        }
    }
}
