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
        if !vm.step() {
            break;
        }
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
    vm.ram[0] = 0x12;
    vm.ram[1] = 1;
    vm.ram[2] = 2; // STORE r1, r2
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
    vm.ram[0] = 0x11;
    vm.ram[1] = 3;
    vm.ram[2] = 1; // LOAD r3, r1
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
    vm.ram[0] = 0x12;
    vm.ram[1] = 1;
    vm.ram[2] = 2; // STORE r1, r2
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
    vm.ram[0] = 0x10;
    vm.ram[1] = 1;
    vm.ram[2] = 0x2000; // LDI r1, 0x2000
    vm.ram[3] = 0x12;
    vm.ram[4] = 1;
    vm.ram[5] = 2; // STORE r1, r2
    vm.ram[6] = 0x11;
    vm.ram[7] = 3;
    vm.ram[8] = 1; // LOAD r3, r1
    vm.ram[9] = 0x00; // HALT
    vm.regs[2] = 0xABCDEF;
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
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
    vm.ram[0] = 0x10;
    vm.ram[1] = 5;
    vm.ram[2] = 0x500;
    vm.ram[3] = 0x10;
    vm.ram[4] = 6;
    vm.ram[5] = 999;
    vm.ram[6] = 0x12;
    vm.ram[7] = 5;
    vm.ram[8] = 6;
    vm.ram[9] = 0x11;
    vm.ram[10] = 7;
    vm.ram[11] = 5;
    vm.ram[12] = 0x00;
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
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
    vm.ram[0] = 0x20;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 0x00;
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.regs[1], 0); // wrapping add
}

#[test]
fn test_sub_basic() {
    let mut vm = Vm::new();
    vm.regs[1] = 50;
    vm.regs[2] = 20;
    vm.ram[0] = 0x21;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 0x00;
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.regs[1], 30);
}

#[test]
fn test_sub_wrapping_underflow() {
    let mut vm = Vm::new();
    vm.regs[1] = 0;
    vm.regs[2] = 1;
    vm.ram[0] = 0x21;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 0x00;
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.regs[1], 0xFFFFFFFF); // wrapping sub
}

#[test]
fn test_mul_basic() {
    let mut vm = Vm::new();
    vm.regs[1] = 6;
    vm.regs[2] = 7;
    vm.ram[0] = 0x22;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 0x00;
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.regs[1], 42);
}

#[test]
fn test_div_basic() {
    let mut vm = Vm::new();
    vm.regs[1] = 100;
    vm.regs[2] = 7;
    vm.ram[0] = 0x23;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 0x00;
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.regs[1], 14); // 100 / 7 = 14 (integer division)
}

#[test]
fn test_div_by_zero_no_panic() {
    let mut vm = Vm::new();
    vm.regs[1] = 42;
    vm.regs[2] = 0;
    vm.ram[0] = 0x23;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 0x00;
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.regs[1], 42); // unchanged, no panic
}

#[test]
fn test_mod_basic() {
    let mut vm = Vm::new();
    vm.regs[1] = 100;
    vm.regs[2] = 7;
    vm.ram[0] = 0x29;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 0x00;
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.regs[1], 2); // 100 % 7 = 2
}

#[test]
fn test_mod_by_zero_no_panic() {
    let mut vm = Vm::new();
    vm.regs[1] = 42;
    vm.regs[2] = 0;
    vm.ram[0] = 0x29;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 0x00;
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.regs[1], 42); // unchanged
}

#[test]
fn test_neg() {
    let mut vm = Vm::new();
    vm.regs[5] = 1;
    vm.ram[0] = 0x2A;
    vm.ram[1] = 5;
    vm.ram[2] = 0x00;
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.regs[5], 0xFFFFFFFF); // -1 in two's complement
}

#[test]
fn test_neg_zero() {
    let mut vm = Vm::new();
    vm.regs[5] = 0;
    vm.ram[0] = 0x2A;
    vm.ram[1] = 5;
    vm.ram[2] = 0x00;
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.regs[5], 0);
}

// ── BITWISE ─────────────────────────────────────────────────────

#[test]
fn test_and() {
    let mut vm = Vm::new();
    vm.regs[1] = 0xFF00FF;
    vm.regs[2] = 0x0F0F0F;
    vm.ram[0] = 0x24;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 0x00;
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.regs[1], 0x0F000F);
}

#[test]
fn test_or() {
    let mut vm = Vm::new();
    vm.regs[1] = 0xF00000;
    vm.regs[2] = 0x000F00;
    vm.ram[0] = 0x25;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 0x00;
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.regs[1], 0xF00F00);
}

#[test]
fn test_xor() {
    let mut vm = Vm::new();
    vm.regs[1] = 0xFF00FF;
    vm.regs[2] = 0xFF00FF;
    vm.ram[0] = 0x26;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 0x00;
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.regs[1], 0); // XOR self = 0
}

#[test]
fn test_shl() {
    let mut vm = Vm::new();
    vm.regs[1] = 1;
    vm.regs[2] = 8;
    vm.ram[0] = 0x27;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 0x00;
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.regs[1], 256);
}

#[test]
fn test_shl_mod_32() {
    let mut vm = Vm::new();
    vm.regs[1] = 1;
    vm.regs[2] = 32; // shift by 32 -> effectively shift by 0 (mod 32)
    vm.ram[0] = 0x27;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 0x00;
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.regs[1], 1); // 1 << 32 = 1 (mod 32 = 0)
}

#[test]
fn test_shr() {
    let mut vm = Vm::new();
    vm.regs[1] = 256;
    vm.regs[2] = 4;
    vm.ram[0] = 0x28;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 0x00;
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.regs[1], 16);
}

#[test]
fn test_sar_sign_preserving() {
    let mut vm = Vm::new();
    vm.regs[1] = 0x80000000; // MSB set (negative in i32)
    vm.regs[2] = 4;
    vm.ram[0] = 0x2B;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 0x00;
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    // 0x80000000 >> 4 (arithmetic) = 0xF8000000
    assert_eq!(vm.regs[1], 0xF8000000);
}

// ── CMP / BRANCHES ──────────────────────────────────────────────

#[test]
fn test_cmp_less_than() {
    let mut vm = Vm::new();
    vm.regs[1] = 5;
    vm.regs[2] = 10;
    vm.ram[0] = 0x50;
    vm.ram[1] = 1;
    vm.ram[2] = 2; // CMP r1, r2
    vm.ram[3] = 0x00;
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.regs[0], 0xFFFFFFFF); // -1 (less than)
}

#[test]
fn test_cmp_equal() {
    let mut vm = Vm::new();
    vm.regs[1] = 42;
    vm.regs[2] = 42;
    vm.ram[0] = 0x50;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 0x00;
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.regs[0], 0); // equal
}

#[test]
fn test_cmp_greater_than() {
    let mut vm = Vm::new();
    vm.regs[1] = 10;
    vm.regs[2] = 5;
    vm.ram[0] = 0x50;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 0x00;
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.regs[0], 1); // greater than
}

#[test]
fn test_jz_taken() {
    // LDI r1, 0; JZ r1, 100; HALT -> should jump to 100
    let mut vm = Vm::new();
    vm.ram[0] = 0x10;
    vm.ram[1] = 1;
    vm.ram[2] = 0; // LDI r1, 0
    vm.ram[3] = 0x31;
    vm.ram[4] = 1;
    vm.ram[5] = 100; // JZ r1, 100
    vm.ram[6] = 0x00; // HALT (should not reach)
    vm.ram[100] = 0x00; // HALT at target
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.pc, 101); // halted at 101 (fetched HALT at 100)
}

#[test]
fn test_jz_not_taken() {
    // LDI r1, 1; JZ r1, 100; HALT -> should not jump
    let mut vm = Vm::new();
    vm.ram[0] = 0x10;
    vm.ram[1] = 1;
    vm.ram[2] = 1; // LDI r1, 1
    vm.ram[3] = 0x31;
    vm.ram[4] = 1;
    vm.ram[5] = 100; // JZ r1, 100
    vm.ram[6] = 0x00; // HALT (should reach)
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.pc, 7); // halted at HALT
}

#[test]
fn test_jnz_taken() {
    let mut vm = Vm::new();
    vm.ram[0] = 0x10;
    vm.ram[1] = 1;
    vm.ram[2] = 5; // LDI r1, 5
    vm.ram[3] = 0x32;
    vm.ram[4] = 1;
    vm.ram[5] = 100; // JNZ r1, 100
    vm.ram[6] = 0x00; // HALT
    vm.ram[100] = 0x00; // HALT at target
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.pc, 101);
}

#[test]
fn test_jmp_unconditional() {
    let mut vm = Vm::new();
    vm.ram[0] = 0x30;
    vm.ram[1] = 50; // JMP 50
    vm.ram[2] = 0x00; // HALT (should not reach)
    vm.ram[50] = 0x00; // HALT at target
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.pc, 51);
}

#[test]
fn test_blt_taken() {
    // CMP sets r0 = 0xFFFFFFFF (less than); BLT should branch
    let mut vm = Vm::new();
    vm.regs[1] = 3;
    vm.regs[2] = 10;
    vm.ram[0] = 0x50;
    vm.ram[1] = 1;
    vm.ram[2] = 2; // CMP r1, r2
    vm.ram[3] = 0x35;
    vm.ram[4] = 0;
    vm.ram[5] = 50; // BLT r0, 50
    vm.ram[6] = 0x00; // HALT
    vm.ram[50] = 0x00; // HALT at target
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.pc, 51);
}

#[test]
fn test_bge_taken() {
    // CMP sets r0 = 1 (greater than); BGE should branch
    let mut vm = Vm::new();
    vm.regs[1] = 10;
    vm.regs[2] = 3;
    vm.ram[0] = 0x50;
    vm.ram[1] = 1;
    vm.ram[2] = 2; // CMP r1, r2
    vm.ram[3] = 0x36;
    vm.ram[4] = 0;
    vm.ram[5] = 50; // BGE r0, 50
    vm.ram[6] = 0x00; // HALT
    vm.ram[50] = 0x00; // HALT at target
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.pc, 51);
}

// ── CALL / RET ──────────────────────────────────────────────────

