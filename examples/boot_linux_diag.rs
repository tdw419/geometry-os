// Diagnostic: trace last N instructions before boot ends.
// cargo run --example boot_linux_diag <instruction_count>

use std::fs;
use geometry_os::riscv::RiscvVm;

fn main() {
    let kernel_path = ".geometry_os/build/linux-6.14/vmlinux";
    let initramfs_path = ".geometry_os/fs/linux/rv32/initramfs.cpio.gz";
    let kernel_image = fs::read(kernel_path).expect("kernel");
    let initramfs = fs::read(initramfs_path).ok();

    let max_instr: u64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(500_000);
    let bootargs = "console=ttyS0 earlycon=sbi panic=1 quiet";

    let (mut vm, r) = RiscvVm::boot_linux(&kernel_image, initramfs.as_deref(), 256, max_instr, bootargs).unwrap();
    
    println!("=== Post-boot diagnostics ===");
    println!("PC: 0x{:08X}  Privilege: {:?}", vm.cpu.pc, vm.cpu.privilege);
    println!("mcause: 0x{:08X}  mepc: 0x{:08X}  mtval: 0x{:08X}", vm.cpu.csr.mcause, vm.cpu.csr.mepc, vm.cpu.csr.mtval);
    println!("scause: 0x{:08X}  sepc: 0x{:08X}  stval: 0x{:08X}", vm.cpu.csr.scause, vm.cpu.csr.sepc, vm.cpu.csr.stval);
    println!("mstatus: 0x{:08X}", vm.cpu.csr.mstatus);
    
    // Decode mcause
    let mcause = vm.cpu.csr.mcause;
    let is_interrupt = (mcause >> 31) & 1 == 1;
    let code = mcause & 0x7FFFFFFF;
    let cause_names = [
        "Instr Misaligned", "Instr Access Fault", "Illegal Instruction",
        "Breakpoint", "Load Misaligned", "Load Access Fault",
        "Store Misaligned", "Store Access Fault", "ECALL_U", "ECALL_S",
        "Reserved(10)", "ECALL_M", "Instr Page Fault", "Load Page Fault",
        "Reserved(14)", "Store Page Fault",
    ];
    let cause_name = if !is_interrupt && (code as usize) < cause_names.len() {
        cause_names[code as usize]
    } else {
        "Unknown"
    };
    println!("mcause decoded: {} #{} ({})", if is_interrupt { "Interrupt" } else { "Exception" }, code, cause_name);
    
    // Read the faulting instruction
    let fw_addr: u32 = 0xC000_0000 - 4096; // fw_addr from boot_linux
    println!("\nfw_addr (trap handler): 0x{:08X}", fw_addr);
    println!("At trap handler? {} (PC == fw_addr)", vm.cpu.pc == fw_addr as u32);
    
    // Read instruction at mepc
    let mepc = vm.cpu.csr.mepc;
    println!("\nFaulting instruction at mepc=0x{:08X}:", mepc);
    if let Ok(word) = vm.bus.read_word(mepc as u64) {
        println!("  raw: 0x{:08X}", word);
        // Basic RISC-V decode
        let opcode = word & 0x7F;
        let rd = (word >> 7) & 0x1F;
        let funct3 = (word >> 12) & 0x7;
        let rs1 = (word >> 15) & 0x1F;
        let rs2 = (word >> 20) & 0x1F;
        let funct7 = (word >> 25) & 0x7F;
        
        let opnames = ["ADD","SLL","SLT","SLTU","XOR","SRL","OR","AND"];
        match opcode {
            0x33 => println!("  R-type: {} {}, {}, {} (funct7={})", 
                if funct7 == 0x20 { "SUB" } else if funct3 < 8 { opnames[funct3 as usize] } else { "?" },
                rd, rs1, rs2, funct7),
            0x13 => {
                let imm = (word >> 20) as i32;
                let ops = ["ADDI","SLLI","SLTI","SLTIU","XORI","SRLI","ORI","ANDI"];
                println!("  I-type: {} {}, {}, {} (imm=0x{:X})", ops[funct3 as usize], rd, rs1, imm, imm);
            },
            0x03 => {
                let ops = ["LB","LH","LW","LBU","LHU","?","?","?"];
                let imm = (word >> 20) as i32;
                println!("  Load: {} {}, {}({}) (imm=0x{:X})", ops[funct3 as usize], rd, rs1, imm, imm);
            },
            0x23 => {
                let imm = (((word >> 25) as i32) << 5) | (((word >> 7) & 0x1F) as i32);
                let ops = ["SB","SH","SW","?","?","?","?","?"];
                println!("  Store: {} {}, {}({}) (imm=0x{:X})", ops[funct3 as usize], rs2, rs1, imm, imm);
            },
            0x73 => {
                let csr_addr = (word >> 20) & 0xFFF;
                let imm_csr = (word >> 15) & 0x1F;
                if funct3 == 0 {
                    if word == 0x00100073 { println!("  EBREAK"); }
                    else if word == 0x00000073 { println!("  ECALL"); }
                    else if word == 0x30200073 { println!("  MRET"); }
                    else if word == 0x10200073 { println!("  SRET"); }
                    else { println!("  SYSTEM funct12=0x{:03X}", (word >> 20) & 0xFFF); }
                } else {
                    let ops = ["","CSRRW","CSRRS","CSRRC","?","CSRRWI","CSRRSI","CSRRCI"];
                    println!("  CSR: {} {}, csr=0x{:03X}, src={}", ops[funct3 as usize], rd, csr_addr, rs1);
                }
            },
            0x6F => {
                // JAL
                let imm20 = (word >> 31) & 1;
                let imm10_1 = (word >> 21) & 0x3FF;
                let imm11 = (word >> 20) & 1;
                let imm19_12 = (word >> 12) & 0xFF;
                let imm = ((imm20 as i32) << 20) | ((imm19_12 as i32) << 12) | ((imm11 as i32) << 11) | ((imm10_1 as i32) << 1);
                println!("  JAL {}, offset=0x{:X} (target=0x{:X})", rd, imm, (mepc as i32).wrapping_add(imm) as u32);
            },
            0x63 => {
                let imm12 = (word >> 31) & 1;
                let imm10_5 = (word >> 25) & 0x3F;
                let imm4_1 = (word >> 8) & 0xF;
                let imm11 = (word >> 7) & 1;
                let imm = ((imm12 as i32) << 12) | ((imm11 as i32) << 11) | ((imm10_5 as i32) << 5) | ((imm4_1 as i32) << 1);
                let ops = ["BEQ","BNE","?","?","BLT","BGE","BLTU","BGEU"];
                println!("  {} {}, {}, offset=0x{:X} (target=0x{:X})", ops[funct3 as usize], rs1, rs2, imm, (mepc as i32).wrapping_add(imm) as u32);
            },
            0x37 => {
                let imm = word & 0xFFFFF000;
                println!("  LUI {}, 0x{:08X}", rd, imm);
            },
            0x17 => {
                let imm = word & 0xFFFFF000;
                println!("  AUIPC {}, 0x{:08X}", rd, imm);
            },
            _ => println!("  opcode=0x{:02X} (unknown or F-extension/compressed?)", opcode),
        }
    } else {
        println!("  (cannot read -- bus error)");
    }
    
    // Also read instruction at PC (in case it's compressed)
    println!("\nInstruction at PC=0x{:08X}:", vm.cpu.pc);
    if let Ok(half) = vm.bus.read_half(vm.cpu.pc as u64) {
        println!("  halfword: 0x{:04X}", half);
        if half & 0x3 != 0x3 {
            // Compressed instruction
            println!("  (compressed instruction)");
        }
    }
    if let Ok(word) = vm.bus.read_word(vm.cpu.pc as u64) {
        println!("  word: 0x{:08X}", word);
    }
    
    // Show some registers
    println!("\nRegisters:");
    for i in (0..32).step_by(4) {
        print!("  x{:02}=0x{:08X}", i, vm.cpu.x[i]);
        if i + 1 < 32 { print!("  x{:02}=0x{:08X}", i+1, vm.cpu.x[i+1]); }
        if i + 2 < 32 { print!("  x{:02}=0x{:08X}", i+2, vm.cpu.x[i+2]); }
        if i + 3 < 32 { print!("  x{:02}=0x{:08X}", i+3, vm.cpu.x[i+3]); }
        println!();
    }
    
    // Check if SBI shutdown was requested
    println!("\nSBI shutdown: {}", vm.bus.sbi.shutdown_requested);
    
    // Read the memory around mtval
    println!("\nMemory at mtval=0x{:08X}:", vm.cpu.csr.mtval);
    for off in (0..16).step_by(4) {
        let addr = vm.cpu.csr.mtval.wrapping_add(off);
        match vm.bus.read_word(addr as u64) {
            Ok(w) => println!("  0x{:08X}: 0x{:08X}", addr, w),
            Err(_) => println!("  0x{:08X}: (bus error)", addr),
        }
    }
}
