/// Diagnostic: verify DTB is valid and accessible, check early kernel register state.

use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    let (mut vm, fw_addr, _entry, dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();

    // 1. Check DTB header
    println!("=== DTB Verification ===");
    println!("DTB address: 0x{:08X}", dtb_addr);
    let magic = vm.bus.read_word(dtb_addr).unwrap_or(0);
    println!("DTB magic: 0x{:08X} (expected 0xD00DFEED)", magic);
    let totalsize = vm.bus.read_word(dtb_addr + 4).unwrap_or(0);
    println!("DTB size: {} bytes", totalsize);
    let off_dt_struct = vm.bus.read_word(dtb_addr + 8).unwrap_or(0);
    let off_dt_strings = vm.bus.read_word(dtb_addr + 12).unwrap_or(0);
    println!("DTB struct offset: 0x{:X}, strings offset: 0x{:X}", off_dt_struct, off_dt_strings);

    // Read first few bytes of DTB strings to see what nodes are defined
    let strings_start = dtb_addr + off_dt_strings as u64;
    println!("\nDTB strings (first 500 bytes):");
    let mut s = String::new();
    for i in 0..500 {
        let b = vm.bus.read_byte(strings_start + i).unwrap_or(0);
        if b == 0 {
            if !s.is_empty() {
                println!("  \"{}\"", s);
                s = String::new();
            }
        } else if b >= 0x20 && b < 0x7F {
            s.push(b as char);
        }
    }
    if !s.is_empty() {
        println!("  \"{}\"", s);
    }

    // 2. Check initial register state
    println!("\n=== Initial Register State ===");
    println!("PC=0x{:08X} priv={:?}", vm.cpu.pc, vm.cpu.privilege);
    println!("a0 (hartid) = {}", vm.cpu.x[10]);
    println!("a1 (dtb)    = 0x{:08X}", vm.cpu.x[11]);
    println!("SP          = 0x{:08X}", vm.cpu.x[2]);
    println!("SATP        = 0x{:08X}", vm.cpu.csr.satp);
    println!("mepc        = 0x{:08X}", vm.cpu.csr.mepc);
    println!("mtvec       = 0x{:08X}", vm.cpu.csr.mtvec);

    // 3. Run for a bit and check SIE status periodically
    println!("\n=== Running 500K instructions, checking SIE ===");
    let max = 500_000u64;
    let mut count: u64 = 0;
    let fw_addr_u32 = fw_addr as u32;

    while count < max {
        if vm.bus.sbi.shutdown_requested { break; }
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine {
            let cause_code = vm.cpu.csr.mcause & !(1u32 << 31);
            if cause_code == 9 || cause_code == 11 {
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16],
                    vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13],
                    vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0, a1)) = result {
                    vm.cpu.x[10] = a0; vm.cpu.x[11] = a1;
                }
                println!("[diag] ECALL at count={}: cause={} a7=0x{:X}", count, cause_code, vm.cpu.x[17]);
            } else {
                let mpp = (vm.cpu.csr.mstatus & 0x1800) >> 11;
                if mpp != 3 {
                    let stvec = vm.cpu.csr.stvec & !0x3u32;
                    if stvec != 0 {
                        vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                        vm.cpu.csr.scause = vm.cpu.csr.mcause;
                        vm.cpu.csr.stval = vm.cpu.csr.mtval;
                        vm.cpu.pc = stvec;
                        vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
                        vm.cpu.tlb.flush_all();
                        count += 1;
                        continue;
                    }
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }
        let _ = vm.step();
        count += 1;
    }

    println!("After 500K: PC=0x{:08X} priv={:?}", vm.cpu.pc, vm.cpu.privilege);
    println!("SIE={} SSTATUS=0x{:08X}", (vm.cpu.csr.mstatus >> 1) & 1, vm.cpu.csr.mstatus);
    println!("SATP=0x{:08X} STVEC=0x{:08X}", vm.cpu.csr.satp, vm.cpu.csr.stvec);
    println!("mepc=0x{:08X} sepc=0x{:08X}", vm.cpu.csr.mepc, vm.cpu.csr.sepc);

    // Check a1 (DTB pointer) after boot - kernel might have corrupted it
    println!("a1 (dtb ptr after 500K) = 0x{:08X}", vm.cpu.x[11]);
}
