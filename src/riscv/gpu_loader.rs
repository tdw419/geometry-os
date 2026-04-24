// riscv/gpu_loader.rs -- RISC-V Cartridge Builders
//
// Cartridges are flat u32 arrays of RISC-V machine code.
// Each tile loads the cartridge into its RAM region at offset 0.
//
// These builders construct test programs that exercise the GPU shader:
// - fibonacci: computes fib(10)=55, writes result to UART
// - counter: counts 0..N, writes each value to UART

/// Build a fibonacci RISC-V cartridge
/// Computes fib(10) = 55 and writes result + ASCII to UART
pub fn build_fibonacci_cartridge() -> Vec<u32> {
    let mut code = Vec::new();
    // 0: ADDI a0, x0, 10    (n = 10)
    code.push(0x00500513);
    // 4: ADDI a1, x0, 0     (fib_prev = 0)
    code.push(0x00000593);
    // 8: ADDI a2, x0, 1     (fib_curr = 1)
    code.push(0x00100613);
    // 12: ADDI a3, x0, 0    (counter = 0)
    code.push(0x00000693);
    // 16: BEQ a3, a0, done  (if counter == n, done) offset = +24 bytes (6 instructions)
    code.push(encode_btype(0, 13, 10, 24));
    // 20: ADD a4, a2, a1    (temp = fib_curr + fib_prev)
    code.push(0x00b60733);
    // 24: ADDI a1, a2, 0    (fib_prev = old fib_curr)
    code.push(0x00060593);
    // 28: ADDI a2, a4, 0    (fib_curr = temp)
    code.push(0x00070613);
    // 32: ADDI a3, a3, 1    (counter++)
    code.push(0x00168693);
    // 36: JAL x0, -20       (back to BEQ at addr 16)
    code.push(0xfedff06f);
    // 40: LUI a5, 0x10000   (UART base)
    code.push(0x100007b7);
    // 44: SW a2, 0(a5)      (write fib result to UART)
    code.push(0x00c7a023);
    // 48: ADDI a5, x0, 0x35 (= '5' ASCII for fib(10)=55)
    code.push(0x03500793);
    // 52: LUI a7, 0x10000
    code.push(0x100008b7);
    // 56: SW a5, 0(a7)      (write '5')
    code.push(0x00f8a023);
    // 58: SW a5, 0(a7)      (write '5' again)
    code.push(0x00f8a023);
    // 60: ECALL (halt)
    code.push(0x00000073);
    code
}

/// Build a counter cartridge - counts 0..N, writes each value to UART
pub fn build_counter_cartridge(n: u32) -> Vec<u32> {
    let mut code = Vec::new();
    // 0: ADDI a0, x0, N     (limit)
    code.push(0x00000513 | ((n & 0xFFF) << 20));
    // 4: ADDI a1, x0, 0     (counter = 0)
    code.push(0x00000593);
    // 8: LUI a2, 0x10000    (UART base)
    code.push(0x10000637);
    // 12: SW a1, 0(a2)      (write counter to UART)  <- loop
    code.push(0x00b62023);
    // 16: ADDI a1, a1, 1    (counter++)
    code.push(0x00158593);
    // 20: BEQ a1, a0, done  (if counter == limit, done) offset = +8 bytes
    code.push(encode_btype(0, 11, 10, 8));
    // 24: JAL x0, -16       (back to loop at 12)
    code.push(0xff1ff06f);
    // 28: ECALL (halt)
    code.push(0x00000073);
    code
}

/// Build an ADDI-only cartridge for minimal testing
/// Just loads an immediate into a register and halts.
pub fn build_addi_cartridge(reg: u32, imm: u32) -> Vec<u32> {
    let mut code = Vec::new();
    // ADDI reg, x0, imm
    code.push(0x00000013 | ((reg & 0x1F) << 7) | ((imm & 0xFFF) << 20));
    // ECALL (halt)
    code.push(0x00000073);
    code
}

/// Encode a B-type (branch) instruction
fn encode_btype(funct3: u32, rs1: u32, rs2: u32, offset_bytes: i32) -> u32 {
    let imm = offset_bytes as u32;
    let imm12 = (imm >> 12) & 0x1;
    let imm10_5 = (imm >> 5) & 0x3F;
    let imm4_1 = (imm >> 1) & 0xF;
    let imm11 = (imm >> 11) & 0x1;

    0x63 | (funct3 << 12)
        | (rs1 << 15)
        | (rs2 << 20)
        | (imm4_1 << 8)
        | (imm10_5 << 25)
        | (imm11 << 7)
        | (imm12 << 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fibonacci_cartridge_structure() {
        let cart = build_fibonacci_cartridge();
        assert!(
            cart.len() > 10,
            "Fibonacci cartridge should have multiple instructions"
        );
        assert_eq!(
            cart[0] & 0x7F,
            0x13,
            "First instruction should be ADDI (opcode 0x13)"
        );
        assert_eq!(
            *cart.last().unwrap(),
            0x00000073,
            "Last instruction should be ECALL"
        );
    }

    #[test]
    fn test_counter_cartridge_structure() {
        let cart = build_counter_cartridge(7);
        assert!(
            cart.len() > 5,
            "Counter cartridge should have multiple instructions"
        );
        assert_eq!(
            *cart.last().unwrap(),
            0x00000073,
            "Last instruction should be ECALL"
        );
    }

    #[test]
    fn test_addi_cartridge() {
        let cart = build_addi_cartridge(5, 42);
        assert_eq!(cart.len(), 2);
        assert_eq!(cart[0] & 0x7F, 0x13, "Should be ADDI");
        assert_eq!((cart[0] >> 7) & 0x1F, 5, "Destination should be reg 5");
        assert_eq!((cart[0] >> 20) & 0xFFF, 42, "Immediate should be 42");
    }

    #[test]
    fn test_encode_btype_beq() {
        // BEQ x0, x0, +8 (always-taken branch forward 2 instructions)
        let encoded = encode_btype(0, 0, 0, 8);
        // BEQ with funct3=0, offset=8
        // imm[4:1]=0100, imm[10:5]=0, imm[11]=0, imm[12]=0
        assert_eq!(encoded & 0x7F, 0x63, "Should be BRANCH opcode");
        assert_eq!((encoded >> 12) & 0x7, 0, "funct3 should be 0 (BEQ)");
    }

    #[test]
    fn test_cartridges_fit_in_ram() {
        use super::super::gpu::RAM_WORDS;
        let fib = build_fibonacci_cartridge();
        assert!(
            fib.len() <= RAM_WORDS,
            "Fibonacci cartridge must fit in RAM"
        );

        for n in [1, 5, 10, 50, 100] {
            let cnt = build_counter_cartridge(n);
            assert!(
                cnt.len() <= RAM_WORDS,
                "Counter({}) cartridge must fit in RAM",
                n
            );
        }
    }
}
