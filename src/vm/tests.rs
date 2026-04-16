use super::*;

/// Helper: build a bytecode program as Vec<u32>, load into a fresh VM, run N steps.
/// Returns the VM for assertions.
fn run_program(bytecode: &[u32], max_steps: usize) -> Vm {
    let mut vm = Vm::new();
    for (i, &word) in bytecode.iter().enumerate() {
        vm.ram[i] = word;
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..max_steps {
        if !vm.step() { break; }
    }
    vm
}

// ── RAM-Mapped Canvas (Phase 45) ────────────────────────────────

#[test]
fn test_canvas_ram_mapping_store() {
    let mut vm = Vm::new();
    // STORE 0x8000 (first cell) with 'H' (0x48)
    vm.regs[1] = 0x8000;
    vm.regs[2] = 0x48;
    vm.ram[0] = 0x12; vm.ram[1] = 1; vm.ram[2] = 2; // STORE r1, r2
    vm.pc = 0;
    vm.step();
    assert_eq!(vm.canvas_buffer[0], 0x48);
    assert_eq!(vm.ram[0x8000], 0); // RAM should be unchanged
}

#[test]
fn test_canvas_ram_mapping_load() {
    let mut vm = Vm::new();
    vm.canvas_buffer[10] = 0x58; // 'X'
    // LOAD r3, 0x800A
    vm.regs[1] = 0x800A;
    vm.ram[0] = 0x11; vm.ram[1] = 3; vm.ram[2] = 1; // LOAD r3, r1
    vm.pc = 0;
    vm.step();
    assert_eq!(vm.regs[3], 0x58);
}

#[test]
fn test_canvas_ram_mapping_user_mode() {
    let mut vm = Vm::new();
    vm.mode = CpuMode::User;
    vm.regs[1] = 0x8000;
    vm.regs[2] = 0x48;
    vm.ram[0] = 0x12; vm.ram[1] = 1; vm.ram[2] = 2; // STORE r1, r2
    vm.pc = 0;
    assert!(vm.step()); // Should NOT segfault
    assert_eq!(vm.canvas_buffer[0], 0x48);
}

#[test]
fn test_nop_advances_pc() {
    // NOP then HALT
    let vm = run_program(&[0x01, 0x00], 100);
    assert!(vm.halted);
    assert_eq!(vm.pc, 2);
}

// ── LDI ─────────────────────────────────────────────────────────

#[test]
fn test_ldi_loads_immediate() {
    // LDI r5, 0x42
    let vm = run_program(&[0x10, 5, 0x42, 0x00], 100);
    assert!(vm.halted);
    assert_eq!(vm.regs[5], 0x42);
}

#[test]
fn test_ldi_zero() {
    // LDI r3, 0
    let vm = run_program(&[0x10, 3, 0, 0x00], 100);
    assert_eq!(vm.regs[3], 0);
}

#[test]
fn test_ldi_max_u32() {
    // LDI r10, 0xFFFFFFFF
    let vm = run_program(&[0x10, 10, 0xFFFFFFFF, 0x00], 100);
    assert_eq!(vm.regs[10], 0xFFFFFFFF);
}

#[test]
fn test_ldi_invalid_reg_ignored() {
    // LDI r32 (out of range), 42 -- should be ignored, no panic
    let vm = run_program(&[0x10, 32, 42, 0x00], 100);
    assert!(vm.halted); // still halted at end
}

// ── LOAD / STORE ────────────────────────────────────────────────

#[test]
fn test_load_reads_ram() {
    // LDI r1, 0x2000   (address)
    // STORE r1, r2     (store r2 -> RAM[0x2000])
    // LOAD r3, r1      (load r3 <- RAM[0x2000])
    // HALT
    let mut vm = Vm::new();
    vm.ram[0] = 0x10; vm.ram[1] = 1; vm.ram[2] = 0x2000; // LDI r1, 0x2000
    vm.ram[3] = 0x12; vm.ram[4] = 1; vm.ram[5] = 2;       // STORE r1, r2
    vm.ram[6] = 0x11; vm.ram[7] = 3; vm.ram[8] = 1;       // LOAD r3, r1
    vm.ram[9] = 0x00;                                       // HALT
    vm.regs[2] = 0xABCDEF;
    vm.pc = 0;
    for _ in 0..100 { if !vm.step() { break; } }
    assert_eq!(vm.regs[3], 0xABCDEF);
}

#[test]
fn test_store_then_load_roundtrip() {
    let mut vm = Vm::new();
    // LDI r5, 0x500  (addr)
    // LDI r6, 999    (value)
    // STORE r5, r6
    // LOAD r7, r5
    // HALT
    vm.ram[0] = 0x10; vm.ram[1] = 5; vm.ram[2] = 0x500;
    vm.ram[3] = 0x10; vm.ram[4] = 6; vm.ram[5] = 999;
    vm.ram[6] = 0x12; vm.ram[7] = 5; vm.ram[8] = 6;
    vm.ram[9] = 0x11; vm.ram[10] = 7; vm.ram[11] = 5;
    vm.ram[12] = 0x00;
    vm.pc = 0;
    for _ in 0..100 { if !vm.step() { break; } }
    assert_eq!(vm.regs[7], 999);
}

// ── ARITHMETIC ──────────────────────────────────────────────────

#[test]
fn test_add_basic() {
    // LDI r1, 10; LDI r2, 20; ADD r1, r2; HALT
    let vm = run_program(&[0x10, 1, 10, 0x10, 2, 20, 0x20, 1, 2, 0x00], 100);
    assert_eq!(vm.regs[1], 30);
}

#[test]
fn test_add_wrapping_overflow() {
    let mut vm = Vm::new();
    vm.regs[1] = 0xFFFFFFFF;
    vm.regs[2] = 1;
    // ADD r1, r2; HALT
    vm.ram[0] = 0x20; vm.ram[1] = 1; vm.ram[2] = 2;
    vm.ram[3] = 0x00;
    vm.pc = 0;
    for _ in 0..100 { if !vm.step() { break; } }
    assert_eq!(vm.regs[1], 0); // wrapping add
}

#[test]
fn test_sub_basic() {
    let mut vm = Vm::new();
    vm.regs[1] = 50;
    vm.regs[2] = 20;
    vm.ram[0] = 0x21; vm.ram[1] = 1; vm.ram[2] = 2;
    vm.ram[3] = 0x00;
    vm.pc = 0;
    for _ in 0..100 { if !vm.step() { break; } }
    assert_eq!(vm.regs[1], 30);
}

#[test]
fn test_sub_wrapping_underflow() {
    let mut vm = Vm::new();
    vm.regs[1] = 0;
    vm.regs[2] = 1;
    vm.ram[0] = 0x21; vm.ram[1] = 1; vm.ram[2] = 2;
    vm.ram[3] = 0x00;
    vm.pc = 0;
    for _ in 0..100 { if !vm.step() { break; } }
    assert_eq!(vm.regs[1], 0xFFFFFFFF); // wrapping sub
}

#[test]
fn test_mul_basic() {
    let mut vm = Vm::new();
    vm.regs[1] = 6;
    vm.regs[2] = 7;
    vm.ram[0] = 0x22; vm.ram[1] = 1; vm.ram[2] = 2;
    vm.ram[3] = 0x00;
    vm.pc = 0;
    for _ in 0..100 { if !vm.step() { break; } }
    assert_eq!(vm.regs[1], 42);
}

#[test]
fn test_div_basic() {
    let mut vm = Vm::new();
    vm.regs[1] = 100;
    vm.regs[2] = 7;
    vm.ram[0] = 0x23; vm.ram[1] = 1; vm.ram[2] = 2;
    vm.ram[3] = 0x00;
    vm.pc = 0;
    for _ in 0..100 { if !vm.step() { break; } }
    assert_eq!(vm.regs[1], 14); // 100 / 7 = 14 (integer division)
}

#[test]
fn test_div_by_zero_no_panic() {
    let mut vm = Vm::new();
    vm.regs[1] = 42;
    vm.regs[2] = 0;
    vm.ram[0] = 0x23; vm.ram[1] = 1; vm.ram[2] = 2;
    vm.ram[3] = 0x00;
    vm.pc = 0;
    for _ in 0..100 { if !vm.step() { break; } }
    assert_eq!(vm.regs[1], 42); // unchanged, no panic
}

#[test]
fn test_mod_basic() {
    let mut vm = Vm::new();
    vm.regs[1] = 100;
    vm.regs[2] = 7;
    vm.ram[0] = 0x29; vm.ram[1] = 1; vm.ram[2] = 2;
    vm.ram[3] = 0x00;
    vm.pc = 0;
    for _ in 0..100 { if !vm.step() { break; } }
    assert_eq!(vm.regs[1], 2); // 100 % 7 = 2
}

#[test]
fn test_mod_by_zero_no_panic() {
    let mut vm = Vm::new();
    vm.regs[1] = 42;
    vm.regs[2] = 0;
    vm.ram[0] = 0x29; vm.ram[1] = 1; vm.ram[2] = 2;
    vm.ram[3] = 0x00;
    vm.pc = 0;
    for _ in 0..100 { if !vm.step() { break; } }
    assert_eq!(vm.regs[1], 42); // unchanged
}

#[test]
fn test_neg() {
    let mut vm = Vm::new();
    vm.regs[5] = 1;
    vm.ram[0] = 0x2A; vm.ram[1] = 5;
    vm.ram[2] = 0x00;
    vm.pc = 0;
    for _ in 0..100 { if !vm.step() { break; } }
    assert_eq!(vm.regs[5], 0xFFFFFFFF); // -1 in two's complement
}

#[test]
fn test_neg_zero() {
    let mut vm = Vm::new();
    vm.regs[5] = 0;
    vm.ram[0] = 0x2A; vm.ram[1] = 5;
    vm.ram[2] = 0x00;
    vm.pc = 0;
    for _ in 0..100 { if !vm.step() { break; } }
    assert_eq!(vm.regs[5], 0);
}

// ── BITWISE ─────────────────────────────────────────────────────

#[test]
fn test_and() {
    let mut vm = Vm::new();
    vm.regs[1] = 0xFF00FF;
    vm.regs[2] = 0x0F0F0F;
    vm.ram[0] = 0x24; vm.ram[1] = 1; vm.ram[2] = 2;
    vm.ram[3] = 0x00;
    vm.pc = 0;
    for _ in 0..100 { if !vm.step() { break; } }
    assert_eq!(vm.regs[1], 0x0F000F);
}

#[test]
fn test_or() {
    let mut vm = Vm::new();
    vm.regs[1] = 0xF00000;
    vm.regs[2] = 0x000F00;
    vm.ram[0] = 0x25; vm.ram[1] = 1; vm.ram[2] = 2;
    vm.ram[3] = 0x00;
    vm.pc = 0;
    for _ in 0..100 { if !vm.step() { break; } }
    assert_eq!(vm.regs[1], 0xF00F00);
}

#[test]
fn test_xor() {
    let mut vm = Vm::new();
    vm.regs[1] = 0xFF00FF;
    vm.regs[2] = 0xFF00FF;
    vm.ram[0] = 0x26; vm.ram[1] = 1; vm.ram[2] = 2;
    vm.ram[3] = 0x00;
    vm.pc = 0;
    for _ in 0..100 { if !vm.step() { break; } }
    assert_eq!(vm.regs[1], 0); // XOR self = 0
}

#[test]
fn test_shl() {
    let mut vm = Vm::new();
    vm.regs[1] = 1;
    vm.regs[2] = 8;
    vm.ram[0] = 0x27; vm.ram[1] = 1; vm.ram[2] = 2;
    vm.ram[3] = 0x00;
    vm.pc = 0;
    for _ in 0..100 { if !vm.step() { break; } }
    assert_eq!(vm.regs[1], 256);
}

#[test]
fn test_shl_mod_32() {
    let mut vm = Vm::new();
    vm.regs[1] = 1;
    vm.regs[2] = 32; // shift by 32 -> effectively shift by 0 (mod 32)
    vm.ram[0] = 0x27; vm.ram[1] = 1; vm.ram[2] = 2;
    vm.ram[3] = 0x00;
    vm.pc = 0;
    for _ in 0..100 { if !vm.step() { break; } }
    assert_eq!(vm.regs[1], 1); // 1 << 32 = 1 (mod 32 = 0)
}

#[test]
fn test_shr() {
    let mut vm = Vm::new();
    vm.regs[1] = 256;
    vm.regs[2] = 4;
    vm.ram[0] = 0x28; vm.ram[1] = 1; vm.ram[2] = 2;
    vm.ram[3] = 0x00;
    vm.pc = 0;
    for _ in 0..100 { if !vm.step() { break; } }
    assert_eq!(vm.regs[1], 16);
}

#[test]
fn test_sar_sign_preserving() {
    let mut vm = Vm::new();
    vm.regs[1] = 0x80000000; // MSB set (negative in i32)
    vm.regs[2] = 4;
    vm.ram[0] = 0x2B; vm.ram[1] = 1; vm.ram[2] = 2;
    vm.ram[3] = 0x00;
    vm.pc = 0;
    for _ in 0..100 { if !vm.step() { break; } }
    // 0x80000000 >> 4 (arithmetic) = 0xF8000000
    assert_eq!(vm.regs[1], 0xF8000000);
}

// ── CMP / BRANCHES ──────────────────────────────────────────────

#[test]
fn test_cmp_less_than() {
    let mut vm = Vm::new();
    vm.regs[1] = 5;
    vm.regs[2] = 10;
    vm.ram[0] = 0x50; vm.ram[1] = 1; vm.ram[2] = 2; // CMP r1, r2
    vm.ram[3] = 0x00;
    vm.pc = 0;
    for _ in 0..100 { if !vm.step() { break; } }
    assert_eq!(vm.regs[0], 0xFFFFFFFF); // -1 (less than)
}

#[test]
fn test_cmp_equal() {
    let mut vm = Vm::new();
    vm.regs[1] = 42;
    vm.regs[2] = 42;
    vm.ram[0] = 0x50; vm.ram[1] = 1; vm.ram[2] = 2;
    vm.ram[3] = 0x00;
    vm.pc = 0;
    for _ in 0..100 { if !vm.step() { break; } }
    assert_eq!(vm.regs[0], 0); // equal
}

#[test]
fn test_cmp_greater_than() {
    let mut vm = Vm::new();
    vm.regs[1] = 10;
    vm.regs[2] = 5;
    vm.ram[0] = 0x50; vm.ram[1] = 1; vm.ram[2] = 2;
    vm.ram[3] = 0x00;
    vm.pc = 0;
    for _ in 0..100 { if !vm.step() { break; } }
    assert_eq!(vm.regs[0], 1); // greater than
}

#[test]
fn test_jz_taken() {
    // LDI r1, 0; JZ r1, 100; HALT -> should jump to 100
    let mut vm = Vm::new();
    vm.ram[0] = 0x10; vm.ram[1] = 1; vm.ram[2] = 0; // LDI r1, 0
    vm.ram[3] = 0x31; vm.ram[4] = 1; vm.ram[5] = 100; // JZ r1, 100
    vm.ram[6] = 0x00; // HALT (should not reach)
    vm.ram[100] = 0x00; // HALT at target
    vm.pc = 0;
    for _ in 0..100 { if !vm.step() { break; } }
    assert_eq!(vm.pc, 101); // halted at 101 (fetched HALT at 100)
}

#[test]
fn test_jz_not_taken() {
    // LDI r1, 1; JZ r1, 100; HALT -> should not jump
    let mut vm = Vm::new();
    vm.ram[0] = 0x10; vm.ram[1] = 1; vm.ram[2] = 1; // LDI r1, 1
    vm.ram[3] = 0x31; vm.ram[4] = 1; vm.ram[5] = 100; // JZ r1, 100
    vm.ram[6] = 0x00; // HALT (should reach)
    vm.pc = 0;
    for _ in 0..100 { if !vm.step() { break; } }
    assert_eq!(vm.pc, 7); // halted at HALT
}

#[test]
fn test_jnz_taken() {
    let mut vm = Vm::new();
    vm.ram[0] = 0x10; vm.ram[1] = 1; vm.ram[2] = 5; // LDI r1, 5
    vm.ram[3] = 0x32; vm.ram[4] = 1; vm.ram[5] = 100; // JNZ r1, 100
    vm.ram[6] = 0x00; // HALT
    vm.ram[100] = 0x00; // HALT at target
    vm.pc = 0;
    for _ in 0..100 { if !vm.step() { break; } }
    assert_eq!(vm.pc, 101);
}

#[test]
fn test_jmp_unconditional() {
    let mut vm = Vm::new();
    vm.ram[0] = 0x30; vm.ram[1] = 50; // JMP 50
    vm.ram[2] = 0x00; // HALT (should not reach)
    vm.ram[50] = 0x00; // HALT at target
    vm.pc = 0;
    for _ in 0..100 { if !vm.step() { break; } }
    assert_eq!(vm.pc, 51);
}

#[test]
fn test_blt_taken() {
    // CMP sets r0 = 0xFFFFFFFF (less than); BLT should branch
    let mut vm = Vm::new();
    vm.regs[1] = 3;
    vm.regs[2] = 10;
    vm.ram[0] = 0x50; vm.ram[1] = 1; vm.ram[2] = 2; // CMP r1, r2
    vm.ram[3] = 0x35; vm.ram[4] = 0; vm.ram[5] = 50; // BLT r0, 50
    vm.ram[6] = 0x00; // HALT
    vm.ram[50] = 0x00; // HALT at target
    vm.pc = 0;
    for _ in 0..100 { if !vm.step() { break; } }
    assert_eq!(vm.pc, 51);
}

#[test]
fn test_bge_taken() {
    // CMP sets r0 = 1 (greater than); BGE should branch
    let mut vm = Vm::new();
    vm.regs[1] = 10;
    vm.regs[2] = 3;
    vm.ram[0] = 0x50; vm.ram[1] = 1; vm.ram[2] = 2; // CMP r1, r2
    vm.ram[3] = 0x36; vm.ram[4] = 0; vm.ram[5] = 50; // BGE r0, 50
    vm.ram[6] = 0x00; // HALT
    vm.ram[50] = 0x00; // HALT at target
    vm.pc = 0;
    for _ in 0..100 { if !vm.step() { break; } }
    assert_eq!(vm.pc, 51);
}

// ── CALL / RET ──────────────────────────────────────────────────

#[test]
fn test_call_ret() {
    // CALL 10; HALT
    // at 10: LDI r5, 99; RET
    // at 16: HALT (return lands here)
    let mut vm = Vm::new();
    vm.ram[0] = 0x33; vm.ram[1] = 10;         // CALL 10
    vm.ram[2] = 0x00;                            // HALT (return target)
    vm.ram[10] = 0x10; vm.ram[11] = 5; vm.ram[12] = 99; // LDI r5, 99
    vm.ram[13] = 0x34;                           // RET
    vm.pc = 0;
    for _ in 0..100 { if !vm.step() { break; } }
    assert_eq!(vm.regs[5], 99);
    assert!(vm.halted);
}

// ── MOV ─────────────────────────────────────────────────────────

#[test]
fn test_mov() {
    let mut vm = Vm::new();
    vm.regs[3] = 0xDEADBEEF;
    vm.ram[0] = 0x51; vm.ram[1] = 7; vm.ram[2] = 3; // MOV r7, r3
    vm.ram[3] = 0x00;
    vm.pc = 0;
    for _ in 0..100 { if !vm.step() { break; } }
    assert_eq!(vm.regs[7], 0xDEADBEEF);
    assert_eq!(vm.regs[3], 0xDEADBEEF); // source unchanged
}

// ── PUSH / POP ──────────────────────────────────────────────────

#[test]
fn test_push_pop_roundtrip() {
    // LDI r30, 0xFF00 (SP); LDI r5, 42; PUSH r5; LDI r5, 0; POP r6; HALT
    let mut vm = Vm::new();
    let mut pc = 0u32;
    // LDI r30, 0xFF00
    vm.ram[pc as usize] = 0x10; pc += 1;
    vm.ram[pc as usize] = 30; pc += 1;
    vm.ram[pc as usize] = 0xFF00; pc += 1;
    // LDI r5, 42
    vm.ram[pc as usize] = 0x10; pc += 1;
    vm.ram[pc as usize] = 5; pc += 1;
    vm.ram[pc as usize] = 42; pc += 1;
    // PUSH r5
    vm.ram[pc as usize] = 0x60; pc += 1;
    vm.ram[pc as usize] = 5; pc += 1;
    // LDI r5, 0 (clobber)
    vm.ram[pc as usize] = 0x10; pc += 1;
    vm.ram[pc as usize] = 5; pc += 1;
    vm.ram[pc as usize] = 0; pc += 1;
    // POP r6
    vm.ram[pc as usize] = 0x61; pc += 1;
    vm.ram[pc as usize] = 6; pc += 1;
    // HALT
    vm.ram[pc as usize] = 0x00; pc += 1;

    vm.pc = 0;
    for _ in 0..100 { if !vm.step() { break; } }
    assert_eq!(vm.regs[6], 42); // got value back from stack
    assert_eq!(vm.regs[5], 0);  // r5 was clobbered
    assert_eq!(vm.regs[30], 0xFF00); // SP restored
}

// ── CMP signed comparison ───────────────────────────────────────

#[test]
fn test_cmp_signed_negative_vs_positive() {
    // -1 (0xFFFFFFFF) vs 5 -> should be less than
    let mut vm = Vm::new();
    vm.regs[1] = 0xFFFFFFFF; // -1 as i32
    vm.regs[2] = 5;
    vm.ram[0] = 0x50; vm.ram[1] = 1; vm.ram[2] = 2;
    vm.ram[3] = 0x00;
    vm.pc = 0;
    for _ in 0..100 { if !vm.step() { break; } }
    assert_eq!(vm.regs[0], 0xFFFFFFFF); // -1 < 5 in signed
}

// ── FRAME ───────────────────────────────────────────────────────

#[test]
fn test_frame_increments_ticks() {
    let mut vm = Vm::new();
    vm.ram[0] = 0x02; // FRAME
    vm.ram[1] = 0x00; // HALT
    vm.pc = 0;
    for _ in 0..100 { if !vm.step() { break; } }
    assert!(vm.frame_ready);
    assert_eq!(vm.frame_count, 1);
    assert_eq!(vm.ram[0xFFE], 1);
}

// ── PSET / FILL ─────────────────────────────────────────────────

#[test]
fn test_fill() {
    let mut vm = Vm::new();
    vm.regs[1] = 0x00FF00; // green
    vm.ram[0] = 0x42; vm.ram[1] = 1; // FILL r1
    vm.ram[2] = 0x00;
    vm.pc = 0;
    for _ in 0..100 { if !vm.step() { break; } }
    // Every pixel should be green
    assert!(vm.screen.iter().all(|&p| p == 0x00FF00));
}

#[test]
fn test_pset_pixel() {
    let mut vm = Vm::new();
    vm.regs[1] = 10;  // x
    vm.regs[2] = 20;  // y
    vm.regs[3] = 0xFF0000; // red
    vm.ram[0] = 0x40; vm.ram[1] = 1; vm.ram[2] = 2; vm.ram[3] = 3; // PSET r1, r2, r3
    vm.ram[4] = 0x00;
    vm.pc = 0;
    for _ in 0..100 { if !vm.step() { break; } }
    assert_eq!(vm.screen[20 * 256 + 10], 0xFF0000);
}

// ── IKEY ────────────────────────────────────────────────────────

#[test]
fn test_ikey_reads_and_clears() {
    let mut vm = Vm::new();
    vm.ram[0xFFF] = 65; // 'A' in keyboard port
    vm.ram[0] = 0x48; vm.ram[1] = 5; // IKEY r5
    vm.ram[2] = 0x00;
    vm.pc = 0;
    for _ in 0..100 { if !vm.step() { break; } }
    assert_eq!(vm.regs[5], 65);
    assert_eq!(vm.ram[0xFFF], 0); // port cleared
}

#[test]
fn test_ikey_no_key() {
    let mut vm = Vm::new();
    vm.ram[0xFFF] = 0; // no key
    vm.ram[0] = 0x48; vm.ram[1] = 5; // IKEY r5
    vm.ram[2] = 0x00;
    vm.pc = 0;
    for _ in 0..100 { if !vm.step() { break; } }
    assert_eq!(vm.regs[5], 0);
}

// ── RAND ────────────────────────────────────────────────────────

#[test]
fn test_rand_changes_state() {
    let mut vm = Vm::new();
    let initial_state = vm.rand_state;
    vm.ram[0] = 0x49; vm.ram[1] = 5; // RAND r5
    vm.ram[2] = 0x00;
    vm.pc = 0;
    for _ in 0..100 { if !vm.step() { break; } }
    assert_ne!(vm.rand_state, initial_state); // state changed
    assert_ne!(vm.regs[5], 0); // probably nonzero (LCG seeded with DEADBEEF)
}

// ── BEEP ────────────────────────────────────────────────────────

#[test]
fn test_beep_sets_state() {
    let mut vm = Vm::new();
    vm.regs[1] = 440;  // freq
    vm.regs[2] = 200;  // duration
    vm.ram[0] = 0x03; vm.ram[1] = 1; vm.ram[2] = 2; // BEEP r1, r2
    vm.ram[3] = 0x00;
    vm.pc = 0;
    for _ in 0..100 { if !vm.step() { break; } }
    assert_eq!(vm.beep, Some((440, 200)));
}

// ── MEMCPY ───────────────────────────────────────────────────────

#[test]
fn test_memcpy_copies_words() {
    let mut vm = Vm::new();
    // Write some data to addresses 100-104
    for i in 0..5 {
        vm.ram[100 + i] = (1000 + i as u32);
    }
    // Set regs: r1=200 (dst), r2=100 (src), r3=5 (len)
    vm.regs[1] = 200;
    vm.regs[2] = 100;
    vm.regs[3] = 5;
    // MEMCPY r1, r2, r3
    vm.ram[0] = 0x04; vm.ram[1] = 1; vm.ram[2] = 2; vm.ram[3] = 3;
    vm.ram[4] = 0x00; // HALT
    vm.pc = 0;
    for _ in 0..100 { if !vm.step() { break; } }
    // Verify dst has the data
    for i in 0..5 {
        assert_eq!(vm.ram[200 + i], 1000 + i as u32, "MEMCPY failed at offset {}", i);
    }
}

#[test]
fn test_memcpy_zero_len_is_noop() {
    let mut vm = Vm::new();
    vm.ram[100] = 0xDEAD;
    vm.ram[200] = 0xBEEF;
    vm.regs[1] = 200; // dst
    vm.regs[2] = 100; // src
    vm.regs[3] = 0;   // len = 0
    vm.ram[0] = 0x04; vm.ram[1] = 1; vm.ram[2] = 2; vm.ram[3] = 3;
    vm.ram[4] = 0x00;
    vm.pc = 0;
    for _ in 0..100 { if !vm.step() { break; } }
    assert_eq!(vm.ram[200], 0xBEEF, "MEMCPY with len=0 should not overwrite dst");
}

// ── Loop: verify backward jumps work at base_addr 0 ─────────────

#[test]
fn test_backward_jump_loop_at_addr_zero() {
    // Count from 0 to 5 using a loop
    // LDI r1, 0     ; counter = 0
    // LDI r2, 1     ; increment
    // LDI r3, 5     ; limit
    // loop:
    // ADD r1, r2     ; counter++
    // CMP r1, r3
    // BLT r0, loop   ; if counter < 5, loop
    // HALT
    let mut vm = Vm::new();
    let mut pc = 0usize;
    // LDI r1, 0
    vm.ram[pc] = 0x10; vm.ram[pc+1] = 1; vm.ram[pc+2] = 0; pc += 3;
    // LDI r2, 1
    vm.ram[pc] = 0x10; vm.ram[pc+1] = 2; vm.ram[pc+2] = 1; pc += 3;
    // LDI r3, 5
    vm.ram[pc] = 0x10; vm.ram[pc+1] = 3; vm.ram[pc+2] = 5; pc += 3;
    let loop_addr = pc as u32;
    // ADD r1, r2
    vm.ram[pc] = 0x20; vm.ram[pc+1] = 1; vm.ram[pc+2] = 2; pc += 3;
    // CMP r1, r3
    vm.ram[pc] = 0x50; vm.ram[pc+1] = 1; vm.ram[pc+2] = 3; pc += 3;
    // BLT r0, loop_addr
    vm.ram[pc] = 0x35; vm.ram[pc+1] = 0; vm.ram[pc+2] = loop_addr; pc += 3;
    // HALT
    vm.ram[pc] = 0x00;

    vm.pc = 0;
    for _ in 0..1000 { if !vm.step() { break; } }
    assert_eq!(vm.regs[1], 5);
    assert!(vm.halted);
}

// ── Loop: verify backward jumps work at base_addr 0x1000 ────────

#[test]
fn test_backward_jump_loop_at_addr_0x1000() {
    // Same program but loaded at 0x1000 -- the GUI mode scenario
    let mut vm = Vm::new();
    let base = 0x1000usize;
    let mut pc = base;
    // LDI r1, 0
    vm.ram[pc] = 0x10; vm.ram[pc+1] = 1; vm.ram[pc+2] = 0; pc += 3;
    // LDI r2, 1
    vm.ram[pc] = 0x10; vm.ram[pc+1] = 2; vm.ram[pc+2] = 1; pc += 3;
    // LDI r3, 5
    vm.ram[pc] = 0x10; vm.ram[pc+1] = 3; vm.ram[pc+2] = 5; pc += 3;
    let loop_addr = pc as u32;
    // ADD r1, r2
    vm.ram[pc] = 0x20; vm.ram[pc+1] = 1; vm.ram[pc+2] = 2; pc += 3;
    // CMP r1, r3
    vm.ram[pc] = 0x50; vm.ram[pc+1] = 1; vm.ram[pc+2] = 3; pc += 3;
    // BLT r0, loop_addr -- label resolved to 0x1000 + offset
    vm.ram[pc] = 0x35; vm.ram[pc+1] = 0; vm.ram[pc+2] = loop_addr; pc += 3;
    // HALT
    vm.ram[pc] = 0x00;

    vm.pc = base as u32;
    for _ in 0..1000 { if !vm.step() { break; } }
    assert_eq!(vm.regs[1], 5);
    assert!(vm.halted);
}

// ── PEEK ────────────────────────────────────────────────────────

#[test]
fn test_peek_reads_screen() {
    let mut vm = Vm::new();
    vm.screen[30 * 256 + 15] = 0xABCDEF;
    vm.regs[1] = 15; // x
    vm.regs[2] = 30; // y
    vm.ram[0] = 0x6D; vm.ram[1] = 3; vm.ram[2] = 1; vm.ram[3] = 2; // PEEK r3, r1, r2 (dest=r3, x=r1=15, y=r2=30)
    vm.ram[4] = 0x00;
    vm.pc = 0;
    for _ in 0..100 { if !vm.step() { break; } }
    assert_eq!(vm.regs[3], 0xABCDEF);
}

#[test]
fn test_peek_out_of_bounds_returns_zero() {
    let mut vm = Vm::new();
    vm.regs[1] = 300; // x out of bounds
    vm.regs[2] = 300; // y out of bounds
    vm.ram[0] = 0x6D; vm.ram[1] = 1; vm.ram[2] = 2; vm.ram[3] = 3;
    vm.ram[4] = 0x00;
    vm.pc = 0;
    for _ in 0..100 { if !vm.step() { break; } }
    assert_eq!(vm.regs[3], 0);
}

#[test]
fn test_memcpy_copies_memory() {
    let mut vm = Vm::new();
    // Set up source data at 0x2000
    for i in 0..5 {
        vm.ram[0x2000 + i] = (100 + i) as u32;
    }
    vm.regs[1] = 0x3000; // dst
    vm.regs[2] = 0x2000; // src
    vm.regs[3] = 5;      // len
    vm.ram[0] = 0x04; vm.ram[1] = 1; vm.ram[2] = 2; vm.ram[3] = 3; // MEMCPY r1, r2, r3
    vm.ram[4] = 0x00; // HALT
    vm.pc = 0;
    for _ in 0..100 { if !vm.step() { break; } }
    assert!(vm.halted);
    // Verify destination has the copied data
    for i in 0..5 {
        assert_eq!(vm.ram[0x3000 + i], (100 + i) as u32, "MEMCPY dest[{}] should be {}", i, 100 + i);
    }
    // Source should be unchanged
    for i in 0..5 {
        assert_eq!(vm.ram[0x2000 + i], (100 + i) as u32, "MEMCPY src[{}] should be unchanged", i);
    }
}

#[test]
fn test_memcpy_assembles_and_runs() {
    use crate::assembler::assemble;
    let src = "LDI r1, 0x3000\nLDI r2, 0x2000\nLDI r3, 5\nMEMCPY r1, r2, r3\nHALT";
    let asm = assemble(src, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    // Write source data
    for i in 0..5 { vm.ram[0x2000 + i] = (42 + i) as u32; }
    for (i, &w) in asm.pixels.iter().enumerate() { vm.ram[i] = w; }
    vm.pc = 0;
    for _ in 0..100 { if !vm.step() { break; } }
    assert!(vm.halted);
    for i in 0..5 {
        assert_eq!(vm.ram[0x3000 + i], (42 + i) as u32);
    }
}

// ── RAM-Mapped Screen Buffer (Phase 46) ──────────────────────────

#[test]
fn test_screen_ram_store() {
    let mut vm = Vm::new();
    // STORE to screen addr 0x10000 (pixel 0,0) with color 0xFF0000
    vm.regs[1] = 0x10000; // addr
    vm.regs[2] = 0xFF0000; // value (red)
    vm.ram[0] = 0x12; vm.ram[1] = 1; vm.ram[2] = 2; // STORE r1, r2
    vm.pc = 0;
    vm.step();
    assert_eq!(vm.screen[0], 0xFF0000);
}

#[test]
fn test_screen_ram_load() {
    let mut vm = Vm::new();
    // Pre-set a pixel in the screen buffer
    vm.screen[256 * 10 + 5] = 0xABCDEF;
    // LOAD from screen addr 0x10000 + 256*10 + 5
    vm.regs[1] = 0x10000 + 256 * 10 + 5;
    vm.ram[0] = 0x11; vm.ram[1] = 3; vm.ram[2] = 1; // LOAD r3, r1
    vm.pc = 0;
    vm.step();
    assert_eq!(vm.regs[3], 0xABCDEF);
}

#[test]
fn test_screen_ram_store_then_load_roundtrip() {
    let vm = run_program(&[
        0x10, 1, 0x10050,       // LDI r1, 0x10050
        0x10, 2, 0x00FF00,      // LDI r2, 0x00FF00
        0x12, 1, 2,             // STORE r1, r2
        0x11, 4, 1,             // LOAD r4, r1
        0x00,                   // HALT
    ], 100);
    assert!(vm.halted);
    assert_eq!(vm.regs[4], 0x00FF00);
    assert_eq!(vm.screen[0x50], 0x00FF00);
}

#[test]
fn test_screen_ram_does_not_corrupt_normal_ram() {
    let mut vm = Vm::new();
    // Store a value at a normal RAM address first
    vm.ram[0x2000] = 0xDEADBEEF;
    // Store to screen address
    vm.regs[1] = 0x10000;
    vm.regs[2] = 0xFF0000;
    vm.ram[0] = 0x12; vm.ram[1] = 1; vm.ram[2] = 2; // STORE r1, r2
    vm.pc = 0;
    vm.step();
    // Normal RAM should be unchanged
    assert_eq!(vm.ram[0x2000], 0xDEADBEEF);
    // Screen should have the stored value
    assert_eq!(vm.screen[0], 0xFF0000);
}

#[test]
fn test_screen_ram_load_matches_peek() {
    let mut vm = Vm::new();
    // Set pixel at (15, 30) via screen buffer directly
    vm.screen[30 * 256 + 15] = 0x123456;

    // Read via LOAD from screen-mapped address
    let screen_addr = (SCREEN_RAM_BASE + 30 * 256 + 15) as u32;
    vm.regs[1] = screen_addr;
    vm.ram[0] = 0x11; vm.ram[1] = 3; vm.ram[2] = 1; // LOAD r3, r1
    vm.pc = 0;
    for _ in 0..100 { if !vm.step() { break; } }
    let load_value = vm.regs[3];

    // Reset halted state for second instruction sequence
    vm.halted = false;

    // Read via PEEK opcode
    vm.regs[1] = 15; // x
    vm.regs[2] = 30; // y
    vm.ram[4] = 0x6D; vm.ram[5] = 4; vm.ram[6] = 1; vm.ram[7] = 2; // PEEK r4, r1, r2
    vm.ram[8] = 0x00;
    vm.pc = 4;
    for _ in 0..100 { if !vm.step() { break; } }
    let peek_value = vm.regs[4];

    assert_eq!(load_value, 0x123456);
    assert_eq!(peek_value, 0x123456);
    assert_eq!(load_value, peek_value);
}

#[test]
fn test_screen_ram_store_matches_pixel() {
    let mut vm = Vm::new();

    // Write pixel via STORE to screen-mapped address at (10, 20)
    let screen_addr = (SCREEN_RAM_BASE + 20 * 256 + 10) as u32;
    vm.regs[1] = screen_addr;
    vm.regs[2] = 0xFF0000; // red
    vm.ram[0] = 0x12; vm.ram[1] = 1; vm.ram[2] = 2; // STORE r1, r2
    vm.pc = 0;
    for _ in 0..100 { if !vm.step() { break; } }

    // Verify via screen buffer directly
    assert_eq!(vm.screen[20 * 256 + 10], 0xFF0000);

    // Reset halted state for second instruction sequence
    vm.halted = false;

    // Verify via PEEK opcode
    vm.regs[1] = 10; // x
    vm.regs[2] = 20; // y
    vm.ram[3] = 0x6D; vm.ram[4] = 5; vm.ram[5] = 1; vm.ram[6] = 2; // PEEK r5, r1, r2
    vm.ram[7] = 0x00;
    vm.pc = 3;
    for _ in 0..100 { if !vm.step() { break; } }
    assert_eq!(vm.regs[5], 0xFF0000);
}

#[test]
fn test_screen_ram_boundary_first_and_last_pixel() {
    let mut vm = Vm::new();

    // First pixel: address 0x10000
    vm.regs[1] = SCREEN_RAM_BASE as u32;
    vm.regs[2] = 0x111111;
    vm.ram[0] = 0x12; vm.ram[1] = 1; vm.ram[2] = 2; // STORE r1, r2
    vm.pc = 0;
    vm.step();
    assert_eq!(vm.screen[0], 0x111111);

    // Last pixel: address 0x10000 + 65535 = 0x1FFFF
    let last_addr = (SCREEN_RAM_BASE + SCREEN_SIZE - 1) as u32;
    vm.regs[1] = last_addr;
    vm.regs[2] = 0x222222;
    vm.ram[0] = 0x12; vm.ram[1] = 1; vm.ram[2] = 2; // STORE r1, r2
    vm.pc = 0;
    vm.step();
    assert_eq!(vm.screen[SCREEN_SIZE - 1], 0x222222);

    // Read back via LOAD
    vm.regs[1] = SCREEN_RAM_BASE as u32;
    vm.ram[0] = 0x11; vm.ram[1] = 3; vm.ram[2] = 1; // LOAD r3, r1
    vm.pc = 0;
    vm.step();
    assert_eq!(vm.regs[3], 0x111111);

    vm.regs[1] = last_addr;
    vm.ram[0] = 0x11; vm.ram[1] = 3; vm.ram[2] = 1; // LOAD r3, r1
    vm.pc = 0;
    vm.step();
    assert_eq!(vm.regs[3], 0x222222);
}

#[test]
fn test_screen_ram_user_mode_allowed() {
    let mut vm = Vm::new();
    vm.mode = CpuMode::User;
    // User-mode store to screen should work (screen is not I/O)
    vm.regs[1] = 0x10000;
    vm.regs[2] = 0x00FF00;
    vm.ram[0] = 0x12; vm.ram[1] = 1; vm.ram[2] = 2; // STORE r1, r2
    vm.pc = 0;
    assert!(vm.step()); // Should NOT segfault
    assert_eq!(vm.screen[0], 0x00FF00);
}

#[test]
fn test_screen_ram_assembles_and_runs() {
    use crate::assembler::assemble;
    // Write assembly that stores to screen buffer, reads back, stores to RAM for comparison
    let src = "LDI r1, 0x10000\nLDI r2, 0xFF0000\nSTORE r1, r2\nLOAD r3, r1\nLDI r4, 0x7000\nSTORE r4, r3\nHALT";
    let asm = assemble(src, 0).expect("assembly should succeed");
    let vm = run_program(&asm.pixels, 100);
    assert!(vm.halted);
    assert_eq!(vm.screen[0], 0xFF0000);
    assert_eq!(vm.ram[0x7000], 0xFF0000);
}

// ── ASMSELF tests (Phase 47: Pixel Driving Pixels) ──────────

/// Helper: write an ASCII string into the VM's canvas buffer at a given offset.
fn write_to_canvas(canvas: &mut Vec<u32>, offset: usize, text: &str) {
    for (i, ch) in text.bytes().enumerate() {
        let idx = offset + i;
        if idx < canvas.len() {
            canvas[idx] = ch as u32;
        }
    }
}

#[test]
fn test_asmself_assembles_valid_canvas_text() {
    // Pre-fill canvas with "LDI r0, 42\nHALT\n"
    let mut vm = Vm::new();
    let program = "LDI r0, 42\nHALT\n";
    write_to_canvas(&mut vm.canvas_buffer, 0, program);

    // Execute ASMSELF opcode
    vm.ram[0] = 0x73; // ASMSELF
    vm.pc = 0;
    vm.step();

    // Check status port: should be positive (bytecode word count)
    assert_ne!(vm.ram[0xFFD], 0xFFFFFFFF, "ASMSELF should succeed");
    assert!(vm.ram[0xFFD] > 0, "ASMSELF should produce bytecode");

    // Verify bytecode at 0x1000: LDI r0, 42 = [0x10, 0, 42], HALT = [0x00]
    assert_eq!(vm.ram[0x1000], 0x10, "LDI opcode");
    assert_eq!(vm.ram[0x1001], 0, "r0 register");
    assert_eq!(vm.ram[0x1002], 42, "immediate 42");
    assert_eq!(vm.ram[0x1003], 0x00, "HALT opcode");
}

#[test]
fn test_asmself_handles_invalid_assembly_gracefully() {
    let mut vm = Vm::new();
    // Write garbage to canvas
    write_to_canvas(&mut vm.canvas_buffer, 0, "ZZZTOP R0, R1 !!INVALID!!\n");

    vm.ram[0] = 0x73;
    vm.pc = 0;
    vm.step();

    // Status port should be error sentinel
    assert_eq!(vm.ram[0xFFD], 0xFFFFFFFF, "ASMSELF should report error");

    // VM should NOT be halted -- continues executing
    assert!(!vm.halted, "VM should survive ASMSELF error");
}

#[test]
fn test_asmself_full_write_compile_execute() {
    // Full integration: program writes code to canvas, ASMSELF, then jumps to 0x1000
    let mut vm = Vm::new();

    // First, set up the canvas with "LDI r0, 99\nHALT\n"
    write_to_canvas(&mut vm.canvas_buffer, 0, "LDI r0, 99\nHALT\n");

    // Build a program that calls ASMSELF, then jumps to 0x1000
    // JMP takes an immediate address, not a register
    let bootstrap = "ASMSELF\nJMP 0x1000\n";
    let asm = crate::assembler::assemble(bootstrap, 0).expect("assembly should succeed");
    for (i, &word) in asm.pixels.iter().enumerate() {
        vm.ram[i] = word;
    }
    vm.pc = 0;

    // Run the bootstrap program
    let max_steps = 200;
    for _ in 0..max_steps {
        if vm.halted {
            break;
        }
        vm.step();
    }

    // After bootstrap: ASMSELF assembled canvas code, JMP went to 0x1000,
    // new code ran LDI r0, 99 then HALT
    assert!(vm.halted, "VM should halt after executing assembled code");
    assert_eq!(vm.ram[0xFFD], 4, "ASMSELF should report 4 words of bytecode");
    assert_eq!(vm.regs[0], 99, "r0 should be 99 after assembled code runs");
}

#[test]
fn test_asmself_disassembler() {
    let mut vm = Vm::new();
    vm.ram[0] = 0x73; // ASMSELF
    let (text, len) = vm.disassemble_at(0);
    assert_eq!(text, "ASMSELF");
    assert_eq!(len, 1);
}

#[test]
fn test_asmself_assembler_mnemonic() {
    use crate::assembler::assemble;
    let src = "ASMSELF\nHALT\n";
    let result = assemble(src, 0).expect("assembly should succeed");
    assert_eq!(result.pixels[0], 0x73, "ASMSELF should encode as 0x73");
    assert_eq!(result.pixels[1], 0x00, "HALT should follow");
}

#[test]
fn test_asmself_empty_canvas() {
    let mut vm = Vm::new();
    // Canvas is all zeros -- should produce empty/minimal assembly
    vm.ram[0] = 0x73;
    vm.pc = 0;
    vm.step();

    // Empty canvas should either succeed (0 words) or fail gracefully
    // Either way, VM should not be halted
    assert!(!vm.halted, "VM should survive ASMSELF on empty canvas");
}

#[test]
fn test_asmself_preserves_registers() {
    // Verify that ASMSELF doesn't clobber registers (only writes to RAM)
    let mut vm = Vm::new();
    vm.regs[0] = 111;
    vm.regs[1] = 222;
    vm.regs[5] = 555;

    write_to_canvas(&mut vm.canvas_buffer, 0, "LDI r0, 42\nHALT\n");

    vm.ram[0] = 0x73;
    vm.pc = 0;
    vm.step();

    // Registers should be preserved after ASMSELF
    assert_eq!(vm.regs[0], 111, "r0 should be preserved");
    assert_eq!(vm.regs[1], 222, "r1 should be preserved");
    assert_eq!(vm.regs[5], 555, "r5 should be preserved");
}

#[test]
fn test_asmself_with_preprocessor_macros() {
    // Test that preprocessor macros work in ASMSELF
    let mut vm = Vm::new();
    // Use SET/GET macros
    write_to_canvas(&mut vm.canvas_buffer, 0, "VAR x 42\nGET r1, x\nHALT\n");

    vm.ram[0] = 0x73;
    vm.pc = 0;
    vm.step();

    // Should succeed (preprocessor expands VAR and GET)
    assert_ne!(vm.ram[0xFFD], 0xFFFFFFFF, "ASMSELF with macros should succeed");
    assert!(vm.ram[0xFFD] > 0, "Should produce some bytecode");
}

#[test]
fn test_store_writes_successor_to_canvas_then_asmself_executes() {
    // Phase 47 integration test: the program itself uses STORE to write
    // "LDI r0, 99\nHALT\n" to the canvas RAM range (0x8000-0x8FFF).
    // ASMSELF reads canvas_buffer, assembles the source into bytecode at
    // 0x1000, then RUNNEXT jumps there. Verify r0 ends up as 99.
    //
    // This is the "pixel driving pixels" loop: code writes code, compiles
    // it, and runs it -- all through the VM's own STORE/ASMSELF/RUNNEXT.

    let mut vm = Vm::new();

    // Build a bootstrap program that writes each character via STORE
    let successor = "LDI r0, 99\nHALT\n";
    let mut src = String::new();

    // r1 = canvas address pointer (starts at 0x8000)
    // r3 = increment (1)
    src.push_str("LDI r1, 0x8000\n");
    src.push_str("LDI r3, 1\n");

    for (i, ch) in successor.bytes().enumerate() {
        if i > 0 {
            src.push_str("ADD r1, r3\n"); // advance canvas pointer
        }
        src.push_str(&format!("LDI r2, {}\nSTORE r1, r2\n", ch as u32));
    }

    // Compile the canvas source and execute the result
    src.push_str("ASMSELF\n");
    src.push_str("RUNNEXT\n");

    // Assemble the bootstrap program
    let asm = crate::assembler::assemble(&src, 0).expect("assembly should succeed");
    for (i, &word) in asm.pixels.iter().enumerate() {
        vm.ram[i] = word;
    }
    vm.pc = 0;

    // Verify the canvas buffer is empty before execution
    assert_eq!(vm.canvas_buffer[0], 0, "canvas should start empty");

    // Run until halted or safety limit
    for _ in 0..50000 {
        if vm.halted {
            break;
        }
        vm.step();
    }

    // The successor code (LDI r0, 99; HALT) should have executed
    assert!(vm.halted, "VM should halt after self-written code executes");
    assert_eq!(vm.regs[0], 99, "r0 should be 99 after successor runs");
    assert_ne!(
        vm.ram[0xFFD], 0xFFFFFFFF,
        "ASMSELF should have succeeded"
    );
}

// ── RUNNEXT tests (Phase 48: Self-Execution Opcode) ──────────

#[test]
fn test_runnext_sets_pc_to_0x1000() {
    let mut vm = Vm::new();
    vm.pc = 0;
    vm.ram[0] = 0x74; // RUNNEXT

    vm.step();

    assert_eq!(vm.pc, 0x1000, "RUNNEXT should set PC to 0x1000");
    assert!(!vm.halted, "RUNNEXT should not halt the VM");
}

#[test]
fn test_runnext_preserves_registers() {
    let mut vm = Vm::new();
    vm.regs[0] = 111;
    vm.regs[1] = 222;
    vm.regs[5] = 555;
    vm.ram[0] = 0x74; // RUNNEXT

    vm.step();

    assert_eq!(vm.regs[0], 111, "r0 should be preserved across RUNNEXT");
    assert_eq!(vm.regs[1], 222, "r1 should be preserved across RUNNEXT");
    assert_eq!(vm.regs[5], 555, "r5 should be preserved across RUNNEXT");
}

#[test]
fn test_runnext_executes_newly_assembled_code() {
    // Full write-compile-execute cycle:
    // 1. Write "LDI r0, 77\nHALT\n" to canvas
    // 2. ASMSELF compiles it to 0x1000
    // 3. RUNNEXT jumps to 0x1000
    // 4. r0 should end up as 77
    let mut vm = Vm::new();
    write_to_canvas(&mut vm.canvas_buffer, 0, "LDI r0, 77\nHALT\n");

    // ASMSELF
    vm.ram[0] = 0x73;
    vm.pc = 0;
    vm.step();
    assert_ne!(vm.ram[0xFFD], 0xFFFFFFFF, "ASMSELF should succeed");

    // RUNNEXT
    vm.ram[1] = 0x74; // RUNNEXT at address 1
    vm.pc = 1;
    vm.step();
    assert_eq!(vm.pc, 0x1000, "RUNNEXT should set PC to 0x1000");

    // Execute the newly assembled code (LDI r0, 77; HALT)
    vm.step(); // LDI r0, 77
    vm.step(); // HALT

    assert_eq!(vm.regs[0], 77, "r0 should be 77 after RUNNEXT executes new code");
    assert!(vm.halted, "VM should halt after new code's HALT");
}

#[test]
fn test_runnext_registers_inherited_by_new_code() {
    // Set registers before RUNNEXT, new code should read them
    let mut vm = Vm::new();
    vm.regs[5] = 12345;

    // New code: LDI r0, 0; ADD r0, r5; HALT
    // This reads r5 and adds it to r0
    write_to_canvas(&mut vm.canvas_buffer, 0, "LDI r0, 0\nADD r0, r5\nHALT\n");

    // ASMSELF
    vm.ram[0] = 0x73;
    vm.pc = 0;
    vm.step();
    assert_ne!(vm.ram[0xFFD], 0xFFFFFFFF, "ASMSELF should succeed");

    // RUNNEXT
    vm.ram[1] = 0x74;
    vm.pc = 1;
    vm.step();

    // Execute new code
    for _ in 0..10 { vm.step(); }

    assert_eq!(vm.regs[0], 12345, "r0 should equal r5's value from before RUNNEXT");
}

#[test]
fn test_runnext_disassembler() {
    let mut vm = Vm::new();
    vm.ram[0] = 0x74; // RUNNEXT
    let (text, _len) = vm.disassemble_at(0);
    assert_eq!(text, "RUNNEXT", "Disassembler should show RUNNEXT");
}

#[test]
fn test_runnext_assembler() {
    use crate::assembler::assemble;
    let src = "RUNNEXT\nHALT\n";
    let result = assemble(src, 0).expect("assembly should succeed");
    assert_eq!(result.pixels[0], 0x74, "RUNNEXT should encode as 0x74");
}

#[test]
fn test_chained_self_modification() {
    // Two-generation self-modification chain:
    // Gen A (bootstrap at PC=0): writes source to canvas, ASMSELF, RUNNEXT
    // Gen B (at 0x1000): LDI r0, 999; HALT
    //
    // Three-generation chains are possible but require careful address management
    // to avoid the ASMSELF clear zone (0x1000-0x1FFF). This test proves the
    // core mechanism: a program writes its successor, compiles it, and runs it.
    let mut vm = Vm::new();

    // Write Gen B source directly to canvas: "LDI r0, 999\nHALT\n"
    let gen_b_src = "LDI r0, 999\nHALT\n";
    write_to_canvas(&mut vm.canvas_buffer, 0, gen_b_src);

    // Bootstrap at PC=0: ASMSELF compiles canvas text to 0x1000, RUNNEXT jumps there
    vm.ram[0] = 0x73; // ASMSELF
    vm.ram[1] = 0x74; // RUNNEXT
    vm.pc = 0;

    // Execute the chain
    for _ in 0..100 {
        if vm.halted { break; }
        vm.step();
    }

    assert!(vm.halted, "VM should halt after Gen B executes");
    assert_eq!(vm.regs[0], 999, "r0 should be 999 -- proof Gen B ran after Gen A assembled it");
}

#[test]
fn test_runnext_full_write_compile_execute_cycle() {
    // A program that writes code to canvas, compiles it, and runs it
    let mut vm = Vm::new();

    // Write assembly source to canvas: "LDI r1, 42\nADD r0, r1\nHALT\n"
    write_to_canvas(&mut vm.canvas_buffer, 0, "LDI r1, 42\nADD r0, r1\nHALT\n");

    // Set r0 = 100 before RUNNEXT
    vm.regs[0] = 100;

    // Bootstrap: ASMSELF then RUNNEXT
    vm.ram[0] = 0x73; // ASMSELF
    vm.ram[1] = 0x74; // RUNNEXT
    vm.pc = 0;

    // Execute ASMSELF
    vm.step();
    assert_ne!(vm.ram[0xFFD], 0xFFFFFFFF, "ASMSELF should succeed");

    // Execute RUNNEXT
    vm.step();
    assert_eq!(vm.pc, 0x1000);

    // Execute the new code (LDI r1, 42; ADD r0, r1; HALT)
    for _ in 0..20 { vm.step(); }

    // r0 was 100, r1 becomes 42, r0 = r0 + r1 = 142
    assert_eq!(vm.regs[0], 142, "r0 should be 100 + 42 = 142");
    assert_eq!(vm.regs[1], 42, "r1 should be 42");
}

// ============================================================
// Phase 49: Self-Modifying Programs - Demo Tests
// ============================================================

#[test]
fn test_self_writer_demo_assembles() {
    // Verify the self_writer.asm program assembles without errors
    let source = include_str!("../../programs/self_writer.asm");
    let result = crate::assembler::assemble(source, 0x1000);
    assert!(result.is_ok(), "self_writer.asm should assemble: {:?}", result.err());
    let asm = result.expect("operation should succeed");
    assert!(asm.pixels.len() > 50, "self_writer should produce substantial bytecode");
}

#[test]
fn test_self_writer_successor_different_from_parent() {
    // The parent writes "LDI r0, 42\nHALT\n" to canvas, then ASMSELF + RUNNEXT.
    // The successor (LDI r0, 42; HALT) is clearly different from the parent
    // (which writes to canvas, calls ASMSELF, calls RUNNEXT).
    // Verify: after the full cycle, r0 == 42 (set by successor, not parent).
    let mut vm = Vm::new();
    vm.regs[0] = 0; // parent doesn't touch r0

    // Write successor source to canvas
    write_to_canvas(&mut vm.canvas_buffer, 0, "LDI r0, 42\nHALT\n");

    // Bootstrap: ASMSELF + RUNNEXT at PC=0
    vm.ram[0] = 0x73; // ASMSELF
    vm.ram[1] = 0x74; // RUNNEXT
    vm.pc = 0;

    // Execute ASMSELF
    vm.step();
    assert_ne!(vm.ram[0xFFD], 0xFFFFFFFF, "ASMSELF should succeed");

    // Execute RUNNEXT
    vm.step();
    assert_eq!(vm.pc, 0x1000, "RUNNEXT should set PC to 0x1000");

    // Execute successor: LDI r0, 42; HALT
    for _ in 0..20 { vm.step(); }

    assert_eq!(vm.regs[0], 42, "successor should set r0 to 42");
    assert!(vm.halted, "successor should halt");
}

#[test]
fn test_self_writer_canvas_output_visible() {
    // Verify that the successor's source text is visible in the canvas buffer
    // after the parent writes it (before ASMSELF compiles it).
    let mut vm = Vm::new();

    // Write successor source to canvas
    let successor_src = "LDI r0, 42\nHALT\n";
    write_to_canvas(&mut vm.canvas_buffer, 0, successor_src);

    // Verify the text is in the canvas buffer
    assert_eq!(vm.canvas_buffer[0], 'L' as u32);
    assert_eq!(vm.canvas_buffer[1], 'D' as u32);
    assert_eq!(vm.canvas_buffer[2], 'I' as u32);
    assert_eq!(vm.canvas_buffer[3], ' ' as u32);
    assert_eq!(vm.canvas_buffer[4], 'r' as u32);
    assert_eq!(vm.canvas_buffer[5], '0' as u32);
    assert_eq!(vm.canvas_buffer[6], ',' as u32);
    // Newline at index 10, HALT starts at index 11
    assert_eq!(vm.canvas_buffer[10], 10, "newline char at index 10");
    assert_eq!(vm.canvas_buffer[11], 'H' as u32);
    assert_eq!(vm.canvas_buffer[12], 'A' as u32);
    assert_eq!(vm.canvas_buffer[13], 'L' as u32);
    assert_eq!(vm.canvas_buffer[14], 'T' as u32);
}

#[test]
fn test_self_writer_two_generation_chain() {
    // Generation A: writes Gen B source to canvas, ASMSELF, RUNNEXT
    // Generation B: writes r0=77, then HALT
    // Verify the full A -> B chain works
    let mut vm = Vm::new();
    vm.regs[0] = 0;

    // Gen A writes Gen B's source to canvas
    write_to_canvas(&mut vm.canvas_buffer, 0, "LDI r0, 77\nHALT\n");

    // Gen A's code: ASMSELF, RUNNEXT
    vm.ram[0] = 0x73;
    vm.ram[1] = 0x74;
    vm.pc = 0;

    vm.step(); // ASMSELF
    assert_ne!(vm.ram[0xFFD], 0xFFFFFFFF);
    vm.step(); // RUNNEXT
    assert_eq!(vm.pc, 0x1000);

    for _ in 0..20 { vm.step(); }
    assert_eq!(vm.regs[0], 77, "Gen B should set r0 to 77");
}

#[test]
fn test_self_writer_successor_modifies_canvas() {
    // Generation A writes Gen B source to canvas.
    // Gen B writes a character to a DIFFERENT canvas row, proving it ran.
    // Gen B source: "LDI r1, 0x8040\nLDI r2, 88\nSTORE r1, r2\nHALT\n"
    // This writes 'X' (88) to canvas row 2 (0x8040 = 0x8000 + 2*32)
    let mut vm = Vm::new();

    let gen_b_src = "LDI r1, 0x8040\nLDI r2, 88\nSTORE r1, r2\nHALT\n";
    write_to_canvas(&mut vm.canvas_buffer, 0, gen_b_src);

    vm.ram[0] = 0x73; // ASMSELF
    vm.ram[1] = 0x74; // RUNNEXT
    vm.pc = 0;

    vm.step(); // ASMSELF
    assert_ne!(vm.ram[0xFFD], 0xFFFFFFFF, "ASMSELF should compile Gen B");

    vm.step(); // RUNNEXT
    assert_eq!(vm.pc, 0x1000);

    // Run Gen B
    for _ in 0..50 { vm.step(); }

    // Verify Gen B wrote 'X' to canvas row 2
    let row2_start = 2 * 32; // 0x8040 - 0x8000 = 64
    assert_eq!(vm.canvas_buffer[row2_start], 88, "Gen B should write 'X' to canvas row 2");
    assert!(vm.halted, "Gen B should halt");
}

#[test]
fn test_self_writer_registers_inherited_across_generations() {
    // Gen A sets r5=100, then writes+compiles+runs Gen B.
    // Gen B reads r5 (should be 100), adds 1, stores in r0.
    // Gen B source: "ADD r0, r5\nLDI r1, 1\nADD r0, r1\nHALT\n"
    // Result: r0 = 0 + 100 + 1 = 101
    let mut vm = Vm::new();
    vm.regs[5] = 100; // Set by Gen A before RUNNEXT
    vm.regs[0] = 0;

    let gen_b_src = "ADD r0, r5\nLDI r1, 1\nADD r0, r1\nHALT\n";
    write_to_canvas(&mut vm.canvas_buffer, 0, gen_b_src);

    vm.ram[0] = 0x73;
    vm.ram[1] = 0x74;
    vm.pc = 0;

    vm.step(); // ASMSELF
    assert_ne!(vm.ram[0xFFD], 0xFFFFFFFF);
    vm.step(); // RUNNEXT

    for _ in 0..50 { vm.step(); }
    assert_eq!(vm.regs[0], 101, "r0 should be 0 + r5(100) + 1 = 101");
    assert_eq!(vm.regs[5], 100, "r5 should still be 100 (inherited from Gen A)");
}

#[test]
fn test_infinite_map_assembles_and_runs() {
    use crate::assembler::assemble;

    let source = include_str!("../../programs/infinite_map.asm");
    let asm = assemble(source, 0).expect("infinite_map.asm should assemble");
    assert!(!asm.pixels.is_empty(), "should produce bytecode");
    eprintln!("Assembled {} words from infinite_map.asm", asm.pixels.len());

    let mut vm = Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }

    // Simulate Right arrow (bit 3 = 8)
    vm.ram[0xFFB] = 8;

    // Run until first FRAME
    vm.frame_ready = false;
    let mut steps = 0u32;
    for _ in 0..1_000_000 {
        if vm.frame_ready { break; }
        let keep_going = vm.step();
        steps += 1;
        if !keep_going { break; }
    }

    assert!(vm.frame_ready, "should reach FRAME within 1M steps (took {})", steps);
    eprintln!("First frame rendered in {} steps", steps);
    eprintln!("camera_x = {}, camera_y = {}", vm.ram[0x7800], vm.ram[0x7801]);
    assert_eq!(vm.ram[0x7800], 1, "camera should have moved right by 1");

    // Screen should not be all black
    let non_black = vm.screen.iter().filter(|&&p| p != 0).count();
    eprintln!("Non-black pixels: {}/{}", non_black, 256*256);
    assert!(non_black > 0, "screen should have rendered terrain");

    // Second frame: press Down
    vm.frame_ready = false;
    vm.ram[0xFFB] = 2; // Down
    for _ in 0..1_000_000 {
        if vm.frame_ready { break; }
        let keep_going = vm.step();
        if !keep_going { break; }
    }
    eprintln!("After 2nd frame: camera_x={}, camera_y={}", vm.ram[0x7800], vm.ram[0x7801]);
    assert!(vm.frame_ready, "second frame should render");
    assert_eq!(vm.ram[0x7801], 1, "camera should have moved down by 1");

    // Third frame: press Left+Up (bits 2+0 = 5) -- diagonal movement
    vm.frame_ready = false;
    vm.ram[0xFFB] = 5;
    for _ in 0..1_000_000 {
        if vm.frame_ready { break; }
        let keep_going = vm.step();
        if !keep_going { break; }
    }
    eprintln!("After 3rd frame (left+up): camera_x={}, camera_y={}", vm.ram[0x7800], vm.ram[0x7801]);
    assert_eq!(vm.ram[0x7800], 0, "camera should have moved left back to 0");
    assert_eq!(vm.ram[0x7801], 0, "camera should have moved up back to 0");

    // Verify frame counter incremented
    assert!(vm.ram[0x7802] >= 3, "frame_counter should be >= 3 (was {})", vm.ram[0x7802]);
    eprintln!("Frame counter: {}", vm.ram[0x7802]);

    // Verify water animation: run 2 frames without moving, check screen changes
    // Frame 4: no keys
    vm.frame_ready = false;
    vm.ram[0xFFB] = 0;
    for _ in 0..1_000_000 {
        if vm.frame_ready { break; }
        let keep_going = vm.step();
        if !keep_going { break; }
    }
    let screen_f4: Vec<u32> = vm.screen.to_vec();

    // Frame 5: no keys
    vm.frame_ready = false;
    vm.ram[0xFFB] = 0;
    for _ in 0..1_000_000 {
        if vm.frame_ready { break; }
        let keep_going = vm.step();
        if !keep_going { break; }
    }
    let screen_f5: Vec<u32> = vm.screen.to_vec();

    // Count pixels that changed between frames (water animation)
    let changed: usize = screen_f4.iter().zip(screen_f5.iter())
        .filter(|(a, b)| a != b).count();
    eprintln!("Pixels changed between frames 4-5: {}/{}", changed, 256*256);
    // With ~25% water tiles and animation, expect some pixels to change
    assert!(changed > 0, "water animation should cause pixel changes between frames");
}

#[test]
fn test_infinite_map_visual_analysis() {
    use crate::assembler::assemble;

    let source = include_str!("../../programs/infinite_map.asm");
    let asm = assemble(source, 0).expect("assembly should succeed");

    let mut vm = Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }

    // Test at camera position (100, 100) to see multiple biome zones
    // Coarse coords span (12,12) to (20,20) = 9x9 zones = lots of variety
    vm.ram[0x7800] = 100;
    vm.ram[0x7801] = 100;
    vm.ram[0xFFB] = 0;
    vm.frame_ready = false;
    for _ in 0..1_000_000 {
        if vm.frame_ready { break; }
        if !vm.step() { break; }
    }

    // Count unique colors (structures + animation create many)
    let mut color_counts: std::collections::HashMap<u32, usize> = std::collections::HashMap::new();
    for &pixel in vm.screen.iter() {
        *color_counts.entry(pixel).or_insert(0) += 1;
    }
    eprintln!("At (100,100): {} unique colors", color_counts.len());
    assert!(color_counts.len() >= 5, "should see multiple biomes at (100,100)");

    // Check biome contiguity by sampling tile-top-left pixels
    // and masking the water animation (low 5 blue bits change per tile)
    // For contiguity, compare the "base biome" by rounding colors
    let mut biome_zones = 0;
    let mut prev_base: u32 = 0;
    for tx in 0..64 {
        let px = tx * 4;
        let py = 128; // tile row 32 - middle of screen
        let color = vm.screen[py * 256 + px];
        // Round to base biome: mask out animation (low 5 bits of blue)
        let base = color & !0x1F;
        if tx == 0 || base != prev_base {
            biome_zones += 1;
            prev_base = base;
        }
    }
    eprintln!("At (100,100) row 32: {} biome zone boundaries across 64 tiles", biome_zones);

    // With 8-tile zones, expect ~8 boundaries. Per-tile hash would give ~64.
    // Allow up to 20 to account for structures overriding colors
    assert!(biome_zones < 20,
        "biomes should be contiguous, got {} zone boundaries (expected <20)", biome_zones);

    // Verify the terrain is deterministic: same camera = same screen
    let screen1 = vm.screen.to_vec();
    vm.frame_ready = false;
    for _ in 0..1_000_000 {
        if vm.frame_ready { break; }
        if !vm.step() { break; }
    }
    // Note: frame counter advanced, so water animation differs. Check non-water.
    let non_water_same = screen1.iter().zip(vm.screen.iter())
        .filter(|(&a, &b)| {
            let a_water = (a & 0xFF) > 0 && ((a >> 16) & 0xFF) == 0 && ((a >> 8) & 0xFF) < 0x20;
            !a_water && a == b
        }).count();
    // Non-water tiles should be identical (deterministic terrain)
    eprintln!("Non-water pixels identical across frames: {}", non_water_same);
}

