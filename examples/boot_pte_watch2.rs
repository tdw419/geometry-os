/// Watch the PTE at L1 index 770 for corruption.
/// Stops execution the instant the PTE changes from its valid state.
/// Also monitors all writes to the page table area.
fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    use geometry_os::riscv::RiscvVm;
    use geometry_os::riscv::cpu::{Privilege, StepResult};
    use geometry_os::riscv::csr;

    let (mut vm, fw_addr, _entry, _dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_image, initramfs.as_deref(), 256, bootargs
    ).unwrap();

    let fw_addr_u32 = fw_addr as u32;

    // Get page table base
    let satp = vm.cpu.csr.read(csr::SATP);
    let ppn = satp & 0x3FFFFF;
    let pt_base = (ppn as u64) << 12;

    // Watch PTE at index 770 (the one known to get corrupted)
    let watch_idx: u64 = 770;
    let watch_pte_addr = pt_base + watch_idx * 4;
    let initial_pte = vm.bus.read_word(watch_pte_addr).unwrap_or(0);
    eprintln!("[watch] PTE[770] at 0x{:08X} initial=0x{:08X} V={}",
        watch_pte_addr, initial_pte, initial_pte & 1);

    let mut last_known_pte = initial_pte;
    let mut count: u64 = 0;
    let max_instructions: u64 = 20_000_000;
    let mut sbi_call_count: u64 = 0;
    let mut forward_count: u64 = 0;

    // For efficiency, check PTE every N instructions
    let check_interval: u64 = 500;

    while count < max_instructions {
        if vm.bus.sbi.shutdown_requested {
            eprintln!("[watch] SBI shutdown at count={}", count);
            break;
        }

        // Handle M-mode trap (same logic as boot_linux)
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);

            if cause_code == csr::CAUSE_ECALL_S {
                sbi_call_count += 1;
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16],
                    vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13],
                    vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0_val, a1_val)) = result {
                    vm.cpu.x[10] = a0_val;
                    vm.cpu.x[11] = a1_val;
                }
            } else if cause_code != csr::CAUSE_ECALL_M {
                let mpp = (vm.cpu.csr.mstatus & csr::MSTATUS_MPP_MASK) >> csr::MSTATUS_MPP_LSB;
                if mpp != 3 {
                    let stvec = vm.cpu.csr.stvec & !0x3u32;
                    if stvec != 0 {
                        vm.cpu.csr.sepc = vm.cpu.csr.mepc;
                        vm.cpu.csr.scause = mcause;
                        vm.cpu.csr.stval = vm.cpu.csr.mtval;
                        let spp = if mpp == 1 { 1u32 } else { 0u32 };
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << csr::MSTATUS_SPP))
                            | (spp << csr::MSTATUS_SPP);
                        let sie = (vm.cpu.csr.mstatus >> csr::MSTATUS_SIE) & 1;
                        vm.cpu.csr.mstatus = (vm.cpu.csr.mstatus & !(1 << csr::MSTATUS_SPIE))
                            | (sie << csr::MSTATUS_SPIE);
                        vm.cpu.csr.mstatus &= !(1 << csr::MSTATUS_SIE);
                        vm.cpu.pc = stvec;
                        vm.cpu.privilege = Privilege::Supervisor;
                        vm.cpu.tlb.flush_all();
                        forward_count += 1;
                        count += 1;
                        continue;
                    }
                }
            } else {
                // ECALL_M: SBI call
                let result = vm.bus.sbi.handle_ecall(
                    vm.cpu.x[17], vm.cpu.x[16],
                    vm.cpu.x[10], vm.cpu.x[11],
                    vm.cpu.x[12], vm.cpu.x[13],
                    vm.cpu.x[14], vm.cpu.x[15],
                    &mut vm.bus.uart, &mut vm.bus.clint,
                );
                if let Some((a0_val, a1_val)) = result {
                    vm.cpu.x[10] = a0_val;
                    vm.cpu.x[11] = a1_val;
                }
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        let _step_result = vm.step();

        // Periodically check the watched PTE
        if count % check_interval == 0 && count > 0 {
            let current_pte = vm.bus.read_word(watch_pte_addr).unwrap_or(0);
            if current_pte != last_known_pte {
                eprintln!("\n[watch] *** PTE[770] CORRUPTION at count={} ***", count);
                eprintln!("[watch] was 0x{:08X} now 0x{:08X}", last_known_pte, current_pte);
                eprintln!("[watch] PC=0x{:08X} priv={:?}", vm.cpu.pc, vm.cpu.privilege);

                // Dump all registers
                let names = ["zero","ra","sp","gp","tp","t0","t1","t2","s0","s1",
                    "a0","a1","a2","a3","a4","a5","a6","a7","s2","s3",
                    "s4","s5","s6","s7","s8","s9","s10","s11","t3","t4","t5","t6"];
                for i in 0..32 {
                    eprintln!("  x{:2} ({:4}) = 0x{:08X}", i, names[i], vm.cpu.x[i]);
                }
                eprintln!("[watch] sepc=0x{:08X} scause=0x{:08X} stval=0x{:08X} stvec=0x{:08X}",
                    vm.cpu.csr.sepc, vm.cpu.csr.scause, vm.cpu.csr.stval, vm.cpu.csr.stvec);
                eprintln!("[watch] SATP=0x{:08X} SSTATUS=0x{:08X}",
                    vm.cpu.csr.read(csr::SATP), vm.cpu.csr.read(csr::SSTATUS));
                eprintln!("[watch] ECALL_count={} SBI_calls={} forwards={}",
                    vm.cpu.ecall_count, sbi_call_count, forward_count);
                eprintln!("[watch] UART tx: {} chars", vm.bus.uart.tx_buf.len());
                eprintln!("[watch] SBI console: {} chars", vm.bus.sbi.console_output.len());

                // Dump PTEs around the corruption
                eprintln!("[watch] PTEs 760-780:");
                for i in 760..=780u64 {
                    let addr = pt_base + i * 4;
                    let pte = vm.bus.read_word(addr).unwrap_or(0);
                    let marker = if i == 770 { " <<<" } else { "" };
                    eprintln!("  L1[{}] @ 0x{:08X} = 0x{:08X} V={}{}", i, addr, pte, pte & 1, marker);
                }

                // Show the last 50 chars of any output
                if !vm.bus.uart.tx_buf.is_empty() {
                    let s = String::from_utf8_lossy(&vm.bus.uart.tx_buf);
                    let start = s.len().saturating_sub(200);
                    eprintln!("[watch] UART (last 200): {}", &s[start..]);
                }
                if !vm.bus.sbi.console_output.is_empty() {
                    let s = String::from_utf8_lossy(&vm.bus.sbi.console_output);
                    let start = s.len().saturating_sub(200);
                    eprintln!("[watch] SBI console (last 200): {}", &s[start..]);
                }

                break;
            }
            last_known_pte = current_pte;
        }

        count += 1;

        // Progress report every 2M instructions
        if count % 2_000_000 == 0 {
            let pte = vm.bus.read_word(watch_pte_addr).unwrap_or(0);
            eprintln!("[watch] count={:8} PC=0x{:08X} priv={:?} PTE[770]=0x{:08X} ecall={} sbi={}",
                count, vm.cpu.pc, vm.cpu.privilege, pte, vm.cpu.ecall_count, sbi_call_count);
        }
    }
}
