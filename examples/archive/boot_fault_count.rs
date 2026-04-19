use geometry_os::riscv::cpu::Privilege;
use geometry_os::riscv::RiscvVm;
use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    let (mut vm, fw_addr, _entry, _dtb_addr) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 512, bootargs).unwrap();

    let fw_addr_u32 = fw_addr as u32;
    let max_count: u64 = 10_000_000;
    let mut count: u64 = 0;
    let mut trap_count: u64 = 0;
    let mut sbi_count: u64 = 0;
    let mut store_fault_count: u64 = 0;
    let mut load_fault_count: u64 = 0;
    let mut fetch_fault_count: u64 = 0;
    let mut illegal_count: u64 = 0;
    let mut ecall_s_count: u64 = 0;
    let mut last_stvec: u32 = 0;
    let mut prev_pc: u32 = 0;
    let mut same_pc_count: u64 = 0;
    let mut loop_detect: u64 = 0;

    while count < max_count {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        // Detect PC loop (same PC for 100+ consecutive instructions)
        if vm.cpu.pc == prev_pc {
            same_pc_count += 1;
            if same_pc_count == 100 && loop_detect == 0 {
                eprintln!(
                    "[loop] Stuck at PC=0x{:08X} for 100+ instructions at count={}",
                    vm.cpu.pc, count
                );
                loop_detect += 1;
            }
        } else {
            same_pc_count = 0;
        }

        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            trap_count += 1;
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            match cause_code {
                2 => illegal_count += 1,
                9 => {
                    ecall_s_count += 1;
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
                12 => fetch_fault_count += 1,
                13 => load_fault_count += 1,
                15 => store_fault_count += 1,
                _ => {}
            }
            vm.cpu.csr.mepc = vm.cpu.csr.mepc.wrapping_add(4);
        }

        vm.bus.tick_clint();
        vm.bus.sync_mip(&mut vm.cpu.csr.mip);
        vm.step();

        // Track stvec changes
        let cur_stvec = vm.cpu.csr.stvec & !3;
        if cur_stvec != last_stvec && cur_stvec != 0 && count > 1000 {
            eprintln!(
                "[stvec] Changed to 0x{:08X} at count={} (prev=0x{:08X})",
                cur_stvec, count, last_stvec
            );
            last_stvec = cur_stvec;
        }

        count += 1;

        if count % 2_000_000 == 0 {
            eprintln!("[{}] PC=0x{:08X} SBI={} traps={} illegal={} stvf={} ldf={} stf={} ecall_s={} loop_detect={}",
                count / 1_000_000, vm.cpu.pc, sbi_count, trap_count, illegal_count,
                fetch_fault_count, load_fault_count, store_fault_count, ecall_s_count, loop_detect);
        }

        prev_pc = vm.cpu.pc;
    }

    eprintln!("\n=== Summary ===");
    eprintln!("Instructions: {}", count);
    eprintln!("M-mode traps: {}", trap_count);
    eprintln!("  Illegal instruction: {}", illegal_count);
    eprintln!("  ECALL_S (SBI): {}", ecall_s_count);
    eprintln!("  Fetch fault: {}", fetch_fault_count);
    eprintln!("  Load fault: {}", load_fault_count);
    eprintln!("  Store fault: {}", store_fault_count);
    eprintln!("PC loops detected: {}", loop_detect);

    let tx = vm.bus.uart.drain_tx();
    eprintln!("UART: {} bytes", tx.len());
    if !tx.is_empty() {
        let s = String::from_utf8_lossy(&tx);
        eprintln!("{}", &s[..s.len().min(500)]);
    }
}
