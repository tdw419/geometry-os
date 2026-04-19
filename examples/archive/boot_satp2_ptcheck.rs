use geometry_os::riscv::cpu::Privilege;
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, fw_addr_u64, _entry, _dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        256,
        "console=ttyS0 loglevel=8",
    )
    .unwrap();

    let fw_addr = fw_addr_u64 as u32;
    let mut count: u64 = 0;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut satp_count = 0;
    let target_satp_changes = 2; // Stop right after 2nd SATP change

    while count < 2_000_000 && satp_count < target_satp_changes {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        if vm.cpu.pc == fw_addr && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);

            if cause_code == 9 {
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
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
                count += 1;
                continue;
            } else if cause_code != 11 {
                let mpp = (vm.cpu.csr.mstatus >> 11) & 3;
                if mpp != 3 {
                    vm.cpu.csr.mepc = vm.cpu.csr.stvec;
                    vm.cpu.privilege = Privilege::Supervisor;
                    count += 1;
                    continue;
                }
            }
        }

        let current_satp = vm.cpu.csr.satp;
        if current_satp != last_satp {
            satp_count += 1;
            eprintln!(
                "[satp] #{}: 0x{:08X} -> 0x{:08X} at count={}",
                satp_count, last_satp, current_satp, count
            );
            last_satp = current_satp;
        }

        let _ = vm.step();
        count += 1;
    }

    // Dump the page table for SATP #2 (0x80000802)
    let satp = vm.cpu.csr.satp;
    let pg_dir_phys = ((satp & 0x003F_FFFF) as u64) << 12;
    eprintln!(
        "[check] SATP=0x{:08X}, Root at PA 0x{:08X}",
        satp, pg_dir_phys
    );

    eprintln!("[pt] === L1 entries for kernel VA range (768-800) ===");
    for i in 760..800 {
        let l1_addr = pg_dir_phys + (i as u64) * 4;
        let l1_pte = vm.bus.read_word(l1_addr).unwrap_or(0);
        if l1_pte != 0 {
            let v = (l1_pte >> 0) & 1;
            let r = (l1_pte >> 1) & 1;
            let w = (l1_pte >> 2) & 1;
            let x = (l1_pte >> 3) & 1;
            let ppn = (l1_pte >> 10) & 0xFFFFF;
            eprintln!(
                "[pt] L1[{}]: 0x{:08X} V={} R={} W={} X={} PPN=0x{:05X} (PA=0x{:08X})",
                i,
                l1_pte,
                v,
                r,
                w,
                x,
                ppn,
                ppn as u64 * 4096
            );
        }
    }

    // Check the first 5 L2 entries for L1[768]
    let l1_768 = vm.bus.read_word(pg_dir_phys + 768 * 4).unwrap_or(0);
    let l2_768_ppn = (l1_768 >> 10) & 0xFFFFF;
    let l2_768_base = (l2_768_ppn as u64) << 12;
    eprintln!("[pt] L2 for L1[768] at PA 0x{:08X}:", l2_768_base);
    for j in 0..10 {
        let l2_addr = l2_768_base + (j as u64) * 4;
        let l2_pte = vm.bus.read_word(l2_addr).unwrap_or(0);
        if l2_pte != 0 {
            let l2_ppn2 = (l2_pte >> 10) & 0xFFFFF;
            let va = 0xC0000000u64 + (j as u64) * 4096;
            eprintln!(
                "[pt]   L2[{}]: 0x{:08X} -> PA 0x{:08X} (VA 0x{:08X})",
                j,
                l2_pte,
                l2_ppn2 as u64 * 4096,
                va
            );
        }
    }
}
