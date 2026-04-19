//! Check kernel page table entries for DTB VA access after setup_vm.
//! cargo run --example check_dtb_pt

use geometry_os::riscv::cpu::{Privilege, StepResult};
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let ir_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_data = std::fs::read(kernel_path).expect("kernel");
    let initramfs_data = std::path::Path::new(ir_path)
        .exists()
        .then(|| std::fs::read(ir_path).unwrap());

    let (mut vm, fw_addr, _entry, dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_data,
        initramfs_data.as_deref(),
        512,
        "console=ttyS0 earlycon=sbi",
    )
    .expect("setup failed");

    let fw_u32 = fw_addr as u32;
    let dtb_va: u32 = dtb_addr.wrapping_add(0xC0000000) as u32;
    eprintln!("DTB: PA=0x{:08X} VA=0x{:08X}", dtb_addr, dtb_va);

    // Run boot loop
    let mut count: u64 = 0;
    let max = 500_000u64;
    let mut last_satp: u32 = vm.cpu.csr.satp;

    while count < max {
        if vm.cpu.pc == fw_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code != 11 {
                let mpp = (vm.cpu.csr.mstatus >> 11) & 3;
                if mpp != 3 {
                    let stvec = vm.cpu.csr.stvec & !0u32;
                    if stvec != 0 {
                        vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                        vm.cpu.csr.scause = mcause;
                        vm.cpu.csr.stval = vm.cpu.csr.mtval;
                        vm.cpu.pc = stvec;
                        vm.cpu.privilege = Privilege::Supervisor;
                        vm.cpu.tlb.flush_all();
                        count += 1;
                        continue;
                    }
                }
                let fault_addr = vm.cpu.csr.mtval;
                let is_pf = cause_code == 12 || cause_code == 13 || cause_code == 15;
                if is_pf && fault_addr < 0x0200_0000 {
                    let satap = vm.cpu.csr.satp;
                    let pg_dir_ppn = (satap & 0x3FFFFF) as u64;
                    if pg_dir_ppn > 0 {
                        let pg_dir_phys = pg_dir_ppn * 4096;
                        let vpn1 = ((fault_addr >> 22) & 0x3FF) as u64;
                        let l1_addr = pg_dir_phys + vpn1 * 4;
                        let existing = vm.bus.read_word(l1_addr).unwrap_or(0);
                        if (existing & 1) == 0 {
                            let pte: u32 = 0x0000_00CF | ((vpn1 as u32) << 20);
                            vm.bus.write_word(l1_addr, pte).ok();
                            vm.cpu.tlb.flush_all();
                        }
                    }
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }
        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        let _ = vm.step();

        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            eprintln!(
                "[{}] SATP: 0x{:08X} -> 0x{:08X}",
                count, last_satp, cur_satp
            );
            last_satp = cur_satp;
        }
        count += 1;
    }

    // After 500K instructions, check the kernel's final page table
    let satap = vm.cpu.csr.satp;
    let pg_dir_ppn = (satap & 0x3FFFFF) as u64;
    let pg_dir_phys = pg_dir_ppn * 4096;
    eprintln!(
        "\nFinal SATP: 0x{:08X}, pg_dir at PA 0x{:08X}",
        satap, pg_dir_phys
    );

    // Check L1[773] for DTB VA range
    let dtb_vpn1 = ((dtb_va >> 22) & 0x3FF) as u64; // should be 773
    eprintln!("DTB VA 0x{:08X} -> VPN1={}", dtb_va, dtb_vpn1);
    let l1_773 = vm.bus.read_word(pg_dir_phys + dtb_vpn1 * 4).unwrap_or(0);
    eprintln!("L1[{}] = 0x{:08X}", dtb_vpn1, l1_773);
    let is_valid = (l1_773 & 1) != 0;
    let is_leaf = (l1_773 & 0xE) != 0;
    let ppn = (l1_773 >> 10) & 0x3FFFFF;
    eprintln!("  valid={} leaf={} ppn=0x{:08X}", is_valid, is_leaf, ppn);

    if is_valid && !is_leaf {
        // Non-leaf: L2 table at PA ppn*4096
        let l2_phys = (ppn as u64) * 4096;
        eprintln!("  L2 table at PA 0x{:08X}", l2_phys);
        let dtb_vpn0 = ((dtb_va >> 12) & 0x3FF) as u64;
        eprintln!("  DTB VPN0={}", dtb_vpn0);
        let l2_entry = vm.bus.read_word(l2_phys + dtb_vpn0 * 4).unwrap_or(0);
        eprintln!("  L2[{}] = 0x{:08X}", dtb_vpn0, l2_entry);
        let l2_ppn = (l2_entry >> 10) & 0x3FFFFF;
        let l2_valid = (l2_entry & 1) != 0;
        eprintln!(
            "  valid={} ppn=0x{:08X} -> PA=0x{:08X}",
            l2_valid,
            l2_ppn,
            l2_ppn << 12
        );
    } else if is_valid && is_leaf {
        // Megapage
        eprintln!("  Megapage -> PA 0x{:08X}", ppn << 22);
    } else {
        eprintln!("  UNMAPPED!");
    }

    // Also check what the boot page table had
    let boot_pt_ppn = (0x8000157Au32 & 0x3FFFFF) as u64;
    let boot_pt_phys = boot_pt_ppn * 4096;
    let l1_773_boot = vm.bus.read_word(boot_pt_phys + dtb_vpn1 * 4).unwrap_or(0);
    eprintln!("\nBoot page table L1[{}] = 0x{:08X}", dtb_vpn1, l1_773_boot);

    // Try to read DTB magic through the kernel page table
    // by reading the PA directly (since we know it's mapped via L1[773])
    let dtb_pa = dtb_addr;
    let magic = vm.bus.read_word(dtb_pa).unwrap_or(0);
    eprintln!("\nDTB magic at PA 0x{:08X}: 0x{:08X}", dtb_pa, magic);

    // Check memblock at CORRECT offsets
    let mb = 0x00803448u64;
    let mem_cnt = vm.bus.read_word(mb + 8).unwrap_or(0);
    let res_cnt = vm.bus.read_word(mb + 28).unwrap_or(0);
    let prb = vm.bus.read_word(0x00C79EAC).unwrap_or(0);
    eprintln!(
        "memblock: memory.cnt={} reserved.cnt={} phys_ram_base=0x{:08X}",
        mem_cnt, res_cnt, prb
    );

    // Check initial_boot_params
    let ibp = vm.bus.read_word(0x00C7A178).unwrap_or(0);
    eprintln!("initial_boot_params: 0x{:08X}", ibp);
}
