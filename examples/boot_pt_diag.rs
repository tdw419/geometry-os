use geometry_os::riscv::cpu::{Privilege, StepResult};
use geometry_os::riscv::RiscvVm;
use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    let (mut vm, fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 128, bootargs).unwrap();

    let fw_addr_u32 = fw_addr as u32;
    let max_count: u64 = 20_000_000;
    let mut count: u64 = 0;
    let mut sbi_count: u64 = 0;

    while count < max_count {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let cause_code = vm.cpu.csr.mcause & !(1u32 << 31);
            if !((vm.cpu.csr.mcause >> 31) & 1 == 1) && cause_code == 9 {
                sbi_count += 1;
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
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        let step_result = vm.step();

        // On first S-mode page fault, dump page table info
        if vm.cpu.privilege == Privilege::Supervisor {
            match step_result {
                StepResult::LoadFault | StepResult::StoreFault | StepResult::FetchFault => {
                    let fault_va = vm.cpu.csr.stval;
                    let fault_pc = vm.cpu.csr.sepc;
                    let satp = vm.cpu.csr.satp;
                    let pg_dir_ppn = (satp & 0x3FFFFF) as u64;
                    let pg_dir_phys = pg_dir_ppn * 4096;

                    eprintln!(
                        "\n=== S-mode {} FAULT at count={} ===",
                        match step_result {
                            StepResult::LoadFault => "LOAD",
                            StepResult::StoreFault => "STORE",
                            _ => "FETCH",
                        },
                        count
                    );
                    eprintln!("  stval (fault VA) = 0x{:08X}", fault_va);
                    eprintln!("  sepc (fault PC)  = 0x{:08X}", fault_pc);
                    eprintln!("  SATP = 0x{:08X}, pg_dir PA = 0x{:08X}", satp, pg_dir_phys);
                    eprintln!("  stvec = 0x{:08X}", vm.cpu.csr.stvec);
                    eprintln!("  SP = 0x{:08X}", vm.cpu.x[2]);

                    // Walk the page table for the faulting VA
                    let vpn1 = ((fault_va >> 22) & 0x3FF) as u64;
                    let vpn0 = ((fault_va >> 12) & 0x3FF) as u64;
                    let l1_addr = pg_dir_phys + vpn1 * 4;
                    let l1_pte = vm.bus.read_word(l1_addr).unwrap_or(0);
                    let l1_v = (l1_pte >> 0) & 1;
                    let l1_r = (l1_pte >> 1) & 1;
                    let l1_ppn = ((l1_pte >> 10) & 0x3FFFFF);

                    eprintln!("\n  Page table walk for VA 0x{:08X}:", fault_va);
                    eprintln!("  L1[{}] at PA 0x{:08X} = 0x{:08X}", vpn1, l1_addr, l1_pte);
                    eprintln!(
                        "    V={} R={} PPN=0x{:06X} (PA=0x{:08X})",
                        l1_v,
                        l1_r,
                        l1_ppn,
                        l1_ppn << 12
                    );

                    if l1_v == 0 {
                        eprintln!("    *** L1 entry NOT VALID -- VA is not mapped at all! ***");
                    } else if l1_r != 0 {
                        // Megapage
                        eprintln!(
                            "    MEGAPAGE: PA = 0x{:08X}",
                            ((fault_va & 0x3FFFFF) | (l1_ppn << 12))
                        );
                    } else {
                        // L2 walk
                        let l2_base = l1_ppn as u64 * 4096;
                        let l2_addr = l2_base + vpn0 * 4;
                        let l2_pte = vm.bus.read_word(l2_addr).unwrap_or(0);
                        let l2_v = (l2_pte >> 0) & 1;
                        let l2_r = (l2_pte >> 1) & 1;
                        let l2_w = (l2_pte >> 2) & 1;
                        let l2_x = (l2_pte >> 3) & 1;
                        let l2_ppn = ((l2_pte >> 10) & 0x3FFFFF);
                        let l2_d = (l2_pte >> 6) & 1; // dirty
                        let l2_a = (l2_pte >> 5) & 1; // accessed

                        eprintln!("  L2[{}] at PA 0x{:08X} = 0x{:08X}", vpn0, l2_addr, l2_pte);
                        eprintln!(
                            "    V={} R={} W={} X={} D={} A={} PPN=0x{:06X} (PA=0x{:08X})",
                            l2_v,
                            l2_r,
                            l2_w,
                            l2_x,
                            l2_d,
                            l2_a,
                            l2_ppn,
                            l2_ppn << 12
                        );

                        if l2_v == 0 {
                            eprintln!("    *** L2 entry NOT VALID -- page not allocated! ***");
                        }
                    }

                    // Also check a few surrounding L1 entries to see if vmalloc area is mapped
                    eprintln!(
                        "\n  Surrounding L1 entries (swapper_pg_dir at PA 0x{:08X}):",
                        pg_dir_phys
                    );
                    for idx in (vpn1 as u32).saturating_sub(2)..=(vpn1 as u32 + 2) {
                        let addr = pg_dir_phys + (idx as u64) * 4;
                        let pte = vm.bus.read_word(addr).unwrap_or(0);
                        let v = (pte >> 0) & 1;
                        let r = (pte >> 1) & 1;
                        let ppn = ((pte >> 10) & 0x3FFFFF);
                        let marker = if idx == vpn1 as u32 { " <-- fault" } else { "" };
                        eprintln!(
                            "    L1[{}] = 0x{:08X} V={} R={} PPN=0x{:06X}{}",
                            idx, pte, v, r, ppn, marker
                        );
                    }

                    // Check linear mapping entries (L1[768..])
                    eprintln!("\n  Linear mapping L1 entries:");
                    for idx in [768, 769, 770, 800, 820, 831] {
                        let addr = pg_dir_phys + (idx as u64) * 4;
                        let pte = vm.bus.read_word(addr).unwrap_or(0);
                        let v = (pte >> 0) & 1;
                        let r = (pte >> 1) & 1;
                        let ppn = ((pte >> 10) & 0x3FFFFF);
                        eprintln!(
                            "    L1[{}] = 0x{:08X} V={} R={} PPN=0x{:06X}",
                            idx, pte, v, r, ppn
                        );
                    }

                    // Check how many known_pt_pages are registered
                    eprintln!("\n  known_pt_pages count: {}", vm.bus.known_pt_pages.len());
                    eprintln!("  auto_pte_fixup: {}", vm.bus.auto_pte_fixup);

                    break;
                }
                _ => {}
            }
        }

        // Check for panic
        if (0xC000252E..=0xC00027A0).contains(&vm.cpu.pc) && count > 1_000_000 && sbi_count == 0 {
            let ptr = vm.cpu.x[10];
            if ptr > 0xC0000000 && ptr < 0xC2000000 {
                let pa = (ptr - 0xC0000000) as u64;
                let mut bytes = Vec::new();
                for i in 0..128u64 {
                    if let Ok(b) = vm.bus.read_byte(pa + i) {
                        if b == 0 {
                            break;
                        }
                        bytes.push(b);
                    } else {
                        break;
                    }
                }
                if let Ok(s) = String::from_utf8(bytes) {
                    eprintln!("\n!!! PANIC: '{}' !!!", &s[..s.len().min(200)]);
                }
            }
            break;
        }

        count += 1;
    }

    eprintln!("\nTotal: {} instr, {} SBI", count, sbi_count);
    let tx = vm.bus.uart.drain_tx();
    eprintln!("UART: {} bytes", tx.len());
    if !tx.is_empty() {
        eprintln!("{}", String::from_utf8_lossy(&tx));
    }
}
