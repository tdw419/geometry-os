// Diagnostic: boot Linux with step-by-step trap logging
// cargo run --example boot_trap_log

use std::fs;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";

    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();

    println!("=== Linux Boot Trap Log ===\n");

    // Call boot_linux with 100K insns to get the VM state
    let bootargs = "console=ttyS0 earlycon=sbi panic=1 quiet";

    // We need manual control. Let's create a minimal manual boot
    // and log traps.
    use geometry_os::riscv::cpu;
    use geometry_os::riscv::csr;

    let result = geometry_os::riscv::RiscvVm::boot_linux(
        &kernel_image,
        initramfs.as_deref(),
        256,
        100_000,
        bootargs,
    );

    match result {
        Ok((mut vm, r)) => {
            println!("Instructions: {}", r.instructions);
            println!("Final PC: 0x{:08X}", vm.cpu.pc);
            println!("Final privilege: {:?}", vm.cpu.privilege);
            println!("mcause: 0x{:08X}", vm.cpu.csr.mcause);
            println!("mepc:   0x{:08X}", vm.cpu.csr.mepc);
            println!("mtval:  0x{:08X}", vm.cpu.csr.mtval);

            // Read the instruction at mepc
            let mepc = vm.cpu.csr.mepc;
            if let Ok(word) = vm.bus.read_word(mepc as u64) {
                println!("\nInstruction at mepc 0x{:08X}: 0x{:08X}", mepc, word);
                // Check nearby
                for off in (-16i32..=16).step_by(4) {
                    let addr = (mepc as i64 + off as i64) as u64;
                    if let Ok(w) = vm.bus.read_word(addr) {
                        let marker = if off == 0 { " <-- mepc" } else { "" };
                        println!("  0x{:08X}: 0x{:08X}{}", addr as u32, w, marker);
                    }
                }
            }

            // Read instruction at current PC
            let pc = vm.cpu.pc;
            if let Ok(word) = vm.bus.read_word(pc as u64) {
                println!("\nInstruction at PC 0x{:08X}: 0x{:08X}", pc, word);
                for off in (-16i32..=16).step_by(4) {
                    let addr = (pc as i64 + off as i64) as u64;
                    if let Ok(w) = vm.bus.read_word(addr) {
                        let marker = if off == 0 { " <-- PC" } else { "" };
                        println!("  0x{:08X}: 0x{:08X}{}", addr as u32, w, marker);
                    }
                }
            }

            // Check what address 0xFF3FD1D7 is (mtval from fault)
            println!("\nmtval (faulting address): 0x{:08X}", vm.cpu.csr.mtval);
            // Is this a device address?
            println!("  Bus RAM base: 0x{:08X}", vm.bus.mem.ram_base);

            // What's the satp telling us?
            let satp = vm.cpu.csr.satp;
            let mode = satp & 0x1;
            let asid = (satp >> 22) & 0x1FF;
            let ppn = (satp >> 1) & 0x3FFFFF;
            println!("\nsatp: mode={} asid={} ppn=0x{:08X} (pt_base=0x{:08X})",
                mode, asid, ppn, ppn << 12);

            // Try to translate mtval through the page tables manually
            let va = vm.cpu.csr.mtval;
            let vpn1 = (va >> 22) & 0x3FF;
            let vpn0 = (va >> 12) & 0x3FF;
            let offset = va & 0xFFF;
            println!("\nManual SV32 translation of mtval 0x{:08X}:", va);
            println!("  VPN[1]=0x{:03X} VPN[0]=0x{:03X} offset=0x{:03X}", vpn1, vpn0, offset);

            let pt_base = (ppn as u64) << 12;
            println!("  Page table base: 0x{:08X}", pt_base as u32);

            // L1 entry
            let l1_addr = pt_base + (vpn1 as u64) * 4;
            if let Ok(l1_entry) = vm.bus.read_word(l1_addr) {
                println!("  L1 PTE at 0x{:08X}: 0x{:08X}", l1_addr as u32, l1_entry);
                let l1_valid = (l1_entry & 1) != 0;
                let l1_r = (l1_entry >> 1) & 1;
                let l1_w = (l1_entry >> 2) & 1;
                let l1_x = (l1_entry >> 3) & 1;
                println!("    V={} R={} W={} X={}", l1_valid, l1_r, l1_w, l1_x);
                if l1_valid && l1_r == 0 && l1_w == 0 && l1_x == 0 {
                    // Pointer to next level
                    let l1_ppn = (l1_entry >> 10) & 0x3FFFFF;
                    let l2_base = (l1_ppn as u64) << 12;
                    let l2_addr = l2_base + (vpn0 as u64) * 4;
                    if let Ok(l2_entry) = vm.bus.read_word(l2_addr) {
                        println!("  L2 PTE at 0x{:08X}: 0x{:08X}", l2_addr as u32, l2_entry);
                        let l2_valid = (l2_entry & 1) != 0;
                        let l2_ppn = (l2_entry >> 10) & 0x3FFFFF;
                        println!("    V={} PPN=0x{:08X} (PA=0x{:08X})",
                            l2_valid, l2_ppn, (l2_ppn << 12) | offset);
                    }
                } else if l1_valid {
                    // Megapage
                    let l1_ppn = (l1_entry >> 10) & 0x3FFFFF;
                    println!("    Megapage! PPN=0x{:08X} (PA=0x{:08X})",
                        l1_ppn, ((l1_ppn << 12) | ((vpn0 as u32) << 12)) | offset);
                }
            }

            // UART
            let mut bridge = geometry_os::riscv::bridge::UartBridge::new();
            let mut canvas = vec![0u32; 128 * 80];
            bridge.drain_uart_to_canvas(&mut vm.bus, &mut canvas);
            let text: String = canvas.iter()
                .filter(|&&c| c >= 32 && c < 127)
                .map(|&c| char::from_u32(c).unwrap_or('?'))
                .collect();
            if !text.is_empty() {
                println!("\nUART Output: {}", &text[..text.len().min(500)]);
            } else {
                println!("\nNo UART output.");
            }
        }
        Err(e) => {
            eprintln!("Boot failed: {}", e);
        }
    }
}