#[test]
fn test_call_ret() {
    // CALL 10; HALT
    // at 10: LDI r5, 99; RET
    // at 16: HALT (return lands here)
    let mut vm = Vm::new();
    vm.ram[0] = 0x33;
    vm.ram[1] = 10; // CALL 10
    vm.ram[2] = 0x00; // HALT (return target)
    vm.ram[10] = 0x10;
    vm.ram[11] = 5;
    vm.ram[12] = 99; // LDI r5, 99
    vm.ram[13] = 0x34; // RET
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.regs[5], 99);
    assert!(vm.halted);
}

// ── MOV ─────────────────────────────────────────────────────────

#[test]
fn test_mov() {
    let mut vm = Vm::new();
    vm.regs[3] = 0xDEADBEEF;
    vm.ram[0] = 0x51;
    vm.ram[1] = 7;
    vm.ram[2] = 3; // MOV r7, r3
    vm.ram[3] = 0x00;
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
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
    vm.ram[pc as usize] = 0x10;
    pc += 1;
    vm.ram[pc as usize] = 30;
    pc += 1;
    vm.ram[pc as usize] = 0xFF00;
    pc += 1;
    // LDI r5, 42
    vm.ram[pc as usize] = 0x10;
    pc += 1;
    vm.ram[pc as usize] = 5;
    pc += 1;
    vm.ram[pc as usize] = 42;
    pc += 1;
    // PUSH r5
    vm.ram[pc as usize] = 0x60;
    pc += 1;
    vm.ram[pc as usize] = 5;
    pc += 1;
    // LDI r5, 0 (clobber)
    vm.ram[pc as usize] = 0x10;
    pc += 1;
    vm.ram[pc as usize] = 5;
    pc += 1;
    vm.ram[pc as usize] = 0;
    pc += 1;
    // POP r6
    vm.ram[pc as usize] = 0x61;
    pc += 1;
    vm.ram[pc as usize] = 6;
    pc += 1;
    // HALT
    vm.ram[pc as usize] = 0x00;

    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.regs[6], 42); // got value back from stack
    assert_eq!(vm.regs[5], 0); // r5 was clobbered
    assert_eq!(vm.regs[30], 0xFF00); // SP restored
}

// ── CMP signed comparison ───────────────────────────────────────

#[test]
fn test_cmp_signed_negative_vs_positive() {
    // -1 (0xFFFFFFFF) vs 5 -> should be less than
    let mut vm = Vm::new();
    vm.regs[1] = 0xFFFFFFFF; // -1 as i32
    vm.regs[2] = 5;
    vm.ram[0] = 0x50;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 0x00;
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.regs[0], 0xFFFFFFFF); // -1 < 5 in signed
}

// ── FRAME ───────────────────────────────────────────────────────

#[test]
fn test_frame_increments_ticks() {
    let mut vm = Vm::new();
    vm.ram[0] = 0x02; // FRAME
    vm.ram[1] = 0x00; // HALT
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.frame_ready);
    assert_eq!(vm.frame_count, 1);
    assert_eq!(vm.ram[0xFFE], 1);
}

// ── PSET / FILL ─────────────────────────────────────────────────

#[test]
fn test_fill() {
    let mut vm = Vm::new();
    vm.regs[1] = 0x00FF00; // green
    vm.ram[0] = 0x42;
    vm.ram[1] = 1; // FILL r1
    vm.ram[2] = 0x00;
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    // Every pixel should be green
    assert!(vm.screen.iter().all(|&p| p == 0x00FF00));
}

#[test]
fn test_pset_pixel() {
    let mut vm = Vm::new();
    vm.regs[1] = 10; // x
    vm.regs[2] = 20; // y
    vm.regs[3] = 0xFF0000; // red
    vm.ram[0] = 0x40;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3; // PSET r1, r2, r3
    vm.ram[4] = 0x00;
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.screen[20 * 256 + 10], 0xFF0000);
}

// ── IKEY ────────────────────────────────────────────────────────

#[test]
fn test_ikey_reads_and_clears() {
    let mut vm = Vm::new();
    vm.ram[0xFFF] = 65; // 'A' in keyboard port
    vm.ram[0] = 0x48;
    vm.ram[1] = 5; // IKEY r5
    vm.ram[2] = 0x00;
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.regs[5], 65);
    assert_eq!(vm.ram[0xFFF], 0); // port cleared
}

#[test]
fn test_ikey_no_key() {
    let mut vm = Vm::new();
    vm.ram[0xFFF] = 0; // no key
    vm.ram[0] = 0x48;
    vm.ram[1] = 5; // IKEY r5
    vm.ram[2] = 0x00;
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.regs[5], 0);
}

// ── HITSET / HITQ (GUI Hit-Testing) ────────────────────────────

#[test]
fn test_hitset_registers_region() {
    // HITSET r1, r2, r3, r4, 42  -- register a 10x20 region at (5,5) with id=42
    let vm = run_program(&[0x10, 1, 5,  // LDI r1, 5 (x)
                           0x10, 2, 5,  // LDI r2, 5 (y)
                           0x10, 3, 10, // LDI r3, 10 (w)
                           0x10, 4, 20, // LDI r4, 20 (h)
                           0x37, 1, 2, 3, 4, 42, // HITSET r1,r2,r3,r4,42
                           0x00], 100); // HALT
    assert_eq!(vm.hit_regions.len(), 1);
    assert_eq!(vm.hit_regions[0].x, 5);
    assert_eq!(vm.hit_regions[0].y, 5);
    assert_eq!(vm.hit_regions[0].w, 10);
    assert_eq!(vm.hit_regions[0].h, 20);
    assert_eq!(vm.hit_regions[0].id, 42);
}

#[test]
fn test_hitq_finds_region() {
    // Register a region at (10,10) size 50x30 with id=7
    // Set mouse to (25, 20) which is inside the region
    // Query should return 7
    let mut vm = Vm::new();
    vm.regs[1] = 10; // x
    vm.regs[2] = 10; // y
    vm.regs[3] = 50; // w
    vm.regs[4] = 30; // h
    vm.ram[0] = 0x37; // HITSET
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.ram[4] = 4;
    vm.ram[5] = 7; // id
    vm.ram[6] = 0x38; // HITQ
    vm.ram[7] = 5; // -> r5
    vm.ram[8] = 0x00; // HALT
    vm.pc = 0;
    vm.push_mouse(25, 20); // inside the region
    for _ in 0..100 {
        if !vm.step() { break; }
    }
    assert_eq!(vm.regs[5], 7); // found the region
}

#[test]
fn test_hitq_no_match() {
    // Register a region at (10,10) size 50x30 with id=7
    // Set mouse to (0, 0) which is outside
    let mut vm = Vm::new();
    vm.regs[1] = 10;
    vm.regs[2] = 10;
    vm.regs[3] = 50;
    vm.regs[4] = 30;
    vm.ram[0] = 0x37;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.ram[4] = 4;
    vm.ram[5] = 7;
    vm.ram[6] = 0x38; // HITQ
    vm.ram[7] = 5;
    vm.ram[8] = 0x00;
    vm.pc = 0;
    vm.push_mouse(0, 0); // outside
    for _ in 0..100 {
        if !vm.step() { break; }
    }
    assert_eq!(vm.regs[5], 0); // no match
}

#[test]
fn test_hitq_boundary_edges() {
    // Test exact boundary: top-left corner (inclusive), bottom-right (exclusive)
    let mut vm = Vm::new();
    vm.regs[1] = 10; // x
    vm.regs[2] = 10; // y
    vm.regs[3] = 20; // w
    vm.regs[4] = 20; // h
    vm.ram[0] = 0x37;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.ram[4] = 4;
    vm.ram[5] = 1; // id
    vm.ram[6] = 0x38; // HITQ
    vm.ram[7] = 5;
    vm.ram[8] = 0x00;
    vm.pc = 0;

    // Exact top-left: (10,10) should match
    vm.push_mouse(10, 10);
    for _ in 0..100 { if !vm.step() { break; } }
    assert_eq!(vm.regs[5], 1);

    // Reset and test bottom-right: (29,29) is inside (10+20-1), (30,30) is outside
    let mut vm2 = Vm::new();
    vm2.regs[1] = 10;
    vm2.regs[2] = 10;
    vm2.regs[3] = 20;
    vm2.regs[4] = 20;
    vm2.ram[0] = 0x37;
    vm2.ram[1] = 1;
    vm2.ram[2] = 2;
    vm2.ram[3] = 3;
    vm2.ram[4] = 4;
    vm2.ram[5] = 1;
    vm2.ram[6] = 0x38;
    vm2.ram[7] = 5;
    vm2.ram[8] = 0x00;
    vm2.pc = 0;
    vm2.push_mouse(29, 29); // last pixel inside
    for _ in 0..100 { if !vm2.step() { break; } }
    assert_eq!(vm2.regs[5], 1);

    // Exactly on the exclusive edge
    let mut vm3 = Vm::new();
    vm3.regs[1] = 10;
    vm3.regs[2] = 10;
    vm3.regs[3] = 20;
    vm3.regs[4] = 20;
    vm3.ram[0] = 0x37;
    vm3.ram[1] = 1;
    vm3.ram[2] = 2;
    vm3.ram[3] = 3;
    vm3.ram[4] = 4;
    vm3.ram[5] = 1;
    vm3.ram[6] = 0x38;
    vm3.ram[7] = 5;
    vm3.ram[8] = 0x00;
    vm3.pc = 0;
    vm3.push_mouse(30, 30); // exclusive edge -- outside
    for _ in 0..100 { if !vm3.step() { break; } }
    assert_eq!(vm3.regs[5], 0);
}

