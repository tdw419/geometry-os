// Trace boot: record every illegal instruction and the first N traps.
// cargo run --example boot_linux_trace

use geometry_os::riscv::cpu::StepResult;
use geometry_os::riscv::RiscvVm;
use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();

    let max_instr: u64 = 500_000;
    let bootargs = "console=ttyS0 earlycon=sbi panic=1 quiet";

    // We can't access the boot_linux step loop directly,
    // so let's look at the kernel binary for F-extension instructions.
    // Search for any 32-bit instruction with opcode 0x53 (OP-FP) or F-extension compressed.

    // The kernel is an ELF. Let's scan the LOAD segments for F-extension opcodes.
    println!("=== Scanning kernel for F-extension instructions ===\n");

    // Parse ELF to find LOAD segments
    if kernel_image.len() < 52 {
        println!("Kernel too small");
        return;
    }

    // Check ELF magic
    if &kernel_image[0..4] != b"\x7fELF" {
        println!("Not an ELF file");
        return;
    }

    let e_phoff = u32::from_le_bytes(kernel_image[28..32].try_into().unwrap()) as usize;
    let e_phentsize = u16::from_le_bytes(kernel_image[42..44].try_into().unwrap()) as usize;
    let e_phnum = u16::from_le_bytes(kernel_image[44..46].try_into().unwrap()) as usize;

    let mut f_ext_count = 0;
    let mut f_ext_examples: Vec<(u32, u32)> = Vec::new(); // (vaddr, raw)
    let mut total_instr_words = 0u64;

    for i in 0..e_phnum {
        let off = e_phoff + i * e_phentsize;
        if off + e_phentsize > kernel_image.len() {
            break;
        }

        let p_type = u32::from_le_bytes(kernel_image[off..off + 4].try_into().unwrap());
        if p_type != 1 {
            continue;
        } // PT_LOAD

        let p_offset =
            u32::from_le_bytes(kernel_image[off + 4..off + 8].try_into().unwrap()) as usize;
        let p_filesz =
            u32::from_le_bytes(kernel_image[off + 16..off + 20].try_into().unwrap()) as usize;
        let p_vaddr = u32::from_le_bytes(kernel_image[off + 8..off + 12].try_into().unwrap());

        if p_filesz == 0 {
            continue;
        }

        // Scan for F-extension instructions in this segment
        let data = &kernel_image[p_offset..p_offset + p_filesz.min(kernel_image.len() - p_offset)];

        let mut j = 0;
        while j + 4 <= data.len() {
            let word = u32::from_le_bytes(data[j..j + 4].try_into().unwrap());
            let opcode = word & 0x7F;
            let vaddr = p_vaddr + j as u32;

            // Skip if this looks like a compressed instruction in the low halfword
            let low_half = (word & 0xFFFF) as u16;
            if low_half & 0x3 != 0x3 {
                // Compressed instruction, skip 2 bytes
                total_instr_words += 1;
                j += 2;
                continue;
            }

            total_instr_words += 1;

            // Check for F-extension opcodes
            // 0x07 = LOAD-FP (FLW, FLD)
            // 0x27 = STORE-FP (FSW, FSD)
            // 0x43 = MADD (FMADD.S)
            // 0x47 = MSUB (FMSUB.S)
            // 0x4B = NMSUB (FNMSUB.S)
            // 0x4F = NMADD (FNMADD.S)
            // 0x53 = OP-FP (FADD.S, FSUB.S, FMUL.S, FDIV.S, etc.)
            if opcode == 0x07
                || opcode == 0x27
                || opcode == 0x43
                || opcode == 0x47
                || opcode == 0x4B
                || opcode == 0x4F
                || opcode == 0x53
            {
                f_ext_count += 1;
                if f_ext_examples.len() < 20 {
                    f_ext_examples.push((vaddr, word));
                }
            }

            j += 4;
        }
    }

    println!(
        "Total instruction-sized words scanned: {}",
        total_instr_words
    );
    println!("F-extension instructions found: {}", f_ext_count);

    if f_ext_count > 0 {
        println!("\nF-extension instruction examples (first 20):");
        for (vaddr, word) in &f_ext_examples {
            let opcode = word & 0x7F;
            let opnames = [
                (0x07, "LOAD-FP"),
                (0x27, "STORE-FP"),
                (0x43, "FMADD"),
                (0x47, "FMSUB"),
                (0x4B, "FNMSUB"),
                (0x4F, "FNMADD"),
                (0x53, "OP-FP"),
            ];
            let name = opnames
                .iter()
                .find(|(op, _)| *op == opcode)
                .map(|(_, n)| *n)
                .unwrap_or("?");
            println!("  0x{:08X}: 0x{:08X} ({})", vaddr, word, name);
        }

        println!("\n*** THE KERNEL CONTAINS F-EXTENSION INSTRUCTIONS! ***");
        println!(
            "Our interpreter is RV32IMAC (no FPU). These will cause Illegal Instruction traps."
        );
        println!("The kernel was likely built with a config that includes FP instructions.");
        println!("Fix: Rebuild the kernel with CONFIG_FPU disabled, or add FPU emulation.");
    } else {
        println!("\nNo F-extension instructions found in kernel.");
        println!("The illegal instruction must be something else.");
    }

    // Also check for any instructions with opcode 0x32 that aren't compressed
    println!("\n=== Checking for non-standard opcodes ===\n");
    let mut unknown_opcodes = std::collections::HashMap::new();
    // Re-scan
    for i in 0..e_phnum {
        let off = e_phoff + i * e_phentsize;
        if off + e_phentsize > kernel_image.len() {
            break;
        }
        let p_type = u32::from_le_bytes(kernel_image[off..off + 4].try_into().unwrap());
        if p_type != 1 {
            continue;
        }
        let p_offset =
            u32::from_le_bytes(kernel_image[off + 4..off + 8].try_into().unwrap()) as usize;
        let p_filesz =
            u32::from_le_bytes(kernel_image[off + 16..off + 20].try_into().unwrap()) as usize;
        let p_vaddr = u32::from_le_bytes(kernel_image[off + 8..off + 12].try_into().unwrap());
        if p_filesz == 0 {
            continue;
        }
        let data = &kernel_image[p_offset..p_offset + p_filesz.min(kernel_image.len() - p_offset)];

        let mut j = 0;
        while j + 4 <= data.len() {
            let word = u32::from_le_bytes(data[j..j + 4].try_into().unwrap());
            let low_half = (word & 0xFFFF) as u16;
            if low_half & 0x3 != 0x3 {
                j += 2;
                continue;
            }
            let opcode = word & 0x7F;
            // Valid opcodes for RV32IMAC:
            let valid = matches!(
                opcode,
                0x37 | 0x17 | 0x6F | 0x67 | 0x63 | 0x03 | 0x23 | 0x13 | 0x33 | 0x73 | 0x0F | 0x2F
            );
            if !valid {
                *unknown_opcodes.entry(opcode).or_insert(0u32) += 1;
            }
            j += 4;
        }
    }

    if unknown_opcodes.is_empty() {
        println!("All 32-bit instructions have valid RV32IMAC opcodes.");
    } else {
        println!("Non-RV32IMAC opcodes found:");
        let mut opcodes: Vec<_> = unknown_opcodes.iter().collect();
        opcodes.sort_by_key(|(_, c)| std::cmp::Reverse(**c));
        for (op, count) in opcodes {
            println!("  opcode 0x{:02X}: {} occurrences", op, count);
        }
    }
}
