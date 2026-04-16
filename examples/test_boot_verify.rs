use std::fs;
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).unwrap();
    let initramfs = fs::read(initramfs_path).ok();
    
    let (mut vm, _fw_addr, _entry, _dtb_addr) = RiscvVm::boot_linux_setup(
        &kernel_image, initramfs.as_deref(), 256,
        "console=ttyS0 earlycon=sbi panic=1 quiet",
    ).unwrap();
    
    // Read back patches from physical addresses (bypassing MMU)
    let pa_10da = vm.bus.read_word(0x10DA).unwrap_or(0);
    let pa_10de = vm.bus.read_word(0x10DE).unwrap_or(0);
    let pa_10f6 = vm.bus.read_word(0x10F6).unwrap_or(0);
    let pa_10fa = vm.bus.read_word(0x10FA).unwrap_or(0);
    println!("=== Patches at physical addresses (bus reads) ===");
    println!("PA 0x10DA: 0x{:08X} (expect 0x00210197 auipc a3,0x210)", pa_10da);
    println!("PA 0x10DE: 0x{:08X} (expect 0xE3A68693 addi a3,a3,-454)", pa_10de);
    println!("PA 0x10F6: 0x{:08X} (expect 0x00000013 nop)", pa_10f6);
    println!("PA 0x10FA: 0x{:08X} (expect 0x00000013 nop)", pa_10fa);
    
    // Also read from memory directly
    println!("\n=== Patches in RAM (mem reads) ===");
    let mem_10da = vm.bus.mem.read_word(0x10DA).unwrap_or(0);
    let mem_10de = vm.bus.mem.read_word(0x10DE).unwrap_or(0);
    println!("RAM 0x10DA: 0x{:08X}", mem_10da);
    println!("RAM 0x10DE: 0x{:08X}", mem_10de);
    
    // Now run a few instructions and check what the CPU actually fetches
    // Step until we reach the auipc at 0xC00010DA
    let max = 200_000;
    let mut count = 0u64;
    let mut last_pc = 0u32;
    let mut spin_count = 0u32;
    
    while count < max {
        let pc = vm.cpu.pc;
        if pc == 0xC00010DA {
            // About to execute our patched auipc. Read the instruction.
            println!("\n=== Reached auipc at PC=0xC00010DA (count={}) ===", count);
            
            // Read instruction via MMU (VA) and directly (PA)
            let instr_va = vm.bus.read_word(pc as u64).unwrap_or(0);
            let instr_pa = vm.bus.mem.read_word(0x10DA).unwrap_or(0);
            println!("Instruction via VA: 0x{:08X}", instr_va);
            println!("Instruction via PA: 0x{:08X}", instr_pa);
            println!("a3 before auipc: 0x{:08X}", vm.cpu.x[13]);
            
            // Execute the auipc
            let _ = vm.step();
            count += 1;
            println!("a3 after auipc: 0x{:08X} (expect 0xC02110DA)", vm.cpu.x[13]);
            println!("PC now: 0x{:08X}", vm.cpu.pc);
            
            // Execute the addi
            let instr2 = vm.bus.read_word(vm.cpu.pc as u64).unwrap_or(0);
            println!("\nNext instruction at 0x{:08X}: 0x{:08X} (expect 0xE3A68693)", vm.cpu.pc, instr2);
            let _ = vm.step();
            count += 1;
            println!("a3 after addi: 0x{:08X} (expect 0xC0210F14)", vm.cpu.x[13]);
            
            // Execute csrw stvec
            let _ = vm.step();
            count += 1;
            println!("STVEC after csrw: 0x{:08X} (expect 0xC0210F14)", vm.cpu.csr.read(0x005));
            break;
        }
        
        let _ = vm.step();
        count += 1;
        
        if vm.cpu.pc == last_pc {
            spin_count += 1;
            if spin_count > 50000 {
                println!("SPIN at 0x{:08X} before reaching auipc", vm.cpu.pc);
                break;
            }
        } else {
            spin_count = 0;
        }
        last_pc = vm.cpu.pc;
    }
}
