//! Check if the kernel actually reads the DTB by watching for reads from the DTB area.
//! Also check if the DTB identity mapping works.
//! Run: cargo run --example boot_dtb_read_check

use geometry_os::riscv::cpu::{Privilege, StepResult};
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let ir_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_data = std::fs::read(kernel_path).expect("kernel");
    let initramfs_data = std::path::Path::new(ir_path)
        .exists()
        .then(|| std::fs::read(ir_path).unwrap());

    eprintln!("[dtb_read] Starting boot...");
    let (mut vm, fw_addr, _entry, dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_data,
        initramfs_data.as_deref(),
        512,
        "console=ttyS0 earlycon=sbi loglevel=7",
    )
    .expect("boot_linux_setup failed");

    vm.bus.auto_pte_fixup = false;

    let dtb_va = ((dtb_addr.wrapping_add(0xC0000000)) & 0xFFFFFFFF) as u32;
    let dtb_pa = dtb_addr as u32;
    let fw_addr_u32 = fw_addr as u32;
    let mut count = 0u64;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut dtb_reads = 0u64;
    let mut last_dtb_count = 0u64;

    // Watch for the first 20 reads from DTB VA range (0xC1579000-0xC1579FFF)
    // or DTB PA range (0x01579000-0x01579FFF)
    while count < 300_000 {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        // M-mode trap handling (simplified)
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            let mpp = (vm.cpu.csr.mstatus >> 11) & 3;

            if cause_code == 9 {
                // ECALL_S = SBI call
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17],
                    vm.cpu.x[16],
                    vm.cpu.x[10],
                    vm.cpu.x[11],
                    vm.cpu.x[12],
                    vm.cpu.x[13],
                    vm.cpu.x[14],
                    vm.cpu.x[15],
                    &mut vm.bus.uart,
                    &mut vm.bus.clint,
                );
                if let Some((a0, a1)) = result {
                    vm.cpu.x[10] = a0;
                    vm.cpu.x[11] = a1;
                }
            } else if cause_code != 11 && mpp != 3 {
                // Forward to S-mode
                let stvec = vm.cpu.csr.stvec & !0x3u32;
                if stvec != 0 {
                    vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                    vm.cpu.csr.scause = mcause;
                    vm.cpu.csr.stval = vm.cpu.csr.mtval;
                    let spp = if mpp == 1 { 1u32 } else { 0u32 };
                    vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 8)) | (spp << 8);
                    let sie = (vm.cpu.csr.mstatus >> 1) & 1;
                    vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 5)) | (sie << 5);
                    vm.cpu.csr.mstatus &= !(1 << 1);
                    if cause_code == 7 {
                        vm.bus.clint.mtimecmp = vm.bus.clint.mtime + 100_000;
                    }
                    vm.cpu.pc = stvec;
                    vm.cpu.privilege = Privilege::Supervisor;
                    vm.cpu.tlb.flush_all();
                    count += 1;
                    continue;
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);

        let pc_before = vm.cpu.pc;
        let result = vm.step();
        count += 1;

        // Check if PC is in DTB VA range
        let pc = vm.cpu.pc;
        if pc >= dtb_va && pc < dtb_va + 0x1000 && vm.cpu.privilege == Privilege::Supervisor {
            dtb_reads += 1;
            if dtb_reads <= 20 && dtb_reads > last_dtb_count {
                eprintln!(
                    "[dtb_read] #{} at count={}: PC=0x{:08X} (DTB VA range 0x{:08X}-0x{:08X})",
                    dtb_reads,
                    count,
                    pc,
                    dtb_va,
                    dtb_va + 0x1000
                );
            }
            last_dtb_count = dtb_reads;
        }

        match result {
            StepResult::Ebreak => break,
            StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault => {
                if count < 200_000 {
                    let ft = match result {
                        StepResult::FetchFault => "fetch",
                        StepResult::LoadFault => "load",
                        _ => "store",
                    };
                    eprintln!("[dtb_read] {} fault at count={}: PC=0x{:08X} scause=0x{:08X} stval=0x{:08X} priv={:?}",
                        ft, count, vm.cpu.pc, vm.cpu.csr.scause, vm.cpu.csr.stval, vm.cpu.privilege);
                }
            }
            _ => {}
        }

        // SATP change handling
        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            eprintln!(
                "[dtb_read] SATP changed: 0x{:08X} -> 0x{:08X} at count={}",
                last_satp, cur_satp, count
            );
            // Inject identity mappings
            let ppn = cur_satp & 0x3FFFFF;
            let pg_dir_phys = (ppn as u64) * 4096;
            for i in 0..64u32 {
                let addr = pg_dir_phys + (i as u64) * 4;
                let existing = vm.bus.read_word(addr).unwrap_or(0);
                if (existing & 1) == 0 {
                    vm.bus.write_word(addr, 0x0000_00CF | (i << 20)).ok();
                }
            }
            for &l1_idx in &[8u32, 48, 64] {
                let addr = pg_dir_phys + (l1_idx as u64) * 4;
                let existing = vm.bus.read_word(addr).unwrap_or(0);
                if (existing & 1) == 0 {
                    vm.bus.write_word(addr, 0x0000_00CF | (l1_idx << 20)).ok();
                }
            }
            vm.cpu.tlb.flush_all();
            // Fix kernel PT
            for l1_scan in 768..780u32 {
                let scan_addr = pg_dir_phys + (l1_scan as u64) * 4;
                let entry = vm.bus.read_word(scan_addr).unwrap_or(0);
                let is_valid = (entry & 1) != 0;
                let is_non_leaf = is_valid && (entry & 0xE) == 0;
                let needs_fix = !is_valid || is_non_leaf;
                if !needs_fix {
                    continue;
                }
                let pa_offset = l1_scan - 768;
                vm.bus
                    .write_word(scan_addr, 0x0000_00CF | (pa_offset << 20))
                    .ok();
            }
            vm.cpu.tlb.flush_all();
            // Restore DTB pointers
            vm.bus.write_word(0x00801008, dtb_va).ok();
            vm.bus.write_word(0x0080100C, dtb_pa).ok();
            eprintln!(
                "[dtb_read] Restored DTB pointers: va=0x{:08X} pa=0x{:08X}",
                dtb_va, dtb_pa
            );

            // Verify: read first 4 bytes of DTB at VA
            let dtb_magic = vm.bus.read_word(dtb_va as u64).unwrap_or(0);
            eprintln!(
                "[dtb_read] DTB magic at VA 0x{:08X}: 0x{:08X} (expect 0xD00DFEED)",
                dtb_va, dtb_magic
            );

            last_satp = cur_satp;
        }
    }

    let sbi_str: String = vm
        .bus
        .sbi
        .console_output
        .iter()
        .map(|&b| b as char)
        .collect();
    eprintln!(
        "\n[dtb_read] Done: count={} dtb_reads={} ecall_count={}",
        count, dtb_reads, vm.cpu.ecall_count
    );
    eprintln!(
        "[dtb_read] PC=0x{:08X} priv={:?}",
        vm.cpu.pc, vm.cpu.privilege
    );
    if !sbi_str.is_empty() {
        eprintln!("[dtb_read] Console: {}", &sbi_str[..sbi_str.len().min(500)]);
    } else {
        eprintln!("[dtb_read] No console output");
    }
}