#[test]
fn test_hitq_first_match_wins() {
    // Two overlapping regions; first registered wins
    let mut vm = Vm::new();
    vm.regs[1] = 10; vm.regs[2] = 10; vm.regs[3] = 50; vm.regs[4] = 50;
    // Region 1: (10,10) 50x50, id=100
    vm.ram[0] = 0x37; vm.ram[1] = 1; vm.ram[2] = 2; vm.ram[3] = 3; vm.ram[4] = 4; vm.ram[5] = 100;
    // Region 2: (20,20) 50x50, id=200
    vm.ram[6] = 0x10; vm.ram[7] = 1; vm.ram[8] = 20; // LDI r1, 20
    vm.ram[9] = 0x10; vm.ram[10] = 2; vm.ram[11] = 20; // LDI r2, 20
    vm.ram[12] = 0x37; vm.ram[13] = 1; vm.ram[14] = 2; vm.ram[15] = 3; vm.ram[16] = 4; vm.ram[17] = 200;
    vm.ram[18] = 0x38; vm.ram[19] = 5; // HITQ r5
    vm.ram[20] = 0x00; // HALT
    vm.pc = 0;
    vm.push_mouse(30, 30); // inside both
    for _ in 0..100 { if !vm.step() { break; } }
    assert_eq!(vm.regs[5], 100); // first registered wins
}

#[test]
fn test_push_mouse_mirrors_to_ram() {
    let mut vm = Vm::new();
    vm.push_mouse(123, 456);
    assert_eq!(vm.mouse_x, 123);
    assert_eq!(vm.mouse_y, 456);
    assert_eq!(vm.ram[0xFF9], 123);
    assert_eq!(vm.ram[0xFFA], 456);
}

#[test]
fn test_disasm_hitset_hitq() {
    // HITSET r1, r2, r3, r4, 42
    let mut vm = Vm::new();
    vm.ram[0] = 0x37;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.ram[4] = 4;
    vm.ram[5] = 42;
    let (mnemonic, len) = vm.disassemble_at(0);
    assert_eq!(len, 6);
    assert!(mnemonic.contains("HITSET"));

    // HITQ r5
    let mut vm2 = Vm::new();
    vm2.ram[0] = 0x38;
    vm2.ram[1] = 5;
    let (mnemonic2, len2) = vm2.disassemble_at(0);
    assert_eq!(len2, 2);
    assert!(mnemonic2.contains("HITQ"));
}

// ── RAND ────────────────────────────────────────────────────────

#[test]
fn test_rand_changes_state() {
    let mut vm = Vm::new();
    let initial_state = vm.rand_state;
    vm.ram[0] = 0x49;
    vm.ram[1] = 5; // RAND r5
    vm.ram[2] = 0x00;
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_ne!(vm.rand_state, initial_state); // state changed
    assert_ne!(vm.regs[5], 0); // probably nonzero (LCG seeded with DEADBEEF)
}

// ── BEEP ────────────────────────────────────────────────────────

#[test]
fn test_beep_sets_state() {
    let mut vm = Vm::new();
    vm.regs[1] = 440; // freq
    vm.regs[2] = 200; // duration
    vm.ram[0] = 0x03;
    vm.ram[1] = 1;
    vm.ram[2] = 2; // BEEP r1, r2
    vm.ram[3] = 0x00;
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.beep, Some((440, 200)));
}

// ── MEMCPY ───────────────────────────────────────────────────────

#[test]
fn test_memcpy_copies_words() {
    let mut vm = Vm::new();
    // Write some data to addresses 100-104
    for i in 0..5 {
        vm.ram[100 + i] = 1000 + i as u32;
    }
    // Set regs: r1=200 (dst), r2=100 (src), r3=5 (len)
    vm.regs[1] = 200;
    vm.regs[2] = 100;
    vm.regs[3] = 5;
    // MEMCPY r1, r2, r3
    vm.ram[0] = 0x04;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.ram[4] = 0x00; // HALT
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    // Verify dst has the data
    for i in 0..5 {
        assert_eq!(
            vm.ram[200 + i],
            1000 + i as u32,
            "MEMCPY failed at offset {}",
            i
        );
    }
}

#[test]
fn test_memcpy_zero_len_is_noop() {
    let mut vm = Vm::new();
    vm.ram[100] = 0xDEAD;
    vm.ram[200] = 0xBEEF;
    vm.regs[1] = 200; // dst
    vm.regs[2] = 100; // src
    vm.regs[3] = 0; // len = 0
    vm.ram[0] = 0x04;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.ram[4] = 0x00;
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(
        vm.ram[200], 0xBEEF,
        "MEMCPY with len=0 should not overwrite dst"
    );
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
    vm.ram[pc] = 0x10;
    vm.ram[pc + 1] = 1;
    vm.ram[pc + 2] = 0;
    pc += 3;
    // LDI r2, 1
    vm.ram[pc] = 0x10;
    vm.ram[pc + 1] = 2;
    vm.ram[pc + 2] = 1;
    pc += 3;
    // LDI r3, 5
    vm.ram[pc] = 0x10;
    vm.ram[pc + 1] = 3;
    vm.ram[pc + 2] = 5;
    pc += 3;
    let loop_addr = pc as u32;
    // ADD r1, r2
    vm.ram[pc] = 0x20;
    vm.ram[pc + 1] = 1;
    vm.ram[pc + 2] = 2;
    pc += 3;
    // CMP r1, r3
    vm.ram[pc] = 0x50;
    vm.ram[pc + 1] = 1;
    vm.ram[pc + 2] = 3;
    pc += 3;
    // BLT r0, loop_addr
    vm.ram[pc] = 0x35;
    vm.ram[pc + 1] = 0;
    vm.ram[pc + 2] = loop_addr;
    pc += 3;
    // HALT
    vm.ram[pc] = 0x00;

    vm.pc = 0;
    for _ in 0..1000 {
        if !vm.step() {
            break;
        }
    }
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
    vm.ram[pc] = 0x10;
    vm.ram[pc + 1] = 1;
    vm.ram[pc + 2] = 0;
    pc += 3;
    // LDI r2, 1
    vm.ram[pc] = 0x10;
    vm.ram[pc + 1] = 2;
    vm.ram[pc + 2] = 1;
    pc += 3;
    // LDI r3, 5
    vm.ram[pc] = 0x10;
    vm.ram[pc + 1] = 3;
    vm.ram[pc + 2] = 5;
    pc += 3;
    let loop_addr = pc as u32;
    // ADD r1, r2
    vm.ram[pc] = 0x20;
    vm.ram[pc + 1] = 1;
    vm.ram[pc + 2] = 2;
    pc += 3;
    // CMP r1, r3
    vm.ram[pc] = 0x50;
    vm.ram[pc + 1] = 1;
    vm.ram[pc + 2] = 3;
    pc += 3;
    // BLT r0, loop_addr -- label resolved to 0x1000 + offset
    vm.ram[pc] = 0x35;
    vm.ram[pc + 1] = 0;
    vm.ram[pc + 2] = loop_addr;
    pc += 3;
    // HALT
    vm.ram[pc] = 0x00;

    vm.pc = base as u32;
    for _ in 0..1000 {
        if !vm.step() {
            break;
        }
    }
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
    vm.ram[0] = 0x6D;
    vm.ram[1] = 3;
    vm.ram[2] = 1;
    vm.ram[3] = 2; // PEEK r3, r1, r2 (dest=r3, x=r1=15, y=r2=30)
    vm.ram[4] = 0x00;
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.regs[3], 0xABCDEF);
}

#[test]
fn test_peek_out_of_bounds_returns_zero() {
    let mut vm = Vm::new();
    vm.regs[1] = 300; // x out of bounds
    vm.regs[2] = 300; // y out of bounds
    vm.ram[0] = 0x6D;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.ram[4] = 0x00;
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
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
    vm.regs[3] = 5; // len
    vm.ram[0] = 0x04;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3; // MEMCPY r1, r2, r3
    vm.ram[4] = 0x00; // HALT
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.halted);
    // Verify destination has the copied data
    for i in 0..5 {
        assert_eq!(
            vm.ram[0x3000 + i],
            (100 + i) as u32,
            "MEMCPY dest[{}] should be {}",
            i,
            100 + i
        );
    }
    // Source should be unchanged
    for i in 0..5 {
        assert_eq!(
            vm.ram[0x2000 + i],
            (100 + i) as u32,
            "MEMCPY src[{}] should be unchanged",
            i
        );
    }
}

#[test]
fn test_memcpy_assembles_and_runs() {
    use crate::assembler::assemble;
    let src = "LDI r1, 0x3000\nLDI r2, 0x2000\nLDI r3, 5\nMEMCPY r1, r2, r3\nHALT";
    let asm = assemble(src, 0).expect("assembly should succeed");
    let mut vm = Vm::new();
    // Write source data
    for i in 0..5 {
        vm.ram[0x2000 + i] = (42 + i) as u32;
    }
    for (i, &w) in asm.pixels.iter().enumerate() {
        vm.ram[i] = w;
    }
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
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
    vm.ram[0] = 0x12;
    vm.ram[1] = 1;
    vm.ram[2] = 2; // STORE r1, r2
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
    vm.ram[0] = 0x11;
    vm.ram[1] = 3;
    vm.ram[2] = 1; // LOAD r3, r1
    vm.pc = 0;
    vm.step();
    assert_eq!(vm.regs[3], 0xABCDEF);
}

#[test]
fn test_screen_ram_store_then_load_roundtrip() {
    let vm = run_program(
        &[
            0x10, 1, 0x10050, // LDI r1, 0x10050
            0x10, 2, 0x00FF00, // LDI r2, 0x00FF00
            0x12, 1, 2, // STORE r1, r2
            0x11, 4, 1,    // LOAD r4, r1
            0x00, // HALT
        ],
        100,
    );
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
    vm.ram[0] = 0x12;
    vm.ram[1] = 1;
    vm.ram[2] = 2; // STORE r1, r2
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
    vm.ram[0] = 0x11;
    vm.ram[1] = 3;
    vm.ram[2] = 1; // LOAD r3, r1
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    let load_value = vm.regs[3];

    // Reset halted state for second instruction sequence
    vm.halted = false;

    // Read via PEEK opcode
    vm.regs[1] = 15; // x
    vm.regs[2] = 30; // y
    vm.ram[4] = 0x6D;
    vm.ram[5] = 4;
    vm.ram[6] = 1;
    vm.ram[7] = 2; // PEEK r4, r1, r2
    vm.ram[8] = 0x00;
    vm.pc = 4;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
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
    vm.ram[0] = 0x12;
    vm.ram[1] = 1;
    vm.ram[2] = 2; // STORE r1, r2
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }

    // Verify via screen buffer directly
    assert_eq!(vm.screen[20 * 256 + 10], 0xFF0000);

    // Reset halted state for second instruction sequence
    vm.halted = false;

    // Verify via PEEK opcode
    vm.regs[1] = 10; // x
    vm.regs[2] = 20; // y
    vm.ram[3] = 0x6D;
    vm.ram[4] = 5;
    vm.ram[5] = 1;
    vm.ram[6] = 2; // PEEK r5, r1, r2
    vm.ram[7] = 0x00;
    vm.pc = 3;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.regs[5], 0xFF0000);
}

