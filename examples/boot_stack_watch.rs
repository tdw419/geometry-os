// Trace the first call to create_pgd_mapping in detail
// Watch sp and ra at every instruction to understand the tail call behavior
use std::fs;
use geometry_os::riscv::RiscvVm;
use geometry_os::riscv::cpu::StepResult;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel = fs::read(kernel_path).unwrap();
    let initramfs = fs::read(initramfs_path).ok();

    let bootargs = "console=ttyS0 earlycon=sbi panic=5 quiet";
    let (mut vm, result) = RiscvVm::boot_linux(
        &kernel,
        initramfs.as_deref(),
        512,
        0,
        bootargs,
    ).unwrap();

    let max_instr = 254_920u64; // Just past first create_pgd_mapping
    let mut count = 0u64;

    let create_pgd: u32 = 0xC0C04DA4;
    let mut in_create_pgd = false;
    let mut entry_count = 0u64;
    let mut logged = false;

    while count < max_instr {
        let pc = vm.cpu.pc;
        let sp = vm.cpu.x[2];
        let ra = vm.cpu.x[1];
        let a5 = vm.cpu.x[15];
        let instr = vm.bus.mem.read_word(pc as u64).unwrap_or(0);
        let opcode = instr & 0x7F;

        // Detect entry
        if opcode == 0x67 && !in_create_pgd {
            let rd = (instr >> 7) & 0x1F;
            let rs1 = (instr >> 15) & 0x1F;
            let imm = ((instr as i32) >> 20) as i32 as u64 as u32;
            let rs1_val = vm.cpu.x[rs1 as usize];
            let target = (rs1_val.wrapping_add(imm)) & !1u32;
            if target == create_pgd && rd == 1 && entry_count == 0 {
                in_create_pgd = true;
                entry_count = count;
                println!(">>> ENTER create_pgd_mapping at step {}, sp=0x{:08X}, ra=0x{:08X}", count, sp, ra);
            }
        }

        if in_create_pgd {
            // Log every instruction in the first call
            let compressed = (instr & 0x3) != 0x3;
            let mark = if pc == 0xC0C04DF6 { " <--- C.JR a5 (tail call to pt_ops)" }
                       else if pc == 0xC0C04E0A { " <--- C.JR a5 (tail call 2)" }
                       else if pc == 0xC0C04E24 { " <--- C.JR a5 (tail call 3)" }
                       else if pc == 0xC0C04E60 { " <--- ret" }
                       else { "" };

            // Only log interesting instructions
            if mark != "" || pc <= create_pgd + 20 || pc >= 0xC0C04E60 - 10 {
                println!("[{}] 0x{:08X}: 0x{:08X}{}  sp=0x{:08X} ra=0x{:08X} a5=0x{:08X}",
                         count, pc, instr, mark, sp, ra, a5);
            }

            // Check if we've returned past create_pgd_mapping
            if pc < create_pgd || pc > 0xC0C04F00 {
                if pc != a5 && count > entry_count + 10 {
                    println!(">>> LEFT create_pgd_mapping at step {}, PC=0x{:08X}", count, pc);
                    println!("    sp=0x{:08X} ra=0x{:08X}", sp, ra);
                    in_create_pgd = false;
                    break;
                }
            }
        }

        let step_result = vm.step();
        count += 1;

        if matches!(step_result, StepResult::FetchFault) {
            println!("\nFetchFault at step {}", count);
            break;
        }
    }

    println!("\n=== Stopped at step {}, PC=0x{:08X} ===", count, vm.cpu.pc);
}
