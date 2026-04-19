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
    let mut satp_changes: Vec<(u64, u32, u32)> = Vec::new();

    // Run until we see 3 SATP changes
    while count < 2_000_000 && satp_changes.len() < 3 {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        if vm.cpu.pc == fw_addr && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);

            if cause_code == 9 {
                // SBI ecall
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
                // Forward S-mode traps to S-mode stvec
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
            eprintln!(
                "[satp] Changed: 0x{:08X} -> 0x{:08X} at count={}",
                last_satp, current_satp, count
            );
            satp_changes.push((count, last_satp, current_satp));
            last_satp = current_satp;
        }

        let _ = vm.step();
        count += 1;
    }

    eprintln!(
        "[check] Final SATP: 0x{:08X}, PC: 0x{:08X}, count: {}",
        vm.cpu.csr.satp, vm.cpu.pc, count
    );

    // Now dump the page table for the failing addresses
    let satp = vm.cpu.csr.satp;
    let pg_dir_phys = ((satp & 0x003F_FFFF) as u64) << 12;
    eprintln!("[check] Page table root at PA 0x{:08X}", pg_dir_phys);

    // Check L1 entries for the failing VAs
    let fail_vas = [0xC0404E12u32, 0xC0210F14u32, 0xC0000000u32, 0xC0210000u32];
    for &va in &fail_vas {
        let vpn1 = (va >> 22) & 0x3FF;
        let vpn0 = (va >> 12) & 0x3FF;
        let l1_addr = pg_dir_phys + (vpn1 as u64) * 4;
        let l1_pte = vm.bus.read_word(l1_addr).unwrap_or(0);
        let valid = (l1_pte >> 0) & 1;
        let is_leaf = ((l1_pte >> 1) & 1) | ((l1_pte >> 2) & 1) | ((l1_pte >> 3) & 1);
        eprintln!(
            "[pt] VA 0x{:08X}: VPN1={}, L1[{}]=0x{:08X} (V={} leaf={})",
            va, vpn1, vpn1, l1_pte, valid, is_leaf
        );

        if valid == 1 && is_leaf == 0 {
            // Non-leaf: check L2
            let l2_ppn = (l1_pte >> 10) & 0xFFFFF;
            let l2_base = (l2_ppn as u64) << 12;
            let l2_addr = l2_base + (vpn0 as u64) * 4;
            let l2_pte = vm.bus.read_word(l2_addr).unwrap_or(0);
            let l2_valid = (l2_pte >> 0) & 1;
            eprintln!(
                "[pt]   L2[{}]=0x{:08X} at PA 0x{:08X} (V={})",
                vpn0, l2_pte, l2_addr, l2_valid
            );
        }
    }

    // Also check stvec and stval at this point
    eprintln!(
        "[regs] stvec=0x{:08X} stval=0x{:08X} mepc=0x{:08X} scause=0x{:08X}",
        vm.cpu.csr.stvec, vm.cpu.csr.stval, vm.cpu.csr.mepc, vm.cpu.csr.scause
    );
    eprintln!("[regs] privilege={:?}", vm.cpu.privilege);
}