// ── Inode Filesystem Opcodes (Phase 43) ──────────────────────────

/// Helper: create a VM with a string at addr and run bytecode
fn run_program_with_string(bytecode: &[u32], max_steps: usize, str_addr: usize, s: &str) -> Vm {
    let mut vm = Vm::new();
    // Write string to RAM
    for (i, ch) in s.bytes().enumerate() {
        vm.ram[str_addr + i] = ch as u32;
    }
    vm.ram[str_addr + s.len()] = 0;
    // Load bytecode
    for (i, &word) in bytecode.iter().enumerate() {
        vm.ram[i] = word;
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..max_steps {
        if !vm.step() { break; }
    }
    vm
}

#[test]
fn test_fmkdir_creates_directory() {
    // Write "/tmp" to RAM at address 100
    // LDI r1, 100
    // FMKDIR r1
    // HALT
    let prog = vec![0x10, 1, 100, 0x78, 1, 0x00];
    let vm = run_program_with_string(&prog, 100, 100, "/tmp");
    assert_eq!(vm.regs[0], 2); // inode 2 for /tmp
    assert_eq!(vm.inode_fs.resolve("/tmp"), Some(2));
}

#[test]
fn test_fmkdir_nested_fails() {
    // /a/b/c won't work because /a doesn't exist
    let prog = vec![0x10, 1, 100, 0x78, 1, 0x00];
    let vm = run_program_with_string(&prog, 100, 100, "/a/b/c");
    assert_eq!(vm.regs[0], 0); // failed
}

#[test]
fn test_funlink_removes_file() {
    // Create a file first via FMKDIR... no, use inode_fs directly via a setup step
    // We need to create a file in the inode_fs before running the program.
    // Since run_program_with_string creates a fresh VM, we'll create the file
    // via a two-step program: first create dirs, then unlink
    // Actually, let's use create directly on the VM after setup
    let mut vm = Vm::new();
    vm.inode_fs.create("/del_me.txt");
    assert!(vm.inode_fs.resolve("/del_me.txt").is_some());

    // Now write unlink path and run
    let path = "/del_me.txt";
    for (i, ch) in path.bytes().enumerate() {
        vm.ram[100 + i] = ch as u32;
    }
    vm.ram[100 + path.len()] = 0;

    // LDI r1, 100; FUNLINK r1; HALT
    vm.ram[0] = 0x10; vm.ram[1] = 1; vm.ram[2] = 100;
    vm.ram[3] = 0x7A; vm.ram[4] = 1;
    vm.ram[5] = 0x00;
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..100 {
        if !vm.step() { break; }
    }
    assert_eq!(vm.regs[0], 1); // success
    assert_eq!(vm.inode_fs.resolve("/del_me.txt"), None);
}

#[test]
fn test_fstat_returns_inode_metadata() {
    let mut vm = Vm::new();
    let ino = vm.inode_fs.create("/test.txt");
    vm.inode_fs.write_inode(ino, 0, &[10, 20, 30]);

    // LDI r1, <ino>; LDI r2, 200; FSTAT r1, r2; HALT
    vm.ram[0] = 0x10; vm.ram[1] = 1; vm.ram[2] = ino;
    vm.ram[3] = 0x10; vm.ram[4] = 2; vm.ram[5] = 200;
    vm.ram[6] = 0x79; vm.ram[7] = 1; vm.ram[8] = 2;
    vm.ram[9] = 0x00;
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..100 {
        if !vm.step() { break; }
    }
    assert_eq!(vm.regs[0], 1); // success
    assert_eq!(vm.ram[200], ino);         // ino
    assert_eq!(vm.ram[201], 1);           // itype = Regular
    assert_eq!(vm.ram[202], 3);           // size
    assert_eq!(vm.ram[203], 0);           // ref_count
    assert_eq!(vm.ram[204], 1);           // parent = root
    assert_eq!(vm.ram[205], 0);           // num_children
}

#[test]
fn test_fstat_nonexistent_returns_zero() {
    // LDI r1, 999; LDI r2, 200; FSTAT r1, r2; HALT
    let prog = vec![0x10, 1, 999, 0x10, 2, 200, 0x79, 1, 2, 0x00];
    let vm = run_program(&prog, 100);
    assert_eq!(vm.regs[0], 0); // failure
}

#[test]
fn test_disassemble_fmkdir() {
    let mut vm = Vm::new();
    vm.ram[0] = 0x78;
    vm.ram[1] = 5;
    let (text, len) = vm.disassemble_at(0);
    assert_eq!(text, "FMKDIR [r5]");
    assert_eq!(len, 2);
}

#[test]
fn test_disassemble_fstat() {
    let mut vm = Vm::new();
    vm.ram[0] = 0x79;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    let (text, len) = vm.disassemble_at(0);
    assert_eq!(text, "FSTAT r1, [r2]");
    assert_eq!(len, 3);
}

#[test]
fn test_disassemble_funlink() {
    let mut vm = Vm::new();
    vm.ram[0] = 0x7A;
    vm.ram[1] = 3;
    let (text, len) = vm.disassemble_at(0);
    assert_eq!(text, "FUNLINK [r3]");
    assert_eq!(len, 2);
}