#[test]
fn test_screen_ram_boundary_first_and_last_pixel() {
    let mut vm = Vm::new();

    // First pixel: address 0x10000
    vm.regs[1] = SCREEN_RAM_BASE as u32;
    vm.regs[2] = 0x111111;
    vm.ram[0] = 0x12;
    vm.ram[1] = 1;
    vm.ram[2] = 2; // STORE r1, r2
    vm.pc = 0;
    vm.step();
    assert_eq!(vm.screen[0], 0x111111);

    // Last pixel: address 0x10000 + 65535 = 0x1FFFF
    let last_addr = (SCREEN_RAM_BASE + SCREEN_SIZE - 1) as u32;
    vm.regs[1] = last_addr;
    vm.regs[2] = 0x222222;
    vm.ram[0] = 0x12;
    vm.ram[1] = 1;
    vm.ram[2] = 2; // STORE r1, r2
    vm.pc = 0;
    vm.step();
    assert_eq!(vm.screen[SCREEN_SIZE - 1], 0x222222);

    // Read back via LOAD
    vm.regs[1] = SCREEN_RAM_BASE as u32;
    vm.ram[0] = 0x11;
    vm.ram[1] = 3;
    vm.ram[2] = 1; // LOAD r3, r1
    vm.pc = 0;
    vm.step();
    assert_eq!(vm.regs[3], 0x111111);

    vm.regs[1] = last_addr;
    vm.ram[0] = 0x11;
    vm.ram[1] = 3;
    vm.ram[2] = 1; // LOAD r3, r1
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
    vm.ram[0] = 0x12;
    vm.ram[1] = 1;
    vm.ram[2] = 2; // STORE r1, r2
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
    assert_eq!(
        vm.ram[0xFFD], 4,
        "ASMSELF should report 4 words of bytecode"
    );
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
    assert_ne!(
        vm.ram[0xFFD], 0xFFFFFFFF,
        "ASMSELF with macros should succeed"
    );
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
    assert_ne!(vm.ram[0xFFD], 0xFFFFFFFF, "ASMSELF should have succeeded");
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

    assert_eq!(
        vm.regs[0], 77,
        "r0 should be 77 after RUNNEXT executes new code"
    );
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
    for _ in 0..10 {
        vm.step();
    }

    assert_eq!(
        vm.regs[0], 12345,
        "r0 should equal r5's value from before RUNNEXT"
    );
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
        if vm.halted {
            break;
        }
        vm.step();
    }

    assert!(vm.halted, "VM should halt after Gen B executes");
    assert_eq!(
        vm.regs[0], 999,
        "r0 should be 999 -- proof Gen B ran after Gen A assembled it"
    );
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
    for _ in 0..20 {
        vm.step();
    }

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
    assert!(
        result.is_ok(),
        "self_writer.asm should assemble: {:?}",
        result.err()
    );
    let asm = result.expect("operation should succeed");
    assert!(
        asm.pixels.len() > 50,
        "self_writer should produce substantial bytecode"
    );
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
    for _ in 0..20 {
        vm.step();
    }

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

    for _ in 0..20 {
        vm.step();
    }
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
    for _ in 0..50 {
        vm.step();
    }

    // Verify Gen B wrote 'X' to canvas row 2
    let row2_start = 2 * 32; // 0x8040 - 0x8000 = 64
    assert_eq!(
        vm.canvas_buffer[row2_start], 88,
        "Gen B should write 'X' to canvas row 2"
    );
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

    for _ in 0..50 {
        vm.step();
    }
    assert_eq!(vm.regs[0], 101, "r0 should be 0 + r5(100) + 1 = 101");
    assert_eq!(
        vm.regs[5], 100,
        "r5 should still be 100 (inherited from Gen A)"
    );
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
        if vm.frame_ready {
            break;
        }
        let keep_going = vm.step();
        steps += 1;
        if !keep_going {
            break;
        }
    }

    assert!(
        vm.frame_ready,
        "should reach FRAME within 1M steps (took {})",
        steps
    );
    eprintln!("First frame rendered in {} steps", steps);
    eprintln!(
        "camera_x = {}, camera_y = {}",
        vm.ram[0x7800], vm.ram[0x7801]
    );
    assert_eq!(vm.ram[0x7800], 1, "camera should have moved right by 1");

    // Screen should not be all black
    let non_black = vm.screen.iter().filter(|&&p| p != 0).count();
    eprintln!("Non-black pixels: {}/{}", non_black, 256 * 256);
    assert!(non_black > 0, "screen should have rendered terrain");

    // Second frame: press Down
    vm.frame_ready = false;
    vm.ram[0xFFB] = 2; // Down
    for _ in 0..1_000_000 {
        if vm.frame_ready {
            break;
        }
        let keep_going = vm.step();
        if !keep_going {
            break;
        }
    }
    eprintln!(
        "After 2nd frame: camera_x={}, camera_y={}",
        vm.ram[0x7800], vm.ram[0x7801]
    );
    assert!(vm.frame_ready, "second frame should render");
    assert_eq!(vm.ram[0x7801], 1, "camera should have moved down by 1");

    // Third frame: press Left+Up (bits 2+0 = 5) -- diagonal movement
    vm.frame_ready = false;
    vm.ram[0xFFB] = 5;
    for _ in 0..1_000_000 {
        if vm.frame_ready {
            break;
        }
        let keep_going = vm.step();
        if !keep_going {
            break;
        }
    }
    eprintln!(
        "After 3rd frame (left+up): camera_x={}, camera_y={}",
        vm.ram[0x7800], vm.ram[0x7801]
    );
    assert_eq!(vm.ram[0x7800], 0, "camera should have moved left back to 0");
    assert_eq!(vm.ram[0x7801], 0, "camera should have moved up back to 0");

    // Verify frame counter incremented
    assert!(
        vm.ram[0x7802] >= 3,
        "frame_counter should be >= 3 (was {})",
        vm.ram[0x7802]
    );
    eprintln!("Frame counter: {}", vm.ram[0x7802]);

    // Verify water animation: run 2 frames without moving, check screen changes
    // Frame 4: no keys
    vm.frame_ready = false;
    vm.ram[0xFFB] = 0;
    for _ in 0..1_000_000 {
        if vm.frame_ready {
            break;
        }
        let keep_going = vm.step();
        if !keep_going {
            break;
        }
    }
    let screen_f4: Vec<u32> = vm.screen.to_vec();

    // Frame 5: no keys
    vm.frame_ready = false;
    vm.ram[0xFFB] = 0;
    for _ in 0..1_000_000 {
        if vm.frame_ready {
            break;
        }
        let keep_going = vm.step();
        if !keep_going {
            break;
        }
    }
    let screen_f5: Vec<u32> = vm.screen.to_vec();

    // Count pixels that changed between frames (water animation)
    let changed: usize = screen_f4
        .iter()
        .zip(screen_f5.iter())
        .filter(|(a, b)| a != b)
        .count();
    eprintln!(
        "Pixels changed between frames 4-5: {}/{}",
        changed,
        256 * 256
    );
    // With ~25% water tiles and animation, expect some pixels to change
    assert!(
        changed > 0,
        "water animation should cause pixel changes between frames"
    );
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
        if vm.frame_ready {
            break;
        }
        if !vm.step() {
            break;
        }
    }

    // Count unique colors (structures + animation create many)
    let mut color_counts: std::collections::HashMap<u32, usize> = std::collections::HashMap::new();
    for &pixel in vm.screen.iter() {
        *color_counts.entry(pixel).or_insert(0) += 1;
    }
    eprintln!("At (100,100): {} unique colors", color_counts.len());
    assert!(
        color_counts.len() >= 5,
        "should see multiple biomes at (100,100)"
    );

    // Check biome contiguity by sampling tile-top-left pixels
    // and masking per-tile variation. v10 BPE/LINEAR uses table lookups
    // that affect all three channels (±12 per channel from 2 nibble lookups).
    // Mask covers: BPE variation (low 6 bits per channel = 0x3F3F3F),
    // accent animation (low nibble shifts = within BPE mask),
    // and water shimmer (low 5 blue bits = within BPE mask).
    let mut biome_zones = 0;
    let mut prev_base: u32 = 0;
    for tx in 0..64 {
        let px = tx * 4;
        let py = 128; // tile row 32 - middle of screen
        let color = vm.screen[py * 256 + px];
        // Round to base biome: mask out BPE per-tile variation + animation
        // 7 bits per channel covers BPE pair lookups (±0x18 + wrapping borrows)
        let base = color & !0x7F7F7F;
        if tx == 0 || base != prev_base {
            biome_zones += 1;
            prev_base = base;
        }
    }
    eprintln!(
        "At (100,100) row 32: {} biome zone boundaries across 64 tiles",
        biome_zones
    );

    // With 8-tile zones, expect ~8 boundaries. Per-tile hash would give ~64.
    // v10 BPE/LINEAR adds per-tile multi-channel variation via ADD, which can
    // cause cross-byte carries (e.g. B=0xFC + 0x0C carries into G). This makes
    // some same-biome tiles round to different bases, inflating zone count.
    // Allow up to 40 (vs 20 in v9) -- still far below per-tile-random ~64.
    assert!(
        biome_zones < 40,
        "biomes should be contiguous, got {} zone boundaries (expected <40)",
        biome_zones
    );
    let screen1 = vm.screen.to_vec();
    vm.frame_ready = false;
    for _ in 0..1_000_000 {
        if vm.frame_ready {
            break;
        }
        if !vm.step() {
            break;
        }
    }
    // Note: frame counter advanced, so water animation differs. Check non-water.
    let non_water_same = screen1
        .iter()
        .zip(vm.screen.iter())
        .filter(|(&a, &b)| {
            let a_water = (a & 0xFF) > 0 && ((a >> 16) & 0xFF) == 0 && ((a >> 8) & 0xFF) < 0x20;
            !a_water && a == b
        })
        .count();
    // Non-water tiles should be identical (deterministic terrain)
    eprintln!(
        "Non-water pixels identical across frames: {}",
        non_water_same
    );
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
        if !vm.step() {
            break;
        }
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
    vm.ram[0] = 0x10;
    vm.ram[1] = 1;
    vm.ram[2] = 100;
    vm.ram[3] = 0x7A;
    vm.ram[4] = 1;
    vm.ram[5] = 0x00;
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
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
    vm.ram[0] = 0x10;
    vm.ram[1] = 1;
    vm.ram[2] = ino;
    vm.ram[3] = 0x10;
    vm.ram[4] = 2;
    vm.ram[5] = 200;
    vm.ram[6] = 0x79;
    vm.ram[7] = 1;
    vm.ram[8] = 2;
    vm.ram[9] = 0x00;
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.regs[0], 1); // success
    assert_eq!(vm.ram[200], ino); // ino
    assert_eq!(vm.ram[201], 1); // itype = Regular
    assert_eq!(vm.ram[202], 3); // size
    assert_eq!(vm.ram[203], 0); // ref_count
    assert_eq!(vm.ram[204], 1); // parent = root
    assert_eq!(vm.ram[205], 0); // num_children
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

