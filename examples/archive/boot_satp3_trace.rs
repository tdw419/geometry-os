// Diagnostic: run boot_linux normally but with a trace callback after 3rd SATP change
// We modify the step loop to capture instructions right after the 3rd SATP switch
use std::fs;
use std::time::Instant;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();

    // Use boot_linux_setup to get the VM before the step loop runs
    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    let setup_result = geometry_os::riscv::RiscvVm::boot_linux_setup(
        &kernel_image,
        initramfs.as_deref(),
        512,
        bootargs,
    )
    .expect("boot setup failed");

    let mut vm = setup_result.0;
    let fw_addr = setup_result.1;
    let entry = setup_result.2;
    let _dtb_addr = setup_result.3;

    // Replicate the MRET and trap setup from boot_linux
    use geometry_os::riscv::cpu::Privilege;
    use geometry_os::riscv::cpu::StepResult;
    use geometry_os::riscv::csr;

    // Write firmware MRET stub
    // mret: 0x30200073
    vm.bus.write_word(fw_addr, 0x30200073).ok();
    // csrw stvec, a0: after MRET, the kernel sets stvec
    // NOP after mret as fallback
    vm.bus.write_word(fw_addr + 4, 0x00000013).ok();

    // Setup MRET
    vm.cpu.csr.write(csr::MEPC, entry);
    vm.cpu.csr.mstatus = 1u32 << csr::MSTATUS_MPP_LSB; // MPP = S
    vm.cpu.csr.mstatus |= 1 << csr::MSTATUS_MPIE;
    let restored = vm.cpu.csr.trap_return(Privilege::Machine);
    vm.cpu.pc = vm.cpu.csr.mepc;
    vm.cpu.privilege = restored;

    // Set MTVEC to fw_addr for trap handling
    vm.cpu.csr.write(csr::MTVEC, fw_addr as u32);

    // Delegate exceptions to S-mode (keep ECALL_S in M-mode)
    vm.cpu.csr.medeleg = 0xA109;
    vm.cpu.csr.mideleg = 0x222;

    // Set initial SATP to boot page table
    let boot_pt_addr = 0x148000u64;
    let satp_val = (1u32 << 31) | ((boot_pt_addr / 4096) as u32);
    vm.cpu.csr.write(csr::SATP, satp_val);

    // Enable low_addr_identity_map and auto_pte_fixup (like boot_linux does)
    vm.bus.low_addr_identity_map = true;
    vm.bus.auto_pte_fixup = true;

    // Apply kernel_map binary patch (same as boot_linux)
    let setup_vm_phys_addr_store: u64 = 0x0040495E;
    let setup_vm_va_kernel_pa_store: u64 = 0x00404964;
    let setup_vm_va_pa_offset_store: u64 = 0x00404968;
    let kernel_map_phys: u64 = 0x00C79E90;

    let sw_a5_12 = vm.bus.read_half(setup_vm_phys_addr_store).unwrap_or(0);
    let sw_a6_24 = vm.bus.read_word(setup_vm_va_kernel_pa_store).unwrap_or(0);
    let sw_a1_20 = vm.bus.read_half(setup_vm_va_pa_offset_store).unwrap_or(0);

    if sw_a5_12 == 0xC4DC && sw_a6_24 == 0x0104AC23 && sw_a1_20 == 0xC8CC {
        vm.bus.write_half(setup_vm_phys_addr_store, 0x0001).ok(); // C.NOP
        vm.bus
            .write_word(setup_vm_va_kernel_pa_store, 0x00000013)
            .ok(); // NOP
        vm.bus.write_half(setup_vm_va_pa_offset_store, 0x0001).ok(); // C.NOP
        vm.bus.write_word(kernel_map_phys + 12, 0x00000000).ok(); // phys_addr = 0
        vm.bus.write_word(kernel_map_phys + 20, 0xC0000000).ok(); // va_pa_offset
        vm.bus.write_word(kernel_map_phys + 24, 0x00000000).ok(); // va_kernel_pa_offset = 0
        eprintln!("[boot] Patched kernel_map: phys_addr=0, va_pa_offset=0xC0000000, va_kernel_pa_offset=0");
    }

    let start = Instant::now();
    let mut last_satp = satp_val;
    let mut satp_count = 0;
    let mut trace_active = false;
    let mut trace_count = 0;
    let max_trace = 1000;
    let max_instr = 1_000_000u64;

    for count in 1..=max_instr {
        let step_result = vm.step();

        // Check SATP changes
        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            satp_count += 1;
            eprintln!(
                "[{}] SATP #{}/3: 0x{:08X} -> 0x{:08X}",
                count, satp_count, last_satp, cur_satp
            );
            last_satp = cur_satp;

            if satp_count == 3 {
                eprintln!("[{}] === 3rd SATP change, starting trace ===", count);
                trace_active = true;
                trace_count = 0;
            }
        }

        if trace_active && trace_count < max_trace {
            let pc = vm.cpu.pc;
            let inst = vm.bus.read_word(pc as u64).unwrap_or(0);
            let opcode = inst & 0x7F;

            // Check for ECALL
            if opcode == 0x73 {
                let funct3 = (inst >> 12) & 7;
                if funct3 == 0 {
                    eprintln!(
                        "  [{:5}] PC=0x{:08X} ECALL (imm={})",
                        trace_count,
                        pc,
                        (inst >> 20) & 0xFFF
                    );
                }
            }
            // Check for JAL/JALR (function calls)
            else if opcode == 0x6F {
                let rd = (inst >> 7) & 0x1F;
                let imm = (((inst >> 31) & 1) << 20)
                    | (((inst >> 12) & 0xFF) << 12)
                    | (((inst >> 20) & 1) << 11)
                    | (((inst >> 21) & 0x3FF) << 1);
                let target = (pc as i32).wrapping_add(imm as i32) as u32;
                eprintln!(
                    "  [{:5}] PC=0x{:08X} JAL x{} -> 0x{:08X}",
                    trace_count, pc, rd, target
                );
            } else if opcode == 0x67 {
                let rd = (inst >> 7) & 0x1F;
                let rs1 = (inst >> 15) & 0x1F;
                let imm = ((inst >> 20) as i32) as i64;
                let base = vm.cpu.x[rs1 as usize] as i64;
                let target = ((base + imm) & 0xFFFFFFFF) as u32;
                if rd != 0 || rs1 != 0 {
                    // Skip C.JR (rd=0, rs1!=0) for now
                    eprintln!(
                        "  [{:5}] PC=0x{:08X} JALR x{} x{} -> 0x{:08X}",
                        trace_count, pc, rd, rs1, target
                    );
                }
            }

            trace_count += 1;
        }

        if trace_active && trace_count >= max_trace {
            eprintln!("[{}] Trace limit reached", count);
            break;
        }

        match step_result {
            StepResult::Ok | StepResult::Ecall | StepResult::Ebreak => {}
            StepResult::FetchFault | StepResult::LoadFault | StepResult::StoreFault => {
                eprintln!("[{}] FAULT at PC=0x{:08X}", count, vm.cpu.pc);
                break;
            }
        }
    }

    let elapsed = start.elapsed();
    eprintln!("Done: {} instructions in {:?}", max_instr, elapsed);

    // Check output
    if !vm.bus.sbi.console_output.is_empty() {
        let s = String::from_utf8_lossy(&vm.bus.sbi.console_output);
        println!("SBI: {}", s);
    }
    let tx = vm.bus.uart.drain_tx();
    if !tx.is_empty() {
        let s = String::from_utf8_lossy(&tx);
        println!("UART: {}", s);
    }
}
