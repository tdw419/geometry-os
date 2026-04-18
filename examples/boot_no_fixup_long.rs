// Test: run for 50M instructions WITHOUT auto_pte_fixup
// to see if the kernel eventually reaches console output
use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::{StepResult, Privilege};
use geometry_os::riscv::csr;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();

    let (mut vm, fw_addr, _entry, _dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        256,
        "console=ttyS0 earlycon=sbi panic=1",
    )
    .unwrap();

    // Disable auto_pte_fixup - we'll rely on in-place PTE fixup at SATP changes only
    vm.bus.auto_pte_fixup = false;
    eprintln!("[test] auto_pte_fixup DISABLED, max 50M instructions");

    let entry = _entry as u32;
    vm.cpu.csr.mepc = entry;
    vm.cpu.csr.mstatus = 1u32 << csr::MSTATUS_MPP_LSB;
    vm.cpu.csr.mstatus |= 1 << csr::MSTATUS_MPIE;
    let restored = vm.cpu.csr.trap_return(Privilege::Machine);
    vm.cpu.pc = vm.cpu.csr.mepc;
    vm.cpu.privilege = restored;

    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let max_instructions: u64 = 50_000_000;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut sbi_count: u64 = 0;
    let mut last_pc: u32 = 0;
    let mut same_pc: u64 = 0;

    while count < max_instructions {
        let prev_pc = vm.cpu.pc;
        let step_result = vm.step();

        if vm.cpu.pc == fw_addr_u32 {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = (mcause & 0xFF) as u32;
            let mpp = (vm.cpu.csr.mstatus >> csr::MSTATUS_MPP_LSB) & 0x3;

            if cause_code == 11 {
                vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
            } else if mpp != 3 {
                let stvec = vm.cpu.csr.stvec & !0x3u32;
                if stvec != 0 && cause_code == 9 {
                    sbi_count += 1;
                    if sbi_count <= 10 {
                        eprintln!(
                            "[test] ECALL_S #{} at count={}: a7={:#x} a6={:#x} a0={:#x}",
                            sbi_count, count, vm.cpu.x[17], vm.cpu.x[16], vm.cpu.x[10]
                        );
                    }
                    vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
                } else if stvec != 0 {
                    let fault_addr = vm.cpu.csr.mtval;
                    let is_pf = cause_code == 12 || cause_code == 13 || cause_code == 15;
                    if is_pf && fault_addr < 0x0200_0000 {
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
                            }
                        }
                    } else if stvec != 0 {
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
                        if cause_code == csr::INT_MTI {
                            vm.bus.clint.mtimecmp = vm.bus.clint.mtime + 100_000;
                        }
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

        vm.bus.tick_clint();

        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            eprintln!("[test] SATP changed: 0x{:08X} -> 0x{:08X} at count={}", last_satp, cur_satp, count);
            last_satp = cur_satp;
        }

        // Detect infinite loops
        if vm.cpu.pc == last_pc {
            same_pc += 1;
        } else {
            same_pc = 0;
            last_pc = vm.cpu.pc;
        }

        count += 1;

        if count % 5_000_000 == 0 {
            eprintln!(
                "[test] {}M: PC=0x{:08X} SBI={} same_pc={}",
                count / 1_000_000, vm.cpu.pc, sbi_count, same_pc
            );
        }

        // Break on SBI output or panic
        if sbi_count > 0 && !vm.bus.uart.tx_buf.is_empty() {
            eprintln!("[test] UART output detected at count={}!", count);
            break;
        }
        if same_pc > 500_000 {
            eprintln!("[test] Infinite loop detected at PC=0x{:08X}", vm.cpu.pc);
            break;
        }
    }

    eprintln!("\n[test] Done: {} instructions, {} SBI calls", count, sbi_count);
    eprintln!("[test] UART: {} chars", vm.bus.uart.tx_buf.len());
    if !vm.bus.uart.tx_buf.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.uart.tx_buf);
        let preview: String = s.chars().take(3000).collect();
        eprintln!("UART:\n{}", preview);
    }
}