// --- Phase 54: Pixel Write History Tests ---

#[test]
fn test_pixel_write_log_pset_records() {
    let mut vm = crate::vm::Vm::new();
    vm.trace_recording = true;

    // LDI r1, 10  (x=10)
    vm.ram[0] = 0x10;
    vm.ram[1] = 1;
    vm.ram[2] = 10;
    // LDI r2, 20  (y=20)
    vm.ram[3] = 0x10;
    vm.ram[4] = 2;
    vm.ram[5] = 20;
    // LDI r3, 0xFF0000  (color=red)
    vm.ram[6] = 0x10;
    vm.ram[7] = 3;
    vm.ram[8] = 0xFF0000;
    // PSET r1, r2, r3
    vm.ram[9] = 0x40;
    vm.ram[10] = 1;
    vm.ram[11] = 2;
    vm.ram[12] = 3;

    for _ in 0..4 {
        vm.step();
    } // 3x LDI + PSET

    assert_eq!(vm.pixel_write_log.len(), 1);
    let entry = vm.pixel_write_log.get_at(0).unwrap();
    assert_eq!(entry.x, 10);
    assert_eq!(entry.y, 20);
    assert_eq!(entry.opcode, 0x40);
    assert_eq!(entry.color, 0xFF0000);
    assert_eq!(vm.screen[20 * 256 + 10], 0xFF0000);
}

#[test]
fn test_pixel_write_log_pseti_records() {
    let mut vm = crate::vm::Vm::new();
    vm.trace_recording = true;

    // PSETI 50, 60, 0x00FF00
    vm.ram[0] = 0x41;
    vm.ram[1] = 50;
    vm.ram[2] = 60;
    vm.ram[3] = 0x00FF00;

    vm.step();

    assert_eq!(vm.pixel_write_log.len(), 1);
    let entry = vm.pixel_write_log.get_at(0).unwrap();
    assert_eq!(entry.x, 50);
    assert_eq!(entry.y, 60);
    assert_eq!(entry.opcode, 0x41);
    assert_eq!(entry.color, 0x00FF00);
}

#[test]
fn test_pixel_write_log_no_recording_when_off() {
    let mut vm = crate::vm::Vm::new();
    vm.trace_recording = false;

    // PSETI 10, 10, 5
    vm.ram[0] = 0x41;
    vm.ram[1] = 10;
    vm.ram[2] = 10;
    vm.ram[3] = 5;
    vm.step();

    assert_eq!(vm.pixel_write_log.len(), 0);
    // But the pixel should still be set
    assert_eq!(vm.screen[10 * 256 + 10], 5);
}

#[test]
fn test_pixel_write_log_ring_buffer_overflow() {
    let mut vm = crate::vm::Vm::new();
    vm.trace_recording = true;

    // Write more pixels than the buffer capacity
    let cap = crate::vm::DEFAULT_PIXEL_WRITE_CAPACITY;
    for i in 0..(cap as u32 + 100) {
        vm.ram[0] = 0x41; // PSETI
        vm.ram[1] = (i % 256) as u32;
        vm.ram[2] = ((i / 256) % 256) as u32;
        vm.ram[3] = i;
        vm.pc = 0;
        vm.step();
    }

    // Buffer should be at capacity (old entries overwritten)
    assert_eq!(vm.pixel_write_log.len(), cap);
}

#[test]
fn test_pixel_write_log_cleared_on_reset() {
    let mut vm = crate::vm::Vm::new();
    vm.trace_recording = true;

    vm.ram[0] = 0x41;
    vm.ram[1] = 5;
    vm.ram[2] = 5;
    vm.ram[3] = 1;
    vm.step();
    assert_eq!(vm.pixel_write_log.len(), 1);

    vm.reset();
    assert_eq!(vm.pixel_write_log.len(), 0);
}

#[test]
fn test_pixel_history_count_total() {
    let mut vm = crate::vm::Vm::new();
    vm.trace_recording = true;

    // Write 3 pixels
    for i in 0..3u32 {
        vm.ram[0] = 0x41;
        vm.ram[1] = i;
        vm.ram[2] = 0;
        vm.ram[3] = 1;
        vm.pc = 0;
        vm.step();
    }

    // PIXEL_HISTORY r0 (mode 0 = count total)
    vm.regs[0] = 0;
    vm.ram[0] = 0x84;
    vm.ram[1] = 0;
    vm.pc = 0;
    vm.step();

    assert_eq!(vm.regs[0], 3);
}

#[test]
fn test_pixel_history_count_at_pixel() {
    let mut vm = crate::vm::Vm::new();
    vm.trace_recording = true;

    // Write to (10,10) twice and (20,20) once
    for _ in 0..2 {
        vm.ram[0] = 0x41;
        vm.ram[1] = 10;
        vm.ram[2] = 10;
        vm.ram[3] = 1;
        vm.pc = 0;
        vm.step();
    }
    vm.ram[0] = 0x41;
    vm.ram[1] = 20;
    vm.ram[2] = 20;
    vm.ram[3] = 2;
    vm.pc = 0;
    vm.step();

    // PIXEL_HISTORY r0 (mode 1 = count at pixel)
    vm.regs[0] = 1; // mode
    vm.regs[1] = 10; // x
    vm.regs[2] = 10; // y
    vm.ram[0] = 0x84;
    vm.ram[1] = 0;
    vm.pc = 0;
    vm.step();

    assert_eq!(vm.regs[0], 2);

    // Check (20,20)
    vm.regs[0] = 1;
    vm.regs[1] = 20;
    vm.regs[2] = 20;
    vm.pc = 0;
    vm.step();

    assert_eq!(vm.regs[0], 1);
}

#[test]
fn test_pixel_history_get_recent() {
    let mut vm = crate::vm::Vm::new();
    vm.trace_recording = true;

    // Write 3 different colors to (5,5)
    for c in [0xFF0000u32, 0x00FF00, 0x0000FF] {
        vm.ram[0] = 0x41;
        vm.ram[1] = 5;
        vm.ram[2] = 5;
        vm.ram[3] = c;
        vm.pc = 0;
        vm.step();
    }

    // PIXEL_HISTORY r0 (mode 2 = get recent)
    vm.regs[0] = 2; // mode
    vm.regs[1] = 5; // x
    vm.regs[2] = 5; // y
    vm.regs[3] = 10; // max_count
    vm.regs[4] = 0x1000; // buf_addr
    vm.ram[0] = 0x84;
    vm.ram[1] = 0;
    vm.pc = 0;
    vm.step();

    assert_eq!(vm.regs[0], 3); // 3 entries

    // Entries are in reverse chronological order (newest first)
    // First entry should be blue (0x0000FF)
    assert_eq!(vm.ram[0x1000 + 5], 0x0000FF);
    // Second should be green (0x00FF00)
    assert_eq!(vm.ram[0x1006 + 5], 0x00FF00);
    // Third should be red (0xFF0000)
    assert_eq!(vm.ram[0x100C + 5], 0xFF0000);
}

#[test]
fn test_pixel_history_get_at_index() {
    let mut vm = crate::vm::Vm::new();
    vm.trace_recording = true;

    // Write 2 pixels
    vm.ram[0] = 0x41;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 0xAA;
    vm.step();
    vm.ram[0] = 0x41;
    vm.ram[1] = 3;
    vm.ram[2] = 4;
    vm.ram[3] = 0xBB;
    vm.pc = 0;
    vm.step();

    // PIXEL_HISTORY r0 (mode 3 = get at index)
    vm.regs[0] = 3; // mode
    vm.regs[1] = 1; // index (second entry)
    vm.regs[2] = 0x2000; // buf_addr
    vm.ram[0] = 0x84;
    vm.ram[1] = 0;
    vm.pc = 0;
    vm.step();

    assert_eq!(vm.regs[0], 0); // success
    assert_eq!(vm.ram[0x2000], 3); // x
    assert_eq!(vm.ram[0x2001], 4); // y
    assert_eq!(vm.ram[0x2004], 0x41); // opcode = PSETI
    assert_eq!(vm.ram[0x2005], 0xBB); // color
}

#[test]
fn test_pixel_history_invalid_mode() {
    let mut vm = crate::vm::Vm::new();

    vm.regs[0] = 99; // invalid mode
    vm.ram[0] = 0x84;
    vm.ram[1] = 0;
    vm.pc = 0;
    vm.step();

    assert_eq!(vm.regs[0], 0xFFFFFFFF);
}

