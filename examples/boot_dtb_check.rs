/// Check if the kernel can read the DTB at its virtual address.
use geometry_os::riscv::RiscvVm;
use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=uart8250,mmio32,0x10000000 panic=5 nosmp maxcpus=0 loglevel=8";
    let (mut vm, fw_addr, _entry, dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 128, bootargs).unwrap();

    eprintln!("DTB physical address: 0x{:08X}", dtb_addr);

    // Check _dtb_early_pa after boot
    let dtb_early_pa = vm.bus.read_word(0x0080100C).unwrap_or(0);
    let dtb_early_va = vm.bus.read_word(0x00801008).unwrap_or(0);
    eprintln!("_dtb_early_pa after setup: 0x{:08X}", dtb_early_pa);
    eprintln!("_dtb_early_va after setup: 0x{:08X}", dtb_early_va);

    // Run for 500K instructions (after setup_vm)
    let fw_addr_u32 = fw_addr as u32;
    let max_count: u64 = 5_000_000;
    let mut count: u64 = 0;

    while count < max_count {
        if vm.bus.sbi.shutdown_requested { break; }
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if (mcause >> 31) & 1 == 0 && cause_code == 9 {
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10],
                    vm.cpu.x[11], vm.cpu.x[12], vm.cpu.x[13],
                    vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0, a1)) = result {
                    vm.cpu.x[10] = a0;
                    vm.cpu.x[11] = a1;
                }
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            } else if (mcause >> 31) & 1 == 0 {
                let stvec = vm.cpu.csr.stvec & !0x3u32;
                if stvec != 0 {
                    vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                    vm.cpu.csr.scause = mcause;
                    vm.cpu.pc = stvec;
                    vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
                }
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            }
        }
        let _ = vm.step();
        count += 1;
    }

    // After boot, check DTB pointers again
    let dtb_early_pa2 = vm.bus.read_word(0x0080100C).unwrap_or(0);
    let dtb_early_va2 = vm.bus.read_word(0x00801008).unwrap_or(0);
    eprintln!("\nAfter 500K instructions:");
    eprintln!("_dtb_early_pa: 0x{:08X}", dtb_early_pa2);
    eprintln!("_dtb_early_va: 0x{:08X}", dtb_early_va2);
    eprintln!("SATP: 0x{:08X}", vm.cpu.csr.satp);

    // Check kernel_map values
    let km_pa: u64 = 0x00C7A098;
    let km_phys_addr = vm.bus.read_word(km_pa + 12).unwrap_or(0xFFFF_FFFF);
    let km_vapa = vm.bus.read_word(km_pa + 20).unwrap_or(0xFFFF_FFFF);
    let km_vkpa = vm.bus.read_word(km_pa + 24).unwrap_or(0xFFFF_FFFF);
    eprintln!("kernel_map: phys_addr=0x{:08X} va_pa_offset=0x{:08X} va_kernel_pa_offset=0x{:08X}", km_phys_addr, km_vapa, km_vkpa);

    // Check if the DTB PA is readable
    let dtb_magic_pa = vm.bus.read_word(dtb_addr).unwrap_or(0);
    let dtb_magic_le = u32::from_be(dtb_magic_pa); // DTB is big-endian
    eprintln!("DTB magic at PA 0x{:08X}: 0x{:08X} (BE: 0x{:08X})", dtb_addr, dtb_magic_pa, dtb_magic_le);

    // Check if the page table maps the DTB virtual address
    if dtb_early_va2 != 0 {
        let dtb_va = dtb_early_va2 as u64;
        let vpn1 = (dtb_va >> 22) & 0x3FF;
        let vpn0 = (dtb_va >> 12) & 0x3FF;
        let satp_ppn = (vm.cpu.csr.satp & 0x3FFFFF) as u64;
        let l1_pa = satp_ppn * 4096;
        let l1_entry = vm.bus.read_word(l1_pa + vpn1 * 4).unwrap_or(0);
        eprintln!("\nDTB VA 0x{:08X}: VPN1={} VPN0={}", dtb_early_va2, vpn1, vpn0);
        eprintln!("L1[{}] at PA 0x{:08X} = 0x{:08X}", vpn1, l1_pa + vpn1 * 4, l1_entry);

        let l1_valid = (l1_entry & 1) != 0;
        let l1_leaf = l1_valid && (l1_entry & 0xE) != 0;
        if l1_valid && !l1_leaf {
            let l2_ppn = ((l1_entry >> 10) & 0x3FFFFF) as u64;
            let l2_pa = l2_ppn * 4096;
            let l2_entry = vm.bus.read_word(l2_pa + vpn0 * 4).unwrap_or(0);
            eprintln!("L2[{}] at PA 0x{:08X} = 0x{:08X}", vpn0, l2_pa + vpn0 * 4, l2_entry);
            let l2_ppn_val = (l2_entry >> 10) & 0x3FFFFF;
            let mapped_pa = l2_ppn_val << 12;
            eprintln!("Mapped PA: 0x{:08X} (expected: 0x{:08X})", mapped_pa, dtb_addr as u32);
        } else if l1_leaf {
            let l1_ppn = ((l1_entry >> 10) & 0xFFFFF) as u64;
            let mapped_pa = l1_ppn << 22; // megapage
            eprintln!("Megapage mapped PA: 0x{:08X}", mapped_pa);
        } else {
            eprintln!("L1 entry NOT VALID - DTB VA is unmapped!");
        }
    }
}
