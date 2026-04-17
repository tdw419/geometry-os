//! Check DTB reservation map entries (big-endian).
use geometry_os::riscv::RiscvVm;

fn read_be32(bus: &mut geometry_os::riscv::bus::Bus, addr: u64) -> u32 {
    let b0 = bus.read_byte(addr).unwrap_or(0) as u32;
    let b1 = bus.read_byte(addr + 1).unwrap_or(0) as u32;
    let b2 = bus.read_byte(addr + 2).unwrap_or(0) as u32;
    let b3 = bus.read_byte(addr + 3).unwrap_or(0) as u32;
    (b0 << 24) | (b1 << 16) | (b2 << 8) | b3
}

fn read_be64(bus: &mut geometry_os::riscv::bus::Bus, addr: u64) -> u64 {
    let hi = read_be32(bus, addr) as u64;
    let lo = read_be32(bus, addr + 4) as u64;
    (hi << 32) | lo
}

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    let (mut vm, _fw, _entry, dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();

    println!("=== DTB at PA 0x{:08X} ===", dtb_addr);

    // Read header (big-endian)
    let magic = read_be32(&mut vm.bus, dtb_addr);
    let totalsize = read_be32(&mut vm.bus, dtb_addr + 4);
    let off_dt_struct = read_be32(&mut vm.bus, dtb_addr + 8);
    let off_dt_strings = read_be32(&mut vm.bus, dtb_addr + 12);
    let off_mem_rsvmap = read_be32(&mut vm.bus, dtb_addr + 16);
    let version = read_be32(&mut vm.bus, dtb_addr + 20);

    println!("magic:     0x{:08X} (expected 0xD00DFEED)", magic);
    println!("totalsize:  {} bytes", totalsize);
    println!("off_dt_struct: 0x{:X}", off_dt_struct);
    println!("off_dt_strings: 0x{:X}", off_dt_strings);
    println!("off_mem_rsvmap: 0x{:X}", off_mem_rsvmap);
    println!("version:    {}", version);

    // Read memory reservation map
    let rsvmap_base = dtb_addr + off_mem_rsvmap as u64;
    println!("\n=== Memory Reservation Map (at PA 0x{:08X}) ===", rsvmap_base);
    for i in 0..10 {
        let entry_addr = rsvmap_base + (i as u64) * 16;
        let addr = read_be64(&mut vm.bus, entry_addr);
        let size = read_be64(&mut vm.bus, entry_addr + 8);
        if addr == 0 && size == 0 {
            println!("[{}] TERMINATOR", i);
            break;
        }
        println!("[{}] addr=0x{:08X} size=0x{:08X} ({}KB)", i, addr, size, size / 1024);
    }

    // Check first few strings
    let strings_base = dtb_addr + off_dt_strings as u64;
    println!("\n=== DTB Strings (first 500 bytes at PA 0x{:08X}) ===", strings_base);
    let mut s = String::new();
    for i in 0..500 {
        let b = vm.bus.read_byte(strings_base + i).unwrap_or(0);
        if b == 0 {
            if !s.is_empty() {
                println!("  \"{}\"", s);
                s = String::new();
            }
        } else if b >= 0x20 && b < 0x7F {
            s.push(b as char);
        }
    }
}
