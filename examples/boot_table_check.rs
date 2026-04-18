use geometry_os::riscv::cpu::Privilege;
use geometry_os::riscv::mmu::{translate, AccessType, TranslateResult};
/// Read the exception handler table and check for bad entries.
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    let (mut vm, fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs).unwrap();

    let max = 200_000u64;
    let mut count: u64 = 0;
    let fw_addr_u32 = fw_addr as u32;

    while count < max {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if cause_code == 11 {
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
            } else {
                let mpp = (vm.cpu.csr.mstatus & 0x1800) >> 11;
                if mpp != 3 {
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
                        vm.cpu.pc = stvec;
                        vm.cpu.privilege = Privilege::Supervisor;
                        vm.cpu.tlb.flush_all();
                        count += 1;
                        continue;
                    }
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        vm.step();
        count += 1;
    }

    println!(
        "[check] After {} instructions, SATP=0x{:08X}",
        count, vm.cpu.csr.satp
    );

    // Read exception handler table entries at VA 0xC0C00AA4
    let table_base_va: u32 = 0xC0C00AA4;
    for i in 0..32u32 {
        let entry_va = table_base_va + i * 4;
        let result = translate(
            entry_va,
            AccessType::Load,
            vm.cpu.privilege,
            false,
            false,
            vm.cpu.csr.satp,
            &mut vm.bus,
            &mut vm.cpu.tlb,
        );
        if let TranslateResult::Ok(pa) = result {
            if let Ok(val) = vm.bus.read_word(pa) {
                if val != 0 {
                    println!(
                        "[check] excp_vect[{:2}] VA=0x{:08X} PA=0x{:08X} => handler=0x{:08X}",
                        i, entry_va, pa, val
                    );
                }
            }
        }
    }

    // Check L1 page table for 0xC0C00000
    let satp = vm.cpu.csr.satp;
    let ppn = satp & 0x3FFFFF;
    let pg_dir_pa = (ppn as u64) * 4096;
    let l1_idx = 780u64;
    if let Ok(l1_pte) = vm.bus.read_word(pg_dir_pa + l1_idx * 4) {
        let l1_ppn = (l1_pte >> 10) & 0x3FFFFF;
        let l1_v = l1_pte & 1;
        let l1_rwx = (l1_pte >> 1) & 7;
        println!(
            "\n[check] L1[780] PTE=0x{:08X} V={} RWX={} PPN=0x{:06X} (expect 12=0x00C00000)",
            l1_pte, l1_v, l1_rwx, l1_ppn
        );

        if l1_v != 0 && l1_rwx == 0 {
            let l2_base = (l1_ppn as u64) << 12;
            let l2_idx = 676u64; // (0xC0C00AA4 >> 12) & 0x3FF
            if let Ok(l2_pte) = vm.bus.read_word(l2_base + l2_idx * 4) {
                let l2_ppn = (l2_pte >> 10) & 0x3FFFFF;
                let l2_offset = (0xC0C00AA4u32 & 0xFFF) as u64;
                let pa = (l2_ppn as u64) << 12 | l2_offset as u64;
                println!(
                    "[check]   L2[676] PTE=0x{:08X} PPN=0x{:06X} => PA=0x{:08X}",
                    l2_pte, l2_ppn, pa
                );
                if let Ok(val) = vm.bus.read_word(pa) {
                    println!("[check]   Value at PA 0x{:08X}: 0x{:08X}", pa, val);
                }
            }
        }
    }
}