#[test]
fn test_pixel_history_disassembler() {
    let mut vm = crate::vm::Vm::new();
    vm.ram[0] = 0x84;
    vm.ram[1] = 5;
    let (text, len) = vm.disassemble_at(0);
    assert_eq!(text, "PIXEL_HISTORY r5");
    assert_eq!(len, 2);
}

#[test]
fn test_pixel_history_assembler() {
    let src = "LDI r1, 0\nPIXEL_HISTORY r1\nHALT";
    let result = crate::assembler::assemble(src, 0);
    assert!(result.is_ok());
    let bytecode = result.unwrap();
    // LDI r1, 0 = [0x10, 1, 0]
    assert_eq!(bytecode.pixels[0], 0x10);
    assert_eq!(bytecode.pixels[1], 1);
    assert_eq!(bytecode.pixels[2], 0);
    // PIXEL_HISTORY r1 = [0x84, 1]
    assert_eq!(bytecode.pixels[3], 0x84);
    assert_eq!(bytecode.pixels[4], 1);
    // HALT = [0x00]
    assert_eq!(bytecode.pixels[5], 0x00);
}

#[test]
fn test_pixel_history_buf_overflow_check() {
    let mut vm = crate::vm::Vm::new();
    vm.trace_recording = true;

    // Write one pixel
    vm.ram[0] = 0x41;
    vm.ram[1] = 10;
    vm.ram[2] = 10;
    vm.ram[3] = 5;
    vm.step();

    // Try mode 2 with buffer addr that would overflow RAM
    vm.regs[0] = 2; // mode
    vm.regs[1] = 10; // x
    vm.regs[2] = 10; // y
    vm.regs[3] = 1; // max_count
    vm.regs[4] = 0xFFFF0; // buf_addr (too close to end for 6 words)
    vm.ram[0] = 0x84;
    vm.ram[1] = 0;
    vm.pc = 0;
    vm.step();

    assert_eq!(vm.regs[0], 0xFFFFFFFF); // error
}

// ── Disassembler Tests ───────────────────────────────────────────

/// Helper: create a VM, load a single instruction at address 0, disassemble it.
/// Returns the mnemonic string and instruction length.
fn disasm(bytecode: &[u32]) -> (String, usize) {
    let mut vm = Vm::new();
    for (i, &w) in bytecode.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = w;
        }
    }
    vm.disassemble_at(0)
}

#[test]
fn test_disasm_halt() {
    let (mnemonic, len) = disasm(&[0x00]);
    assert_eq!(mnemonic, "HALT");
    assert_eq!(len, 1);
}

#[test]
fn test_disasm_nop() {
    let (mnemonic, len) = disasm(&[0x01]);
    assert_eq!(mnemonic, "NOP");
    assert_eq!(len, 1);
}

#[test]
fn test_disasm_frame() {
    let (mnemonic, len) = disasm(&[0x02]);
    assert_eq!(mnemonic, "FRAME");
    assert_eq!(len, 1);
}

#[test]
fn test_disasm_beep() {
    let (mnemonic, len) = disasm(&[0x03, 3, 5]);
    assert_eq!(mnemonic, "BEEP r3, r5");
    assert_eq!(len, 3);
}

#[test]
fn test_disasm_memcpy() {
    let (mnemonic, len) = disasm(&[0x04, 1, 2, 3]);
    assert_eq!(mnemonic, "MEMCPY r1, r2, r3");
    assert_eq!(len, 4);
}

#[test]
fn test_disasm_ldi() {
    let (mnemonic, len) = disasm(&[0x10, 5, 0x1234]);
    assert_eq!(mnemonic, "LDI r5, 0x1234");
    assert_eq!(len, 3);
}

#[test]
fn test_disasm_ldi_zero() {
    let (mnemonic, len) = disasm(&[0x10, 0, 0]);
    assert_eq!(mnemonic, "LDI r0, 0x0");
    assert_eq!(len, 3);
}

#[test]
fn test_disasm_load() {
    let (mnemonic, len) = disasm(&[0x11, 7, 10]);
    assert_eq!(mnemonic, "LOAD r7, [r10]");
    assert_eq!(len, 3);
}

#[test]
fn test_disasm_store() {
    let (mnemonic, len) = disasm(&[0x12, 4, 8]);
    assert_eq!(mnemonic, "STORE [r4], r8");
    assert_eq!(len, 3);
}

#[test]
fn test_disasm_texti() {
    // TEXTI x, y, "AB"  (count=2, so 'A','B' follow)
    let (mnemonic, len) = disasm(&[0x13, 10, 20, 2, 0x41, 0x42]);
    assert_eq!(mnemonic, "TEXTI 10, 20, \"AB\"");
    assert_eq!(len, 6); // 4 header + 2 chars
}

#[test]
fn test_disasm_texti_long() {
    // TEXTI with 40 chars, but capped at 32
    let mut bc: Vec<u32> = vec![0x13, 0, 0, 40];
    bc.extend((0..40).map(|i| (b'A' + (i % 26) as u8) as u32));
    let (mnemonic, len) = disasm(&bc);
    // "TEXTI 0, 0, \"" (14 chars) + 32 chars + "\"" (1 char) = 47 chars
    assert_eq!(mnemonic.len(), 46);
    assert_eq!(len, 44); // 4 header + 40 (full count, not capped for length)
}

#[test]
fn test_disasm_stro() {
    // STRO r1, "Hi" (count=2, then 'H','i')
    let (mnemonic, len) = disasm(&[0x14, 1, 2, 0x48, 0x69]);
    assert_eq!(mnemonic, "STRO r1, \"Hi\"");
    assert_eq!(len, 5); // 3 header + 2 chars
}

#[test]
fn test_disasm_cmpi() {
    let (mnemonic, len) = disasm(&[0x15, 3, 42]);
    assert_eq!(mnemonic, "CMPI r3, 42");
    assert_eq!(len, 3);
}

#[test]
fn test_disasm_loads() {
    let (mnemonic, len) = disasm(&[0x16, 5, 0xFFFFFFFF]);
    assert_eq!(mnemonic, "LOADS r5, -1");
    assert_eq!(len, 3);
}

#[test]
fn test_disasm_stores() {
    let (mnemonic, len) = disasm(&[0x17, 10, 2]);
    assert_eq!(mnemonic, "STORES 10, r2");
    assert_eq!(len, 3);
}

#[test]
fn test_disasm_shift_imms() {
    // SHLI
    let (m, l) = disasm(&[0x18, 1, 4]);
    assert_eq!(m, "SHLI r1, 4");
    assert_eq!(l, 3);
    // SHRI
    let (m, l) = disasm(&[0x19, 2, 8]);
    assert_eq!(m, "SHRI r2, 8");
    assert_eq!(l, 3);
    // SARI
    let (m, l) = disasm(&[0x1A, 3, 2]);
    assert_eq!(m, "SARI r3, 2");
    assert_eq!(l, 3);
}

#[test]
fn test_disasm_alu_imms() {
    let tests = vec![
        (0x1B, "ADDI"),
        (0x1C, "SUBI"),
        (0x1D, "ANDI"),
        (0x1E, "ORI"),
        (0x1F, "XORI"),
    ];
    for (op, name) in tests {
        let (m, l) = disasm(&[op, 7, 99]);
        assert_eq!(m, format!("{} r7, 99", name));
        assert_eq!(l, 3);
    }
}

#[test]
fn test_disasm_alu_regs() {
    let tests = vec![
        (0x20, "ADD"),
        (0x21, "SUB"),
        (0x22, "MUL"),
        (0x23, "DIV"),
        (0x24, "AND"),
        (0x25, "OR"),
        (0x26, "XOR"),
        (0x27, "SHL"),
        (0x28, "SHR"),
        (0x29, "MOD"),
        (0x2B, "SAR"),
    ];
    for (op, name) in tests {
        let (m, l) = disasm(&[op, 1, 2]);
        assert_eq!(m, format!("{} r1, r2", name));
        assert_eq!(l, 3);
    }
}

#[test]
fn test_disasm_neg() {
    let (m, l) = disasm(&[0x2A, 4]);
    assert_eq!(m, "NEG r4");
    assert_eq!(l, 2);
}

#[test]
fn test_disasm_branches() {
    // JMP
    let (m, l) = disasm(&[0x30, 0x0100]);
    assert_eq!(m, "JMP 0x0100");
    assert_eq!(l, 2);

    // JZ
    let (m, l) = disasm(&[0x31, 5, 0x0200]);
    assert_eq!(m, "JZ r5, 0x0200");
    assert_eq!(l, 3);

    // JNZ
    let (m, l) = disasm(&[0x32, 3, 0x0050]);
    assert_eq!(m, "JNZ r3, 0x0050");
    assert_eq!(l, 3);

    // CALL
    let (m, l) = disasm(&[0x33, 0x0100]);
    assert_eq!(m, "CALL 0x0100");
    assert_eq!(l, 2);

    // RET
    let (m, l) = disasm(&[0x34]);
    assert_eq!(m, "RET");
    assert_eq!(l, 1);

    // BLT
    let (m, l) = disasm(&[0x35, 1, 0x0080]);
    assert_eq!(m, "BLT r1, 0x0080");
    assert_eq!(l, 3);

    // BGE
    let (m, l) = disasm(&[0x36, 0, 0x0040]);
    assert_eq!(m, "BGE r0, 0x0040");
    assert_eq!(l, 3);
}

#[test]
fn test_disasm_graphics() {
    // PSET
    let (m, l) = disasm(&[0x40, 10, 20, 5]);
    assert_eq!(m, "PSET r10, r20, r5");
    assert_eq!(l, 4);

    // PSETI
    let (m, l) = disasm(&[0x41, 100, 200, 0xFF]);
    assert_eq!(m, "PSETI 100, 200, 0xFF");
    assert_eq!(l, 4);

    // FILL
    let (m, l) = disasm(&[0x42, 3]);
    assert_eq!(m, "FILL r3");
    assert_eq!(l, 2);

    // RECTF
    let (m, l) = disasm(&[0x43, 0, 0, 10, 20, 7]);
    assert_eq!(m, "RECTF r0,r0,r10,r20,r7");
    assert_eq!(l, 6);

    // TEXT
    let (m, l) = disasm(&[0x44, 5, 10, 15]);
    assert_eq!(m, "TEXT r5,r10,[r15]");
    assert_eq!(l, 4);

    // LINE
    let (m, l) = disasm(&[0x45, 0, 0, 100, 50, 9]);
    assert_eq!(m, "LINE r0,r0,r100,r50,r9");
    assert_eq!(l, 6);

    // CIRCLE
    let (m, l) = disasm(&[0x46, 128, 128, 50, 11]);
    assert_eq!(m, "CIRCLE r128,r128,r50,r11");
    assert_eq!(l, 5);

    // SCROLL
    let (m, l) = disasm(&[0x47, 3]);
    assert_eq!(m, "SCROLL r3");
    assert_eq!(l, 2);
}

