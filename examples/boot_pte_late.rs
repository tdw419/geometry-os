// Diagnostic: check PTEs right before the fault at ~186500.
// Run: cargo run --example boot_pte_late

use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";

    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1 quiet";

    let (mut vm, fw_addr, _entry, _dtb_addr) =
        geometry_os::riscv::RiscvVm::boot_linux_setup(
            &kernel_image,
            initramfs.as_deref(),
            256,
            bootargs,
        ).expect("boot setup failed");

    let fw_addr_u32 = fw_addr as u32;
    let max_instr: u64 = 186_500;
    let mut count: u64 = 0;
    let mut last_satp: u32 = vm.cpu.csr.satp;

    use geometry_os::riscv::cpu::StepResult;

    while count < max_instr {
        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            let mode = (cur_satp >> 31) & 1;
            if mode == 1 {
                let ppn = cur_satp & 0x3FFFFF;
                let pg_dir_phys = (ppn as u64) * 4096;
                let device_l1: &[u32] = &[0, 1, 2, 3, 4, 5, 8, 48, 64];
                for &l1_idx in device_l1 {
                    let addr = pg_dir_phys + (l1_idx as u64) * 4;
                    let existing = vm.bus.read_word(addr).unwrap_or(0);
                    if (existing & 1) == 0 {
                        vm.bus.write_word(addr, 0xCF | (l1_idx << 20)).ok();
                    }
                }
                vm.cpu.tlb.flush_all();
            }
            last_satp = cur_satp;
        }

        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            let mpp = (vm.cpu.csr.mstatus & 0x1800) >> 11;
            if cause_code != 11 && mpp != 3 {
                let stvec = vm.cpu.csr.stvec & !0x3u32;
                if stvec != 0 {
                    vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                    vm.cpu.csr.scause = mcause;
                    vm.cpu.csr.stval = vm.cpu.csr.mtval;
                    let spp = if mpp == 1 { 1u32 } else { 0u32 };
                    vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 5)) | (spp << 5);
                    let sie = (vm.cpu.csr.mstatus >> 1) & 1;
                    vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 5)) | (sie << 5);
                    vm.cpu.csr.mstatus &= !(1 << 1);
                    if cause_code == 7 { vm.bus.clint.mtimecmp = vm.bus.clint.mtime + 100_000; }
                    vm.cpu.pc = stvec;
                    vm.cpu.privilege = geometry_os::riscv::cpu::Privilege::Supervisor;
                    vm.cpu.tlb.flush_all();
                    count += 1;
                    continue;
                }
            }
            if cause_code == 11 {
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13], vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0, a1)) = result { vm.cpu.x[10] = a0; vm.cpu.x[11] = a1; }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        vm.step();
        count += 1;
    }

    eprintln!("[diag] Stopped at count={}, PC=0x{:08X}, SATP=0x{:08X}", count, vm.cpu.pc, vm.cpu.csr.satp);

    // Check PTEs
    let satp = vm.cpu.csr.satp;
    let ppn = satp & 0x3FFFFF;
    let pg_dir_phys = (ppn as u64) * 4096;
    eprintln!("[diag] Page directory at PA 0x{:08X}", pg_dir_phys);

    // Check L1 entries that the kernel might have modified
    eprintln!("\n[diag] L1 entries for kernel VA range (at count={}):", count);
    for i in 768..780 {
        let l1_addr = pg_dir_phys + (i as u64) * 4;
        let l1_pte = vm.bus.read_word(l1_addr).unwrap_or(0);
        let v = (l1_pte & 1) != 0;
        let r = (l1_pte >> 1) & 1;
        let w = (l1_pte >> 2) & 1;
        let x = (l1_pte >> 3) & 1;
        let ppn1 = (l1_pte >> 20) & 0xFFF;
        let is_leaf = r != 0 || w != 0 || x != 0;

        if v {
            if is_leaf {
                let va_start = (i as u64) << 22;
                let pa_start = (ppn1 as u64) << 22;
                eprintln!("  L1[{}] = 0x{:08X} -> megapage VA 0x{:08X} -> PA 0x{:08X} (RWX={}{})",
                    i, l1_pte, va_start, pa_start, r, x);
            } else {
                let l2_phys = (l1_pte as u64) & 0xFFFFF000;
                eprintln!("  L1[{}] = 0x{:08X} -> L2 table at PA 0x{:08X}", i, l1_pte, l2_phys);
            }
        } else {
            eprintln!("  L1[{}] = 0x{:08X} -> NOT VALID", i, l1_pte);
        }
    }

    // Check what the kernel sees at VA 0xC04047DC (create_pgd_mapping)
    // By reading the PA that the MMU would translate to
    // L1[769] maps VA 0xC0400000 -> PA 0x00400000 (if still megapage)
    let l1_769 = vm.bus.read_word(pg_dir_phys + 769 * 4).unwrap_or(0);
    let r769 = (l1_769 >> 1) & 1;
    let is_leaf_769 = r769 != 0 || ((l1_769 >> 2) & 1) != 0 || ((l1_769 >> 3) & 1) != 0;

    eprintln!("\n[diag] Checking VA 0xC04047DC (create_pgd_mapping):");
    if is_leaf_769 {
        let ppn1 = ((l1_769 >> 20) & 0xFFF) as u64;
        let pa_base = ppn1 << 22;
        let pa = pa_base + (0xC04047DC & 0x3FFFFF);
        let inst = vm.bus.read_word(pa).unwrap_or(0);
        eprintln!("  L1[769] is megapage, PA base = 0x{:08X}", pa_base);
        eprintln!("  PA 0x{:08X} -> inst = 0x{:08X}", pa, inst);
        eprintln!("  Expected: 0x1101 (addi sp, sp, -32)");
    } else {
        let l2_phys = (l1_769 as u64) & 0xFFFFF000;
        let vpn2 = (0xC04047DCu32 >> 12) & 0x3FF;
        let l2_pte = vm.bus.read_word(l2_phys + (vpn2 as u64) * 4).unwrap_or(0);
        let l2_ppn = ((l2_pte >> 10) & 0x3FFFFF) as u64;
        let l2_pa = (l2_ppn << 12) + (0xC04047DC & 0xFFF);
        let inst = vm.bus.read_word(l2_pa).unwrap_or(0);
        eprintln!("  L1[769] points to L2 table at PA 0x{:08X}", l2_phys);
        eprintln!("  L2[{}] = 0x{:08X} -> PA 0x{:08X}", vpn2, l2_pte, l2_ppn << 12);
        eprintln!("  PA 0x{:08X} -> inst = 0x{:08X}", l2_pa, inst);
        eprintln!("  Expected: 0x1101 (addi sp, sp, -32)");
    }

    // Also check L1[769] changes: check at count=180000 vs now
    // Read what the kernel binary has at PA 0x004047DC
    let direct = vm.bus.read_word(0x004047DC).unwrap_or(0);
    eprintln!("\n  Direct read PA 0x004047DC = 0x{:08X} (should be 0x1101)", direct);

    // Check if the kernel has modified PTEs between 180K and 186K
    // The kernel's create_pgd_mapping function modifies page tables
    // Check if L1[769] was changed from megapage to L2 pointer
    eprintln!("\n[diag] Checking if kernel modified PTEs:");
    // Read L1[769] from the kernel's early_pg_dir at PA 0x00802000
    let early_l1_769 = vm.bus.read_word(0x00802000 + 769 * 4).unwrap_or(0);
    eprintln!("  early_pg_dir L1[769] = 0x{:08X}", early_l1_769);

    // Also check the trampoline pg_dir
    let tramp_l1_769 = vm.bus.read_word(0x01484000 + 769 * 4).unwrap_or(0);
    eprintln!("  trampoline_pg_dir L1[769] = 0x{:08X}", tramp_l1_769);

    // Check kernel_map values
    let km_phys: u64 = 0x00C79E90;
    let km_pa = vm.bus.read_word(km_phys + 12).unwrap_or(0);
    let km_vapo = vm.bus.read_word(km_phys + 20).unwrap_or(0);
    let km_vkpo = vm.bus.read_word(km_phys + 24).unwrap_or(0);
    let km_va = vm.bus.read_word(km_phys + 4).unwrap_or(0);
    let km_vo = vm.bus.read_word(km_phys + 8).unwrap_or(0);
    let km_sz = vm.bus.read_word(km_phys + 16).unwrap_or(0);
    eprintln!("\n[diag] kernel_map at PA 0x{:08X}:", km_phys);
    eprintln!("  page_offset (0)  = 0x{:08X}", vm.bus.read_word(km_phys + 0).unwrap_or(0));
    eprintln!("  virt_addr (4)    = 0x{:08X}", km_va);
    eprintln!("  virt_offset (8)  = 0x{:08X}", km_vo);
    eprintln!("  phys_addr (12)   = 0x{:08X}", km_pa);
    eprintln!("  size (16)        = 0x{:08X}", km_sz);
    eprintln!("  va_pa_offset (20)= 0x{:08X}", km_vapo);
    eprintln!("  va_kernel_pa_off (24)= 0x{:08X}", km_vkpo);
}
