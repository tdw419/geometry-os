use geometry_os::riscv::cpu::{Privilege, StepResult};
use geometry_os::riscv::RiscvVm;
use std::fs;
use std::time::Instant;

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
    let mut page_faults: u64 = 0;
    let mut last_satp: u32 = vm.cpu.csr.satp;

    while count < max_count {
        if vm.bus.sbi.shutdown_requested {
            break;
        }

        // Handle M-mode traps at fw_addr
        if vm.cpu.pc == fw_addr_u32 && vm.cpu.privilege == Privilege::Machine {
            let mcause = vm.cpu.csr.mcause;
            let cause_code = mcause & !(1u32 << 31);
            if !((mcause >> 31) & 1 == 1) && cause_code == 9 {
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

        // Track S-mode page faults
        match step_result {
            StepResult::LoadFault | StepResult::StoreFault | StepResult::FetchFault => {
                if vm.cpu.privilege == Privilege::Supervisor {
                    page_faults += 1;
                    if page_faults <= 10 {
                        let fault_type = match step_result {
                            StepResult::LoadFault => "LOAD",
                            StepResult::StoreFault => "STORE",
                            StepResult::FetchFault => "FETCH",
                            _ => "?",
                        };
                        eprintln!("[FAULT#{}] {} at count={} PC=0x{:08X} stval=0x{:08X} scause=0x{:08X} SP=0x{:08X}",
                            page_faults, fault_type, count, vm.cpu.pc, vm.cpu.csr.stval,
                            vm.cpu.csr.scause, vm.cpu.x[2]);
                        // Decode the faulting instruction
                        if let Ok(instr_bytes) = vm.bus.read_word(vm.cpu.pc as u64) {
                            eprintln!("    instruction: 0x{:08X}", instr_bytes);
                        }
                    }
                }
            }
            _ => {}
        }

        // Track SATP changes
        if vm.cpu.csr.satp != last_satp {
            eprintln!(
                "[satp] 0x{:08X} -> 0x{:08X} at count={}",
                last_satp, vm.cpu.csr.satp, count
            );
            last_satp = vm.cpu.csr.satp;
        }

        // Check for panic
        if (0xC000252E..=0xC00027A0).contains(&vm.cpu.pc) && count > 1_000_000 && sbi_count == 0 {
            eprintln!("\n!!! PANIC at count={} PC=0x{:08X} !!!", count, vm.cpu.pc);
            eprintln!(
                "    sepc=0x{:08X} scause=0x{:08X} stval=0x{:08X}",
                vm.cpu.csr.sepc, vm.cpu.csr.scause, vm.cpu.csr.stval
            );
            eprintln!("    Page faults seen: {}", page_faults);
            // Read panic string from A0
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
                    eprintln!("    Message: '{}'", &s[..s.len().min(200)]);
                }
            }
            let tx = vm.bus.uart.drain_tx();
            eprintln!("    UART: {} bytes", tx.len());
            if !tx.is_empty() {
                eprintln!("    {}", String::from_utf8_lossy(&tx));
            }
            break;
        }

        count += 1;

        if count % 5_000_000 == 0 {
            eprintln!(
                "[{}M] PC=0x{:08X} SBI={} faults={}",
                count / 1_000_000,
                vm.cpu.pc,
                sbi_count,
                page_faults
            );
        }
    }

    eprintln!(
        "\nTotal: {} instr, {} SBI, {} page_faults",
        count, sbi_count, page_faults
    );
}