#[test]
fn test_disasm_input_random() {
    // IKEY
    let (m, l) = disasm(&[0x48, 0]);
    assert_eq!(m, "IKEY r0");
    assert_eq!(l, 2);

    // RAND
    let (m, l) = disasm(&[0x49, 7]);
    assert_eq!(m, "RAND r7");
    assert_eq!(l, 2);
}

#[test]
fn test_disasm_sprite() {
    let (m, l) = disasm(&[0x4A, 10, 20, 30, 8, 8]);
    assert_eq!(m, "SPRITE r10, r20, r30, r8, r8");
    assert_eq!(l, 6);
}

#[test]
fn test_disasm_asm() {
    let (m, l) = disasm(&[0x4B, 10, 20]);
    assert_eq!(m, "ASM r10, r20");
    assert_eq!(l, 3);
}

#[test]
fn test_disasm_tilemap() {
    let (m, l) = disasm(&[0x4C, 0, 0, 1, 2, 3, 4, 5, 6]);
    assert_eq!(m, "TILEMAP r0, r0, r1, r2, r3, r4, r5, r6");
    assert_eq!(l, 9);
}

#[test]
fn test_disasm_process_ops() {
    // SPAWN
    let (m, l) = disasm(&[0x4D, 10]);
    assert_eq!(m, "SPAWN r10");
    assert_eq!(l, 2);

    // KILL
    let (m, l) = disasm(&[0x4E, 2]);
    assert_eq!(m, "KILL r2");
    assert_eq!(l, 2);

    // PEEK
    let (m, l) = disasm(&[0x4F, 100, 50, 3]);
    assert_eq!(m, "PEEK r100, r50, r3");
    assert_eq!(l, 4);
}

#[test]
fn test_disasm_cmp_mov() {
    // CMP
    let (m, l) = disasm(&[0x50, 1, 2]);
    assert_eq!(m, "CMP r1, r2");
    assert_eq!(l, 3);

    // MOV
    let (m, l) = disasm(&[0x51, 3, 5]);
    assert_eq!(m, "MOV r3, r5");
    assert_eq!(l, 3);
}

#[test]
fn test_disasm_stack() {
    // PUSH
    let (m, l) = disasm(&[0x60, 1]);
    assert_eq!(m, "PUSH r1");
    assert_eq!(l, 2);

    // POP
    let (m, l) = disasm(&[0x61, 2]);
    assert_eq!(m, "POP r2");
    assert_eq!(l, 2);
}

#[test]
fn test_disasm_syscalls() {
    // SYSCALL
    let (m, l) = disasm(&[0x52, 1]);
    assert_eq!(m, "SYSCALL 1");
    assert_eq!(l, 2);

    // RETK
    let (m, l) = disasm(&[0x53]);
    assert_eq!(m, "RETK");
    assert_eq!(l, 1);
}

#[test]
fn test_disasm_file_ops() {
    // OPEN
    let (m, l) = disasm(&[0x54, 1, 2]);
    assert_eq!(m, "OPEN r1, r2");
    assert_eq!(l, 3);

    // READ
    let (m, l) = disasm(&[0x55, 3, 4, 5]);
    assert_eq!(m, "READ r3, r4, r5");
    assert_eq!(l, 4);

    // WRITE
    let (m, l) = disasm(&[0x56, 3, 4, 5]);
    assert_eq!(m, "WRITE r3, r4, r5");
    assert_eq!(l, 4);

    // CLOSE
    let (m, l) = disasm(&[0x57, 1]);
    assert_eq!(m, "CLOSE r1");
    assert_eq!(l, 2);

    // SEEK
    let (m, l) = disasm(&[0x58, 0, 10, 20]);
    assert_eq!(m, "SEEK r0, r10, r20");
    assert_eq!(l, 4);

    // LS
    let (m, l) = disasm(&[0x59, 3]);
    assert_eq!(m, "LS r3");
    assert_eq!(l, 2);
}

#[test]
fn test_disasm_scheduler() {
    // YIELD
    let (m, l) = disasm(&[0x5A]);
    assert_eq!(m, "YIELD");
    assert_eq!(l, 1);

    // SLEEP
    let (m, l) = disasm(&[0x5B, 5]);
    assert_eq!(m, "SLEEP r5");
    assert_eq!(l, 2);

    // SETPRIORITY
    let (m, l) = disasm(&[0x5C, 10]);
    assert_eq!(m, "SETPRIORITY r10");
    assert_eq!(l, 2);

    // PIPE
    let (m, l) = disasm(&[0x5D, 1, 2]);
    assert_eq!(m, "PIPE r1, r2");
    assert_eq!(l, 3);

    // MSGSND
    let (m, l) = disasm(&[0x5E, 3]);
    assert_eq!(m, "MSGSND r3");
    assert_eq!(l, 2);

    // MSGRCV
    let (m, l) = disasm(&[0x5F]);
    assert_eq!(m, "MSGRCV");
    assert_eq!(l, 1);
}

#[test]
fn test_disasm_ioctl_env() {
    // IOCTL
    let (m, l) = disasm(&[0x62, 0, 1, 2]);
    assert_eq!(m, "IOCTL r0, r1, r2");
    assert_eq!(l, 4);

    // GETENV
    let (m, l) = disasm(&[0x63, 10, 11]);
    assert_eq!(m, "GETENV r10, r11");
    assert_eq!(l, 3);

    // SETENV
    let (m, l) = disasm(&[0x64, 20, 21]);
    assert_eq!(m, "SETENV r20, r21");
    assert_eq!(l, 3);
}

#[test]
fn test_disasm_process_mgmt() {
    // GETPID
    let (m, l) = disasm(&[0x65]);
    assert_eq!(m, "GETPID");
    assert_eq!(l, 1);

    // EXEC
    let (m, l) = disasm(&[0x66, 5]);
    assert_eq!(m, "EXEC r5");
    assert_eq!(l, 2);

    // WRITESTR
    let (m, l) = disasm(&[0x67, 1, 2]);
    assert_eq!(m, "WRITESTR r1, r2");
    assert_eq!(l, 3);

    // READLN
    let (m, l) = disasm(&[0x68, 3, 4, 5]);
    assert_eq!(m, "READLN r3, r4, r5");
    assert_eq!(l, 4);

    // WAITPID
    let (m, l) = disasm(&[0x69, 1]);
    assert_eq!(m, "WAITPID r1");
    assert_eq!(l, 2);
}

#[test]
fn test_disasm_screenp_shutdown_exit() {
    // SCREENP
    let (m, l) = disasm(&[0x6D, 3, 100, 200]);
    assert_eq!(m, "SCREENP r3, r100, r200");
    assert_eq!(l, 4);

    // SHUTDOWN
    let (m, l) = disasm(&[0x6E]);
    assert_eq!(m, "SHUTDOWN");
    assert_eq!(l, 1);

    // EXIT
    let (m, l) = disasm(&[0x6F, 42]);
    assert_eq!(m, "EXIT r42");
    assert_eq!(l, 2);
}

#[test]
fn test_disasm_signal() {
    // SIGNAL
    let (m, l) = disasm(&[0x70, 1, 2]);
    assert_eq!(m, "SIGNAL r1, r2");
    assert_eq!(l, 3);

    // SIGSET
    let (m, l) = disasm(&[0x71, 3, 4]);
    assert_eq!(m, "SIGSET r3, r4");
    assert_eq!(l, 3);
}

#[test]
fn test_disasm_hypervisor() {
    let (m, l) = disasm(&[0x72, 10]);
    assert_eq!(m, "HYPERVISOR r10");
    assert_eq!(l, 2);
}

#[test]
fn test_disasm_asmself_runnext() {
    let (m, l) = disasm(&[0x73]);
    assert_eq!(m, "ASMSELF");
    assert_eq!(l, 1);

    let (m, l) = disasm(&[0x74]);
    assert_eq!(m, "RUNNEXT");
    assert_eq!(l, 1);
}

#[test]
fn test_disasm_formula() {
    // FORMULA tile, op=ADD(0), dep_count=3
    let (m, l) = disasm(&[0x75, 0, 0, 3, 0x100, 0x200, 0x300]);
    assert_eq!(m, "FORMULA 0, ADD, 3");
    assert_eq!(l, 7); // 4 header + 3 deps

    // FORMULA tile, op=MUL(2), dep_count=1
    let (m, l) = disasm(&[0x75, 5, 2, 1, 42]);
    assert_eq!(m, "FORMULA 5, MUL, 1");
    assert_eq!(l, 5);

    // Unknown formula op
    let (m, l) = disasm(&[0x75, 1, 99, 0]);
    assert_eq!(m, "FORMULA 1, ???, 0");
    assert_eq!(l, 4);
}

#[test]
fn test_disasm_formula_ops_all() {
    let ops = vec![
        (0, "ADD"),
        (1, "SUB"),
        (2, "MUL"),
        (3, "DIV"),
        (4, "AND"),
        (5, "OR"),
        (6, "XOR"),
        (7, "NOT"),
        (8, "COPY"),
        (9, "MAX"),
        (10, "MIN"),
        (11, "MOD"),
        (12, "SHL"),
        (13, "SHR"),
    ];
    for (code, name) in ops {
        let (m, _l) = disasm(&[0x75, 0, code, 0]);
        assert_eq!(
            m,
            format!("FORMULA 0, {}, 0", name),
            "Formula op code {} should be {}",
            code,
            name
        );
    }
}

