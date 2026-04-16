/// Trace the last instructions before PC drops below 0xC0000000 (leaves kernel VA space).
/// Dump register state at the transition point.

use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = std::fs::read(kernel_path).expect("kernel");
    let initramfs = std::fs::read(initramfs_path).ok();
    let bootargs = "console=ttyS0 earlycon=sbi panic=1";

    let (mut vm, fw_addr, _entry, _dtb) =
        RiscvVm::boot_linux_setup(&kernel_image, initramfs.as_deref(), 256, bootargs)
            .expect("boot setup failed");

    let fw_addr_u32 = fw_addr as u32;
    let mut count: u64 = 0;
    let max_instructions: u64 = 300_000;
    let mut last_satp: u32 = vm.cpu.csr.satp;
    let mut transition_found = false;

    // Ring buffer: store (count, pc, instruction_word)
    let ring_size = 300;
    let mut ring: Vec<(u64, u32, u32)> = Vec::with_capacity(ring_size);

    while count < max_instructions {
        if vm.bus.sbi.shutdown_requested {
            eprintln!("[diag] SBI shutdown at count={}", count);
            break;
        }

        // Track SATP changes
        let cur_satp = vm.cpu.csr.satp;
        if cur_satp != last_satp {
            eprintln!("[{}] SATP: 0x{:08X} -> 0x{:08X} (PC=0x{:08X})",
                count, last_satp, cur_satp, vm.cpu.pc);
            last_satp = cur_satp;
        }

        let pc = vm.cpu.pc;

        // Record in ring buffer
        let instr_word = vm.bus.read_word(pc as u64).unwrap_or(0);
        ring.push((count, pc, instr_word));
        if ring.len() > ring_size {
            ring.remove(0);
        }

        // Check for transition out of kernel VA space
        if pc >= 0xC0000000 && count > 1000 {
            let next_count = count + 1;
            vm.step();
            let next_pc = vm.cpu.pc;

            if next_pc < 0xC0000000 && next_pc != fw_addr_u32 && !transition_found {
                transition_found = true;
                eprintln!("\n=== TRANSITION at count={} ===", next_count);
                eprintln!("PC went from 0x{:08X} to 0x{:08X}", pc, next_pc);
                eprintln!("\nRegister state:");
                for i in (0..32).step_by(4) {
                    eprintln!("  x{}={:08X} x{}={:08X} x{}={:08X} x{}={:08X}",
                        i, vm.cpu.x[i],
                        i+1, vm.cpu.x[i+1],
                        i+2, vm.cpu.x[i+2],
                        i+3, vm.cpu.x[i+3]);
                }
                eprintln!("  mepc=0x{:08X} mcause=0x{:08X} sepc=0x{:08X} scause=0x{:08X}",
                    vm.cpu.csr.mepc, vm.cpu.csr.mcause, vm.cpu.csr.sepc, vm.cpu.csr.scause);
                eprintln!("  satp=0x{:08X} stvec=0x{:08X} mtvec=0x{:08X}",
                    vm.cpu.csr.satp, vm.cpu.csr.stvec, vm.cpu.csr.mtvec);

                eprintln!("\nLast 50 instructions before transition:");
                let start = ring.len().saturating_sub(50);
                for &(c, p, w) in &ring[start..] {
                    eprintln!("  [{}] 0x{:08X}: {:08X}", c, p, w);
                }

                // Continue a bit more to see what happens
                eprintln!("\nNext 30 instructions after transition:");
                for _ in 0..30 {
                    let npc = vm.cpu.pc;
                    let nw = vm.bus.read_word(npc as u64).unwrap_or(0);
                    eprintln!("  [{}] 0x{:08X}: {:08X}", count, npc, nw);
                    count += 1;
                    vm.step();
                    if vm.bus.sbi.shutdown_requested { break; }
                }
                break;
            }
        } else {
            vm.step();
        }

        count += 1;
    }

    if !transition_found {
        eprintln!("No transition found in {} instructions. Final PC=0x{:08X}", count, vm.cpu.pc);
    }
}
