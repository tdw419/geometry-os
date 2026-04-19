/// Diagnostic: Trace PC every 10K instructions to find what the kernel is doing
/// between SATP change (177K) and panic (330K).

use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";

    let kernel = std::fs::read(kernel_path).expect("kernel not found");
    let initramfs = std::fs::read(initramfs_path).expect("initramfs not found");

    let (mut vm, fw_addr, _entry, _dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel,
        Some(&initramfs),
        128,
        "console=ttyS0 earlycon=sbi panic=5 quiet nosmp",
    )
    .expect("boot setup failed");

    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let max: u64 = 500_000;
    let mut last_satap: u32 = vm.cpu.csr.satp;
    let mut last_pc: u32 = 0;
    let mut same_pc_count: u64 = 0;
    let mut last_report_pc: u32 = 0;
    let mut trap_count: u64 = 0;

    // Track when we first see M-mode traps
    while count < max {
        // Log PC at regular intervals
        let log_interval = if count < 180_000 {
            5_000
        } else if count < 250_000 {
            1_000
        } else {
            5_000
        };

        if count % log_interval == 0 && count > 0 {
            let pc = vm.cpu.pc;
            let priv_str = match vm.cpu.privilege {
                geometry_os::riscv::cpu::Privilege::Machine => "M",
                geometry_os::riscv::cpu::Privilege::Supervisor => "S",
                geometry_os::riscv::cpu::Privilege::User => "U",
            };
            eprintln!(
                "[trace] {:6}K PC=0x{:08X} ({}) satp=0x{:08X} ecall_count={} uart_len={}",
                count / 1000,
                pc,
                priv_str,
                vm.cpu.csr.satp,
                vm.cpu.ecall_count,
                vm.bus.uart.tx_buf.len()
            );
        }

        // Detect tight loops
        if vm.cpu.pc == last_pc {
            same_pc_count += 1;
        } else {
            if same_pc_count > 100 {
                eprintln!(
                    "[loop] Count {}: PC stuck at 0x{:08X} for {} instructions",
                    count, last_pc, same_pc_count
                );
            }
            same_pc_count = 0;
            last_pc = vm.cpu.pc;
        }

        // Check for M-mode trap at our handler
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == geometry_os::riscv::cpu::Privilege::Machine {
            trap_count += 1;
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if trap_count <= 30 {
                eprintln!(
                    "[trap] #{} count={} cause={} mepc=0x{:08X} mpp={}",
                    trap_count,
                    count,
                    cause_code,
                    vm.cpu.csr.mepc,
                    (vm.cpu.csr.mstatus >> 11) & 0x3
                );

                // Handle ECALL_S (SBI call)
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
                    if let Some((a0_val, a1_val)) = result {
                        vm.cpu.x[10] = a0_val;
                        vm.cpu.x[11] = a1_val;
                    }
                } else if cause_code != 11 {
                    // Forward non-ECALL_M traps to S-mode
                    let mpp = (vm.cpu.csr.mstatus >> 11) & 0x3;
                    if mpp != 3 {
                        let fault_addr = vm.cpu.csr.mtval;
                        let is_page_fault = cause_code == 12 || cause_code == 13 || cause_code == 15;
                        if is_page_fault && fault_addr < 0xC000_0000 {
                            let satp = vm.cpu.csr.satp;
                            let pg_dir_ppn = (satp & 0x3FFFFF) as u64;
                            if pg_dir_ppn > 0 {
                                let pg_dir_phys = pg_dir_ppn * 4096;
                                let vpn1 = ((fault_addr >> 22) & 0x3FF) as u64;
                                let l1_addr = pg_dir_phys + vpn1 * 4;
                                let existing = vm.bus.read_word(l1_addr).unwrap_or(0);
                                if (existing & 1) == 0 {
                                    let pte: u32 = 0x0000_00CF | ((vpn1 as u32) << 20);
                                    vm.bus.write_word(l1_addr, pte).ok();
                                    vm.cpu.tlb.flush_all();
                                    if trap_count <= 10 {
                                        eprintln!("  -> injected identity map L1[{}] = 0x{:08X}", vpn1, pte);
                                    }
                                } else {
                                    // Forward to S-mode
                                    let stvec = vm.cpu.csr.stvec & !0x3u32;
                                    if stvec != 0 {
                                        vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                                        vm.cpu.csr.scause = mcause;
                                        vm.cpu.csr.stval = vm.cpu.csr.mtval;
                                        let spp = if mpp == 1 { 1u32 } else { 0u32 };
                                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 5))
                                            | (spp << 5);
                                        let sie = (vm.cpu.csr.mstatus >> 1) & 1;
                                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << 5))
                                            | (sie << 5);
                                        vm.cpu.csr.mstatus &= !(1u32 << 1);
                                        vm.cpu.pc = stvec;
                                    }
                                }
                            }
                        }
                    }
                }

                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            }
        }

        // Track SATP changes
        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satap {
            eprintln!(
                "[satp] 0x{:08X} -> 0x{:08X} at count={}",
                last_satap, cur_satp, count
            );
            last_satap = cur_satp;
        }

        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        let _ = vm.step();
        count += 1;
    }

    eprintln!(
        "\n[summary] {} instructions, {} M-mode traps, {} UART bytes",
        count,
        trap_count,
        vm.bus.uart.tx_buf.len()
    );
    eprintln!("Final PC: 0x{:08X}", vm.cpu.pc);
}