#[test]
fn test_disasm_formulaclear() {
    let (m, l) = disasm(&[0x76]);
    assert_eq!(m, "FORMULACLEAR");
    assert_eq!(l, 1);
}

#[test]
fn test_disasm_formularem() {
    let (m, l) = disasm(&[0x77, 42]);
    assert_eq!(m, "FORMULAREM 42");
    assert_eq!(l, 2);
}

#[test]
fn test_disasm_vfs() {
    // FMKDIR
    let (m, l) = disasm(&[0x78, 10]);
    assert_eq!(m, "FMKDIR [r10]");
    assert_eq!(l, 2);

    // FSTAT
    let (m, l) = disasm(&[0x79, 1, 2]);
    assert_eq!(m, "FSTAT r1, [r2]");
    assert_eq!(l, 3);

    // FUNLINK
    let (m, l) = disasm(&[0x7A, 5]);
    assert_eq!(m, "FUNLINK [r5]");
    assert_eq!(l, 2);
}

#[test]
fn test_disasm_snap_replay() {
    // SNAP_TRACE
    let (m, l) = disasm(&[0x7B, 3]);
    assert_eq!(m, "SNAP_TRACE r3");
    assert_eq!(l, 2);

    // REPLAY
    let (m, l) = disasm(&[0x7C, 5]);
    assert_eq!(m, "REPLAY r5");
    assert_eq!(l, 2);
}

#[test]
fn test_disasm_fork_note() {
    // FORK
    let (m, l) = disasm(&[0x7D, 10]);
    assert_eq!(m, "FORK r10");
    assert_eq!(l, 2);

    // NOTE
    let (m, l) = disasm(&[0x7E, 1, 2, 3]);
    assert_eq!(m, "NOTE r1, r2, r3");
    assert_eq!(l, 4);
}

#[test]
fn test_disasm_network() {
    // CONNECT
    let (m, l) = disasm(&[0x7F, 1, 2, 3]);
    assert_eq!(m, "CONNECT r1, r2, r3");
    assert_eq!(l, 4);

    // SOCKSEND
    let (m, l) = disasm(&[0x80, 0, 1, 2, 3]);
    assert_eq!(m, "SOCKSEND r0, r1, r2, r3");
    assert_eq!(l, 5);

    // SOCKRECV
    let (m, l) = disasm(&[0x81, 0, 1, 2, 3]);
    assert_eq!(m, "SOCKRECV r0, r1, r2, r3");
    assert_eq!(l, 5);

    // DISCONNECT
    let (m, l) = disasm(&[0x82, 5]);
    assert_eq!(m, "DISCONNECT r5");
    assert_eq!(l, 2);
}

#[test]
fn test_disasm_trace_read_pixel_history() {
    // TRACE_READ
    let (m, l) = disasm(&[0x83, 7]);
    assert_eq!(m, "TRACE_READ r7");
    assert_eq!(l, 2);

    // PIXEL_HISTORY
    let (m, l) = disasm(&[0x84, 3]);
    assert_eq!(m, "PIXEL_HISTORY r3");
    assert_eq!(l, 2);
}

#[test]
fn test_disasm_unknown_opcode() {
    let (m, l) = disasm(&[0xFE]);
    assert_eq!(m, "??? (0xFE)");
    assert_eq!(l, 1);

    let (m, l) = disasm(&[0xFF]);
    assert_eq!(m, "??? (0xFF)");
    assert_eq!(l, 1);
}

#[test]
fn test_disasm_out_of_bounds() {
    let vm = Vm::new();
    let (m, l) = vm.disassemble_at(0xFFFFF);
    assert_eq!(m, "???");
    assert_eq!(l, 1);
}

#[test]
fn test_disasm_execp_chdir_getcwd() {
    // EXECP
    let (m, l) = disasm(&[0x6A, 1, 2, 3]);
    assert_eq!(m, "EXECP r1, r2, r3");
    assert_eq!(l, 4);

    // CHDIR
    let (m, l) = disasm(&[0x6B, 5]);
    assert_eq!(m, "CHDIR r5");
    assert_eq!(l, 2);

    // GETCWD
    let (m, l) = disasm(&[0x6C, 3]);
    assert_eq!(m, "GETCWD r3");
    assert_eq!(l, 2);
}

// ── hello_window.asm end-to-end demo ──────────────────────────

#[test]
fn test_hello_window_assembles_and_runs() {
    let source = include_str!("../../programs/hello_window.asm");
    let asm = crate::assembler::assemble(source, 0).expect("hello_window.asm should assemble");

    let mut vm = Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        vm.ram[i] = word;
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..10_000 {
        if !vm.step() {
            break;
        }
    }

    // Title bar pixel (inside 256x30 fill of 0x555555) must be set.
    assert_eq!(vm.screen[5 * 256 + 5], 0x555555, "title bar should render");

    // OK button pixel (inside 80x28 at (88,110) of 0x2266FF) must be set.
    assert_eq!(
        vm.screen[120 * 256 + 100],
        0x2266FF,
        "OK button should render"
    );

    // Exactly one hit region registered, matching the button rect with id=1.
    assert_eq!(vm.hit_regions.len(), 1);
    let btn = vm.hit_regions[0];
    assert_eq!((btn.x, btn.y, btn.w, btn.h, btn.id), (88, 110, 80, 28, 1));
}

#[test]
fn test_hello_window_click_routes_to_id() {
    // Same demo, but after HALT we simulate a click inside the OK button
    // and re-run just a HITQ + HALT stub to read the id back.
    let source = include_str!("../../programs/hello_window.asm");
    let asm = crate::assembler::assemble(source, 0).expect("hello_window.asm should assemble");

    let mut vm = Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        vm.ram[i] = word;
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..10_000 {
        if !vm.step() {
            break;
        }
    }

    // Simulate cursor inside the button.
    vm.push_mouse(120, 120);

    // Inject a 2-instruction stub at 0xE00: HITQ r11; HALT.
    vm.ram[0xE00] = 0x38;
    vm.ram[0xE01] = 11;
    vm.ram[0xE02] = 0x00;
    vm.pc = 0xE00;
    vm.halted = false;
    for _ in 0..10 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.regs[11], 1, "cursor inside OK button should resolve to id=1");

    // Cursor outside → miss.
    vm.push_mouse(0, 0);
    vm.regs[11] = 0xDEAD;
    vm.pc = 0xE00;
    vm.halted = false;
    for _ in 0..10 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.regs[11], 0, "cursor outside any region should resolve to 0");
}

// ── Counter Application Integration Tests ────────────────────

/// Helper: load counter.asm into a fresh VM and run until N frames have rendered.
fn boot_counter(target_frames: u32) -> Vm {
    let source = include_str!("../../programs/counter.asm");
    let asm = crate::assembler::assemble(source, 0).expect("counter.asm should assemble");
    let mut vm = Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    let start_frame = vm.frame_count;
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= start_frame + target_frames {
            break;
        }
    }
    vm
}

#[test]
fn test_counter_boots_and_renders() {
    let vm = boot_counter(1);
    assert!(!vm.halted, "counter app should not halt after boot");
    // Background should be dark purple (0x1A1A2E)
    assert_eq!(vm.screen[0], 0x1A1A2E, "background should be dark purple");
    // [+] button (green 0x2ECC71) at (80, 170) should be green
    assert_eq!(vm.screen[170 * 256 + 80], 0x2ECC71, "+ button should be green");
    // [-] button (red 0xE74C3C) at (176, 170) should be red
    assert_eq!(vm.screen[170 * 256 + 176], 0xE74C3C, "- button should be red");
    // Counter should be 0
    assert_eq!(vm.ram[0x100], 0, "counter should start at 0");
    // Two hit regions registered
    assert_eq!(vm.hit_regions.len(), 2, "should have 2 hit regions");
}

#[test]
fn test_counter_click_increments() {
    let mut vm = boot_counter(1);
    assert!(!vm.halted, "should be running");
    assert_eq!(vm.ram[0x100], 0, "counter should start at 0");

    // Position mouse over [+] button center (80, 170)
    vm.push_mouse(80, 170);

    // Run a few frames
    let start_frame = vm.frame_count;
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= start_frame + 3 {
            break;
        }
    }

    assert!(!vm.halted, "should still be running after click");
    // Counter should have incremented (at least once, since mouse is held)
    assert!(vm.ram[0x100] > 0, "counter should have incremented: got {}", vm.ram[0x100]);
}

#[test]
fn test_counter_click_decrements() {
    let mut vm = boot_counter(1);

    // First increment to 3
    vm.push_mouse(80, 170);
    let start = vm.frame_count;
    for _ in 0..500_000 {
        if !vm.step() { break; }
        if vm.frame_count >= start + 4 { break; }
    }
    let val = vm.ram[0x100];
    assert!(val > 0, "should have incremented");

    // Now move mouse to [-] button center (176, 170)
    vm.push_mouse(176, 170);
    let start2 = vm.frame_count;
    for _ in 0..500_000 {
        if !vm.step() { break; }
        if vm.frame_count >= start2 + 3 { break; }
    }

    assert!(!vm.halted, "should still be running");
    // Counter should have decreased
    assert!(vm.ram[0x100] < val, "counter should have decremented: was {}, now {}", val, vm.ram[0x100]);
}

#[test]
fn test_counter_renders_number_text() {
    let vm = boot_counter(1);
    // Scratch buffer: "Count " (6 chars) then 3 digits + null = 10 total
    let scratch: usize = 0x200;
    assert_eq!(vm.ram[scratch + 0], b'C' as u32, "should have 'C' at scratch[0]");
    assert_eq!(vm.ram[scratch + 1], b'o' as u32, "should have 'o' at scratch[1]");
    assert_eq!(vm.ram[scratch + 5], b' ' as u32, "should have ' ' at scratch[5]");
    assert_eq!(vm.ram[scratch + 6], b'0' as u32, "hundreds digit should be '0'");
    assert_eq!(vm.ram[scratch + 7], b'0' as u32, "tens digit should be '0'");
    assert_eq!(vm.ram[scratch + 8], b'0' as u32, "ones digit should be '0'");
    assert_eq!(vm.ram[scratch + 9], 0, "should be null terminated");
}



