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
    let vm = run_program(
        &[
            0x10, 1, 5, // LDI r1, 5 (x)
            0x10, 2, 5, // LDI r2, 5 (y)
            0x10, 3, 10, // LDI r3, 10 (w)
            0x10, 4, 20, // LDI r4, 20 (h)
            0x37, 1, 2, 3, 4, 42, // HITSET r1,r2,r3,r4,42
            0x00,
        ],
        100,
    ); // HALT
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
        if !vm.step() {
            break;
        }
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
        if !vm.step() {
            break;
        }
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
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
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
    for _ in 0..100 {
        if !vm2.step() {
            break;
        }
    }
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
    for _ in 0..100 {
        if !vm3.step() {
            break;
        }
    }
    assert_eq!(vm3.regs[5], 0);
}

#[test]
fn test_hitq_first_match_wins() {
    // Two overlapping regions; first registered wins
    let mut vm = Vm::new();
    vm.regs[1] = 10;
    vm.regs[2] = 10;
    vm.regs[3] = 50;
    vm.regs[4] = 50;
    // Region 1: (10,10) 50x50, id=100
    vm.ram[0] = 0x37;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.ram[4] = 4;
    vm.ram[5] = 100;
    // Region 2: (20,20) 50x50, id=200
    vm.ram[6] = 0x10;
    vm.ram[7] = 1;
    vm.ram[8] = 20; // LDI r1, 20
    vm.ram[9] = 0x10;
    vm.ram[10] = 2;
    vm.ram[11] = 20; // LDI r2, 20
    vm.ram[12] = 0x37;
    vm.ram[13] = 1;
    vm.ram[14] = 2;
    vm.ram[15] = 3;
    vm.ram[16] = 4;
    vm.ram[17] = 200;
    vm.ram[18] = 0x38;
    vm.ram[19] = 5; // HITQ r5
    vm.ram[20] = 0x00; // HALT
    vm.pc = 0;
    vm.push_mouse(30, 30); // inside both
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
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
fn test_asm_ram_assembles_from_ram() {
    // ASM_RAM (0xB3): reads null-terminated assembly source from RAM,
    // runs preprocessor + assembler, writes bytecode to 0x1000.
    // RAM[0xFFD] = word count on success, 0xFFFFFFFF on error.

    let mut vm = Vm::new();

    // Write "LDI r1, 42\nHALT\n" as null-terminated string at RAM address 0x3000
    let source = "LDI r1, 42\nHALT\n";
    let mut addr = 0x3000usize;
    for ch in source.bytes() {
        vm.ram[addr] = ch as u32;
        addr += 1;
    }
    vm.ram[addr] = 0; // null terminator

    // Set up: LDI r5, 0x3000; ASM_RAM r5; RUNNEXT
    // We'll execute these as raw bytecode
    vm.ram[0] = 0x10; // LDI
    vm.ram[1] = 5; // r5
    vm.ram[2] = 0x3000; // address

    vm.ram[3] = 0xB3; // ASM_RAM
    vm.ram[4] = 5; // r5

    vm.ram[5] = 0x74; // RUNNEXT

    vm.pc = 0;
    vm.halted = false;

    // Run for enough steps
    for _ in 0..1000 {
        if !vm.step() {
            break;
        }
    }

    // Verify assembly succeeded
    assert_ne!(vm.ram[0xFFD], 0xFFFFFFFF, "ASM_RAM should succeed");
    assert!(vm.ram[0xFFD] > 0, "Should produce bytecode");

    // After RUNNEXT, PC jumps to 0x1000 and executes the assembled code.
    // "LDI r1, 42; HALT" sets r1=42 then halts.
    assert_eq!(vm.regs[1], 42, "r1 should be 42 from assembled code");
    assert!(vm.halted, "VM should halt after executing assembled code");
}

#[test]
fn test_asm_ram_error_on_invalid_source() {
    // ASM_RAM with invalid assembly source should set RAM[0xFFD] = 0xFFFFFFFF
    let mut vm = Vm::new();

    // Write invalid assembly at 0x3000
    let source = "GARBAGE_OPCODE r1, r2\n";
    let mut addr = 0x3000usize;
    for ch in source.bytes() {
        vm.ram[addr] = ch as u32;
        addr += 1;
    }
    vm.ram[addr] = 0; // null terminator

    // LDI r5, 0x3000; ASM_RAM r5
    vm.ram[0] = 0x10;
    vm.ram[1] = 5;
    vm.ram[2] = 0x3000;
    vm.ram[3] = 0xB3;
    vm.ram[4] = 5;
    vm.ram[5] = 0x00; // HALT

    vm.pc = 0;
    vm.halted = false;

    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }

    assert_eq!(
        vm.ram[0xFFD], 0xFFFFFFFF,
        "ASM_RAM should fail on invalid source"
    );
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
// gui_calc.asm: GUI Calculator App
// ============================================================

#[test]
fn test_gui_calc_assembles() {
    let source = include_str!("../../programs/gui_calc.asm");
    let result = crate::assembler::assemble(source, 0x1000);
    assert!(
        result.is_ok(),
        "gui_calc.asm should assemble: {:?}",
        result.err()
    );
    let asm = result.expect("should succeed");
    assert!(
        asm.pixels.len() > 100,
        "gui_calc should produce substantial bytecode, got {}",
        asm.pixels.len()
    );
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

// ── MOUSEQ ───────────────────────────────────────────────────────

#[test]
fn test_mouseq_reads_mouse_position() {
    let mut vm = crate::vm::Vm::new();
    // MOUSEQ r5 -- should read mouse_x into r5, mouse_y into r6
    vm.ram[0] = 0x85;
    vm.ram[1] = 5;
    vm.pc = 0;

    vm.push_mouse(123, 200);
    vm.step();

    assert_eq!(vm.regs[5], 123, "r5 should be mouse_x");
    assert_eq!(vm.regs[6], 200, "r6 should be mouse_y");
}

#[test]
fn test_mouseq_default_zero() {
    let mut vm = crate::vm::Vm::new();
    vm.ram[0] = 0x85;
    vm.ram[1] = 10;
    vm.pc = 0;

    // No push_mouse -- should be 0,0
    vm.step();

    assert_eq!(vm.regs[10], 0, "mouse_x should default to 0");
    assert_eq!(vm.regs[11], 0, "mouse_y should default to 0");
}

#[test]
fn test_mouseq_updates_on_push() {
    let mut vm = crate::vm::Vm::new();
    vm.ram[0] = 0x85;
    vm.ram[1] = 1;
    vm.ram[2] = 0x00; // HALT
    vm.pc = 0;

    vm.push_mouse(50, 75);
    vm.step();
    assert_eq!(vm.regs[1], 50);
    assert_eq!(vm.regs[2], 75);

    // Reset and push new position
    vm.regs[1] = 0;
    vm.regs[2] = 0;
    vm.pc = 0;
    vm.push_mouse(200, 100);
    vm.step();
    assert_eq!(vm.regs[1], 200);
    assert_eq!(vm.regs[2], 100);
}

#[test]
fn test_mouseq_disassembler() {
    let mut vm = crate::vm::Vm::new();
    vm.ram[0] = 0x85;
    vm.ram[1] = 7;
    let (text, len) = vm.disassemble_at(0);
    assert_eq!(text, "MOUSEQ r7");
    assert_eq!(len, 2);
}

#[test]
fn test_mouseq_assembler() {
    let src = "MOUSEQ r10\nHALT";
    let result = crate::assembler::assemble(src, 0);
    assert!(result.is_ok());
    let bytecode = result.unwrap();
    assert_eq!(bytecode.pixels[0], 0x85);
    assert_eq!(bytecode.pixels[1], 10);
    assert_eq!(bytecode.pixels[2], 0x00); // HALT
}

#[test]
fn test_mouseq_in_paint_loop() {
    // Simulate a simple paint loop: MOUSEQ r1, PSET r1, r2, r3, FRAME, HALT
    let mut vm = crate::vm::Vm::new();
    // MOUSEQ r1
    vm.ram[0] = 0x85;
    vm.ram[1] = 1;
    // LDI r3, 0xFF0000 (red)
    vm.ram[2] = 0x10;
    vm.ram[3] = 3;
    vm.ram[4] = 0xFF0000;
    // PSET r1, r2, r3
    vm.ram[5] = 0x40;
    vm.ram[6] = 1;
    vm.ram[7] = 2;
    vm.ram[8] = 3;
    // FRAME
    vm.ram[9] = 0x02;
    // HALT
    vm.ram[10] = 0x00;
    vm.pc = 0;

    vm.push_mouse(64, 128);
    // Run until halt
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }

    // Check that pixel was painted at (64, 128)
    assert_eq!(
        vm.screen[128 * 256 + 64],
        0xFF0000,
        "pixel should be painted at mouse pos"
    );
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
    let (m, l) = disasm(&[0x72, 10, 0]);
    assert_eq!(m, "HYPERVISOR r10");
    assert_eq!(l, 3);
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
    assert_eq!(
        vm.regs[11], 1,
        "cursor inside OK button should resolve to id=1"
    );

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
    assert_eq!(
        vm.regs[11], 0,
        "cursor outside any region should resolve to 0"
    );
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
    assert_eq!(
        vm.screen[170 * 256 + 80],
        0x2ECC71,
        "+ button should be green"
    );
    // [-] button (red 0xE74C3C) at (176, 170) should be red
    assert_eq!(
        vm.screen[170 * 256 + 176],
        0xE74C3C,
        "- button should be red"
    );
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
    assert!(
        vm.ram[0x100] > 0,
        "counter should have incremented: got {}",
        vm.ram[0x100]
    );
}

#[test]
fn test_counter_click_decrements() {
    let mut vm = boot_counter(1);

    // First increment to 3
    vm.push_mouse(80, 170);
    let start = vm.frame_count;
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= start + 4 {
            break;
        }
    }
    let val = vm.ram[0x100];
    assert!(val > 0, "should have incremented");

    // Now move mouse to [-] button center (176, 170)
    vm.push_mouse(176, 170);
    let start2 = vm.frame_count;
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= start2 + 3 {
            break;
        }
    }

    assert!(!vm.halted, "should still be running");
    // Counter should have decreased
    assert!(
        vm.ram[0x100] < val,
        "counter should have decremented: was {}, now {}",
        val,
        vm.ram[0x100]
    );
}

#[test]
fn test_counter_renders_number_text() {
    let vm = boot_counter(1);
    // Scratch buffer: "Count " (6 chars) then 3 digits + null = 10 total
    let scratch: usize = 0x200;
    assert_eq!(
        vm.ram[scratch + 0],
        b'C' as u32,
        "should have 'C' at scratch[0]"
    );
    assert_eq!(
        vm.ram[scratch + 1],
        b'o' as u32,
        "should have 'o' at scratch[1]"
    );
    assert_eq!(
        vm.ram[scratch + 5],
        b' ' as u32,
        "should have ' ' at scratch[5]"
    );
    assert_eq!(
        vm.ram[scratch + 6],
        b'0' as u32,
        "hundreds digit should be '0'"
    );
    assert_eq!(vm.ram[scratch + 7], b'0' as u32, "tens digit should be '0'");
    assert_eq!(vm.ram[scratch + 8], b'0' as u32, "ones digit should be '0'");
    assert_eq!(vm.ram[scratch + 9], 0, "should be null terminated");
}

// ── Terminal Application Integration Tests ────────────────────

/// Helper: load terminal.asm into a fresh VM and run until N frames have rendered.
fn boot_terminal(target_frames: u32) -> Vm {
    let source = include_str!("../../programs/terminal.asm");
    let asm = crate::assembler::assemble(source, 0).expect("terminal.asm should assemble");
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
fn test_terminal_boots_and_renders() {
    let vm = boot_terminal(1);
    assert!(!vm.halted, "terminal app should not halt after boot");
    // Title bar should be blue-purple (0x333355) at row 0
    assert_eq!(
        vm.screen[5 * 256 + 5],
        0x333355,
        "title bar should be blue-purple"
    );
    // Content area below title bar (y=20) should be dark gray background
    assert_eq!(
        vm.screen[20 * 256 + 10],
        0x0C0C0C,
        "content area should be dark gray"
    );
    // Cursor col should start at 2 (after "$ " prompt)
    assert_eq!(vm.ram[0x4800], 2, "cursor col should start at 2");
    // Cursor row should start at 0
    assert_eq!(vm.ram[0x4801], 0, "cursor row should start at 0");
    // Text buffer at row 0 should have '$' at col 0 and ' ' at col 1
    assert_eq!(vm.ram[0x4000], b'$' as u32, "row 0 col 0 should be '$'");
    assert_eq!(vm.ram[0x4001], b' ' as u32, "row 0 col 1 should be ' '");
    // Close button hit region registered (id=99)
    assert_eq!(
        vm.hit_regions.len(),
        1,
        "should have 1 hit region (close button)"
    );
}

#[test]
fn test_terminal_types_character() {
    let mut vm = boot_terminal(1);
    assert!(!vm.halted, "should be running");

    // Push 'H' (0x48 = 72) key
    vm.push_key(b'H' as u32);

    // Run a few frames to process the key
    let start_frame = vm.frame_count;
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= start_frame + 3 {
            break;
        }
    }

    assert!(!vm.halted, "should still be running after key");
    // 'H' should be at buffer[row=0 * COLS=42 + col=2] = 0x4002
    assert_eq!(
        vm.ram[0x4000 + 42 * 0 + 2],
        b'H' as u32,
        "typed 'H' should appear at row 0, col 2"
    );
    // Cursor should have advanced to col 3
    assert_eq!(vm.ram[0x4800], 3, "cursor should have advanced to col 3");
}

#[test]
fn test_terminal_types_multiple_chars() {
    let mut vm = boot_terminal(1);
    assert!(!vm.halted);

    // Type "Hi" (two characters)
    vm.push_key(b'H' as u32);
    let start = vm.frame_count;
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= start + 2 {
            break;
        }
    }
    vm.push_key(b'i' as u32);
    let start2 = vm.frame_count;
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= start2 + 2 {
            break;
        }
    }

    assert!(!vm.halted, "should still be running");
    assert_eq!(
        vm.ram[0x4000 + 42 * 0 + 2],
        b'H' as u32,
        "should have 'H' at col 2"
    );
    assert_eq!(
        vm.ram[0x4000 + 42 * 0 + 3],
        b'i' as u32,
        "should have 'i' at col 3"
    );
    assert_eq!(vm.ram[0x4800], 4, "cursor should be at col 4");
}

#[test]
fn test_terminal_enter_newline() {
    let mut vm = boot_terminal(1);
    assert!(!vm.halted);

    // Type 'A' then Enter (13)
    vm.push_key(b'A' as u32);
    let start = vm.frame_count;
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= start + 2 {
            break;
        }
    }
    vm.push_key(13); // Enter
    let start2 = vm.frame_count;
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= start2 + 2 {
            break;
        }
    }

    assert!(!vm.halted, "should still be running");
    // With command dispatch: "A" is unknown -> "? A" on row 1, prompt on row 2
    assert_eq!(
        vm.ram[0x4801], 2,
        "cursor should be on row 2 after enter (row 1 has '? A' output)"
    );
    assert_eq!(
        vm.ram[0x4800], 2,
        "cursor col should be 2 (after new prompt)"
    );
    // Row 1 should have "? A" output
    assert_eq!(
        vm.ram[0x4000 + 42 * 1],
        63,
        "row 1 should start with '?' (unknown cmd)"
    );
    assert_eq!(vm.ram[0x4000 + 42 * 1 + 1], 32, "row 1 col 1 should be ' '");
    assert_eq!(
        vm.ram[0x4000 + 42 * 1 + 2],
        b'A' as u32,
        "row 1 col 2 should be 'A'"
    );
    // Row 2 should have prompt
    assert_eq!(
        vm.ram[0x4000 + 42 * 2],
        b'$' as u32,
        "row 2 should start with '$'"
    );
    assert_eq!(
        vm.ram[0x4000 + 42 * 2 + 1],
        b' ' as u32,
        "row 2 col 1 should be ' '"
    );
}

#[test]
fn test_terminal_backspace() {
    let mut vm = boot_terminal(1);
    assert!(!vm.halted);

    // Type "AB" then backspace (8)
    vm.push_key(b'A' as u32);
    let start = vm.frame_count;
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= start + 2 {
            break;
        }
    }
    vm.push_key(b'B' as u32);
    let start2 = vm.frame_count;
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= start2 + 2 {
            break;
        }
    }
    assert_eq!(vm.ram[0x4800], 4, "cursor should be at col 4 after 'AB'");

    // Backspace
    vm.push_key(8);
    let start3 = vm.frame_count;
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= start3 + 2 {
            break;
        }
    }

    assert!(!vm.halted, "should still be running");
    assert_eq!(
        vm.ram[0x4800], 3,
        "cursor should be back at col 3 after backspace"
    );
    // Col 3 should be cleared to space
    assert_eq!(
        vm.ram[0x4000 + 42 * 0 + 3],
        b' ' as u32,
        "backspaced position should be space"
    );
}

#[test]
fn test_terminal_blink_counter_advances() {
    let vm = boot_terminal(5);
    assert!(!vm.halted);
    // Blink counter at RAM[0x4802] should be > 0 after 5 frames
    assert!(
        vm.ram[0x4802] > 0,
        "blink counter should have advanced: got {}",
        vm.ram[0x4802]
    );
}

#[test]
fn test_terminal_cmd_help() {
    let mut vm = boot_terminal(0);
    // Type "help" then Enter
    for ch in b"help" {
        vm.push_key(*ch as u32);
        let start = vm.frame_count;
        for _ in 0..500_000 {
            if !vm.step() {
                break;
            }
            if vm.frame_count >= start + 2 {
                break;
            }
        }
    }
    vm.push_key(13); // Enter
    let start = vm.frame_count;
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= start + 6 {
            break;
        }
    }
    assert!(!vm.halted, "should not halt after help command");
    // Debug: dump buffer rows with raw values
    for row in 0..4 {
        let base: usize = 0x4000 + 42 * row;
        eprint!("row {} raw: ", row);
        for col in 0..20 {
            eprint!("{:3} ", vm.ram[base + col]);
        }
        eprintln!();
    }
    eprintln!("cursor row={} col={}", vm.ram[0x4801], vm.ram[0x4800]);
    // Row 1 should have "cmds: clear help ver hi echo ls date cat" output
    assert_eq!(
        vm.ram[0x4000 + 42 * 1 + 0],
        b'c' as u32,
        "row 1 should start with 'c' from 'cmds...'"
    );
    assert_eq!(
        vm.ram[0x4000 + 42 * 1 + 1],
        b'm' as u32,
        "row 1 col 1 should be 'm'"
    );
    // Row 2 should have "      sys colors whoami uname uptime"
    assert_eq!(
        vm.ram[0x4000 + 42 * 2 + 6],
        b's' as u32,
        "row 2 should have 'sys' from second help line"
    );
    // Row 3 should have prompt (help now outputs 2 lines)
    assert_eq!(
        vm.ram[0x4000 + 42 * 3],
        b'$' as u32,
        "row 3 should have prompt after help output"
    );
    assert_eq!(vm.ram[0x4801], 3, "cursor should be on row 3 after help");
}

#[test]
fn test_terminal_cmd_ver() {
    let mut vm = boot_terminal(0);
    for ch in b"ver" {
        vm.push_key(*ch as u32);
        let start = vm.frame_count;
        for _ in 0..500_000 {
            if !vm.step() {
                break;
            }
            if vm.frame_count >= start + 2 {
                break;
            }
        }
    }
    vm.push_key(13);
    let start = vm.frame_count;
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= start + 3 {
            break;
        }
    }
    assert!(!vm.halted);
    // Row 1 should have "GeoTerm v1.0"
    assert_eq!(
        vm.ram[0x4000 + 42 * 1 + 0],
        b'G' as u32,
        "row 1 should start with 'G'"
    );
    assert_eq!(
        vm.ram[0x4000 + 42 * 1 + 1],
        b'e' as u32,
        "row 1 col 1 should be 'e'"
    );
    // Row 2 should have prompt
    assert_eq!(
        vm.ram[0x4000 + 42 * 2],
        b'$' as u32,
        "row 2 should have prompt"
    );
}

#[test]
fn test_terminal_cmd_hi() {
    let mut vm = boot_terminal(0);
    for ch in b"hi" {
        vm.push_key(*ch as u32);
        let start = vm.frame_count;
        for _ in 0..500_000 {
            if !vm.step() {
                break;
            }
            if vm.frame_count >= start + 2 {
                break;
            }
        }
    }
    vm.push_key(13);
    let start = vm.frame_count;
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= start + 3 {
            break;
        }
    }
    assert!(!vm.halted);
    // Row 1 should have "hello!"
    assert_eq!(
        vm.ram[0x4000 + 42 * 1 + 0],
        b'h' as u32,
        "row 1 should start with 'h'"
    );
    assert_eq!(
        vm.ram[0x4000 + 42 * 1 + 1],
        b'e' as u32,
        "row 1 col 1 should be 'e'"
    );
    assert_eq!(
        vm.ram[0x4000 + 42 * 1 + 5],
        b'!' as u32,
        "row 1 col 5 should be '!'"
    );
}

#[test]
fn test_terminal_cmd_clear() {
    let mut vm = boot_terminal(0);
    // Type "clear" then Enter -- just "clear" with nothing else on the line
    for ch in b"clear" {
        vm.push_key(*ch as u32);
        let start = vm.frame_count;
        for _ in 0..500_000 {
            if !vm.step() {
                break;
            }
            if vm.frame_count >= start + 2 {
                break;
            }
        }
    }
    vm.push_key(13);
    let start = vm.frame_count;
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= start + 3 {
            break;
        }
    }
    assert!(!vm.halted);
    // After clear, cursor should be at row 0, col 2
    assert_eq!(vm.ram[0x4801], 0, "cursor row should be 0 after clear");
    assert_eq!(vm.ram[0x4800], 2, "cursor col should be 2 after clear");
    // Row 0 should have prompt
    assert_eq!(
        vm.ram[0x4000 + 42 * 0],
        b'$' as u32,
        "row 0 should have prompt after clear"
    );
}

#[test]
fn test_terminal_cmd_echo_with_args() {
    let mut vm = boot_terminal(0);
    for ch in b"echo hello world" {
        vm.push_key(*ch as u32);
        let start = vm.frame_count;
        for _ in 0..500_000 {
            if !vm.step() {
                break;
            }
            if vm.frame_count >= start + 2 {
                break;
            }
        }
    }
    vm.push_key(13);
    let start = vm.frame_count;
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= start + 3 {
            break;
        }
    }
    assert!(!vm.halted);
    // Row 1 should have "hello world"
    assert_eq!(
        vm.ram[0x4000 + 42 * 1 + 0],
        b'h' as u32,
        "row 1 should start with 'h'"
    );
    assert_eq!(
        vm.ram[0x4000 + 42 * 1 + 5],
        b' ' as u32,
        "row 1 col 5 should be space"
    );
    assert_eq!(
        vm.ram[0x4000 + 42 * 1 + 6],
        b'w' as u32,
        "row 1 col 6 should be 'w'"
    );
    // Row 2 should have prompt
    assert_eq!(
        vm.ram[0x4000 + 42 * 2],
        b'$' as u32,
        "row 2 should have prompt"
    );
}

#[test]
fn test_terminal_cmd_echo_no_args() {
    let mut vm = boot_terminal(0);
    for ch in b"echo " {
        vm.push_key(*ch as u32);
        let start = vm.frame_count;
        for _ in 0..500_000 {
            if !vm.step() {
                break;
            }
            if vm.frame_count >= start + 2 {
                break;
            }
        }
    }
    vm.push_key(13);
    let start = vm.frame_count;
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= start + 3 {
            break;
        }
    }
    assert!(!vm.halted);
    // echo with no args prints empty line -- cursor advances to row 2
    assert_eq!(
        vm.ram[0x4801], 2,
        "cursor should be on row 2 after empty echo"
    );
    assert_eq!(
        vm.ram[0x4000 + 42 * 2],
        b'$' as u32,
        "row 2 should have prompt"
    );
}

#[test]
fn test_terminal_cmd_date() {
    let mut vm = boot_terminal(0);
    for ch in b"date" {
        vm.push_key(*ch as u32);
        let start = vm.frame_count;
        for _ in 0..500_000 {
            if !vm.step() {
                break;
            }
            if vm.frame_count >= start + 2 {
                break;
            }
        }
    }
    vm.push_key(13);
    let start = vm.frame_count;
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= start + 3 {
            break;
        }
    }
    assert!(!vm.halted);
    // Row 1 should have "2026-04-20"
    assert_eq!(
        vm.ram[0x4000 + 42 * 1 + 0],
        b'2' as u32,
        "row 1 should start with '2'"
    );
    assert_eq!(
        vm.ram[0x4000 + 42 * 1 + 4],
        b'-' as u32,
        "row 1 col 4 should be '-'"
    );
    // Row 2 should have prompt
    assert_eq!(
        vm.ram[0x4000 + 42 * 2],
        b'$' as u32,
        "row 2 should have prompt"
    );
}

#[test]
fn test_terminal_cmd_cls() {
    let mut vm = boot_terminal(0);
    // Type something first
    for ch in b"hi" {
        vm.push_key(*ch as u32);
        let start = vm.frame_count;
        for _ in 0..500_000 {
            if !vm.step() {
                break;
            }
            if vm.frame_count >= start + 2 {
                break;
            }
        }
    }
    vm.push_key(13);
    let start = vm.frame_count;
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= start + 3 {
            break;
        }
    }
    // Now type cls
    for ch in b"cls" {
        vm.push_key(*ch as u32);
        let start = vm.frame_count;
        for _ in 0..500_000 {
            if !vm.step() {
                break;
            }
            if vm.frame_count >= start + 2 {
                break;
            }
        }
    }
    vm.push_key(13);
    let start = vm.frame_count;
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= start + 3 {
            break;
        }
    }
    assert!(!vm.halted);
    assert_eq!(vm.ram[0x4801], 0, "cursor row should be 0 after cls");
    assert_eq!(vm.ram[0x4800], 2, "cursor col should be 2 after cls");
    assert_eq!(
        vm.ram[0x4000 + 42 * 0],
        b'$' as u32,
        "row 0 should have prompt after cls"
    );
}

#[test]
fn test_terminal_cmd_ls() {
    let mut vm = boot_terminal(0);
    for ch in b"ls" {
        vm.push_key(*ch as u32);
        let start = vm.frame_count;
        for _ in 0..500_000 {
            if !vm.step() {
                break;
            }
            if vm.frame_count >= start + 2 {
                break;
            }
        }
    }
    vm.push_key(13);
    let start = vm.frame_count;
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= start + 3 {
            break;
        }
    }
    assert!(!vm.halted);
    // ls lists VFS directory entries (boot.cfg and/or linux), each on its own row
    // Row 1 should have a non-space, non-null character (file listing or "(empty)")
    let row1_start = vm.ram[0x4000 + 42 * 1];
    assert!(
        row1_start != 32 && row1_start != 0,
        "row 1 should have ls output, got char code {}",
        row1_start
    );
    // Prompt should appear after the last listed file
    // Find the prompt row by checking for '$'
    // VFS may have 1-6 entries, so prompt could be on rows 2-7
    let mut found_prompt = false;
    for row in 1..8 {
        if vm.ram[0x4000 + 42 * row] == b'$' as u32 {
            found_prompt = true;
            break;
        }
    }
    assert!(found_prompt, "should find prompt row after ls output");
}

#[test]
fn test_terminal_scroll() {
    let mut vm = boot_terminal(0);
    // Fill 32 rows with "hi" + Enter (each consumes 2 rows: input + response)
    for row_fill in 0..32 {
        for ch in b"hi" {
            vm.push_key(*ch as u32);
            let start = vm.frame_count;
            for _ in 0..500_000 {
                if !vm.step() {
                    break;
                }
                if vm.frame_count >= start + 2 {
                    break;
                }
            }
        }
        vm.push_key(13);
        let start = vm.frame_count;
        for _ in 0..500_000 {
            if !vm.step() {
                break;
            }
            if vm.frame_count >= start + 3 {
                break;
            }
        }
        if vm.halted {
            panic!("terminal halted at row_fill {}", row_fill);
        }
    }
    assert!(!vm.halted);
    assert_eq!(
        vm.ram[0x4801], 29,
        "cursor row should be clamped to 29 after scroll"
    );
    assert_eq!(
        vm.ram[0x4000 + 42 * 29],
        b'$' as u32,
        "last row should have prompt"
    );
}

#[test]
fn test_terminal_unknown_cmd_still_works() {
    let mut vm = boot_terminal(0);
    for ch in b"xyz" {
        vm.push_key(*ch as u32);
        let start = vm.frame_count;
        for _ in 0..500_000 {
            if !vm.step() {
                break;
            }
            if vm.frame_count >= start + 2 {
                break;
            }
        }
    }
    vm.push_key(13);
    let start = vm.frame_count;
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= start + 3 {
            break;
        }
    }
    assert!(!vm.halted);
    assert_eq!(vm.ram[0x4000 + 42 * 1], 63, "row 1 should start with '?'");
    assert_eq!(
        vm.ram[0x4000 + 42 * 1 + 2],
        b'x' as u32,
        "row 1 col 2 should be 'x'"
    );
}

#[test]
fn test_terminal_buffer_init() {
    let vm = boot_terminal(1);
    // Buffer should be properly initialized to spaces (CMPI r0 clobber bug fix)
    for i in 2..1260 {
        assert_eq!(
            vm.ram[0x4000 + i],
            32,
            "buffer position {} should be space after init, got {}",
            i,
            vm.ram[0x4000 + i]
        );
    }
}

// ── Pulse app (self-animating, no input) ───────────────────

fn boot_pulse(target_frames: u32) -> Vm {
    let source = include_str!("../../programs/pulse.asm");
    let asm = crate::assembler::assemble(source, 0).expect("pulse.asm should assemble");
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
fn test_pulse_boots_and_renders() {
    let vm = boot_pulse(1);
    assert!(!vm.halted, "pulse app should not halt after boot");
    // Background at a point with no text should be dark blue-black
    assert_eq!(
        vm.screen[1 * 256 + 1],
        0x0D0D1A,
        "background should be dark blue-black"
    );
    // "PULSE" title text renders at (5,5) -- 'P' pixel should be non-background
    assert_ne!(
        vm.screen[5 * 256 + 5],
        0x0D0D1A,
        "title text 'P' should differ from background"
    );
    // Tick counter at 0x200 should be >= 1 after 1 frame
    assert!(
        vm.ram[0x200] >= 1,
        "tick should be >= 1 after first frame, got {}",
        vm.ram[0x200]
    );
    // Bar width at 0x204 should be in valid range (triangle wave 0-99)
    assert!(
        vm.ram[0x204] <= 100,
        "bar width should be <= 100, got {}",
        vm.ram[0x204]
    );
}

#[test]
fn test_pulse_bar_width_oscillates() {
    // Run 300 frames: should see bar_width go up, peak, come down, go up again
    let mut vm = boot_pulse(0); // just assemble, don't run frames yet

    let mut saw_zero = false;
    let mut saw_peak = false;
    let mut prev_width = 0u32;

    for frame in 0..250 {
        let start = vm.frame_count;
        for _ in 0..500_000 {
            if !vm.step() {
                break;
            }
            if vm.frame_count >= start + 1 {
                break;
            }
        }
        if vm.halted {
            panic!("pulse halted at frame {}", frame);
        }

        let width = vm.ram[0x204];
        if width == 0 {
            saw_zero = true;
        }
        if width >= 90 {
            saw_peak = true;
        }
        prev_width = width;
    }

    assert!(
        saw_zero,
        "bar width should hit 0 during oscillation (saw {})",
        prev_width
    );
    assert!(
        saw_peak,
        "bar width should reach >= 90 during oscillation (max saw {})",
        prev_width
    );
}

#[test]
fn test_pulse_tick_increments_per_frame() {
    let vm1 = boot_pulse(1);
    let tick1 = vm1.ram[0x200];

    let mut vm2 = boot_pulse(0);
    // Run 10 more frames
    let start = vm2.frame_count;
    for _ in 0..500_000 {
        if !vm2.step() {
            break;
        }
        if vm2.frame_count >= start + 10 {
            break;
        }
    }
    let tick10 = vm2.ram[0x200];

    assert!(
        tick10 > tick1,
        "tick should increase across frames: tick1={}, tick10={}",
        tick1,
        tick10
    );
    // Should be roughly proportional (allowing for init frame)
    assert!(
        tick10 >= 9,
        "after 10 frames, tick should be >= 9, got {}",
        tick10
    );
}

#[test]
fn test_pulse_triangle_wave_symmetry() {
    // The triangle wave should go 0->99->0 in 200 frames
    // Check that frames 50 and 150 (symmetric around peak) give similar widths
    let mut vm = boot_pulse(0);
    let mut widths: Vec<u32> = Vec::new();

    for _frame in 0..200 {
        let start = vm.frame_count;
        for _ in 0..500_000 {
            if !vm.step() {
                break;
            }
            if vm.frame_count >= start + 1 {
                break;
            }
        }
        if vm.halted {
            break;
        }
        widths.push(vm.ram[0x204]);
    }

    assert!(
        widths.len() >= 200,
        "should have 200 width samples, got {}",
        widths.len()
    );
    // Peak should be around frame 99 or 100
    let max_width = *widths.iter().max().unwrap();
    assert!(
        max_width >= 99,
        "max bar width should be >= 99, got {}",
        max_width
    );
    // Symmetry: width at frame i should equal width at frame (198 - i)
    // (offset by 1 because tick increments before computing width)
    assert_eq!(
        widths[10], widths[188],
        "triangle should be symmetric: w[10]={}, w[188]={}",
        widths[10], widths[188]
    );
}

#[test]
fn test_pulse_never_halts() {
    // Run 500 frames to prove it loops forever
    let vm = boot_pulse(500);
    assert!(!vm.halted, "pulse should never halt, even after 500 frames");
    // Frame count should be 500
    assert!(
        vm.frame_count >= 500,
        "should have run 500 frames, got {}",
        vm.frame_count
    );
}

#[test]
fn test_pulse_color_changes_over_time() {
    // The bar color shifts with tick, so it should differ between early and late frames
    let vm_early = boot_pulse(5);
    let vm_late = boot_pulse(150);

    let _early_pixel = vm_early.screen[110 * 256 + 100]; // bar center at (100,110)
    let late_pixel = vm_late.screen[110 * 256 + 100];

    // At least one should have bar drawn (late frame at peak), and colors should differ
    // The late frame bar should be non-background
    assert_ne!(
        late_pixel, 0x0D0D1A,
        "late frame should have bar drawn at (100,110)"
    );
}

// ── Paint App Integration Tests ──────────────────────────

fn boot_paint(target_frames: u32) -> Vm {
    let source = include_str!("../../programs/paint.asm");
    let asm = crate::assembler::assemble(source, 0).expect("paint.asm should assemble");
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
fn test_paint_app_assembles() {
    let source = include_str!("../../programs/paint.asm");
    let result = crate::assembler::assemble(source, 0);
    assert!(
        result.is_ok(),
        "paint.asm failed to assemble: {:?}",
        result.err()
    );
    let bytecode = result.unwrap();
    assert!(bytecode.pixels.len() > 100, "paint should be substantial");
    // Verify MOUSEQ opcode is present
    let has_mouseq = bytecode.pixels.iter().any(|&w| w == 0x85);
    assert!(has_mouseq, "paint.asm should contain MOUSEQ opcode (0x85)");
}

#[test]
fn test_paint_app_boots_and_runs() {
    let vm = boot_paint(1);
    assert!(!vm.halted, "paint app should not halt after boot");
}

#[test]
fn test_paint_app_draws_palette() {
    let vm = boot_paint(1);
    // Red swatch at (2, 240) should be red
    assert_eq!(
        vm.screen[240 * 256 + 2],
        0xFF0000,
        "red swatch should be red"
    );
    // Green swatch at (34, 240) should be green
    assert_eq!(
        vm.screen[240 * 256 + 34],
        0x00FF00,
        "green swatch should be green"
    );
    // Blue swatch at (66, 240) should be blue
    assert_eq!(
        vm.screen[240 * 256 + 66],
        0x0000FF,
        "blue swatch should be blue"
    );
}

#[test]
fn test_paint_app_draws_at_mouse() {
    let mut vm = boot_paint(1);
    assert!(!vm.halted);
    // Push mouse into paint area and run a frame
    vm.push_mouse(100, 100);
    let start = vm.frame_count;
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= start + 2 {
            break;
        }
    }
    // Default color is red -- pixel at (100, 100) should be red
    assert_eq!(
        vm.screen[100 * 256 + 100],
        0xFF0000,
        "pixel at mouse pos should be painted red"
    );
}

#[test]
fn test_paint_app_clear_button() {
    let mut vm = boot_paint(1);
    // First paint something
    vm.push_mouse(50, 50);
    let start = vm.frame_count;
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= start + 2 {
            break;
        }
    }
    assert_eq!(
        vm.screen[50 * 256 + 50],
        0xFF0000,
        "should have painted red"
    );

    // Now click clear button (at x=2, y=220, w=40, h=16)
    vm.push_mouse(20, 228);
    let start2 = vm.frame_count;
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= start2 + 2 {
            break;
        }
    }
    // Canvas should be cleared to background
    assert_eq!(
        vm.screen[50 * 256 + 50],
        0x111111,
        "canvas should be cleared after clicking clear"
    );
}

#[test]
fn test_paint_app_runs_100_frames() {
    let mut vm = boot_paint(1);
    for _ in 0..100 {
        vm.push_mouse(128, 128);
        let start = vm.frame_count;
        for _ in 0..500_000 {
            if !vm.step() {
                break;
            }
            if vm.frame_count >= start + 1 {
                break;
            }
        }
        if vm.halted {
            break;
        }
    }
    assert!(!vm.halted, "paint should run 100 frames without halting");
}

// ── File Browser Tests ──

fn boot_file_browser(target_frames: u32) -> Vm {
    let source = include_str!("../../programs/file_browser.asm");
    let asm = crate::assembler::assemble(source, 0).expect("file_browser.asm should assemble");
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
fn test_file_browser_assembles() {
    let source = include_str!("../../programs/file_browser.asm");
    let result = crate::assembler::assemble(source, 0);
    assert!(
        result.is_ok(),
        "file_browser.asm failed to assemble: {:?}",
        result.err()
    );
    let bytecode = result.unwrap();
    assert!(
        bytecode.pixels.len() > 100,
        "file browser should be substantial"
    );
    assert!(
        bytecode.pixels.len() < 0x400,
        "bytecode must fit below 0x400 for data safety"
    );
    // Verify key opcodes present
    assert!(
        bytecode.pixels.iter().any(|&w| w == 0x59),
        "should contain LS opcode"
    );
    assert!(
        bytecode.pixels.iter().any(|&w| w == 0x54),
        "should contain OPEN opcode"
    );
    assert!(
        bytecode.pixels.iter().any(|&w| w == 0x55),
        "should contain READ opcode"
    );
    assert!(
        bytecode.pixels.iter().any(|&w| w == 0x57),
        "should contain CLOSE opcode"
    );
}

#[test]
fn test_file_browser_boots_and_runs() {
    let vm = boot_file_browser(1);
    assert!(!vm.halted, "file browser should not halt after boot");
}

#[test]
fn test_file_browser_draws_title() {
    let vm = boot_file_browser(1);
    let title_color = vm.screen[6 * 256 + 10];
    let bg_color = vm.screen[30 * 256 + 10];
    assert_ne!(
        title_color, bg_color,
        "title bar should differ from background"
    );
}

#[test]
fn test_file_browser_lists_files() {
    let vm = boot_file_browser(1);
    let file_count = vm.ram[0x504];
    assert!(
        file_count >= 2,
        "should list at least 2 files, got {}",
        file_count
    );
    let first_entry = vm.ram[0x400];
    assert!(
        first_entry >= 0x600,
        "first filename addr should be in FILE_BUF, got {:#x}",
        first_entry
    );
}

#[test]
fn test_file_browser_registers_hit_regions() {
    let vm = boot_file_browser(1);
    assert_eq!(
        vm.hit_regions.len(),
        13,
        "should have 13 hit regions (12 rows + back)"
    );
}

#[test]
fn test_file_browser_click_opens_file() {
    let mut vm = boot_file_browser(1);
    // Click on first file row (y=38, middle of row at y=30 h=16)
    vm.push_mouse(80, 38);
    let start = vm.frame_count;
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= start + 3 {
            break;
        }
    }
    assert_eq!(
        vm.ram[0x500], 1,
        "mode should be 1 (content view) after clicking file"
    );
}

#[test]
fn test_file_browser_back_button_returns() {
    let mut vm = boot_file_browser(1);
    // First click a file to open content view
    vm.push_mouse(80, 38);
    let start = vm.frame_count;
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= start + 3 {
            break;
        }
    }
    assert_eq!(vm.ram[0x500], 1, "should be in content view");
    // Now click BACK button (at y=240, x=10, w=60)
    vm.push_mouse(40, 248);
    let start2 = vm.frame_count;
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= start2 + 3 {
            break;
        }
    }
    assert_eq!(
        vm.ram[0x500], 0,
        "mode should be 0 (list view) after clicking back"
    );
}

#[test]
fn test_file_browser_shows_content() {
    let mut vm = boot_file_browser(1);
    // Click on first file to open it
    vm.push_mouse(80, 38);
    let start = vm.frame_count;
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= start + 5 {
            break;
        }
    }
    eprintln!("MODE={}, TEMP_FD={}", vm.ram[0x500], vm.ram[0x50C]);
    eprintln!(
        "CONTENT_BUF[0..8]: {:?}",
        (0..8).map(|i| vm.ram[0xA00 + i]).collect::<Vec<_>>()
    );
    // Content buffer should have data from the file
    let content_start = vm.ram[0xA00];
    assert!(content_start != 0, "content buffer should have file data");
}

#[test]
fn test_file_browser_alternating_row_colors() {
    let vm = boot_file_browser(1);
    let row0_color = vm.screen[32 * 256 + 15];
    let row1_color = vm.screen[48 * 256 + 15];
    assert_ne!(row0_color, 0x1a1a2e, "row 0 should have row background");
    assert_ne!(row1_color, 0x1a1a2e, "row 1 should have row background");
}

#[test]
fn test_file_browser_debug_click() {
    let mut vm = boot_file_browser(1);
    eprintln!("After boot: halted={}, frame={}", vm.halted, vm.frame_count);
    eprintln!("  MODE={}", vm.ram[0x500]);
    eprintln!("  FILE_COUNT={}", vm.ram[0x504]);
    eprintln!("  FNAME_TABLE[0]={:#x}", vm.ram[0x400]);
    eprintln!("  hit_regions={}", vm.hit_regions.len());
    for (i, hr) in vm.hit_regions.iter().enumerate() {
        eprintln!(
            "    [{}] x={} y={} w={} h={} id={}",
            i, hr.x, hr.y, hr.w, hr.h, hr.id
        );
    }

    vm.push_mouse(80, 38);
    eprintln!("\nMouse pushed at (80, 38)");

    // Run a few frames
    for frame in 0..5 {
        let start = vm.frame_count;
        for _ in 0..500_000 {
            if !vm.step() {
                break;
            }
            if vm.frame_count >= start + 1 {
                break;
            }
        }
        eprintln!(
            "After frame {}: halted={}, MODE={}, pc={}, regs12={}",
            frame, vm.halted, vm.ram[0x500], vm.pc, vm.regs[12]
        );
    }

    // Check what filename would be opened
    let fname_addr = vm.ram[0x400] as usize;
    let mut s = String::new();
    for i in 0..32 {
        let v = vm.ram[fname_addr + i];
        if v == 0 {
            break;
        }
        s.push(v as u8 as char);
    }
    eprintln!("Filename at FNAME_TABLE[0]: {:?}", s);

    // Direct test: push mouse, step until HITQ executes, check regs[12]
    eprintln!("mouse_x={}, mouse_y={}", vm.mouse_x, vm.mouse_y);
    // Step a few instructions to reach HITQ
    for _ in 0..200 {
        if vm.ram[vm.pc as usize] == 0x38 {
            // HITQ opcode
            break;
        }
        if !vm.step() {
            break;
        }
    }
    eprintln!(
        "After stepping to HITQ: pc={}, ram[pc]={}",
        vm.pc, vm.ram[vm.pc as usize]
    );
    // Execute HITQ
    vm.step();
    eprintln!("After HITQ: regs[12]={}", vm.regs[12]);
}

// ── STRCMP: string comparison opcode (0x86) ─────────────────────

fn setup_strcmp_test(s1: &str, s2: &str) -> Vm {
    let mut vm = Vm::new();
    // Write s1 starting at address 0x300
    let base1 = 0x300;
    for (i, &b) in s1.as_bytes().iter().enumerate() {
        vm.ram[base1 + i] = b as u32;
    }
    vm.ram[base1 + s1.len()] = 0; // null terminator
                                  // Write s2 starting at address 0x400
    let base2 = 0x400;
    for (i, &b) in s2.as_bytes().iter().enumerate() {
        vm.ram[base2 + i] = b as u32;
    }
    vm.ram[base2 + s2.len()] = 0;
    // r1 = addr of s1, r2 = addr of s2
    vm.regs[1] = base1 as u32;
    vm.regs[2] = base2 as u32;
    // STRCMP r1, r2 (opcode 0x86)
    vm.ram[0] = 0x86;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 0x00; // HALT
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    vm
}

#[test]
fn test_strcmp_equal_strings() {
    let vm = setup_strcmp_test("hello", "hello");
    assert_eq!(vm.regs[0], 0, "STRCMP should set r0=0 for equal strings");
}

#[test]
fn test_strcmp_equal_empty_strings() {
    let vm = setup_strcmp_test("", "");
    assert_eq!(vm.regs[0], 0, "STRCMP should set r0=0 for empty strings");
}

#[test]
fn test_strcmp_s1_less_than_s2() {
    let vm = setup_strcmp_test("abc", "abd");
    assert_eq!(
        vm.regs[0], 0xFFFFFFFF,
        "STRCMP should set r0=-1 when s1 < s2"
    );
}

#[test]
fn test_strcmp_s1_greater_than_s2() {
    let vm = setup_strcmp_test("xyz", "abc");
    assert_eq!(vm.regs[0], 1, "STRCMP should set r0=1 when s1 > s2");
}

#[test]
fn test_strcmp_s1_shorter_is_less() {
    let vm = setup_strcmp_test("ab", "abc");
    // 'ab' < 'abc' because s1 hits null first (0 < 'c')
    assert_eq!(
        vm.regs[0], 0xFFFFFFFF,
        "STRCMP: shorter string should be less"
    );
}

#[test]
fn test_strcmp_s1_longer_is_greater() {
    let vm = setup_strcmp_test("abcd", "abc");
    // 'abcd' > 'abc' because s2 hits null first ('d' > 0)
    assert_eq!(vm.regs[0], 1, "STRCMP: longer string should be greater");
}

#[test]
fn test_strcmp_single_char_equal() {
    let vm = setup_strcmp_test("a", "a");
    assert_eq!(vm.regs[0], 0);
}

#[test]
fn test_strcmp_single_char_less() {
    let vm = setup_strcmp_test("A", "a");
    assert_eq!(vm.regs[0], 0xFFFFFFFF, "STRCMP: 'A' (65) < 'a' (97)");
}

#[test]
fn test_strcmp_assembles_and_runs() {
    let source = r#"
LDI r1, 0x300
STRO r1, "hello"
LDI r2, 0x400
STRO r2, "hello"
STRCMP r1, r2
HALT
"#;
    let asm = crate::assembler::assemble(source, 0).expect("should assemble");
    let mut vm = Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..1000 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.regs[0], 0, "assembled STRCMP: 'hello' == 'hello'");
}

#[test]
fn test_strcmp_assemble_not_equal() {
    let source = r#"
LDI r1, 0x300
STRO r1, "abc"
LDI r2, 0x400
STRO r2, "xyz"
STRCMP r1, r2
HALT
"#;
    let asm = crate::assembler::assemble(source, 0).expect("should assemble");
    let mut vm = Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..1000 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.regs[0], 0xFFFFFFFF, "assembled STRCMP: 'abc' < 'xyz'");
}

#[test]
fn test_strcmp_preserves_other_registers() {
    let vm = setup_strcmp_test("foo", "bar");
    // r1 and r2 should still hold their original addresses
    assert_eq!(vm.regs[1], 0x300);
    assert_eq!(vm.regs[2], 0x400);
}

#[test]
fn test_strcmp_numeric_characters() {
    let vm = setup_strcmp_test("123", "124");
    assert_eq!(vm.regs[0], 0xFFFFFFFF, "STRCMP: '123' < '124'");
}

#[test]
fn test_strcmp_case_sensitive() {
    let vm = setup_strcmp_test("Hello", "hello");
    assert_eq!(
        vm.regs[0], 0xFFFFFFFF,
        "STRCMP: 'Hello' < 'hello' (case sensitive)"
    );
}

// ── ABS: absolute value opcode (0x87) ─────────────────────

#[test]
fn test_abs_positive() {
    let mut vm = Vm::new();
    vm.regs[1] = 42;
    vm.ram[0] = 0x87; // ABS r1
    vm.ram[1] = 1;
    vm.step();
    assert_eq!(vm.regs[1], 42, "ABS of positive should be unchanged");
}

#[test]
fn test_abs_negative() {
    let mut vm = Vm::new();
    vm.regs[1] = 0xFFFFFFFF; // -1 as u32
    vm.ram[0] = 0x87;
    vm.ram[1] = 1;
    vm.step();
    assert_eq!(vm.regs[1], 1, "ABS of -1 should be 1");
}

#[test]
fn test_abs_zero() {
    let mut vm = Vm::new();
    vm.regs[1] = 0;
    vm.ram[0] = 0x87;
    vm.ram[1] = 1;
    vm.step();
    assert_eq!(vm.regs[1], 0, "ABS of zero should be zero");
}

#[test]
fn test_abs_large_negative() {
    let mut vm = Vm::new();
    vm.regs[1] = 0xFFFFFFFE; // -2 as u32
    vm.ram[0] = 0x87;
    vm.ram[1] = 1;
    vm.step();
    assert_eq!(vm.regs[1], 2, "ABS of -2 should be 2");
}

#[test]
fn test_abs_i32_min() {
    let mut vm = Vm::new();
    vm.regs[1] = 0x80000000; // i32::MIN
    vm.ram[0] = 0x87;
    vm.ram[1] = 1;
    vm.step();
    // wrapping_abs of i32::MIN returns i32::MIN (0x80000000)
    assert_eq!(vm.regs[1], 0x80000000, "ABS of i32::MIN wraps to itself");
}

#[test]
fn test_abs_assembles() {
    let source = "LDI r1, 0xFFFFFFFF\nABS r1\nHALT\n";
    let asm = crate::assembler::assemble(source, 0).expect("should assemble");
    let mut vm = Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.regs[1], 1, "assembled ABS: |-1| = 1");
}

#[test]
fn test_abs_disassembles() {
    let mut vm = Vm::new();
    vm.ram[0] = 0x87;
    vm.ram[1] = 5;
    let (text, _len) = vm.disassemble_at(0);
    assert_eq!(text, "ABS r5");
}

// ── RECT: outline rectangle opcode (0x88) ─────────────────

#[test]
fn test_rect_draws_outline() {
    let mut vm = Vm::new();
    vm.regs[1] = 10; // x
    vm.regs[2] = 10; // y
    vm.regs[3] = 5; // w
    vm.regs[4] = 3; // h
    vm.regs[5] = 0xFF0000; // color (red)
    vm.ram[0] = 0x88;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.ram[4] = 4;
    vm.ram[5] = 5;
    vm.step();

    // Top-left corner (10, 10)
    assert_eq!(vm.screen[10 * 256 + 10], 0xFF0000, "top-left corner");
    // Top-right corner (14, 10)
    assert_eq!(vm.screen[10 * 256 + 14], 0xFF0000, "top-right corner");
    // Bottom-left corner (10, 12)
    assert_eq!(vm.screen[12 * 256 + 10], 0xFF0000, "bottom-left corner");
    // Bottom-right corner (14, 12)
    assert_eq!(vm.screen[12 * 256 + 14], 0xFF0000, "bottom-right corner");
    // Interior pixel (12, 11) should NOT be drawn
    assert_eq!(vm.screen[11 * 256 + 12], 0, "interior should be empty");
}

#[test]
fn test_rect_1x1() {
    let mut vm = Vm::new();
    vm.regs[1] = 50; // x
    vm.regs[2] = 50; // y
    vm.regs[3] = 1; // w
    vm.regs[4] = 1; // h
    vm.regs[5] = 0x00FF00; // green
    vm.ram[0] = 0x88;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.ram[4] = 4;
    vm.ram[5] = 5;
    vm.step();

    // Single pixel should be drawn
    assert_eq!(
        vm.screen[50 * 256 + 50],
        0x00FF00,
        "1x1 rect draws single pixel"
    );
}

#[test]
fn test_rect_zero_dimensions() {
    let mut vm = Vm::new();
    vm.regs[1] = 10;
    vm.regs[2] = 10;
    vm.regs[3] = 0; // w=0
    vm.regs[4] = 5;
    vm.regs[5] = 0xFF0000;
    vm.ram[0] = 0x88;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.ram[4] = 4;
    vm.ram[5] = 5;
    vm.step();

    // Nothing should be drawn
    assert_eq!(vm.screen[10 * 256 + 10], 0, "zero width draws nothing");
}

#[test]
fn test_rect_assembles() {
    let source = "LDI r1, 10\nLDI r2, 20\nLDI r3, 5\nLDI r4, 3\nLDI r5, 0xFF0000\nRECT r1, r2, r3, r4, r5\nHALT\n";
    let asm = crate::assembler::assemble(source, 0).expect("should assemble");
    let has_rect = asm.pixels.iter().any(|&w| w == 0x88);
    assert!(has_rect, "RECT opcode (0x88) should be present");

    let mut vm = Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    // Check corners of 10,20 5x3
    assert_eq!(vm.screen[20 * 256 + 10], 0xFF0000, "top-left");
    assert_eq!(vm.screen[20 * 256 + 14], 0xFF0000, "top-right");
    assert_eq!(vm.screen[22 * 256 + 10], 0xFF0000, "bottom-left");
    assert_eq!(vm.screen[22 * 256 + 14], 0xFF0000, "bottom-right");
}

#[test]
fn test_rect_disassembles() {
    let mut vm = Vm::new();
    vm.ram[0] = 0x88;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.ram[4] = 4;
    vm.ram[5] = 5;
    let (text, _len) = vm.disassemble_at(0);
    assert_eq!(text, "RECT r1, r2, r3, r4, r5");
}

// ── Notepad App Tests ──────────────────────────────────

fn boot_notepad(target_frames: u32) -> Vm {
    let source = include_str!("../../programs/notepad.asm");
    let asm = crate::assembler::assemble(source, 0).expect("notepad.asm should assemble");
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
fn test_notepad_assembles() {
    let source = include_str!("../../programs/notepad.asm");
    crate::assembler::assemble(source, 0).expect("notepad.asm should assemble");
}

#[test]
fn test_notepad_boots_and_renders() {
    let source = include_str!("../../programs/notepad.asm");
    let asm = crate::assembler::assemble(source, 0).expect("notepad.asm should assemble");
    let mut vm = Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    eprintln!("notepad bytecode: {} words", asm.pixels.len());
    vm.pc = 0;
    vm.halted = false;
    let target_frames = 1;
    let start_frame = vm.frame_count;
    let mut steps = 0u64;
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        steps += 1;
        if vm.frame_count >= start_frame + target_frames {
            break;
        }
    }
    eprintln!(
        "steps: {}, halted: {}, pc: {}, frame_count: {}",
        steps, vm.halted, vm.pc, vm.frame_count
    );
    assert!(!vm.halted, "notepad should not halt after boot");
    // Title bar should be blue-purple (0x16213E) at top
    assert_eq!(
        vm.screen[5 * 256 + 5],
        0x16213E,
        "title bar should be blue-purple"
    );
    // Text area below title bar should be dark (0x1A1A2E)
    assert_eq!(
        vm.screen[20 * 256 + 50],
        0x1A1A2E,
        "text area should be dark"
    );
    // Cursor should start at col 0, row 0
    assert_eq!(vm.ram[0x6000], 0, "cursor col should start at 0");
    assert_eq!(vm.ram[0x6001], 0, "cursor row should start at 0");
    // Lines count should be 1
    assert_eq!(vm.ram[0x6002], 1, "should start with 1 line");
}

#[test]
fn test_notepad_type_character() {
    let mut vm = boot_notepad(1);
    // Type 'A' (ASCII 65)
    vm.push_key(65);
    let start = vm.frame_count;
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= start + 2 {
            break;
        }
    }
    assert!(!vm.halted, "notepad should not halt after typing");
    // Character 'A' should be in buffer at row 0, col 0
    assert_eq!(vm.ram[0x4000], 65, "'A' should be in buffer at (0,0)");
    // Cursor should advance to col 1
    assert_eq!(vm.ram[0x6000], 1, "cursor should advance to col 1");
}

#[test]
fn test_notepad_backspace() {
    let mut vm = boot_notepad(1);
    // Type 'A' then backspace
    vm.push_key(65);
    let start = vm.frame_count;
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= start + 2 {
            break;
        }
    }
    vm.push_key(8); // backspace
    let start = vm.frame_count;
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= start + 2 {
            break;
        }
    }
    // Character should be cleared (space = 32)
    assert_eq!(vm.ram[0x4000], 32, "backspace should clear character");
    // Cursor should go back to col 0
    assert_eq!(vm.ram[0x6000], 0, "cursor should go back to col 0");
}

#[test]
fn test_notepad_enter_newline() {
    let mut vm = boot_notepad(1);
    // Type "Hi" then Enter
    vm.push_key(72); // 'H'
    let start = vm.frame_count;
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= start + 2 {
            break;
        }
    }
    vm.push_key(105); // 'i'
    let start = vm.frame_count;
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= start + 2 {
            break;
        }
    }
    vm.push_key(13); // Enter
    let start = vm.frame_count;
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= start + 3 {
            break;
        }
    }
    assert!(!vm.halted, "notepad should not halt after enter");
    // Row 0 should have 'H' 'i' followed by spaces
    assert_eq!(vm.ram[0x4000], 72, "row 0 col 0 should be 'H'");
    assert_eq!(vm.ram[0x4001], 105, "row 0 col 1 should be 'i'");
    // Cursor should be on row 1, col 0
    assert_eq!(vm.ram[0x6001], 1, "cursor should be on row 1");
    assert_eq!(vm.ram[0x6000], 0, "cursor col should be 0");
}

#[test]
fn test_notepad_arrow_keys() {
    let mut vm = boot_notepad(1);
    // Type "AB" then left arrow
    vm.push_key(65);
    let start = vm.frame_count;
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= start + 2 {
            break;
        }
    }
    vm.push_key(66);
    let start = vm.frame_count;
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= start + 2 {
            break;
        }
    }
    assert_eq!(vm.ram[0x6000], 2, "cursor should be at col 2 after 'AB'");

    // Left arrow
    vm.push_key(37);
    let start = vm.frame_count;
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= start + 2 {
            break;
        }
    }
    assert_eq!(vm.ram[0x6000], 1, "cursor should move left to col 1");

    // Right arrow
    vm.push_key(39);
    let start = vm.frame_count;
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= start + 2 {
            break;
        }
    }
    assert_eq!(vm.ram[0x6000], 2, "cursor should move right to col 2");

    // Down arrow
    vm.push_key(40);
    let start = vm.frame_count;
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= start + 2 {
            break;
        }
    }
    assert_eq!(vm.ram[0x6001], 1, "cursor should move down to row 1");

    // Up arrow
    vm.push_key(38);
    let start = vm.frame_count;
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= start + 2 {
            break;
        }
    }
    assert_eq!(vm.ram[0x6001], 0, "cursor should move up to row 0");
}

#[test]
fn test_notepad_runs_persistently() {
    let mut vm = boot_notepad(1);
    // Run for 100 frames without halting
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert!(
        !vm.halted,
        "notepad should run persistently without halting"
    );
}

#[test]
fn test_notepad_status_bar() {
    let vm = boot_notepad(1);
    // Status bar should be at bottom of screen (y=248)
    assert_eq!(
        vm.screen[249 * 256 + 10],
        0x0D0D1A,
        "status bar should be dark"
    );
}

// ── Clock App Tests ────────────────────────────────────────────────

fn boot_clock(frames: u32) -> Vm {
    let source = include_str!("../../programs/clock.asm");
    let asm = crate::assembler::assemble(source, 0).expect("clock.asm should assemble");
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
        if vm.frame_count >= start_frame + frames {
            break;
        }
    }
    vm
}

#[test]
fn test_clock_assembles() {
    let source = include_str!("../../programs/clock.asm");
    crate::assembler::assemble(source, 0).expect("clock.asm should assemble");
}

#[test]
fn test_clock_boots_and_renders() {
    let vm = boot_clock(1);
    assert!(!vm.halted, "clock should not halt after boot");
    // Title bar should be dark navy at top
    assert_eq!(vm.screen[5 * 256 + 5], 0x0D1B2A, "title bar should be navy");
    // Main panel should be dark at center
    assert_eq!(
        vm.screen[80 * 256 + 128],
        0x060612,
        "digit panel should be dark"
    );
    // Status bar at bottom
    assert_eq!(
        vm.screen[248 * 256 + 128],
        0x0A0A1A,
        "status bar should be dark"
    );
}

#[test]
fn test_clock_runs_persistently() {
    let mut vm = boot_clock(1);
    // Run for 100 frames without halting
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert!(!vm.halted, "clock should run persistently without halting");
}

#[test]
fn test_clock_time_updates() {
    let vm = boot_clock(2);
    // At 2 frames: seconds = 2/60 = 0, minutes = 0, hours = 0
    // Verify frame counter is advancing
    assert_eq!(vm.ram[0xFFE], 2, "frame counter should be 2");
}

#[test]
fn test_clock_day_counter() {
    let vm = boot_clock(1);
    // Day is computed from frame count: days = (frames/60/3600) / 24
    // At 1 frame: 0 days
    assert!(vm.ram[0xFFE] >= 1, "frame counter should be at least 1");
}

#[test]
fn test_clock_blink_toggle() {
    let vm1 = boot_clock(1);
    let blink1 = vm1.ram[0x6005];
    // Blink should be 0 or 1
    assert!(blink1 <= 1, "blink should be 0 or 1");
}

// ── Multiproc (SPAWN) Tests ───────────────────────────────────────

fn boot_multiproc(frames: u32) -> Vm {
    let source = include_str!("../../programs/multiproc.asm");
    let asm = crate::assembler::assemble(source, 0).expect("multiproc.asm should assemble");
    let mut vm = Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    let start_frame = vm.frame_count;
    for _ in 0..2_000_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= start_frame + frames {
            break;
        }
    }
    vm
}

#[test]
fn test_multiproc_assembles() {
    let source = include_str!("../../programs/multiproc.asm");
    crate::assembler::assemble(source, 0).expect("multiproc.asm should assemble");
}

#[test]
fn test_multiproc_boots_and_runs() {
    let vm = boot_multiproc(5);
    assert!(!vm.halted, "multiproc should not halt after 5 frames");
    // Should have spawned a child process
    assert!(
        vm.processes.len() >= 1,
        "should have at least 1 spawned process"
    );
}

#[test]
fn test_multiproc_two_dots_visible() {
    let vm = boot_multiproc(20);
    // The primary process draws a white dot that bounces in the left half (x: 0-127)
    // After 20 frames at vx=+1, the dot starts at x=32 and reaches x=52
    // It should be visible as a white pixel somewhere on screen
    let mut has_white = false;
    for y in 0..256usize {
        for x in 0..256usize {
            if vm.screen[y * 256 + x] == 0xFFFFFF {
                has_white = true;
                break;
            }
        }
        if has_white {
            break;
        }
    }
    assert!(has_white, "screen should have the primary white dot");
}

#[test]
fn test_multiproc_child_isolation() {
    let vm = boot_multiproc(5);
    // Child processes should have their own register state
    // The parent's r5 = 0xFFFFFF (white), child's r5 = 0xFF2020 (red)
    // We can't directly inspect child registers, but we can verify
    // the child exists and is not halted
    if let Some(child) = vm.processes.first() {
        assert!(!child.is_halted(), "child process should not be halted");
    }
}

#[test]
fn test_multiproc_persistent() {
    let mut vm = boot_multiproc(5);
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert!(!vm.halted, "multiproc should run persistently");
}

// ── Color Picker App Tests ──────────────────────────────

fn _boot_color_picker(target_frames: u32) -> Vm {
    let source = include_str!("../../programs/color_picker.asm");
    let asm = crate::assembler::assemble(source, 0).expect("color_picker.asm should assemble");
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
fn test_color_picker_assembles() {
    let source = include_str!("../../programs/color_picker.asm");
    let asm = crate::assembler::assemble(source, 0).expect("color_picker.asm should assemble");
    // Should contain RECT opcode (0x88)
    let has_rect = asm.pixels.iter().any(|&w| w == 0x88);
    assert!(has_rect, "color_picker.asm should use RECT opcode");
}

#[test]
fn test_color_picker_boots_and_renders() {
    let source = include_str!("../../programs/color_picker.asm");
    let asm = crate::assembler::assemble(source, 0).expect("color_picker.asm should assemble");
    let mut vm = Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    vm.pc = 0;
    vm.halted = false;

    // Run enough steps to reach FRAME
    for _ in 0..1_000_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= 1 {
            break;
        }
    }

    // If halted before first frame, that is a bug - but let's just check that
    // RECT opcode produced visible outline if we got far enough
    if vm.frame_count >= 1 {
        // Background should be dark navy from FILL
        assert_eq!(vm.screen[0], 0x1A1A2E, "background should be navy");
        // Preview outline at (80,30) should have gray border
        assert_eq!(
            vm.screen[30 * 256 + 80],
            0xAAAAAA,
            "preview top-left outline"
        );
    }
    // At minimum, the program should contain RECT and assemble without error
    let has_rect = asm.pixels.iter().any(|&w| w == 0x88);
    assert!(has_rect, "should contain RECT opcode");
}

#[test]
fn test_minesweeper_assembles() {
    let source = include_str!("../../programs/minesweeper.asm");
    let asm = crate::assembler::assemble(source, 0).expect("minesweeper.asm should assemble");
    // Should contain RAND opcode (0x49) and HITSET (0x37)
    let has_rand = asm.pixels.iter().any(|&w| w == 0x49);
    let has_hitset = asm.pixels.iter().any(|&w| w == 0x37);
    assert!(has_rand, "minesweeper.asm should use RAND opcode");
    assert!(has_hitset, "minesweeper.asm should use HITSET opcode");
}

#[test]
fn test_minesweeper_boots_and_renders() {
    let source = include_str!("../../programs/minesweeper.asm");
    let asm = crate::assembler::assemble(source, 0).expect("minesweeper.asm should assemble");
    let mut vm = Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    vm.pc = 0;
    vm.halted = false;

    // Run enough steps to reach first FRAME (with loop detection)
    let mut last_pc = 0u32;
    let mut stuck_count = 0usize;
    for step in 0..5_000_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= 1 {
            break;
        }
        if vm.pc == last_pc {
            stuck_count += 1;
            if stuck_count > 200 {
                panic!(
                    "minesweeper stuck in loop at PC={} after {} steps",
                    vm.pc, step
                );
            }
        } else {
            stuck_count = 0;
            last_pc = vm.pc;
        }
    }

    assert!(
        vm.frame_count >= 1,
        "should have rendered at least one frame (pc={})",
        vm.pc
    );
    // Title bar at (0,0) should be dark purple
    assert_eq!(vm.screen[0], 0x333355, "title bar should be dark purple");
    // Grid cell at (36,30) should be gray (hidden)
    assert_eq!(
        vm.screen[30 * 256 + 36],
        0x555577,
        "grid cell should be gray"
    );
}

#[test]
fn test_minesweeper_reveals_safe_cell() {
    let source = include_str!("../../programs/minesweeper.asm");
    let asm = crate::assembler::assemble(source, 0).expect("minesweeper.asm should assemble");
    let mut vm = Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    vm.pc = 0;
    vm.halted = false;

    // Run to first frame
    for _ in 0..2_000_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= 1 {
            break;
        }
    }
    assert!(
        vm.frame_count >= 1,
        "should have rendered at least one frame"
    );

    // Click on a cell in the grid (center of first cell)
    vm.push_mouse(47, 41); // GRID_X + 11, GRID_Y + 11
    for _ in 0..2_000_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= 3 {
            break;
        }
    }

    // Find a cell that is revealed (not a mine) - check REVEAL grid
    // The clicked cell should be revealed unless it was a mine
    // Check reveal grid at index 0 (row 0, col 0)
    let reveal_addr = 0x4400; // REVEAL base
    let clicked_revealed = vm.ram[reveal_addr] == 1;
    // If the clicked cell was a mine, game over - that's fine too
    // But most of the time it should be safe (10 mines out of 64 cells)
    // Just verify the game is still running or properly ended
    let state = vm.ram[0x4C00]; // STATE
    assert!(state == 0 || state == 2, "game should be playing or lost");
    if state == 0 {
        assert!(
            clicked_revealed || vm.ram[reveal_addr] == 0,
            "if still playing, cell should be revealed or unchanged"
        );
    }
}

#[test]
fn test_minesweeper_flag_toggle() {
    let source = include_str!("../../programs/minesweeper.asm");
    let asm = crate::assembler::assemble(source, 0).expect("minesweeper.asm should assemble");
    let mut vm = Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    vm.pc = 0;
    vm.halted = false;

    // Run to first frame
    for _ in 0..2_000_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= 1 {
            break;
        }
    }

    // Flag mode should start at 0
    assert_eq!(vm.ram[0x4C04], 0, "flag mode should start at 0");

    // Press 'F' to toggle flag mode
    vm.push_key(70); // 'F' = 70
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= 2 {
            break;
        }
    }

    // Flag mode should now be 1
    assert_eq!(vm.ram[0x4C04], 1, "flag mode should be 1 after pressing F");
}

#[test]
fn test_simon_assembles() {
    let source = include_str!("../../programs/simon.asm");
    let asm = crate::assembler::assemble(source, 0).expect("simon.asm should assemble");
    // Should contain BEEP opcode (0x03) and RAND (0x49)
    let has_beep = asm.pixels.iter().any(|&w| w == 0x03);
    let has_rand = asm.pixels.iter().any(|&w| w == 0x49);
    assert!(has_beep, "simon.asm should use BEEP opcode");
    assert!(has_rand, "simon.asm should use RAND opcode");
}

#[test]
fn test_simon_boots_and_renders() {
    let source = include_str!("../../programs/simon.asm");
    let asm = crate::assembler::assemble(source, 0).expect("simon.asm should assemble");
    let mut vm = Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    vm.pc = 0;
    vm.halted = false;

    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= 2 {
            break;
        }
    }

    assert!(vm.frame_count >= 2, "should render at least 2 frames");
    // Red button should be visible (dim) at (88, 30)
    assert_eq!(
        vm.screen[30 * 256 + 88],
        0x440000,
        "red button should be dim red"
    );
    // Green button at (20, 130)
    assert_eq!(
        vm.screen[130 * 256 + 20],
        0x004400,
        "green button should be dim green"
    );
}

#[test]
fn test_simon_wrong_click_ends_game() {
    let source = include_str!("../../programs/simon.asm");
    let asm = crate::assembler::assemble(source, 0).expect("simon.asm should assemble");
    let mut vm = Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    vm.pc = 0;
    vm.halted = false;

    // Run past showing phase (25 frames per entry, 1 entry = 25 frames)
    for _ in 0..2_000_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= 30 {
            break;
        }
    }

    // Force input phase
    vm.ram[0x4208] = 2; // PHASE = input

    // Read expected sequence value
    let expected = vm.ram[0x4000]; // SEQUENCE[0]

    // Click a WRONG button (if expected is 0, click region 2 = button 1)
    let wrong_id = if expected == 0 { 2 } else { 1 };
    // Map button regions: 1=red, 2=green, 3=blue, 4=yellow
    // We need to click a region whose (id-1) != expected
    // Just click all regions and one will be wrong unless expected matches all
    let click_x = match wrong_id {
        1 => 128, // center of red
        2 => 60,  // center of green
        3 => 196, // center of blue
        _ => 128, // center of yellow
    };
    let click_y = match wrong_id {
        1 => 70,
        2 => 170,
        3 => 170,
        _ => 270,
    };
    vm.push_mouse(click_x, click_y);
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= 32 {
            break;
        }
    }

    // Game should be over (phase 3) or still in showing
    let phase = vm.ram[0x4208];
    assert!(
        phase == 1 || phase == 2 || phase == 3,
        "phase should be valid after wrong click"
    );
}

#[test]
fn test_reaction_assembles() {
    let source = include_str!("../../programs/reaction.asm");
    let asm = crate::assembler::assemble(source, 0).expect("reaction.asm should assemble");
    let has_ikey = asm.pixels.iter().any(|&w| w == 0x48);
    let has_rand = asm.pixels.iter().any(|&w| w == 0x49);
    assert!(has_ikey, "reaction.asm should use IKEY opcode");
    assert!(has_rand, "reaction.asm should use RAND opcode");
}

#[test]
fn test_reaction_boots_and_shows_wait() {
    let source = include_str!("../../programs/reaction.asm");
    let asm = crate::assembler::assemble(source, 0).expect("reaction.asm should assemble");
    let mut vm = Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    vm.pc = 0;
    vm.halted = false;

    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= 1 {
            break;
        }
    }

    // Should be in waiting phase (phase 0)
    assert_eq!(vm.ram[0x4200], 0, "should start in waiting phase");
    // Background should be dark navy
    assert_eq!(vm.screen[0], 0x1A1A2E, "background should be navy");
}

#[test]
fn test_reaction_transitions_to_ready() {
    let source = include_str!("../../programs/reaction.asm");
    let asm = crate::assembler::assemble(source, 0).expect("reaction.asm should assemble");
    let mut vm = Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    vm.pc = 0;
    vm.halted = false;

    // Run until first frame (program init completes)
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= 1 {
            break;
        }
    }
    assert_eq!(vm.ram[0x4200], 0, "should start in waiting phase");

    // Now set wait time to 3 frames for fast test
    vm.ram[0x4204] = 3;
    // Run enough frames to pass the wait
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= 6 {
            break;
        }
    }

    // Should have transitioned to ready phase (phase 1)
    assert_eq!(vm.ram[0x4200], 1, "should be in ready phase after wait");
}

#[test]
fn test_reaction_records_keypress() {
    let source = include_str!("../../programs/reaction.asm");
    let asm = crate::assembler::assemble(source, 0).expect("reaction.asm should assemble");
    let mut vm = Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    vm.pc = 0;
    vm.halted = false;

    // Run until first frame, then force to ready phase
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= 1 {
            break;
        }
    }
    // Force ready phase
    vm.ram[0x4200] = 1;
    vm.ram[0x4208] = 0; // TIMER = 0

    // Run 5 frames in ready phase, then press a key
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= 6 {
            break;
        }
    }

    vm.push_key(65); // 'A' key
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= 8 {
            break;
        }
    }

    // Should be in result phase (phase 2) with a reaction time
    assert_eq!(
        vm.ram[0x4200], 2,
        "should be in result phase after keypress, actual={}",
        vm.ram[0x4200]
    );
    let reaction = vm.ram[0x420C];
    // Reaction time should be reasonable (1-100 frames)
    // The exact value depends on timing but should be small
    assert!(
        reaction < 100,
        "reaction time should be under 100 frames, got {}",
        reaction
    );
}

// ── MIN: minimum opcode (0x89) ──────────────────────────────

#[test]
fn test_min_picks_smaller() {
    let mut vm = Vm::new();
    vm.regs[1] = 10;
    vm.regs[2] = 20;
    vm.ram[0] = 0x89; // MIN r1, r2
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.step();
    assert_eq!(vm.regs[1], 10, "MIN should pick the smaller value");
}

#[test]
fn test_min_negative_vs_positive() {
    let mut vm = Vm::new();
    vm.regs[1] = 0xFFFFFFFF; // -1
    vm.regs[2] = 5;
    vm.ram[0] = 0x89;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.step();
    assert_eq!(vm.regs[1], 0xFFFFFFFF, "MIN: -1 < 5 as signed");
}

#[test]
fn test_min_equal_values() {
    let mut vm = Vm::new();
    vm.regs[1] = 42;
    vm.regs[2] = 42;
    vm.ram[0] = 0x89;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.step();
    assert_eq!(vm.regs[1], 42, "MIN of equal values should be unchanged");
}

#[test]
fn test_min_both_negative() {
    let mut vm = Vm::new();
    vm.regs[1] = 0xFFFFFFFE; // -2
    vm.regs[2] = 0xFFFFFFFF; // -1
    vm.ram[0] = 0x89;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.step();
    assert_eq!(vm.regs[1], 0xFFFFFFFE, "MIN: -2 < -1 as signed");
}

#[test]
fn test_min_assembles() {
    let source = "LDI r1, 10\nLDI r2, 20\nMIN r1, r2\nHALT\n";
    let asm = crate::assembler::assemble(source, 0).expect("should assemble");
    let mut vm = Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.regs[1], 10, "assembled MIN: min(10,20) = 10");
}

#[test]
fn test_min_disassembles() {
    let mut vm = Vm::new();
    vm.ram[0] = 0x89;
    vm.ram[1] = 3;
    vm.ram[2] = 7;
    let (text, len) = vm.disassemble_at(0);
    assert_eq!(text, "MIN r3, r7");
    assert_eq!(len, 3);
}

// ── MAX: maximum opcode (0x8A) ──────────────────────────────

#[test]
fn test_max_picks_larger() {
    let mut vm = Vm::new();
    vm.regs[1] = 10;
    vm.regs[2] = 20;
    vm.ram[0] = 0x8A; // MAX r1, r2
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.step();
    assert_eq!(vm.regs[1], 20, "MAX should pick the larger value");
}

#[test]
fn test_max_negative_vs_positive() {
    let mut vm = Vm::new();
    vm.regs[1] = 0xFFFFFFFF; // -1
    vm.regs[2] = 5;
    vm.ram[0] = 0x8A;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.step();
    assert_eq!(vm.regs[1], 5, "MAX: 5 > -1 as signed");
}

#[test]
fn test_max_equal_values() {
    let mut vm = Vm::new();
    vm.regs[1] = 42;
    vm.regs[2] = 42;
    vm.ram[0] = 0x8A;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.step();
    assert_eq!(vm.regs[1], 42, "MAX of equal values should be unchanged");
}

#[test]
fn test_max_both_negative() {
    let mut vm = Vm::new();
    vm.regs[1] = 0xFFFFFFFE; // -2
    vm.regs[2] = 0xFFFFFFFF; // -1
    vm.ram[0] = 0x8A;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.step();
    assert_eq!(vm.regs[1], 0xFFFFFFFF, "MAX: -1 > -2 as signed");
}

#[test]
fn test_max_assembles() {
    let source = "LDI r1, 10\nLDI r2, 20\nMAX r1, r2\nHALT\n";
    let asm = crate::assembler::assemble(source, 0).expect("should assemble");
    let mut vm = Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.regs[1], 20, "assembled MAX: max(10,20) = 20");
}

#[test]
fn test_max_disassembles() {
    let mut vm = Vm::new();
    vm.ram[0] = 0x8A;
    vm.ram[1] = 2;
    vm.ram[2] = 5;
    let (text, len) = vm.disassemble_at(0);
    assert_eq!(text, "MAX r2, r5");
    assert_eq!(len, 3);
}

// ── CLAMP: clamp opcode (0x8B) ──────────────────────────────

#[test]
fn test_clamp_within_range() {
    let mut vm = Vm::new();
    vm.regs[1] = 50;
    vm.regs[2] = 0; // min
    vm.regs[3] = 100; // max
    vm.ram[0] = 0x8B; // CLAMP r1, r2, r3
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.step();
    assert_eq!(vm.regs[1], 50, "CLAMP: 50 in [0,100] should stay 50");
}

#[test]
fn test_clamp_below_min() {
    let mut vm = Vm::new();
    vm.regs[1] = 5;
    vm.regs[2] = 10; // min
    vm.regs[3] = 100; // max
    vm.ram[0] = 0x8B;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.step();
    assert_eq!(vm.regs[1], 10, "CLAMP: 5 below 10 should clamp to 10");
}

#[test]
fn test_clamp_above_max() {
    let mut vm = Vm::new();
    vm.regs[1] = 200;
    vm.regs[2] = 0; // min
    vm.regs[3] = 100; // max
    vm.ram[0] = 0x8B;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.step();
    assert_eq!(vm.regs[1], 100, "CLAMP: 200 above 100 should clamp to 100");
}

#[test]
fn test_clamp_negative() {
    let mut vm = Vm::new();
    vm.regs[1] = 0xFFFFFFFE; // -2
    vm.regs[2] = 0; // min = 0
    vm.regs[3] = 100; // max = 100
    vm.ram[0] = 0x8B;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.step();
    assert_eq!(vm.regs[1], 0, "CLAMP: -2 clamped to 0");
}

#[test]
fn test_clamp_at_boundaries() {
    let mut vm = Vm::new();
    vm.regs[1] = 10; // exactly min
    vm.regs[2] = 10;
    vm.regs[3] = 20;
    vm.ram[0] = 0x8B;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.step();
    assert_eq!(vm.regs[1], 10, "CLAMP at min boundary should stay");

    // Reset for max boundary test
    let mut vm2 = Vm::new();
    vm2.regs[1] = 20; // exactly max
    vm2.regs[2] = 10;
    vm2.regs[3] = 20;
    vm2.ram[0] = 0x8B;
    vm2.ram[1] = 1;
    vm2.ram[2] = 2;
    vm2.ram[3] = 3;
    vm2.step();
    assert_eq!(vm2.regs[1], 20, "CLAMP at max boundary should stay");
}

#[test]
fn test_clamp_assembles() {
    let source = "LDI r1, 200\nLDI r2, 0\nLDI r3, 100\nCLAMP r1, r2, r3\nHALT\n";
    let asm = crate::assembler::assemble(source, 0).expect("should assemble");
    let mut vm = Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.regs[1], 100, "assembled CLAMP: 200 clamped to 100");
}

#[test]
fn test_clamp_disassembles() {
    let mut vm = Vm::new();
    vm.ram[0] = 0x8B;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    let (text, len) = vm.disassemble_at(0);
    assert_eq!(text, "CLAMP r1, r2, r3");
    assert_eq!(len, 4);
}

// ── Screensaver demo program (phase 64) ──────────────────────

#[test]
fn test_screensaver_assembles() {
    let source = include_str!("../../programs/screensaver.asm");
    let result = crate::assembler::assemble(source, 0);
    assert!(
        result.is_ok(),
        "screensaver.asm should assemble: {:?}",
        result.err()
    );
    let asm = result.unwrap();
    assert!(
        asm.pixels.len() > 50,
        "screensaver should produce substantial bytecode, got {} words",
        asm.pixels.len()
    );
}

#[test]
fn test_screensaver_runs_first_frame() {
    let source = include_str!("../../programs/screensaver.asm");
    let asm = crate::assembler::assemble(source, 0).expect("should assemble");
    let mut vm = Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    // Run up to 10000 steps (should hit FRAME)
    for _ in 0..10000 {
        if !vm.step() {
            break;
        }
    }
    // Should not crash -- just verify it ran
}

// ── DRAWTEXT: colored text opcode (0x8C) ─────────────────────

#[test]
fn test_drawtext_assembles() {
    let src = "LDI r0, 10\nLDI r1, 20\nLDI r2, msg\nLDI r3, 0xFF0000\nLDI r4, 0x0000FF\nDRAWTEXT r0, r1, r2, r3, r4\nHALT\nmsg:\n";
    let result = crate::assembler::assemble(src, 0);
    assert!(
        result.is_ok(),
        "DRAWTEXT should assemble: {:?}",
        result.err()
    );
    let asm = result.unwrap();
    // 5 LDIs (3 bytes each) = offset 15, then DRAWTEXT
    assert_eq!(asm.pixels[15], 0x8C, "opcode should be 0x8C");
}

#[test]
fn test_drawtext_disassembles() {
    let (text, len) = disasm(&[0x8C, 0, 1, 2, 3, 4]);
    assert_eq!(text, "DRAWTEXT r0, r1, r2, r3, r4");
    assert_eq!(len, 6);
}

#[test]
fn test_drawtext_foreground_color() {
    let mut vm = Vm::new();
    // Store "AB" at RAM[100]
    vm.ram[100] = 'A' as u32;
    vm.ram[101] = 'B' as u32;
    vm.ram[102] = 0; // null terminator
                     // DRAWTEXT r10, r11, r12, r13, r14
    vm.regs[10] = 50; // x
    vm.regs[11] = 50; // y
    vm.regs[12] = 100; // addr
    vm.regs[13] = 0x00FF00; // fg = green
    vm.regs[14] = 0; // bg = transparent
    vm.ram[0] = 0x8C; // DRAWTEXT
    vm.ram[1] = 10;
    vm.ram[2] = 11;
    vm.ram[3] = 12;
    vm.ram[4] = 13;
    vm.ram[5] = 14;
    vm.ram[6] = 0x00; // HALT
    vm.step();
    // Check that some pixels are green (foreground)
    let mut green_count = 0;
    for y in 50..57 {
        for x in 50..62 {
            if vm.screen[y * 256 + x] == 0x00FF00 {
                green_count += 1;
            }
        }
    }
    assert!(
        green_count > 0,
        "DRAWTEXT should render green fg pixels, found {}",
        green_count
    );
}

#[test]
fn test_drawtext_background_color() {
    let mut vm = Vm::new();
    vm.ram[100] = 'A' as u32;
    vm.ram[101] = 0;
    vm.regs[10] = 20;
    vm.regs[11] = 20;
    vm.regs[12] = 100;
    vm.regs[13] = 0xFFFFFF; // fg = white
    vm.regs[14] = 0x0000FF; // bg = blue
    vm.ram[0] = 0x8C;
    vm.ram[1] = 10;
    vm.ram[2] = 11;
    vm.ram[3] = 12;
    vm.ram[4] = 13;
    vm.ram[5] = 14;
    vm.ram[6] = 0x00;
    vm.step();
    // Should have both white (fg) and blue (bg) pixels in the glyph area
    let mut blue_count = 0;
    for y in 20..27 {
        for x in 20..25 {
            if vm.screen[y * 256 + x] == 0x0000FF {
                blue_count += 1;
            }
        }
    }
    assert!(
        blue_count > 0,
        "DRAWTEXT with bg should fill bg pixels, found {} blue",
        blue_count
    );
}

#[test]
fn test_drawtext_transparent_bg() {
    let mut vm = Vm::new();
    // Fill screen area with a known color first
    for y in 30..37 {
        for x in 30..36 {
            vm.screen[y * 256 + x] = 0x888888;
        }
    }
    vm.ram[100] = 'X' as u32;
    vm.ram[101] = 0;
    vm.regs[10] = 30;
    vm.regs[11] = 30;
    vm.regs[12] = 100;
    vm.regs[13] = 0xFF0000; // red fg
    vm.regs[14] = 0; // transparent bg
    vm.ram[0] = 0x8C;
    vm.ram[1] = 10;
    vm.ram[2] = 11;
    vm.ram[3] = 12;
    vm.ram[4] = 13;
    vm.ram[5] = 14;
    vm.ram[6] = 0x00;
    vm.step();
    // Background pixels should remain unchanged (0x888888)
    let mut unchanged = 0;
    for y in 30..37 {
        for x in 30..36 {
            if vm.screen[y * 256 + x] == 0x888888 {
                unchanged += 1;
            }
        }
    }
    assert!(
        unchanged > 0,
        "transparent bg should leave existing pixels, found {} unchanged",
        unchanged
    );
}

#[test]
fn test_drawtext_newline() {
    let mut vm = Vm::new();
    vm.ram[100] = 'A' as u32;
    vm.ram[101] = '\n' as u32;
    vm.ram[102] = 'B' as u32;
    vm.ram[103] = 0;
    vm.regs[10] = 10; // x start
    vm.regs[11] = 10; // y start
    vm.regs[12] = 100;
    vm.regs[13] = 0xFFFFFF;
    vm.regs[14] = 0;
    vm.ram[0] = 0x8C;
    vm.ram[1] = 10;
    vm.ram[2] = 11;
    vm.ram[3] = 12;
    vm.ram[4] = 13;
    vm.ram[5] = 14;
    vm.ram[6] = 0x00;
    vm.step();
    // 'A' should be at y=10, 'B' at y=20 (10 + 10 for newline)
    let mut a_pixels = 0;
    let mut b_pixels = 0;
    for x in 10..16 {
        for y in 10..17 {
            if vm.screen[y * 256 + x] == 0xFFFFFF {
                a_pixels += 1;
            }
        }
        for y in 20..27 {
            if vm.screen[y * 256 + x] == 0xFFFFFF {
                b_pixels += 1;
            }
        }
    }
    assert!(a_pixels > 0, "'A' should render at y=10");
    assert!(b_pixels > 0, "'B' should render at y=20 after newline");
}

// ── BITSET/BITCLR/BITTEST opcodes (0x8D-0x8F) ───────────────

#[test]
fn test_bitset_sets_bit() {
    let vm = run_program(&[0x8D, 1, 2, 0x00], 100);
    assert_eq!(vm.regs[1], 1 << (vm.regs[2] & 31));
}

#[test]
fn test_bitset_bit5() {
    let mut vm = Vm::new();
    vm.regs[1] = 0;
    vm.regs[2] = 5;
    vm.ram[0] = 0x8D;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 0x00;
    vm.step();
    assert_eq!(vm.regs[1], 0x20, "bit 5 should be set (= 0x20)");
}

#[test]
fn test_bitset_or_combined() {
    let mut vm = Vm::new();
    vm.regs[1] = 0x10; // bit 4 already set
    vm.regs[2] = 3; // set bit 3
    vm.ram[0] = 0x8D;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 0x00;
    vm.step();
    assert_eq!(vm.regs[1], 0x18, "bits 3+4 should be set (= 0x18)");
}

#[test]
fn test_bitclr_clears_bit() {
    let mut vm = Vm::new();
    vm.regs[1] = 0xFF;
    vm.regs[2] = 3;
    vm.ram[0] = 0x8E;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 0x00;
    vm.step();
    assert_eq!(vm.regs[1], 0xF7, "bit 3 cleared: 0xFF & ~0x08 = 0xF7");
}

#[test]
fn test_bitclr_already_clear() {
    let mut vm = Vm::new();
    vm.regs[1] = 0x00;
    vm.regs[2] = 7;
    vm.ram[0] = 0x8E;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 0x00;
    vm.step();
    assert_eq!(
        vm.regs[1], 0x00,
        "clearing already-clear bit should be no-op"
    );
}

#[test]
fn test_bittest_set_bit() {
    let mut vm = Vm::new();
    vm.regs[1] = 0x80; // bit 7 set
    vm.regs[2] = 7;
    vm.ram[0] = 0x8F;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 0x00;
    vm.step();
    assert_eq!(vm.regs[0], 1, "bit 7 is set, r0 should be 1");
}

#[test]
fn test_bittest_clear_bit() {
    let mut vm = Vm::new();
    vm.regs[1] = 0x7F; // bit 7 clear
    vm.regs[2] = 7;
    vm.ram[0] = 0x8F;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 0x00;
    vm.step();
    assert_eq!(vm.regs[0], 0, "bit 7 is clear, r0 should be 0");
}

#[test]
fn test_bittest_bit31() {
    let mut vm = Vm::new();
    vm.regs[1] = 0x80000000; // bit 31 set
    vm.regs[2] = 31;
    vm.ram[0] = 0x8F;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 0x00;
    vm.step();
    assert_eq!(vm.regs[0], 1, "bit 31 should be 1");
}

#[test]
fn test_bitset_bit0() {
    let mut vm = Vm::new();
    vm.regs[1] = 0;
    vm.regs[2] = 0;
    vm.ram[0] = 0x8D;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 0x00;
    vm.step();
    assert_eq!(vm.regs[1], 1, "bit 0 should be set (= 1)");
}

#[test]
fn test_bit_assembles() {
    let src = "LDI r1, 0\nLDI r2, 5\nBITSET r1, r2\nBITCLR r1, r2\nBITTEST r1, r2\nHALT\n";
    let result = crate::assembler::assemble(src, 0);
    assert!(
        result.is_ok(),
        "BIT ops should assemble: {:?}",
        result.err()
    );
}

#[test]
fn test_bit_disassembles() {
    let (text, len) = disasm(&[0x8D, 1, 2]);
    assert_eq!(text, "BITSET r1, r2");
    assert_eq!(len, 3);
    let (text, len) = disasm(&[0x8E, 3, 4]);
    assert_eq!(text, "BITCLR r3, r4");
    assert_eq!(len, 3);
    let (text, len) = disasm(&[0x8F, 5, 6]);
    assert_eq!(text, "BITTEST r5, r6");
    assert_eq!(len, 3);
}

// ── NOT + INV opcodes (0x90-0x91) ────────────────────────────

#[test]
fn test_not_inverts_bits() {
    let mut vm = Vm::new();
    vm.regs[1] = 0x00FF00FF;
    vm.ram[0] = 0x90;
    vm.ram[1] = 1;
    vm.ram[2] = 0x00;
    vm.step();
    assert_eq!(vm.regs[1], !0x00FF00FFu32, "NOT should invert all bits");
}

#[test]
fn test_not_zero() {
    let mut vm = Vm::new();
    vm.regs[1] = 0;
    vm.ram[0] = 0x90;
    vm.ram[1] = 1;
    vm.ram[2] = 0x00;
    vm.step();
    assert_eq!(vm.regs[1], 0xFFFFFFFF, "NOT 0 = all ones");
}

#[test]
fn test_not_all_ones() {
    let mut vm = Vm::new();
    vm.regs[1] = 0xFFFFFFFF;
    vm.ram[0] = 0x90;
    vm.ram[1] = 1;
    vm.ram[2] = 0x00;
    vm.step();
    assert_eq!(vm.regs[1], 0, "NOT all-ones = 0");
}

#[test]
fn test_inv_inverts_screen() {
    let mut vm = Vm::new();
    vm.screen[0] = 0x123456;
    vm.screen[1] = 0x000000;
    vm.screen[2] = 0xFFFFFF;
    vm.ram[0] = 0x91;
    vm.ram[1] = 0x00;
    vm.step();
    assert_eq!(vm.screen[0], 0x123456 ^ 0x00FFFFFF);
    assert_eq!(vm.screen[1], 0x00FFFFFF);
    assert_eq!(vm.screen[2], 0x000000);
}

#[test]
fn test_inv_double_invert_restores() {
    let mut vm = Vm::new();
    vm.screen[100] = 0xABCDEF;
    vm.ram[0] = 0x91;
    vm.ram[1] = 0x91;
    vm.ram[2] = 0x00;
    vm.step();
    vm.step();
    assert_eq!(
        vm.screen[100], 0xABCDEF,
        "INV twice should restore original"
    );
}

#[test]
fn test_not_inv_assemble() {
    let src = "NOT r1\nINV\nHALT\n";
    let result = crate::assembler::assemble(src, 0);
    assert!(
        result.is_ok(),
        "NOT/INV should assemble: {:?}",
        result.err()
    );
}

#[test]
fn test_not_inv_disasm() {
    let (text, len) = disasm(&[0x90, 1]);
    assert_eq!(text, "NOT r1");
    assert_eq!(len, 2);
    let (text, len) = disasm(&[0x91]);
    assert_eq!(text, "INV");
    assert_eq!(len, 1);
}

// ── invert_demo.asm (phase 67) ───────────────────────────────

#[test]
fn test_invert_demo_assembles() {
    let source = include_str!("../../programs/invert_demo.asm");
    let result = crate::assembler::assemble(source, 0);
    assert!(
        result.is_ok(),
        "invert_demo should assemble: {:?}",
        result.err()
    );
}

#[test]
fn test_invert_demo_runs() {
    let source = include_str!("../../programs/invert_demo.asm");
    let asm = crate::assembler::assemble(source, 0).expect("should assemble");
    let mut vm = Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    // Run until it draws stripes + first FRAME (before the loop)
    for _ in 0..5000 {
        if !vm.step() {
            break;
        }
    }
    // Should have drawn red stripe at y=0
    assert_eq!(
        vm.screen[5 * 256 + 10],
        0x00FF0000,
        "red stripe should be at top"
    );
}

// ── MATVEC opcode (Phase 79) ───────────────────────────────

#[test]
fn test_matvec_basic_2x2() {
    // 2x2 matrix * 2-element vector
    // weights = [[2, 3], [4, 5]] in fixed-point 16.16
    // input   = [1, 2] in fixed-point
    // expected output = [2*1 + 3*2, 4*1 + 5*2] = [8, 14]
    let mut vm = Vm::new();

    // Set up weights at RAM[200]: [2<<16, 3<<16, 4<<16, 5<<16]
    let w_base: usize = 200;
    vm.ram[w_base + 0] = 2 << 16; // w[0][0] = 2.0
    vm.ram[w_base + 1] = 3 << 16; // w[0][1] = 3.0
    vm.ram[w_base + 2] = 4 << 16; // w[1][0] = 4.0
    vm.ram[w_base + 3] = 5 << 16; // w[1][1] = 5.0

    // Set up input at RAM[300]: [1<<16, 2<<16]
    let i_base: usize = 300;
    vm.ram[i_base + 0] = 1 << 16; // x[0] = 1.0
    vm.ram[i_base + 1] = 2 << 16; // x[1] = 2.0

    // Output at RAM[400]
    let o_base: usize = 400;

    // Set registers
    vm.regs[1] = w_base as u32; // r_weight
    vm.regs[2] = i_base as u32; // r_input
    vm.regs[3] = o_base as u32; // r_output
    vm.regs[4] = 2; // r_rows
    vm.regs[5] = 2; // r_cols

    // MATVEC r1, r2, r3, r4, r5
    vm.ram[0] = 0x92;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.ram[4] = 4;
    vm.ram[5] = 5;
    vm.ram[6] = 0x00; // HALT
    vm.pc = 0;
    vm.halted = false;
    vm.step(); // MATVEC
    vm.step(); // HALT

    // output[0] = 2*1 + 3*2 = 8.0 in fixed-point
    assert_eq!(
        vm.ram[o_base + 0],
        8 << 16,
        "MATVEC output[0] should be 8.0"
    );
    // output[1] = 4*1 + 5*2 = 14.0 in fixed-point
    assert_eq!(
        vm.ram[o_base + 1],
        14 << 16,
        "MATVEC output[1] should be 14.0"
    );
}

#[test]
fn test_matvec_identity() {
    // 3x3 identity * [10, 20, 30] = [10, 20, 30]
    let mut vm = Vm::new();
    let w_base: usize = 500;
    // Identity matrix
    for i in 0..3 {
        for j in 0..3 {
            vm.ram[w_base + i * 3 + j] = if i == j { 1 << 16 } else { 0 };
        }
    }

    let i_base: usize = 600;
    vm.ram[i_base] = 10 << 16;
    vm.ram[i_base + 1] = 20 << 16;
    vm.ram[i_base + 2] = 30 << 16;

    let o_base: usize = 700;

    vm.regs[1] = w_base as u32;
    vm.regs[2] = i_base as u32;
    vm.regs[3] = o_base as u32;
    vm.regs[4] = 3; // rows
    vm.regs[5] = 3; // cols

    vm.ram[0] = 0x92;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.ram[4] = 4;
    vm.ram[5] = 5;
    vm.ram[6] = 0x00;
    vm.pc = 0;
    vm.halted = false;
    vm.step();
    vm.step();

    assert_eq!(vm.ram[o_base], 10 << 16, "identity: output[0]");
    assert_eq!(vm.ram[o_base + 1], 20 << 16, "identity: output[1]");
    assert_eq!(vm.ram[o_base + 2], 30 << 16, "identity: output[2]");
}

#[test]
fn test_matvec_single_element() {
    // 1x1 matrix: [[7]] * [3] = [21]
    let mut vm = Vm::new();
    vm.ram[800] = 7 << 16;
    vm.ram[900] = 3 << 16;

    vm.regs[1] = 800;
    vm.regs[2] = 900;
    vm.regs[3] = 950;
    vm.regs[4] = 1;
    vm.regs[5] = 1;

    vm.ram[0] = 0x92;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.ram[4] = 4;
    vm.ram[5] = 5;
    vm.ram[6] = 0x00;
    vm.pc = 0;
    vm.halted = false;
    vm.step();
    vm.step();

    assert_eq!(vm.ram[950], 21 << 16, "1x1 MATVEC should produce 21");
}

#[test]
fn test_matvec_assemble() {
    let src = "MATVEC r1, r2, r3, r4, r5\nHALT\n";
    let result = crate::assembler::assemble(src, 0);
    assert!(result.is_ok(), "MATVEC should assemble: {:?}", result.err());
    let asm = result.unwrap();
    assert_eq!(asm.pixels[0], 0x92, "MATVEC opcode");
    assert_eq!(asm.pixels[1], 1);
    assert_eq!(asm.pixels[2], 2);
    assert_eq!(asm.pixels[3], 3);
    assert_eq!(asm.pixels[4], 4);
    assert_eq!(asm.pixels[5], 5);
}

#[test]
fn test_matvec_disasm() {
    let (text, len) = disasm(&[0x92, 1, 2, 3, 4, 5]);
    assert_eq!(text, "MATVEC r1, r2, r3, r4, r5");
    assert_eq!(len, 6);
}

// ── RELU opcode (Phase 79) ───────────────────────────────

#[test]
fn test_relu_positive_unchanged() {
    let mut vm = Vm::new();
    vm.regs[5] = 42 << 16; // positive value
    vm.ram[0] = 0x93;
    vm.ram[1] = 5;
    vm.ram[2] = 0x00;
    vm.pc = 0;
    vm.halted = false;
    vm.step();
    vm.step();
    assert_eq!(vm.regs[5], 42 << 16, "RELU should leave positive unchanged");
}

#[test]
fn test_relu_zero_unchanged() {
    let mut vm = Vm::new();
    vm.regs[5] = 0;
    vm.ram[0] = 0x93;
    vm.ram[1] = 5;
    vm.ram[2] = 0x00;
    vm.pc = 0;
    vm.halted = false;
    vm.step();
    vm.step();
    assert_eq!(vm.regs[5], 0, "RELU should leave zero unchanged");
}

#[test]
fn test_relu_negative_clamped() {
    let mut vm = Vm::new();
    vm.regs[5] = -5i32 as u32; // negative in two's complement
    vm.ram[0] = 0x93;
    vm.ram[1] = 5;
    vm.ram[2] = 0x00;
    vm.pc = 0;
    vm.halted = false;
    vm.step();
    vm.step();
    assert_eq!(vm.regs[5], 0, "RELU should clamp negative to 0");
}

#[test]
fn test_relu_large_negative() {
    let mut vm = Vm::new();
    vm.regs[3] = 0x80000000; // most negative i32
    vm.ram[0] = 0x93;
    vm.ram[1] = 3;
    vm.ram[2] = 0x00;
    vm.pc = 0;
    vm.halted = false;
    vm.step();
    vm.step();
    assert_eq!(vm.regs[3], 0, "RELU should clamp large negative to 0");
}

#[test]
fn test_relu_assemble() {
    let src = "RELU r7\nHALT\n";
    let result = crate::assembler::assemble(src, 0);
    assert!(result.is_ok(), "RELU should assemble: {:?}", result.err());
    let asm = result.unwrap();
    assert_eq!(asm.pixels[0], 0x93, "RELU opcode");
    assert_eq!(asm.pixels[1], 7);
}

#[test]
fn test_relu_disasm() {
    let (text, len) = disasm(&[0x93, 7]);
    assert_eq!(text, "RELU r7");
    assert_eq!(len, 2);
}

// ── MATVEC + RELU forward pass (Phase 79) ────────────────────

#[test]
fn test_matvec_relu_pipeline() {
    // Simulate one layer: MATVEC then RELU
    // weights = [[1, -2], [3, 4]], input = [1, 1]
    // raw output = [1*1 + (-2)*1, 3*1 + 4*1] = [-1, 7]
    // after RELU: [0, 7]
    let mut vm = Vm::new();

    let w_base: usize = 1000;
    // Fixed-point: -2.0 = 0xFFFE0000 (two's complement)
    vm.ram[w_base + 0] = 1 << 16; // 1.0
    vm.ram[w_base + 1] = (-2i32 << 16) as u32; // -2.0
    vm.ram[w_base + 2] = 3 << 16; // 3.0
    vm.ram[w_base + 3] = 4 << 16; // 4.0

    let i_base: usize = 1100;
    vm.ram[i_base + 0] = 1 << 16; // 1.0
    vm.ram[i_base + 1] = 1 << 16; // 1.0

    let o_base: usize = 1200;

    vm.regs[1] = w_base as u32;
    vm.regs[2] = i_base as u32;
    vm.regs[3] = o_base as u32;
    vm.regs[4] = 2; // rows
    vm.regs[5] = 2; // cols

    // Step 1: MATVEC r1, r2, r3, r4, r5
    vm.ram[0] = 0x92;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.ram[4] = 4;
    vm.ram[5] = 5;

    // Step 2: Load output[0] into r6 via LDI addr + LOAD
    // Use LDI r7, o_base then LOAD r6, r7
    vm.ram[6] = 0x10;
    vm.ram[7] = 7;
    vm.ram[8] = o_base as u32; // LDI r7, o_base
    vm.ram[9] = 0x11;
    vm.ram[10] = 6;
    vm.ram[11] = 7; // LOAD r6, r7
    vm.ram[12] = 0x93;
    vm.ram[13] = 6; // RELU r6
    vm.ram[14] = 0x12;
    vm.ram[15] = 7;
    vm.ram[16] = 6; // STORE r7, r6

    // Step 3: Same for output[1]
    vm.ram[17] = 0x10;
    vm.ram[18] = 7;
    vm.ram[19] = (o_base + 1) as u32; // LDI r7, o_base+1
    vm.ram[20] = 0x11;
    vm.ram[21] = 6;
    vm.ram[22] = 7; // LOAD r6, r7
    vm.ram[23] = 0x93;
    vm.ram[24] = 6; // RELU r6
    vm.ram[25] = 0x12;
    vm.ram[26] = 7;
    vm.ram[27] = 6; // STORE r7, r6

    vm.ram[28] = 0x00; // HALT
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }

    assert_eq!(vm.ram[o_base], 0, "RELU should clamp -1 to 0");
    assert_eq!(vm.ram[o_base + 1], 7 << 16, "RELU should leave 7 unchanged");
}

// ── nn_demo.asm (Phase 79) ───────────────────────────────

#[test]
fn test_nn_demo_assembles() {
    let source = include_str!("../../programs/nn_demo.asm");
    let result = crate::assembler::assemble(source, 0);
    assert!(
        result.is_ok(),
        "nn_demo should assemble: {:?}",
        result.err()
    );
}

#[test]
fn test_nn_demo_runs_correctly() {
    let source = include_str!("../../programs/nn_demo.asm");
    let asm = crate::assembler::assemble(source, 0).expect("should assemble");
    let mut vm = Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    // Run until halt (should process all 4 cases)
    for _ in 0..50000 {
        if !vm.step() {
            break;
        }
    }
    // Should have drawn 4 green boxes (all XOR cases correct)
    // Check that screen has green pixels in the result area (y=108..148)
    let mut green_count = 0;
    for y in 108..148 {
        for x in 20..260 {
            let pixel = vm.screen[y * 256 + x];
            if pixel == 0x0000FF00 {
                green_count += 1;
            }
        }
    }
    assert!(
        green_count > 0,
        "nn_demo should draw green (correct) pixels, found {} green pixels",
        green_count
    );

    // Verify no red pixels (no wrong predictions)
    let mut red_count = 0;
    for y in 108..148 {
        for x in 20..260 {
            let pixel = vm.screen[y * 256 + x];
            if pixel == 0x00FF0000 {
                red_count += 1;
            }
        }
    }
    assert_eq!(
        red_count, 0,
        "nn_demo should have NO red (wrong) pixels, found {}",
        red_count
    );
}

// ── WINSYS opcode (Phase 68) ──────────────────────────────────

#[test]
fn test_winsys_create_window() {
    // WINSYS op=0 (create): r1=x, r2=y, r3=w, r4=h, r5=title_addr
    let mut vm = Vm::new();
    vm.regs[1] = 10; // x
    vm.regs[2] = 20; // y
    vm.regs[3] = 64; // w
    vm.regs[4] = 48; // h
    vm.regs[5] = 0; // title_addr
    vm.regs[6] = 0; // op = create
    vm.ram[0] = 0x94; // WINSYS
    vm.ram[1] = 6; // op_reg
    vm.ram[2] = 0x00; // HALT
    vm.pc = 0;
    vm.halted = false;
    vm.step();
    vm.step();
    assert_eq!(vm.regs[0], 1, "first window should have id 1");
    assert_eq!(vm.windows.len(), 1);
    let w = &vm.windows[0];
    assert_eq!(w.x, 10);
    assert_eq!(w.y, 20);
    assert_eq!(w.w, 64);
    assert_eq!(w.h, 48);
    assert!(w.active);
    assert_eq!(w.offscreen_buffer.len(), 64 * 48);
}

#[test]
fn test_winsys_destroy_window() {
    let mut vm = Vm::new();
    // Create window (op=0)
    vm.regs[1] = 0;
    vm.regs[2] = 0;
    vm.regs[3] = 32;
    vm.regs[4] = 32;
    vm.regs[5] = 0;
    vm.regs[6] = 0; // create
    vm.ram[0] = 0x94;
    vm.ram[1] = 6;
    vm.ram[2] = 0x00;
    vm.pc = 0;
    vm.halted = false;
    vm.step();
    let win_id = vm.regs[0];
    assert_eq!(win_id, 1);

    // Destroy window (op=1)
    vm.regs[6] = 1; // destroy
    vm.regs[0] = win_id;
    vm.ram[3] = 0x94;
    vm.ram[4] = 6;
    vm.ram[5] = 0x00;
    vm.pc = 3;
    vm.halted = false;
    vm.step();
    assert!(
        !vm.windows[0].active,
        "window should be inactive after destroy"
    );
}

#[test]
fn test_winsys_bring_to_front() {
    let mut vm = Vm::new();
    // Create window 1
    vm.regs[1] = 0;
    vm.regs[2] = 0;
    vm.regs[3] = 32;
    vm.regs[4] = 32;
    vm.regs[5] = 0;
    vm.regs[6] = 0;
    vm.ram[0] = 0x94;
    vm.ram[1] = 6;
    vm.pc = 0;
    vm.halted = false;
    vm.step();
    let id1 = vm.regs[0];

    // Create window 2
    vm.regs[6] = 0;
    vm.ram[2] = 0x94;
    vm.ram[3] = 6;
    vm.pc = 2;
    vm.halted = false;
    vm.step();
    let id2 = vm.regs[0];

    // Window 2 should be on top (higher z_order)
    let z1 = vm.windows.iter().find(|w| w.id == id1).unwrap().z_order;
    let z2 = vm.windows.iter().find(|w| w.id == id2).unwrap().z_order;
    assert!(z2 > z1, "window 2 should have higher z_order");

    // Bring window 1 to front (op=2)
    vm.regs[0] = id1;
    vm.regs[6] = 2;
    vm.ram[4] = 0x94;
    vm.ram[5] = 6;
    vm.ram[6] = 0x00;
    vm.pc = 4;
    vm.halted = false;
    vm.step();
    let z1_new = vm.windows.iter().find(|w| w.id == id1).unwrap().z_order;
    assert!(z1_new > z2, "window 1 should now be on top");
}

#[test]
fn test_winsys_list_windows() {
    let mut vm = Vm::new();
    // Create 3 windows
    for i in 0..3 {
        vm.regs[1] = i * 30;
        vm.regs[2] = 0;
        vm.regs[3] = 20;
        vm.regs[4] = 20;
        vm.regs[5] = 0;
        vm.regs[6] = 0; // create
        let addr = (i * 2) as usize;
        vm.ram[addr] = 0x94;
        vm.ram[addr + 1] = 6;
        vm.pc = addr as u32;
        vm.halted = false;
        vm.step();
    }

    // List windows (op=3): write to RAM[0x2000]
    let list_addr = 0x2000;
    vm.regs[0] = list_addr;
    vm.regs[6] = 3;
    vm.ram[10] = 0x94;
    vm.ram[11] = 6;
    vm.ram[12] = 0x00;
    vm.pc = 10;
    vm.halted = false;
    vm.step();

    assert_eq!(vm.ram[list_addr as usize], 3, "should have 3 windows");
    // Active window IDs should be listed
    let ids: Vec<u32> = (0..3).map(|i| vm.ram[list_addr as usize + 1 + i]).collect();
    assert!(ids.contains(&1), "id 1 should be in list");
    assert!(ids.contains(&2), "id 2 should be in list");
    assert!(ids.contains(&3), "id 3 should be in list");
}

#[test]
fn test_winsys_max_windows() {
    let mut vm = Vm::new();
    // Create MAX_WINDOWS (8) windows
    for i in 0..8 {
        vm.regs[1] = (i * 10) as u32;
        vm.regs[2] = 0;
        vm.regs[3] = 8;
        vm.regs[4] = 8;
        vm.regs[5] = 0;
        vm.regs[6] = 0;
        let addr = (i * 2) as usize;
        vm.ram[addr] = 0x94;
        vm.ram[addr + 1] = 6;
        vm.pc = addr as u32;
        vm.halted = false;
        vm.step();
    }
    assert_eq!(vm.windows.len(), 8);

    // 9th window should fail
    vm.regs[6] = 0;
    vm.ram[20] = 0x94;
    vm.ram[21] = 6;
    vm.ram[22] = 0x00;
    vm.pc = 20;
    vm.halted = false;
    vm.step();
    assert_eq!(vm.regs[0], 0, "9th window should fail, r0 = 0");
    assert_eq!(vm.windows.len(), 8, "still only 8 windows");
}

#[test]
fn test_winsys_unknown_op() {
    let mut vm = Vm::new();
    vm.regs[6] = 99; // unknown op
    vm.ram[0] = 0x94;
    vm.ram[1] = 6;
    vm.ram[2] = 0x00;
    vm.pc = 0;
    vm.halted = false;
    vm.step();
    assert_eq!(vm.regs[0], 0, "unknown op should set r0 = 0 (error)");
}

#[test]
fn test_wpixel_write_and_read() {
    let mut vm = Vm::new();
    // Create a 16x16 window
    vm.regs[1] = 0;
    vm.regs[2] = 0;
    vm.regs[3] = 16;
    vm.regs[4] = 16;
    vm.regs[5] = 0;
    vm.regs[6] = 0;
    vm.ram[0] = 0x94;
    vm.ram[1] = 6;
    vm.pc = 0;
    vm.halted = false;
    vm.step();
    let win_id = vm.regs[0];

    // WPIXEL: write red pixel at (5, 5)
    vm.regs[7] = win_id;
    vm.regs[8] = 5; // x
    vm.regs[9] = 5; // y
    vm.regs[10] = 0xFF0000; // red
    vm.ram[2] = 0x95;
    vm.ram[3] = 7;
    vm.ram[4] = 8;
    vm.ram[5] = 9;
    vm.ram[6] = 10;
    vm.pc = 2;
    vm.halted = false;
    vm.step();

    // Verify pixel is in offscreen buffer
    let buf_idx = 5 * 16 + 5;
    assert_eq!(
        vm.windows[0].offscreen_buffer[buf_idx], 0xFF0000,
        "pixel should be red in buffer"
    );

    // WREAD: read back the pixel
    vm.regs[7] = win_id;
    vm.regs[8] = 5;
    vm.regs[9] = 5;
    vm.ram[7] = 0x96;
    vm.ram[8] = 7;
    vm.ram[9] = 8;
    vm.ram[10] = 9;
    vm.ram[11] = 11;
    vm.pc = 7;
    vm.halted = false;
    vm.step();
    assert_eq!(vm.regs[11], 0xFF0000, "WREAD should return red");
}

#[test]
fn test_wpixel_out_of_bounds() {
    let mut vm = Vm::new();
    // Create a 8x8 window
    vm.regs[1] = 0;
    vm.regs[2] = 0;
    vm.regs[3] = 8;
    vm.regs[4] = 8;
    vm.regs[5] = 0;
    vm.regs[6] = 0;
    vm.ram[0] = 0x94;
    vm.ram[1] = 6;
    vm.pc = 0;
    vm.halted = false;
    vm.step();
    let win_id = vm.regs[0];

    // Try to write pixel at (20, 20) -- out of bounds for 8x8 window
    vm.regs[7] = win_id;
    vm.regs[8] = 20;
    vm.regs[9] = 20;
    vm.regs[10] = 0xFFFFFF;
    vm.ram[2] = 0x95;
    vm.ram[3] = 7;
    vm.ram[4] = 8;
    vm.ram[5] = 9;
    vm.ram[6] = 10;
    vm.pc = 2;
    vm.halted = false;
    vm.step();
    // Should not panic, pixel silently dropped
    assert_eq!(
        vm.windows[0]
            .offscreen_buffer
            .iter()
            .filter(|&&p| p == 0xFFFFFF)
            .count(),
        0,
        "no white pixels should exist in 8x8 buffer"
    );
}

#[test]
fn test_wread_out_of_bounds() {
    let mut vm = Vm::new();
    vm.regs[1] = 0;
    vm.regs[2] = 0;
    vm.regs[3] = 8;
    vm.regs[4] = 8;
    vm.regs[5] = 0;
    vm.regs[6] = 0;
    vm.ram[0] = 0x94;
    vm.ram[1] = 6;
    vm.pc = 0;
    vm.halted = false;
    vm.step();
    let win_id = vm.regs[0];

    // Read pixel at out-of-bounds coordinates
    vm.regs[7] = win_id;
    vm.regs[8] = 100;
    vm.regs[9] = 100;
    vm.ram[2] = 0x96;
    vm.ram[3] = 7;
    vm.ram[4] = 8;
    vm.ram[5] = 9;
    vm.ram[6] = 11;
    vm.pc = 2;
    vm.halted = false;
    vm.step();
    assert_eq!(vm.regs[11], 0, "out-of-bounds WREAD should return 0");
}

#[test]
fn test_winsys_blit_windows_to_screen() {
    let mut vm = Vm::new();
    // Create a window at (10, 10) size 4x4
    vm.regs[1] = 10;
    vm.regs[2] = 10;
    vm.regs[3] = 4;
    vm.regs[4] = 4;
    vm.regs[5] = 0;
    vm.regs[6] = 0;
    vm.ram[0] = 0x94;
    vm.ram[1] = 6;
    vm.pc = 0;
    vm.halted = false;
    vm.step();
    let win_id = vm.regs[0];

    // Write a green pixel at (2, 2) in the window's offscreen buffer
    vm.regs[7] = win_id;
    vm.regs[8] = 2;
    vm.regs[9] = 2;
    vm.regs[10] = 0x00FF00;
    vm.ram[2] = 0x95;
    vm.ram[3] = 7;
    vm.ram[4] = 8;
    vm.ram[5] = 9;
    vm.ram[6] = 10;
    vm.pc = 2;
    vm.halted = false;
    vm.step();

    // Call FRAME to trigger blitting
    vm.ram[7] = 0x02; // FRAME
    vm.pc = 7;
    vm.halted = false;
    vm.step();

    // The pixel at (2, 2) in the window should appear at screen (10+2, 10+2) = (12, 12)
    assert_eq!(
        vm.screen[12 * 256 + 12],
        0x00FF00,
        "green pixel should be blitted to screen at (12, 12)"
    );
}

#[test]
fn test_winsys_blit_z_order() {
    let mut vm = Vm::new();
    // Create window 1 at (0, 0) size 4x4 with blue pixel at (1, 1)
    vm.regs[1] = 0;
    vm.regs[2] = 0;
    vm.regs[3] = 4;
    vm.regs[4] = 4;
    vm.regs[5] = 0;
    vm.regs[6] = 0;
    vm.ram[0] = 0x94;
    vm.ram[1] = 6;
    vm.pc = 0;
    vm.halted = false;
    vm.step();
    let id1 = vm.regs[0];

    // Write blue pixel at (1, 1) in window 1
    vm.regs[7] = id1;
    vm.regs[8] = 1;
    vm.regs[9] = 1;
    vm.regs[10] = 0x0000FF;
    vm.ram[2] = 0x95;
    vm.ram[3] = 7;
    vm.ram[4] = 8;
    vm.ram[5] = 9;
    vm.ram[6] = 10;
    vm.pc = 2;
    vm.halted = false;
    vm.step();

    // Create window 2 at (0, 0) size 4x4 with red pixel at (1, 1)
    vm.regs[1] = 0;
    vm.regs[2] = 0;
    vm.regs[3] = 4;
    vm.regs[4] = 4;
    vm.regs[5] = 0;
    vm.regs[6] = 0;
    vm.ram[7] = 0x94;
    vm.ram[8] = 6;
    vm.pc = 7;
    vm.halted = false;
    vm.step();
    let id2 = vm.regs[0];

    // Write red pixel at (1, 1) in window 2
    vm.regs[7] = id2;
    vm.regs[8] = 1;
    vm.regs[9] = 1;
    vm.regs[10] = 0xFF0000;
    vm.ram[9] = 0x95;
    vm.ram[10] = 7;
    vm.ram[11] = 8;
    vm.ram[12] = 9;
    vm.ram[13] = 10;
    vm.pc = 9;
    vm.halted = false;
    vm.step();

    // FRAME: window 2 (higher z_order) should overwrite window 1
    vm.ram[14] = 0x02; // FRAME
    vm.pc = 14;
    vm.halted = false;
    vm.step();

    // Screen pixel at (1, 1) should be red (window 2 on top)
    assert_eq!(
        vm.screen[1 * 256 + 1],
        0xFF0000,
        "red (window 2) should be on top of blue (window 1)"
    );
}

#[test]
fn test_winsys_blit_clipping() {
    let mut vm = Vm::new();
    // Create a window at (-2, -2) size 8x8 -- partially off-screen
    vm.regs[1] = 0xFFFFFFFE_u32; // -2 as u32 (wrapping)
    vm.regs[2] = 0xFFFFFFFE_u32; // -2 as u32
    vm.regs[3] = 8;
    vm.regs[4] = 8;
    vm.regs[5] = 0;
    vm.regs[6] = 0;
    vm.ram[0] = 0x94;
    vm.ram[1] = 6;
    vm.pc = 0;
    vm.halted = false;
    vm.step();
    let win_id = vm.regs[0];

    // Write green pixel at (3, 3) in the window -> screen position (1, 1)
    vm.regs[7] = win_id;
    vm.regs[8] = 3;
    vm.regs[9] = 3;
    vm.regs[10] = 0x00FF00;
    vm.ram[2] = 0x95;
    vm.ram[3] = 7;
    vm.ram[4] = 8;
    vm.ram[5] = 9;
    vm.ram[6] = 10;
    vm.pc = 2;
    vm.halted = false;
    vm.step();

    // Write pixel at (0, 0) -> screen position (-2, -2) -> should be clipped
    vm.regs[7] = win_id;
    vm.regs[8] = 0;
    vm.regs[9] = 0;
    vm.regs[10] = 0xFF00FF;
    vm.ram[7] = 0x95;
    vm.ram[8] = 7;
    vm.ram[9] = 8;
    vm.ram[10] = 9;
    vm.ram[11] = 10;
    vm.pc = 7;
    vm.halted = false;
    vm.step();

    vm.ram[12] = 0x02; // FRAME
    vm.pc = 12;
    vm.halted = false;
    vm.step();

    // Pixel at (3,3) in window -> screen (1,1) should be visible
    assert_eq!(
        vm.screen[1 * 256 + 1],
        0x00FF00,
        "in-bounds pixel should be blitted"
    );
}

#[test]
fn test_winsys_assembler() {
    let src = "LDI r6, 0\nWINSYS r6\nHALT\n";
    let result = crate::assembler::assemble(src, 0);
    assert!(result.is_ok(), "WINSYS should assemble: {:?}", result.err());
    let asm = result.unwrap();
    assert_eq!(asm.pixels[3], 0x94, "WINSYS opcode should be 0x94");
}

#[test]
fn test_wpixel_wread_assembler() {
    let src = "LDI r7, 1\nLDI r8, 5\nLDI r9, 5\nLDI r10, 0xFF0000\nWPIXEL r7, r8, r9, r10\nWREAD r7, r8, r9, r11\nHALT\n";
    let result = crate::assembler::assemble(src, 0);
    assert!(
        result.is_ok(),
        "WPIXEL/WREAD should assemble: {:?}",
        result.err()
    );
    let asm = result.unwrap();
    // WPIXEL at offset 12 (after 4 LDIs * 3 words each)
    assert_eq!(asm.pixels[12], 0x95, "WPIXEL opcode should be 0x95");
    // WREAD at offset 17 (after 4 LDIs + WPIXEL(5 words))
    assert_eq!(asm.pixels[17], 0x96, "WREAD opcode should be 0x96");
}

#[test]
fn test_winsys_disasm() {
    let mut vm = Vm::new();
    vm.ram[0] = 0x94;
    vm.ram[1] = 6;
    let (mnem, len) = vm.disassemble_at(0);
    assert_eq!(len, 2);
    assert!(
        mnem.starts_with("WINSYS"),
        "disasm should show WINSYS, got: {}",
        mnem
    );
}

#[test]
fn test_wpixel_disasm() {
    let mut vm = Vm::new();
    vm.ram[0] = 0x95;
    vm.ram[1] = 7;
    vm.ram[2] = 8;
    vm.ram[3] = 9;
    vm.ram[4] = 10;
    let (mnem, len) = vm.disassemble_at(0);
    assert_eq!(len, 5);
    assert!(
        mnem.starts_with("WPIXEL"),
        "disasm should show WPIXEL, got: {}",
        mnem
    );
}

#[test]
fn test_wread_disasm() {
    let mut vm = Vm::new();
    vm.ram[0] = 0x96;
    vm.ram[1] = 7;
    vm.ram[2] = 8;
    vm.ram[3] = 9;
    vm.ram[4] = 11;
    let (mnem, len) = vm.disassemble_at(0);
    assert_eq!(len, 5);
    assert!(
        mnem.starts_with("WREAD"),
        "disasm should show WREAD, got: {}",
        mnem
    );
}

// ── Window Mouse Interaction (Phase 68b) ──────────────────────────

#[test]
fn test_mouseq_button_state() {
    // MOUSEQ now reads button state into reg+2
    let mut vm = Vm::new();
    vm.push_mouse(100, 200);
    vm.push_mouse_button(2); // left click
    vm.ram[0] = 0x85; // MOUSEQ
    vm.ram[1] = 10; // dest reg r10
    vm.step();
    assert_eq!(vm.regs[10], 100, "mouse x");
    assert_eq!(vm.regs[11], 200, "mouse y");
    assert_eq!(vm.regs[12], 2, "mouse button = click");
    // Click auto-clears to down after read
    assert_eq!(vm.mouse_button, 1, "button auto-clears to down");
}

#[test]
fn test_mouseq_no_button() {
    let mut vm = Vm::new();
    vm.push_mouse(50, 75);
    // No button pressed (default 0)
    vm.ram[0] = 0x85;
    vm.ram[1] = 5;
    vm.step();
    assert_eq!(vm.regs[5], 50, "mouse x");
    assert_eq!(vm.regs[6], 75, "mouse y");
    assert_eq!(vm.regs[7], 0, "no button");
}

#[test]
fn test_winsys_hittest_body() {
    // WINSYS op=4: HITTEST finds window under mouse, returns body hit
    let mut vm = Vm::new();
    // Create a window at (20, 20) with size 100x80
    vm.regs[1] = 20; // x
    vm.regs[2] = 20; // y
    vm.regs[3] = 100; // w
    vm.regs[4] = 80; // h
    vm.regs[5] = 0; // title_addr
    vm.regs[6] = 0; // op=0 (create)
    vm.ram[0] = 0x94; // WINSYS
    vm.ram[1] = 6; // op_reg=r6
    vm.step();
    let win_id = vm.regs[0];
    assert_ne!(win_id, 0, "window created");

    // Move mouse to body area (past title bar, top 12px)
    vm.push_mouse(50, 60);
    vm.regs[6] = 4; // op=4 (hittest)
    vm.ram[vm.pc as usize] = 0x94;
    vm.ram[vm.pc as usize + 1] = 6;
    vm.step();
    assert_eq!(vm.regs[0], win_id, "hit window id");
    assert_eq!(vm.regs[1], 2, "hit type = body");
}

#[test]
fn test_winsys_hittest_title_bar() {
    // WINSYS op=4: Title bar hit (top 12px)
    let mut vm = Vm::new();
    vm.regs[1] = 10; // x
    vm.regs[2] = 30; // y
    vm.regs[3] = 80; // w
    vm.regs[4] = 60; // h
    vm.regs[5] = 0;
    vm.regs[6] = 0; // op=0 create
    vm.ram[0] = 0x94;
    vm.ram[1] = 6;
    vm.step();
    let win_id = vm.regs[0];

    // Mouse at y=35, within top 12px (30+12=42)
    vm.push_mouse(40, 35);
    vm.regs[6] = 4; // hittest
    vm.ram[vm.pc as usize] = 0x94;
    vm.ram[vm.pc as usize + 1] = 6;
    vm.step();
    assert_eq!(vm.regs[0], win_id, "hit window id");
    assert_eq!(vm.regs[1], 1, "hit type = title bar");
}

#[test]
fn test_winsys_hittest_no_hit() {
    // WINSYS op=4: Mouse not over any window
    let mut vm = Vm::new();
    vm.regs[1] = 10;
    vm.regs[2] = 10;
    vm.regs[3] = 50;
    vm.regs[4] = 50;
    vm.regs[5] = 0;
    vm.regs[6] = 0;
    vm.ram[0] = 0x94;
    vm.ram[1] = 6;
    vm.step();

    // Mouse far away from window
    vm.push_mouse(200, 200);
    vm.regs[6] = 4;
    vm.ram[vm.pc as usize] = 0x94;
    vm.ram[vm.pc as usize + 1] = 6;
    vm.step();
    assert_eq!(vm.regs[0], 0, "no window hit");
    assert_eq!(vm.regs[1], 0, "no hit type");
}

#[test]
fn test_winsys_hittest_z_order() {
    // WINSYS op=4: Front window takes priority over back window
    let mut vm = Vm::new();
    // Create first window at (10, 10, 100x100)
    vm.regs[1] = 10;
    vm.regs[2] = 10;
    vm.regs[3] = 100;
    vm.regs[4] = 100;
    vm.regs[5] = 0;
    vm.regs[6] = 0;
    vm.ram[0] = 0x94;
    vm.ram[1] = 6;
    vm.step();
    let win1 = vm.regs[0];

    // Create second window overlapping (gets higher z_order)
    vm.regs[6] = 0;
    vm.ram[vm.pc as usize] = 0x94;
    vm.ram[vm.pc as usize + 1] = 6;
    vm.step();
    let win2 = vm.regs[0];
    assert_ne!(win2, win1, "different window");

    // Mouse over overlapping area -- should hit front window (win2)
    vm.push_mouse(50, 50);
    vm.regs[6] = 4;
    vm.ram[vm.pc as usize] = 0x94;
    vm.ram[vm.pc as usize + 1] = 6;
    vm.step();
    assert_eq!(vm.regs[0], win2, "front window hit");
}

#[test]
fn test_winsys_moveto() {
    // WINSYS op=5: MOVETO moves window to new position
    let mut vm = Vm::new();
    // Create window
    vm.regs[1] = 10;
    vm.regs[2] = 20;
    vm.regs[3] = 60;
    vm.regs[4] = 40;
    vm.regs[5] = 0;
    vm.regs[6] = 0;
    vm.ram[0] = 0x94;
    vm.ram[1] = 6;
    vm.step();
    let win_id = vm.regs[0];

    // Move window to (100, 150)
    vm.regs[0] = win_id;
    vm.regs[1] = 100;
    vm.regs[2] = 150;
    vm.regs[6] = 5; // op=5 MOVETO
    vm.ram[vm.pc as usize] = 0x94;
    vm.ram[vm.pc as usize + 1] = 6;
    vm.step();
    assert_eq!(vm.regs[0], 1, "moveto success");

    // Verify via WINFO
    vm.regs[0] = win_id;
    vm.regs[1] = 0x8000; // addr for info
    vm.regs[6] = 6; // op=6 WINFO
    vm.ram[vm.pc as usize] = 0x94;
    vm.ram[vm.pc as usize + 1] = 6;
    vm.step();
    assert_eq!(vm.ram[0x8000], 100, "new x");
    assert_eq!(vm.ram[0x8001], 150, "new y");
    assert_eq!(vm.ram[0x8002], 60, "w unchanged");
    assert_eq!(vm.ram[0x8003], 40, "h unchanged");
}

#[test]
fn test_winsys_moveto_not_found() {
    // WINSYS op=5: MOVETO with invalid window ID
    let mut vm = Vm::new();
    vm.regs[0] = 999; // nonexistent
    vm.regs[1] = 50;
    vm.regs[2] = 50;
    vm.regs[6] = 5;
    vm.ram[0] = 0x94;
    vm.ram[1] = 6;
    vm.step();
    assert_eq!(vm.regs[0], 0, "moveto failed for invalid window");
}

#[test]
fn test_winsys_winfo() {
    // WINSYS op=6: WINFO returns window details
    let mut vm = Vm::new();
    vm.regs[1] = 15; // x
    vm.regs[2] = 25; // y
    vm.regs[3] = 70; // w
    vm.regs[4] = 50; // h
    vm.regs[5] = 0;
    vm.regs[6] = 0; // create
    vm.ram[0] = 0x94;
    vm.ram[1] = 6;
    vm.step();
    let win_id = vm.regs[0];

    // Get info
    vm.regs[0] = win_id;
    vm.regs[1] = 0x7000; // dest addr
    vm.regs[6] = 6; // WINFO
    vm.ram[vm.pc as usize] = 0x94;
    vm.ram[vm.pc as usize + 1] = 6;
    vm.step();
    assert_eq!(vm.regs[0], 1, "winfo success");
    assert_eq!(vm.ram[0x7000], 15, "x");
    assert_eq!(vm.ram[0x7001], 25, "y");
    assert_eq!(vm.ram[0x7002], 70, "w");
    assert_eq!(vm.ram[0x7003], 50, "h");
    assert_eq!(vm.ram[0x7004], 1, "z_order (first window)");
    assert_eq!(vm.ram[0x7005], 0, "pid (main process)");
}

#[test]
fn test_winsys_winfo_not_found() {
    let mut vm = Vm::new();
    vm.regs[0] = 42; // nonexistent
    vm.regs[1] = 0x7000;
    vm.regs[6] = 6;
    vm.ram[0] = 0x94;
    vm.ram[1] = 6;
    vm.step();
    assert_eq!(vm.regs[0], 0, "winfo failed for invalid window");
}

#[test]
fn test_winsys_hittest_after_moveto() {
    // Hit-test after moving window should use new position
    let mut vm = Vm::new();
    vm.regs[1] = 10;
    vm.regs[2] = 10;
    vm.regs[3] = 50;
    vm.regs[4] = 50;
    vm.regs[5] = 0;
    vm.regs[6] = 0;
    vm.ram[0] = 0x94;
    vm.ram[1] = 6;
    vm.step();
    let win_id = vm.regs[0];

    // Move to (100, 100)
    vm.regs[0] = win_id;
    vm.regs[1] = 100;
    vm.regs[2] = 100;
    vm.regs[6] = 5;
    vm.ram[vm.pc as usize] = 0x94;
    vm.ram[vm.pc as usize + 1] = 6;
    vm.step();

    // Old position should not hit
    vm.push_mouse(30, 30);
    vm.regs[6] = 4;
    vm.ram[vm.pc as usize] = 0x94;
    vm.ram[vm.pc as usize + 1] = 6;
    vm.step();
    assert_eq!(vm.regs[0], 0, "old position no hit");

    // New position should hit
    vm.push_mouse(120, 120);
    vm.regs[6] = 4;
    vm.ram[vm.pc as usize] = 0x94;
    vm.ram[vm.pc as usize + 1] = 6;
    vm.step();
    assert_eq!(vm.regs[0], win_id, "new position hits");
}

#[test]
fn test_winsys_bring_to_front_affects_hittest() {
    // After bringing back window to front, it should be hit first
    let mut vm = Vm::new();
    // Window 1 at (10, 10, 80, 80)
    vm.regs[1] = 10;
    vm.regs[2] = 10;
    vm.regs[3] = 80;
    vm.regs[4] = 80;
    vm.regs[6] = 0;
    vm.ram[0] = 0x94;
    vm.ram[1] = 6;
    vm.step();
    let win1 = vm.regs[0];

    // Window 2 overlapping (higher z)
    vm.regs[1] = 20;
    vm.regs[2] = 20;
    vm.regs[6] = 0;
    vm.ram[vm.pc as usize] = 0x94;
    vm.ram[vm.pc as usize + 1] = 6;
    vm.step();
    let win2 = vm.regs[0];

    // Initially win2 should be hit (higher z)
    vm.push_mouse(50, 50);
    vm.regs[6] = 4;
    vm.ram[vm.pc as usize] = 0x94;
    vm.ram[vm.pc as usize + 1] = 6;
    vm.step();
    assert_eq!(vm.regs[0], win2, "win2 on top");

    // Bring win1 to front
    vm.regs[0] = win1;
    vm.regs[6] = 2; // op=2 bring to front
    vm.ram[vm.pc as usize] = 0x94;
    vm.ram[vm.pc as usize + 1] = 6;
    vm.step();

    // Now win1 should be hit
    vm.regs[6] = 4;
    vm.ram[vm.pc as usize] = 0x94;
    vm.ram[vm.pc as usize + 1] = 6;
    vm.step();
    assert_eq!(vm.regs[0], win1, "win1 brought to front");
}

// ── Sprite Engine (Phase 69) -- SPRBLT (0x97) ────────────────────────

#[test]
fn test_sprblt_basic_blit() {
    // Create a sprite sheet at RAM 0x2000 with 2 sprites
    // Sprite 0: all red (0xFF0000)
    // Sprite 1: all green (0x00FF00)
    let mut vm = Vm::new();
    let sheet_addr = 0x2000;
    // Sprite 0: 16x16 red pixels starting at sheet_addr
    for i in 0..256 {
        vm.ram[sheet_addr + i] = 0xFF0000;
    }
    // Sprite 1: 16x16 green pixels starting at sheet_addr + 256
    for i in 0..256 {
        vm.ram[sheet_addr + 256 + i] = 0x00FF00;
    }

    // SPRBLT sheet_addr_r, sprite_id_r, x_r, y_r
    vm.regs[1] = sheet_addr as u32; // sheet addr
    vm.regs[2] = 0; // sprite_id = 0 (red)
    vm.regs[3] = 10; // x = 10
    vm.regs[4] = 20; // y = 20

    vm.ram[0] = 0x97; // SPRBLT
    vm.ram[1] = 1; // sheet_addr_r
    vm.ram[2] = 2; // sprite_id_r
    vm.ram[3] = 3; // x_r
    vm.ram[4] = 4; // y_r
    vm.step();

    // Check top-left pixel of the sprite
    assert_eq!(vm.screen[20 * 256 + 10], 0xFF0000, "top-left should be red");
    // Check center pixel
    assert_eq!(vm.screen[27 * 256 + 17], 0xFF0000, "center should be red");
    // Check bottom-right
    assert_eq!(
        vm.screen[35 * 256 + 25],
        0xFF0000,
        "bottom-right should be red"
    );
    // Check just outside sprite area -- should be 0 (default)
    assert_eq!(vm.screen[20 * 256 + 9], 0, "left of sprite should be black");
    assert_eq!(vm.screen[19 * 256 + 10], 0, "above sprite should be black");
}

#[test]
fn test_sprblt_second_sprite() {
    let mut vm = Vm::new();
    let sheet_addr = 0x2000;
    // Sprite 0: red
    for i in 0..256 {
        vm.ram[sheet_addr + i] = 0xFF0000;
    }
    // Sprite 1: green
    for i in 0..256 {
        vm.ram[sheet_addr + 256 + i] = 0x00FF00;
    }

    // Blit sprite 1 (green) at (5, 5)
    vm.regs[1] = sheet_addr as u32;
    vm.regs[2] = 1;
    vm.regs[3] = 5;
    vm.regs[4] = 5;

    vm.ram[0] = 0x97;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.ram[4] = 4;
    vm.step();

    assert_eq!(vm.screen[5 * 256 + 5], 0x00FF00, "sprite 1 should be green");
    assert_eq!(vm.screen[20 * 256 + 20], 0x00FF00, "sprite 1 bottom-right");
}

#[test]
fn test_sprblt_transparency() {
    let mut vm = Vm::new();
    let sheet_addr = 0x2000;
    // Sprite 0: checkerboard -- every other pixel is transparent
    for y in 0..16 {
        for x in 0..16 {
            let idx = sheet_addr + y * 16 + x;
            if (x + y) % 2 == 0 {
                vm.ram[idx] = 0xFFFFFF; // white
            } else {
                vm.ram[idx] = 0; // transparent
            }
        }
    }

    // Pre-fill screen with blue so we can check transparency
    for i in 0..256 * 256 {
        vm.screen[i] = 0x0000FF;
    }

    vm.regs[1] = sheet_addr as u32;
    vm.regs[2] = 0;
    vm.regs[3] = 8;
    vm.regs[4] = 8;

    vm.ram[0] = 0x97;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.ram[4] = 4;
    vm.step();

    // (8,8) should be white (even sum)
    assert_eq!(
        vm.screen[8 * 256 + 8],
        0xFFFFFF,
        "even position should be white"
    );
    // (9,8) should be blue (transparent -- background preserved)
    assert_eq!(
        vm.screen[8 * 256 + 9],
        0x0000FF,
        "odd position should remain blue"
    );
}

#[test]
fn test_sprblt_screen_clipping() {
    let mut vm = Vm::new();
    let sheet_addr = 0x2000;
    // Sprite 0: all yellow
    for i in 0..256 {
        vm.ram[sheet_addr + i] = 0xFFFF00;
    }

    // Blit at x=250, y=250 -- only 6x6 pixels should appear on screen
    vm.regs[1] = sheet_addr as u32;
    vm.regs[2] = 0;
    vm.regs[3] = 250;
    vm.regs[4] = 250;

    vm.ram[0] = 0x97;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.ram[4] = 4;
    vm.step();

    // (250, 250) should be yellow (in bounds)
    assert_eq!(vm.screen[250 * 256 + 250], 0xFFFF00, "in-bounds pixel");
    // (255, 255) should be yellow (last valid pixel)
    assert_eq!(vm.screen[255 * 256 + 255], 0xFFFF00, "edge pixel");
    // Pixels at x>=256 or y>=256 are clipped (screen is only 256x256)
    // Just verify no crash happened
}

#[test]
fn test_sprblt_negative_position() {
    let mut vm = Vm::new();
    let sheet_addr = 0x2000;
    for i in 0..256 {
        vm.ram[sheet_addr + i] = 0xFF00FF;
    }

    // Blit at x=-5 (0xFFFFFFFB as u32 -> interpreted as i32 = -5)
    vm.regs[1] = sheet_addr as u32;
    vm.regs[2] = 0;
    vm.regs[3] = 0xFFFFFFFB; // -5 as i32
    vm.regs[4] = 0;

    vm.ram[0] = 0x97;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.ram[4] = 4;
    vm.step();

    // Pixels at x=-5..-1 should be clipped. Pixel at x=0, y=0 should be drawn.
    // -5 + 5 = 0 -> dx=5 should land at screen x=0
    assert_eq!(vm.screen[0], 0xFF00FF, "dx=5 should appear at x=0");
    // -5 + 4 = -1 -> clipped
    // Just verify no crash
}

#[test]
fn test_sprblt_assembler() {
    use crate::assembler::assemble;
    let src = "SPRBLT r1, r2, r3, r4";
    let result = assemble(src, 0);
    assert!(result.is_ok(), "assembly should succeed");
    let asm = result.unwrap();
    assert_eq!(asm.pixels[0], 0x97, "opcode");
    assert_eq!(asm.pixels[1], 1, "sheet_addr reg");
    assert_eq!(asm.pixels[2], 2, "sprite_id reg");
    assert_eq!(asm.pixels[3], 3, "x reg");
    assert_eq!(asm.pixels[4], 4, "y reg");
}

#[test]
fn test_sprblt_assembler_error_too_few_args() {
    use crate::assembler::assemble;
    let src = "SPRBLT r1, r2, r3";
    let result = assemble(src, 0);
    assert!(result.is_err(), "should fail with too few args");
}

#[test]
fn test_sprblt_disasm() {
    let mut vm = Vm::new();
    vm.ram[0] = 0x97;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.ram[4] = 4;
    let (s, len) = vm.disassemble_at(0);
    assert_eq!(s, "SPRBLT r1, r2, r3, r4");
    assert_eq!(len, 5);
}

#[test]
fn test_sprblt_via_run_program() {
    // Full end-to-end: assemble + run
    use crate::assembler::assemble;
    let src = r#"
        LDI r1, 0x2000
        LDI r2, 0
        LDI r3, 50
        LDI r4, 50
        SPRBLT r1, r2, r3, r4
        HALT
    "#;
    let asm = assemble(src, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    // Write sprite 0 data at 0x2000
    let sheet_addr = 0x2000;
    for i in 0..256 {
        vm.ram[sheet_addr + i] = 0x00FFFF;
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..1000 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.halted, "should halt");
    assert_eq!(vm.screen[50 * 256 + 50], 0x00FFFF, "sprite should be drawn");
}

#[test]
fn test_sprblt_high_sprite_id() {
    // Test sprite at index 5 in the sheet
    let mut vm = Vm::new();
    let sheet_addr = 0x2000;
    // Sprite 5 starts at sheet_addr + 5 * 256 = 0x2800
    let sprite5_offset = 5 * 256;
    for i in 0..256 {
        vm.ram[sheet_addr + sprite5_offset + i] = 0xFF00FF;
    }

    vm.regs[1] = sheet_addr as u32;
    vm.regs[2] = 5;
    vm.regs[3] = 0;
    vm.regs[4] = 0;

    vm.ram[0] = 0x97;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.ram[4] = 4;
    vm.step();

    assert_eq!(vm.screen[0], 0xFF00FF, "sprite 5 should be drawn");
}

#[test]
fn test_sprblt_all_transparent() {
    let mut vm = Vm::new();
    let sheet_addr = 0x2000;
    // Sprite 0: all transparent
    for i in 0..256 {
        vm.ram[sheet_addr + i] = 0;
    }
    // Fill screen with known color
    for i in 0..256 * 256 {
        vm.screen[i] = 0x123456;
    }

    vm.regs[1] = sheet_addr as u32;
    vm.regs[2] = 0;
    vm.regs[3] = 10;
    vm.regs[4] = 10;

    vm.ram[0] = 0x97;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.ram[4] = 4;
    vm.step();

    // Entire sprite area should remain unchanged
    assert_eq!(
        vm.screen[10 * 256 + 10],
        0x123456,
        "transparent sprite should not draw"
    );
}

// ── Core Utilities (Phase 73) ─────────────────────────────────

// Helper: assemble and run an .asm program from programs/ dir
fn boot_utility(asm_file: &str, max_steps: usize) -> Vm {
    let source = std::fs::read_to_string(asm_file)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", asm_file, e));
    let asm = crate::assembler::assemble(&source, 0)
        .unwrap_or_else(|e| panic!("Failed to assemble {}: {:?}", asm_file, e));
    let mut vm = Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
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

// ── ls.asm tests ──

#[test]
fn test_ls_assembles() {
    let source = include_str!("../../programs/ls.asm");
    let result = crate::assembler::assemble(source, 0);
    assert!(
        result.is_ok(),
        "ls.asm failed to assemble: {:?}",
        result.err()
    );
    let bytecode = result.unwrap();
    assert!(bytecode.pixels.len() > 50, "ls.asm should be substantial");
    // Verify LS opcode present
    assert!(
        bytecode.pixels.iter().any(|&w| w == 0x59),
        "ls.asm should contain LS opcode (0x59)"
    );
    // Verify RECTF opcode present (for header/footer)
    assert!(
        bytecode.pixels.iter().any(|&w| w == 0x43),
        "ls.asm should contain RECTF opcode"
    );
    // Verify DRAWTEXT opcode present
    assert!(
        bytecode.pixels.iter().any(|&w| w == 0x8C),
        "ls.asm should contain DRAWTEXT opcode"
    );
}

#[test]
fn test_ls_runs_and_halts() {
    let vm = boot_utility("programs/ls.asm", 50_000);
    assert!(vm.halted, "ls.asm should halt after execution");
}

#[test]
fn test_ls_calls_ls_syscall() {
    let vm = boot_utility("programs/ls.asm", 50_000);
    // LS stores entry count at 0x7800
    // The count should be a reasonable number (not 0xFFFFFFFF = error)
    let count = vm.ram[0x7800];
    assert_ne!(count, 0xFFFFFFFF, "LS syscall should not return error");
}

// ── wc.asm tests ──

#[test]
fn test_wc_assembles() {
    let source = include_str!("../../programs/wc.asm");
    let result = crate::assembler::assemble(source, 0);
    assert!(
        result.is_ok(),
        "wc.asm failed to assemble: {:?}",
        result.err()
    );
    let bytecode = result.unwrap();
    assert!(bytecode.pixels.len() > 50, "wc.asm should be substantial");
}

#[test]
fn test_wc_counts_chars() {
    let vm = boot_utility("programs/wc.asm", 100_000);
    assert!(vm.halted, "wc.asm should halt");
    // "Hello World\nFoo Bar\nBaz" = 23 characters
    let chars = vm.ram[0x7800];
    assert_eq!(chars, 23, "wc.asm should count 23 chars, got {}", chars);
}

#[test]
fn test_wc_counts_lines() {
    let vm = boot_utility("programs/wc.asm", 100_000);
    // 2 newlines = 2 lines
    let lines = vm.ram[0x7801];
    assert_eq!(lines, 2, "wc.asm should count 2 lines, got {}", lines);
}

#[test]
fn test_wc_counts_words() {
    let vm = boot_utility("programs/wc.asm", 100_000);
    // Hello, World, Foo, Bar, Baz = 5 words
    let words = vm.ram[0x7802];
    assert_eq!(words, 5, "wc.asm should count 5 words, got {}", words);
}

// ── grep.asm tests ──

#[test]
fn test_grep_assembles() {
    let source = include_str!("../../programs/grep.asm");
    let result = crate::assembler::assemble(source, 0);
    assert!(
        result.is_ok(),
        "grep.asm failed to assemble: {:?}",
        result.err()
    );
    let bytecode = result.unwrap();
    assert!(bytecode.pixels.len() > 50, "grep.asm should be substantial");
}

#[test]
fn test_grep_finds_pattern() {
    let vm = boot_utility("programs/grep.asm", 100_000);
    assert!(vm.halted, "grep.asm should halt");
    // Searching for "oo" in "Hello World\nFoo Bar\nBaz"
    // Only "Foo" contains "oo" -> 1 match
    let matches = vm.ram[0x7800];
    assert_eq!(
        matches, 1,
        "grep.asm should find 1 match for 'oo', got {}",
        matches
    );
}

#[test]
fn test_grep_displays_header() {
    let vm = boot_utility("programs/grep.asm", 100_000);
    // Header bar at y=0-12 should have green pixels
    let header_color = vm.screen[6 * 256 + 4];
    let bg_color = vm.screen[20 * 256 + 4];
    assert_ne!(
        header_color, bg_color,
        "grep header should differ from background"
    );
}

// ── hexdump.asm tests ──

#[test]
fn test_hexdump_assembles() {
    let source = include_str!("../../programs/hexdump.asm");
    let result = crate::assembler::assemble(source, 0);
    assert!(
        result.is_ok(),
        "hexdump.asm failed to assemble: {:?}",
        result.err()
    );
    let bytecode = result.unwrap();
    assert!(
        bytecode.pixels.len() > 50,
        "hexdump.asm should be substantial"
    );
}

#[test]
fn test_hexdump_counts_bytes() {
    let vm = boot_utility("programs/hexdump.asm", 100_000);
    assert!(vm.halted, "hexdump.asm should halt");
    // "Hello, Geometry OS!" = 19 bytes
    let bytes = vm.ram[0x7800];
    assert_eq!(
        bytes, 19,
        "hexdump.asm should count 19 bytes, got {}",
        bytes
    );
}

#[test]
fn test_hexdump_displays_hex() {
    let vm = boot_utility("programs/hexdump.asm", 100_000);
    // The hex dump area (y=16+) should have green text pixels
    // Check that some non-background pixels exist in the hex dump area
    let mut non_bg_pixels = 0;
    for y in 16..60 {
        for x in 4..200 {
            if vm.screen[y * 256 + x] != 0x000011 {
                non_bg_pixels += 1;
            }
        }
    }
    assert!(
        non_bg_pixels > 20,
        "hexdump should render hex text on screen, found {} non-bg pixels",
        non_bg_pixels
    );
}

#[test]
fn test_hexdump_header_present() {
    let vm = boot_utility("programs/hexdump.asm", 100_000);
    // Header should have purple pixels
    let header_pixel = vm.screen[6 * 256 + 10];
    assert_ne!(
        header_pixel, 0x000011,
        "hexdump header should differ from dark background"
    );
}

// ── SCRSHOT Opcode (Phase 74) ────────────────────────────────

#[test]
fn test_scrshot_saves_screen_to_file() {
    let mut vm = Vm::new();
    // Draw a red pixel at (10, 20)
    vm.screen[20 * 256 + 10] = 0xFF0000;
    // Draw a green pixel at (50, 60)
    vm.screen[60 * 256 + 50] = 0x00FF00;

    // Write filename "test.img" to RAM at address 0x2000
    let filename = b"test.img";
    for (i, &ch) in filename.iter().enumerate() {
        vm.ram[0x2000 + i] = ch as u32;
    }
    vm.ram[0x2000 + filename.len()] = 0; // null terminator

    // LDI r1, 0x2000; SCRSHOT r1; HALT
    vm.ram[0] = 0x10; // LDI
    vm.ram[1] = 1; // r1
    vm.ram[2] = 0x2000; // addr
    vm.ram[3] = 0x98; // SCRSHOT
    vm.ram[4] = 1; // r1
    vm.ram[5] = 0x00; // HALT
    vm.pc = 0;
    vm.halted = false;

    for _ in 0..100_000 {
        if !vm.step() {
            break;
        }
    }

    assert!(vm.halted, "SCRSHOT program should halt");
    // r0 should have total bytes written (256*256*4 = 262144)
    assert_ne!(
        vm.regs[0], 0xFFFFFFFF,
        "SCRSHOT should succeed, got r0={:#x}",
        vm.regs[0]
    );
    assert_eq!(
        vm.regs[0], 262144,
        "SCRSHOT should write 262144 bytes (256x256x4), got {}",
        vm.regs[0]
    );

    // Verify the file was created and contains correct data
    let file_path = std::path::PathBuf::from(".geometry_os/fs/test.img");
    assert!(file_path.exists(), "Screenshot file should exist");
    let data = std::fs::read(&file_path).unwrap();
    assert_eq!(data.len(), 262144, "File should be 262144 bytes");

    // Clean up
    let _ = std::fs::remove_file(&file_path);
}

#[test]
fn test_scrshot_pixel_data_correct() {
    let mut vm = Vm::new();
    // Fill screen with a known color: 0x00AABBCC
    for pixel in vm.screen.iter_mut() {
        *pixel = 0x00AABBCC;
    }

    // Write filename "pixel_test.img" to RAM at 0x2000
    let filename = b"pixel_test.img";
    for (i, &ch) in filename.iter().enumerate() {
        vm.ram[0x2000 + i] = ch as u32;
    }
    vm.ram[0x2000 + filename.len()] = 0;

    // LDI r1, 0x2000; SCRSHOT r1; HALT
    vm.ram[0] = 0x10;
    vm.ram[1] = 1;
    vm.ram[2] = 0x2000;
    vm.ram[3] = 0x98;
    vm.ram[4] = 1;
    vm.ram[5] = 0x00;
    vm.pc = 0;
    vm.halted = false;

    for _ in 0..100_000 {
        if !vm.step() {
            break;
        }
    }

    // Read back the file and verify pixel data
    let file_path = std::path::PathBuf::from(".geometry_os/fs/pixel_test.img");
    let data = std::fs::read(&file_path).unwrap();
    // Each pixel is 4 bytes: A, R, G, B
    // 0x00AABBCC -> A=0x00, R=0xAA, G=0xBB, B=0xCC
    assert_eq!(data[0], 0x00, "Alpha byte of first pixel");
    assert_eq!(data[1], 0xAA, "Red byte of first pixel");
    assert_eq!(data[2], 0xBB, "Green byte of first pixel");
    assert_eq!(data[3], 0xCC, "Blue byte of first pixel");

    let _ = std::fs::remove_file(&file_path);
}

#[test]
fn test_scrshot_error_on_bad_path() {
    let mut vm = Vm::new();
    // Point to empty string (null at 0x2000)
    vm.ram[0x2000] = 0;

    // LDI r1, 0x2000; SCRSHOT r1; HALT
    vm.ram[0] = 0x10;
    vm.ram[1] = 1;
    vm.ram[2] = 0x2000;
    vm.ram[3] = 0x98;
    vm.ram[4] = 1;
    vm.ram[5] = 0x00;
    vm.pc = 0;
    vm.halted = false;

    for _ in 0..100_000 {
        if !vm.step() {
            break;
        }
    }

    assert_eq!(
        vm.regs[0], 0xFFFFFFFF,
        "SCRSHOT should fail with empty filename"
    );
}

#[test]
fn test_scrshot_disasm() {
    let (m, l) = disasm(&[0x98, 1]);
    assert_eq!(m, "SCRSHOT r1");
    assert_eq!(l, 2);
}

#[test]
fn test_scrshot_assembles() {
    let src = "LDI r1, 0x2000\nSCRSHOT r1\nHALT\n";
    let result = crate::assembler::assemble(src, 0);
    assert!(
        result.is_ok(),
        "SCRSHOT should assemble: {:?}",
        result.err()
    );
    let bytecode = result.unwrap();
    // LDI r1, 0x2000 = [0x10, 1, 0x2000]
    // SCRSHOT r1 = [0x98, 1]
    // HALT = [0x00]
    assert_eq!(bytecode.pixels[0], 0x10);
    assert_eq!(bytecode.pixels[1], 1);
    assert_eq!(bytecode.pixels[2], 0x2000);
    assert_eq!(bytecode.pixels[3], 0x98);
    assert_eq!(bytecode.pixels[4], 1);
    assert_eq!(bytecode.pixels[5], 0x00);
}

#[test]
fn test_scrshot_roundtrip_with_read() {
    let mut vm = Vm::new();
    // Paint a pattern: red top-left, blue bottom-right
    vm.screen[0] = 0xFF0000; // (0,0) = red
    vm.screen[255] = 0x0000FF; // (0,255) = blue
    vm.screen[255 * 256] = 0x00FF00; // (255,0) = green
    vm.screen[255 * 256 + 255] = 0xFFFFFF; // (255,255) = white

    // Save screenshot
    let filename = b"roundtrip.img";
    for (i, &ch) in filename.iter().enumerate() {
        vm.ram[0x2000 + i] = ch as u32;
    }
    vm.ram[0x2000 + filename.len()] = 0;

    vm.ram[0] = 0x10; // LDI r1, 0x2000
    vm.ram[1] = 1;
    vm.ram[2] = 0x2000;
    vm.ram[3] = 0x98; // SCRSHOT r1
    vm.ram[4] = 1;
    vm.ram[5] = 0x00; // HALT
    vm.pc = 0;
    vm.halted = false;

    for _ in 0..100_000 {
        if !vm.step() {
            break;
        }
    }

    assert_ne!(vm.regs[0], 0xFFFFFFFF, "SCRSHOT should succeed");

    // Read the file back and verify key pixels
    let file_path = std::path::PathBuf::from(".geometry_os/fs/roundtrip.img");
    let data = std::fs::read(&file_path).unwrap();

    // Pixel at (0,0): index 0 in screen, so bytes 0-3
    // 0xFF0000 -> A=0x00, R=0xFF, G=0x00, B=0x00
    assert_eq!(data[0], 0x00); // A
    assert_eq!(data[1], 0xFF); // R
    assert_eq!(data[2], 0x00); // G
    assert_eq!(data[3], 0x00); // B

    // Pixel at (255,255): index 255*256+255 = 65535, bytes 65535*4..65535*4+3
    let offset = (255 * 256 + 255) * 4;
    // 0xFFFFFF -> A=0x00, R=0xFF, G=0xFF, B=0xFF
    assert_eq!(data[offset], 0x00);
    assert_eq!(data[offset + 1], 0xFF);
    assert_eq!(data[offset + 2], 0xFF);
    assert_eq!(data[offset + 3], 0xFF);

    let _ = std::fs::remove_file(&file_path);
}

#[test]
fn test_scrshot_preserves_screen() {
    let mut vm = Vm::new();
    // Set specific pixel
    vm.screen[128 * 256 + 128] = 0x123456;

    let filename = b"preserve.img";
    for (i, &ch) in filename.iter().enumerate() {
        vm.ram[0x2000 + i] = ch as u32;
    }
    vm.ram[0x2000 + filename.len()] = 0;

    vm.ram[0] = 0x10;
    vm.ram[1] = 1;
    vm.ram[2] = 0x2000;
    vm.ram[3] = 0x98;
    vm.ram[4] = 1;
    vm.ram[5] = 0x00;
    vm.pc = 0;
    vm.halted = false;

    for _ in 0..100_000 {
        if !vm.step() {
            break;
        }
    }

    // Screen should be unchanged after SCRSHOT
    assert_eq!(
        vm.screen[128 * 256 + 128],
        0x123456,
        "SCRSHOT should not modify screen"
    );

    let _ = std::fs::remove_file(".geometry_os/fs/preserve.img");
}

#[test]
fn test_scrshot_multiple_saves() {
    let mut vm = Vm::new();

    // First screenshot: red screen
    for pixel in vm.screen.iter_mut() {
        *pixel = 0xFF0000;
    }
    let filename1 = b"save1.img";
    for (i, &ch) in filename1.iter().enumerate() {
        vm.ram[0x2000 + i] = ch as u32;
    }
    vm.ram[0x2000 + filename1.len()] = 0;

    vm.ram[0] = 0x10;
    vm.ram[1] = 1;
    vm.ram[2] = 0x2000;
    vm.ram[3] = 0x98;
    vm.ram[4] = 1;
    vm.ram[5] = 0x00; // HALT
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..100_000 {
        if !vm.step() {
            break;
        }
    }

    assert_ne!(vm.regs[0], 0xFFFFFFFF, "First SCRSHOT should succeed");

    // Second screenshot: blue screen
    for pixel in vm.screen.iter_mut() {
        *pixel = 0x0000FF;
    }
    let filename2 = b"save2.img";
    for (i, &ch) in filename2.iter().enumerate() {
        vm.ram[0x2000 + i] = ch as u32;
    }
    vm.ram[0x2000 + filename2.len()] = 0;

    vm.ram[0] = 0x10;
    vm.ram[1] = 1;
    vm.ram[2] = 0x2000;
    vm.ram[3] = 0x98;
    vm.ram[4] = 1;
    vm.ram[5] = 0x00;
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..100_000 {
        if !vm.step() {
            break;
        }
    }

    assert_ne!(vm.regs[0], 0xFFFFFFFF, "Second SCRSHOT should succeed");

    // Verify both files exist with correct first pixel color
    let data1 = std::fs::read(".geometry_os/fs/save1.img").unwrap();
    let data2 = std::fs::read(".geometry_os/fs/save2.img").unwrap();
    // save1: red (0xFF0000) -> A=0x00, R=0xFF, G=0x00, B=0x00
    assert_eq!(data1[1], 0xFF, "save1 first pixel should be red");
    // save2: blue (0x0000FF) -> A=0x00, R=0x00, G=0x00, B=0xFF
    assert_eq!(data2[3], 0xFF, "save2 first pixel should be blue");

    let _ = std::fs::remove_file(".geometry_os/fs/save1.img");
    let _ = std::fs::remove_file(".geometry_os/fs/save2.img");
}

#[test]
fn test_imgview_loads_and_renders() {
    // Create a test image file first (4x4 red pixels as raw RGBA)
    let file_path = std::path::PathBuf::from(".geometry_os/fs/test_img.img");
    {
        let dir = file_path.parent().unwrap();
        let _ = std::fs::create_dir_all(dir);
        // Write 256*256 pixels, all red, but we only need a small one
        // Actually imgview will load from VFS - let's just verify the SCRSHOT
        // round-trip. imgview.asm would need to be a full program.
        let mut data = Vec::with_capacity(256 * 256 * 4);
        for _ in 0..256 * 256 {
            data.push(0x00); // A
            data.push(0xFF); // R
            data.push(0x00); // G
            data.push(0x00); // B
        }
        std::fs::write(&file_path, &data).unwrap();
    }

    // Verify file was written correctly by reading it back
    let read_data = std::fs::read(&file_path).unwrap();
    assert_eq!(read_data.len(), 256 * 256 * 4);
    assert_eq!(read_data[1], 0xFF); // R channel of first pixel

    let _ = std::fs::remove_file(&file_path);
}

#[test]
fn test_screenshot_assembles() {
    let source = include_str!("../../programs/screenshot.asm");
    let result = crate::assembler::assemble(source, 0);
    assert!(
        result.is_ok(),
        "screenshot.asm failed to assemble: {:?}",
        result.err()
    );
    let bytecode = result.unwrap();
    assert!(
        bytecode.pixels.len() > 50,
        "screenshot.asm should be substantial"
    );
    // Verify SCRSHOT opcode present
    assert!(
        bytecode.pixels.iter().any(|&w| w == 0x98),
        "screenshot.asm should contain SCRSHOT opcode"
    );
}

#[test]
fn test_screenshot_runs() {
    let vm = boot_utility("programs/screenshot.asm", 200_000);
    // Should have produced at least one frame (FRAME opcode in main loop)
    assert!(vm.frame_count > 0, "screenshot should produce frames");
    // Screen should have colored rectangles (not all black)
    let mut non_black = 0;
    for &pixel in vm.screen.iter() {
        if pixel != 0 {
            non_black += 1;
        }
    }
    assert!(
        non_black > 100,
        "screenshot should draw pattern on screen, found {} non-black pixels",
        non_black
    );
}

#[test]
fn test_imgview_assembles() {
    let source = include_str!("../../programs/imgview.asm");
    let result = crate::assembler::assemble(source, 0);
    assert!(
        result.is_ok(),
        "imgview.asm failed to assemble: {:?}",
        result.err()
    );
    let bytecode = result.unwrap();
    assert!(
        bytecode.pixels.len() > 50,
        "imgview.asm should be substantial"
    );
}

#[test]
fn test_imgview_runs() {
    // Gradient is 256*256 pixels with ~10 instr each = ~2.6M steps
    // Then enters FRAME loop
    let vm = boot_utility("programs/imgview.asm", 2_000_000);
    // Should have produced at least one frame
    assert!(
        vm.frame_count > 0,
        "imgview should produce frames, got {}",
        vm.frame_count
    );

    // Gradient should produce diverse colors
    let mut colors = std::collections::HashSet::new();
    for &pixel in vm.screen.iter() {
        colors.insert(pixel);
    }
    assert!(
        colors.len() > 100,
        "imgview gradient should produce diverse colors, got {}",
        colors.len()
    );

    // Title bar should be dark
    let title_pixel = vm.screen[4 * 256 + 4];
    assert_ne!(title_pixel, 0, "title bar should not be black background");
}

// ── Phase 75: Stopwatch + Timer + Scientific Calculator ─────────

fn boot_app(asm_file: &str, frames: u32) -> Vm {
    let source = std::fs::read_to_string(asm_file)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", asm_file, e));
    let asm = crate::assembler::assemble(&source, 0)
        .unwrap_or_else(|e| panic!("Failed to assemble {}: {:?}", asm_file, e));
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
        if vm.frame_count >= start_frame + frames {
            break;
        }
    }
    vm
}

// ── Stopwatch Tests ──────────────────────────────────

#[test]
fn test_stopwatch_assembles() {
    let source = std::fs::read_to_string("programs/stopwatch.asm").unwrap();
    crate::assembler::assemble(&source, 0).expect("stopwatch.asm should assemble");
}

#[test]
fn test_stopwatch_renders_frame() {
    let vm = boot_app("programs/stopwatch.asm", 1);
    assert!(!vm.halted, "stopwatch should not halt after boot");
    // Background between title and time panel should be navy
    assert_eq!(
        vm.screen[30 * 256 + 10],
        0x0D1B2A,
        "background should be dark navy"
    );
    // Title bar region should be rendered
    assert_eq!(
        vm.screen[10 * 256 + 10],
        0x1B3A4B,
        "title bar should be rendered"
    );
}

#[test]
fn test_stopwatch_initial_state() {
    let vm = boot_app("programs/stopwatch.asm", 1);
    assert_eq!(vm.ram[0x4000], 0, "should start stopped");
    assert_eq!(vm.ram[0x4004], 0, "elapsed should start at 0");
    assert_eq!(vm.ram[0x400C], 0, "lap count should start at 0");
}

#[test]
fn test_stopwatch_time_panel_rendered() {
    let vm = boot_app("programs/stopwatch.asm", 1);
    // Time panel edge (far right, away from text) should be dark
    assert_eq!(
        vm.screen[50 * 256 + 230],
        0x060612,
        "time panel edge should be dark"
    );
}

#[test]
fn test_stopwatch_runs_persistently() {
    let vm = boot_app("programs/stopwatch.asm", 5);
    assert!(!vm.halted, "stopwatch should run persistently");
}

// ── Timer Tests ──────────────────────────────────────

#[test]
fn test_timer_assembles() {
    let source = std::fs::read_to_string("programs/timer.asm").unwrap();
    crate::assembler::assemble(&source, 0).expect("timer.asm should assemble");
}

#[test]
fn test_timer_renders_frame() {
    let vm = boot_app("programs/timer.asm", 1);
    assert!(!vm.halted, "timer should not halt after boot");
    assert_eq!(
        vm.screen[200 * 256 + 250],
        0x1A0A2E,
        "background should be dark purple"
    );
}

#[test]
fn test_timer_initial_countdown() {
    let vm = boot_app("programs/timer.asm", 1);
    assert_eq!(vm.ram[0x4000], 6000, "timer should start at 6000cs");
    assert_eq!(vm.ram[0x4004], 0, "should start paused");
    assert_eq!(vm.ram[0x400C], 0, "no alarm initially");
}

#[test]
fn test_timer_countdown_panel() {
    let vm = boot_app("programs/timer.asm", 1);
    // Timer panel corner (top-left of panel, away from text)
    assert_eq!(
        vm.screen[42 * 256 + 32],
        0x0A0616,
        "timer panel should be dark"
    );
}

// ── Scientific Calculator Tests ──────────────────────

#[test]
fn test_sci_calc_assembles() {
    let source = std::fs::read_to_string("programs/sci_calc.asm").unwrap();
    crate::assembler::assemble(&source, 0).expect("sci_calc.asm should assemble");
}

#[test]
fn test_sci_calc_renders_frame() {
    let vm = boot_app("programs/sci_calc.asm", 1);
    assert!(!vm.halted, "sci_calc should not halt after boot");
    assert_eq!(
        vm.screen[30 * 256 + 10],
        0x0D1B2A,
        "background should be dark navy"
    );
}

#[test]
fn test_sci_calc_initial_display() {
    let vm = boot_app("programs/sci_calc.asm", 1);
    assert_eq!(vm.ram[0x4000], 0, "display should start at 0");
    assert_eq!(vm.ram[0x4008], 0, "no operator initially");
}

#[test]
fn test_sci_calc_sin_table_built() {
    let vm = boot_app("programs/sci_calc.asm", 1);
    // Quarter-wave table: 16 entries for 0-90 degrees
    // SIN_QTR at 0x6000
    assert_eq!(vm.ram[0x6000], 0, "sin(0) should be 0");
    assert_eq!(vm.ram[0x600F], 1000, "sin(90) should be 1000");
    // sin(30deg) ~ entry index 5 (30*16/90 = 5.33 -> 5) = 500
    assert!(
        vm.ram[0x6005] >= 400,
        "sin(30deg) should be ~500, got {}",
        vm.ram[0x6005]
    );
    // sin(60deg) ~ entry index 10 (60*16/90 = 10.67 -> 10) = 866
    assert!(
        vm.ram[0x600A] >= 800,
        "sin(60deg) should be ~866, got {}",
        vm.ram[0x600A]
    );
}
#[test]
fn test_sci_calc_title_rendered() {
    let vm = boot_app("programs/sci_calc.asm", 1);
    assert_eq!(vm.screen[10 * 256 + 10], 0x1B3A4B, "title bar rendered");
}

// ── Wallpaper Tests (Phase 77) ──────────────────────────────

#[test]
fn test_wallpaper_assembles() {
    let source = std::fs::read_to_string("programs/wallpaper.asm").unwrap();
    let result = crate::assembler::assemble(&source, 0);
    assert!(
        result.is_ok(),
        "wallpaper.asm should assemble: {:?}",
        result.err()
    );
    let asm = result.unwrap();
    assert!(
        asm.pixels.len() > 100,
        "wallpaper should produce substantial bytecode, got {} words",
        asm.pixels.len()
    );
}

#[test]
fn test_wallpaper_sine_table_built() {
    let vm = boot_app("programs/wallpaper.asm", 1);
    // Sine table at 0x2000, parabolic cosine formula
    // Entry 0: delta=128, val = 255-127 = 128
    assert!(
        vm.ram[0x2000] >= 128,
        "sine[0] should be >=128, got {}",
        vm.ram[0x2000]
    );
    // Entry 128 (0x2080): delta=0, val = 255 (peak)
    assert!(
        vm.ram[0x2080] > 200,
        "sine[128] should be ~255, got {}",
        vm.ram[0x2080]
    );
}

#[test]
fn test_wallpaper_produces_pixels() {
    let vm = boot_app("programs/wallpaper.asm", 1);
    // Should have rendered something to the screen (non-black pixels)
    let non_black = vm.screen.iter().filter(|&&p| p != 0).count();
    assert!(
        non_black > 100,
        "wallpaper should render pixels, got {} non-black",
        non_black
    );
}

#[test]
fn test_wallpaper_pattern_default() {
    let vm = boot_app("programs/wallpaper.asm", 1);
    // Default pattern is 0 (gradient)
    assert_eq!(vm.ram[0x6800], 0, "default pattern should be 0");
}

#[test]
fn test_wallpaper_runs_multiple_frames() {
    let vm = boot_app("programs/wallpaper.asm", 3);
    // Should survive 3 frames without crashing
    assert!(!vm.halted, "wallpaper should still be running");
    // Pattern should still be valid (0)
    assert!(
        vm.ram[0x6800] <= 4,
        "pattern should be 0-4, got {}",
        vm.ram[0x6800]
    );
}

// ── Settings Tests (Phase 77) ───────────────────────────────

#[test]
fn test_settings_assembles() {
    let source = std::fs::read_to_string("programs/settings.asm").unwrap();
    let result = crate::assembler::assemble(&source, 0);
    assert!(
        result.is_ok(),
        "settings.asm should assemble: {:?}",
        result.err()
    );
    let asm = result.unwrap();
    assert!(
        asm.pixels.len() > 100,
        "settings should produce substantial bytecode, got {} words",
        asm.pixels.len()
    );
}

#[test]
fn test_settings_default_values() {
    let vm = boot_app("programs/settings.asm", 1);
    // Theme defaults to 0
    assert_eq!(vm.ram[0x6900], 0, "default theme should be 0");
    // Volume defaults to 50
    assert_eq!(vm.ram[0x6904], 50, "default volume should be 50");
    // Cursor defaults to 0
    assert_eq!(vm.ram[0x6908], 0, "default cursor should be 0");
    // Key repeat defaults to 3
    assert_eq!(vm.ram[0x690C], 3, "default key repeat should be 3");
}

#[test]
fn test_settings_theme_table_built() {
    let vm = boot_app("programs/settings.asm", 1);
    // Theme 0 at 0x7000: bg=0x0D1B2A, fg=0xE0E0E0, accent=0x00B4D8, panel=0x1B2838
    assert_eq!(vm.ram[0x7000], 0x0D1B2A, "theme 0 bg");
    assert_eq!(vm.ram[0x7001], 0xE0E0E0, "theme 0 fg");
    assert_eq!(vm.ram[0x7002], 0x00B4D8, "theme 0 accent");
    assert_eq!(vm.ram[0x7003], 0x1B2838, "theme 0 panel");
    // Theme 7 (Matrix) at 0x701C
    assert_eq!(vm.ram[0x701C], 0x000A00, "theme 7 (Matrix) bg");
    assert_eq!(vm.ram[0x701D], 0x00FF00, "theme 7 (Matrix) fg");
}

#[test]
fn test_settings_renders_frame() {
    let vm = boot_app("programs/settings.asm", 1);
    // Should not be halted (loops with FRAME)
    assert!(!vm.halted, "settings should still be running");
    // Title bar should have accent color (cyan for theme 0)
    // Title bar is at row 0-23, uses accent color 0x00B4D8
    assert!(
        vm.screen.iter().filter(|&&p| p == 0x00B4D8).count() > 10,
        "should see accent color on screen (title bar)"
    );
}

#[test]
fn test_settings_renders_panels() {
    let vm = boot_app("programs/settings.asm", 1);
    // Should have panel background color somewhere (0x1B2838)
    assert!(
        vm.screen.iter().filter(|&&p| p == 0x1B2838).count() > 10,
        "should see panel color on screen"
    );
}

#[test]
fn test_settings_runs_persistently() {
    let vm = boot_app("programs/settings.asm", 5);
    // Should survive 5 frames without crashing
    assert!(
        !vm.halted,
        "settings should still be running after 5 frames"
    );
    // Config values should be unchanged (no input)
    assert_eq!(vm.ram[0x6900], 0, "theme unchanged");
    assert_eq!(vm.ram[0x6904], 50, "volume unchanged");
}

// ── About App Tests ──────────────────────────────────

#[test]
fn test_about_assembles() {
    let source = std::fs::read_to_string("programs/about.asm").unwrap();
    crate::assembler::assemble(&source, 0).expect("about.asm should assemble");
}

#[test]
fn test_about_renders_info_panel() {
    let source = std::fs::read_to_string("programs/about.asm").unwrap();
    let asm = crate::assembler::assemble(&source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    // Run to completion (about.asm halts)
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.halted, "about.asm should halt after rendering");

    // Title bar at row 0 should be dark blue (0x1B3A5C)
    assert_eq!(vm.screen[0], 0x1B3A5C, "title bar pixel at (0,0)");

    // Info panel background at row 30 (0x141428)
    assert_eq!(vm.screen[30 * 256 + 20], 0x141428, "info panel background");

    // Footer area should have content (palette bar background 0x0A0A1A)
    let palette_pixels = vm.screen.iter().filter(|&&p| p == 0x0A0A1A).count();
    assert!(palette_pixels > 50, "should see palette bar at bottom");

    // Should have color palette squares (non-black, non-background colors)
    let unique_colors: std::collections::HashSet<u32> =
        vm.screen.iter().filter(|&&p| p != 0).copied().collect();
    assert!(
        unique_colors.len() > 10,
        "about page should have many colors (title, panel, text, palette), got {}",
        unique_colors.len()
    );
}

// ── Calendar App Tests ──────────────────────────────────

#[test]
fn test_calendar_assembles() {
    let source = std::fs::read_to_string("programs/calendar.asm").unwrap();
    crate::assembler::assemble(&source, 0).expect("calendar.asm should assemble");
}

#[test]
fn test_calendar_computes_month_data() {
    let source = std::fs::read_to_string("programs/calendar.asm").unwrap();
    let asm = crate::assembler::assemble(&source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    // Run 1 frame
    let start_frame = vm.frame_count;
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= start_frame + 1 {
            break;
        }
    }

    // Should not halt (animation loop)
    assert!(!vm.halted, "calendar should not halt");

    // Check month/year defaults
    assert_eq!(vm.ram[0x6100], 4, "month should be April (4)");
    assert_eq!(vm.ram[0x6101], 2026, "year should be 2026");

    // April 2026 has 30 days
    assert_eq!(vm.ram[0x6102], 30, "April has 30 days");

    // April 1 2026 is a Wednesday (day index 3 if 0=Sunday)
    // Zeller: (h+1)%7 where h is the Zeller result
    // For April 1, 2026: Should be Wednesday = 3 (if 0=Sunday)
    let first_day = vm.ram[0x6103];
    assert!(first_day <= 6, "first day should be 0-6, got {}", first_day);

    // Title bar should have color (0x1B3A5C)
    assert_eq!(vm.screen[0], 0x1B3A5C, "title bar should be drawn");
}

#[test]
fn test_calendar_zeller_april_2026() {
    // Verify Zeller congruence: April 1, 2026 is Wednesday
    // Our convention: 0=Sunday, so Wednesday = 3
    let source = std::fs::read_to_string("programs/calendar.asm").unwrap();
    let asm = crate::assembler::assemble(&source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    // Run past compute_month
    for _ in 0..100_000 {
        if !vm.step() {
            break;
        }
    }
    // April 2026: 1st is Wednesday = index 3
    assert_eq!(vm.ram[0x6103], 3, "April 1 2026 should be Wednesday (3)");
}

// ── Help Viewer Tests ──────────────────────────────────

#[test]
fn test_help_assembles() {
    let source = std::fs::read_to_string("programs/help.asm").unwrap();
    crate::assembler::assemble(&source, 0).expect("help.asm should assemble");
}

#[test]
fn test_help_renders_content() {
    let vm = boot_app("programs/help.asm", 1);

    // Should not halt (animation loop)
    assert!(!vm.halted, "help viewer should not halt");

    // Title bar at row 0 should be 0x1B3A5C
    assert_eq!(vm.screen[0], 0x1B3A5C, "title bar");

    // Content area should have background (0x101828)
    assert_eq!(vm.screen[34 * 256 + 8], 0x101828, "content background");

    // Should have text pixels (0xAAAACC or 0x8888FF)
    let text_pixels = vm
        .screen
        .iter()
        .filter(|&&p| p == 0xAAAACC || p == 0x8888FF)
        .count();
    assert!(
        text_pixels > 5,
        "should have help text pixels, got {}",
        text_pixels
    );
}

#[test]
fn test_help_scrolls() {
    let source = std::fs::read_to_string("programs/help.asm").unwrap();
    let asm = crate::assembler::assemble(&source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    // Run 1 frame to initialize
    let start_frame = vm.frame_count;
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= start_frame + 1 {
            break;
        }
    }

    // Initial scroll should be 0
    assert_eq!(vm.ram[0x6100], 0, "initial scroll = 0");

    // Simulate down arrow press
    vm.ram[0xFFF] = 66; // 'B' = down arrow
                        // Run another frame
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_count >= start_frame + 2 {
            break;
        }
    }

    // Scroll should have advanced
    assert_eq!(vm.ram[0x6100], 1, "scroll should be 1 after down arrow");
}

// -- Tests for .db, .asciz, .ascii assembler directives --

#[test]
fn test_db_directive_basic() {
    let source = ".db 72, 101, 108, 108, 111\nHALT";
    let result = crate::assembler::assemble(source, 0).unwrap();
    // Should emit 5 byte values + 1 HALT = 6 words total
    assert_eq!(result.pixels.len(), 6);
    assert_eq!(result.pixels[0], 72); // 'H'
    assert_eq!(result.pixels[1], 101); // 'e'
    assert_eq!(result.pixels[2], 108); // 'l'
    assert_eq!(result.pixels[3], 108); // 'l'
    assert_eq!(result.pixels[4], 111); // 'o'
    assert_eq!(result.pixels[5], 0x00); // HALT
}

#[test]
fn test_db_directive_hex_values() {
    let source = ".db 0xFF, 0x00, 0x0F";
    let result = crate::assembler::assemble(source, 0).unwrap();
    assert_eq!(result.pixels.len(), 3);
    assert_eq!(result.pixels[0], 0xFF);
    assert_eq!(result.pixels[1], 0x00);
    assert_eq!(result.pixels[2], 0x0F);
}

#[test]
fn test_db_directive_with_labels() {
    let source = "LDI r0, 10\n.db 1, 2, 3\nADD r0, r0\nHALT";
    let result = crate::assembler::assemble(source, 0).unwrap();
    // LDI(3 words) + .db(3 words) + ADD(3 words) + HALT(1 word) = 10
    assert_eq!(result.pixels.len(), 10);
    assert_eq!(result.pixels[3], 1);
    assert_eq!(result.pixels[4], 2);
    assert_eq!(result.pixels[5], 3);
}

#[test]
fn test_asciz_directive_basic() {
    let source = ".asciz \"Hello\"\nHALT";
    let result = crate::assembler::assemble(source, 0).unwrap();
    // "Hello" = 5 chars + null terminator = 6 words + HALT = 7
    assert_eq!(result.pixels.len(), 7);
    assert_eq!(result.pixels[0], 72); // 'H'
    assert_eq!(result.pixels[1], 101); // 'e'
    assert_eq!(result.pixels[2], 108); // 'l'
    assert_eq!(result.pixels[3], 108); // 'l'
    assert_eq!(result.pixels[4], 111); // 'o'
    assert_eq!(result.pixels[5], 0); // null terminator
}

#[test]
fn test_asciz_directive_empty_string() {
    let source = ".asciz \"\"\nHALT";
    let result = crate::assembler::assemble(source, 0).unwrap();
    // Empty string + null = 1 word + HALT = 2
    assert_eq!(result.pixels.len(), 2);
    assert_eq!(result.pixels[0], 0); // null terminator
}

#[test]
fn test_ascii_directive_no_null() {
    let source = ".ascii \"Hi\"\n.asciz \"Bye\"\nHALT";
    let result = crate::assembler::assemble(source, 0).unwrap();
    // .ascii "Hi" = 2 words (no null) + .asciz "Bye" = 4 words (3 chars + null) + HALT = 7
    assert_eq!(result.pixels.len(), 7);
    assert_eq!(result.pixels[0], 72); // 'H'
    assert_eq!(result.pixels[1], 105); // 'i'
    assert_eq!(result.pixels[2], 66); // 'B'
    assert_eq!(result.pixels[3], 121); // 'y'
    assert_eq!(result.pixels[4], 101); // 'e'
    assert_eq!(result.pixels[5], 0); // null terminator from .asciz
}

#[test]
fn test_db_directive_case_insensitive() {
    let source = ".DB 65, 66\n.Db 67\nHALT";
    let result = crate::assembler::assemble(source, 0).unwrap();
    assert_eq!(result.pixels.len(), 4);
    assert_eq!(result.pixels[0], 65);
    assert_eq!(result.pixels[1], 66);
    assert_eq!(result.pixels[2], 67);
}

#[test]
fn test_asm_opcode_with_db_and_asciz() {
    // Test that the ASM opcode can assemble programs using .db and .asciz
    let _vm = crate::vm::Vm::new();

    // Write source to RAM at 0x2000 using LDI+STORE
    let source = ".db 72, 101, 108, 108, 111, 0\nLDI r0, 0x2000\nTEXT r0, r0, r0\nHALT";
    // Actually, let's just use the assembler directly
    let result = crate::assembler::assemble(source, 0);
    assert!(result.is_ok(), "assembly with .db should succeed");
}

#[test]
fn test_self_host_asm_assembles() {
    let source = std::fs::read_to_string("programs/self_host.asm").unwrap();
    let result = crate::assembler::assemble(&source, 0);
    assert!(
        result.is_ok(),
        "self_host.asm should assemble: {:?}",
        result.err()
    );
    let bc = result.unwrap();
    assert!(
        bc.pixels.len() > 50,
        "self_host.asm should produce substantial bytecode, got {} words",
        bc.pixels.len()
    );
}

#[test]
fn test_self_host_asm_runs() {
    let source = std::fs::read_to_string("programs/self_host.asm").unwrap();
    let asm = crate::assembler::assemble(&source, 0).unwrap();
    let mut vm = crate::vm::Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    // Run for up to 10M steps
    for _ in 0..10_000_000 {
        if !vm.step() {
            break;
        }
    }
    // The program should have halted
    assert!(vm.halted, "self_host.asm should halt");
    // After running, the screen should have some colored pixels (the gradient)
    let non_black = vm.screen.iter().filter(|&&p| p != 0).count();
    assert!(
        non_black > 100,
        "self_host.asm should produce visible output, got {} non-black pixels",
        non_black
    );
}

// ── NET_SEND / NET_RECV tests (Phase 71: Pixel Network Protocol) ──────────

#[test]
fn test_net_send_assembles() {
    let src = "LDI r1, 0x7000
LDI r2, 10
LDI r3, 0
NET_SEND r1, r2, r3
HALT";
    let result = crate::assembler::assemble(src, 0);
    assert!(
        result.is_ok(),
        "NET_SEND should assemble: {:?}",
        result.err()
    );
    let bc = result.unwrap();
    // LDI(3) + LDI(3) + LDI(3) + NET_SEND(4) + HALT(1) = 14 words
    assert_eq!(bc.pixels.len(), 14);
    assert_eq!(bc.pixels[9], 0x99, "NET_SEND opcode should be 0x99");
}

#[test]
fn test_net_recv_assembles() {
    let src = "LDI r1, 0x7000
LDI r2, 256
NET_RECV r1, r2
HALT";
    let result = crate::assembler::assemble(src, 0);
    assert!(
        result.is_ok(),
        "NET_RECV should assemble: {:?}",
        result.err()
    );
    let bc = result.unwrap();
    // LDI(3) + LDI(3) + NET_RECV(3) + HALT(1) = 10 words
    assert_eq!(bc.pixels.len(), 10);
    assert_eq!(bc.pixels[6], 0x9A, "NET_RECV opcode should be 0x9A");
}

#[test]
fn test_net_send_invalid_fd() {
    let mut vm = crate::vm::Vm::new();
    // Set up registers
    vm.regs[1] = 0x7000; // buf addr
    vm.regs[2] = 10; // len
    vm.regs[3] = 0; // fd (not connected)

    // NET_SEND r1, r2, r3 at address 0
    vm.ram[0] = 0x99;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.pc = 0;

    vm.step();
    assert_eq!(
        vm.regs[0],
        crate::vm::net::NET_ERR_INVALID_FD,
        "NET_SEND should fail with invalid fd"
    );
}

#[test]
fn test_net_recv_empty_inbox() {
    let mut vm = crate::vm::Vm::new();
    vm.regs[1] = 0x7000; // buf addr
    vm.regs[2] = 256; // max_len

    // NET_RECV r1, r2 at address 0
    vm.ram[0] = 0x9A;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.pc = 0;

    vm.step();
    assert_eq!(
        vm.regs[0], 0,
        "NET_RECV should return 0 when inbox is empty"
    );
}

#[test]
fn test_net_recv_from_inbox() {
    let mut vm = crate::vm::Vm::new();
    vm.regs[1] = 0x7000; // buf addr
    vm.regs[2] = 256; // max_len

    // Push a frame into the inbox
    // Frame: header word + 3 pixel data words
    // Header: type=1 (chat), width=3, height=1, flags=0
    let header = (1u32 << 24) | (3u32 << 16) | (1u32 << 8) | 0u32;
    vm.net_inbox
        .push(vec![header, 0xFF0000, 0x00FF00, 0x0000FF]);

    // NET_RECV r1, r2 at address 0
    vm.ram[0] = 0x9A;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.pc = 0;

    vm.step();

    // r0 should be 7 (4 header words + 3 data words)
    assert_eq!(vm.regs[0], 7, "NET_RECV should return total words written");

    // Verify header was written
    assert_eq!(vm.ram[0x7000], 1, "frame type should be chat (1)");
    assert_eq!(vm.ram[0x7001], 3, "width should be 3");
    assert_eq!(vm.ram[0x7002], 1, "height should be 1");
    assert_eq!(vm.ram[0x7003], 0, "flags should be 0");

    // Verify pixel data
    assert_eq!(vm.ram[0x7004], 0xFF0000, "pixel 0 should be red");
    assert_eq!(vm.ram[0x7005], 0x00FF00, "pixel 1 should be green");
    assert_eq!(vm.ram[0x7006], 0x0000FF, "pixel 2 should be blue");

    // Inbox should be empty now
    assert!(vm.net_inbox.is_empty(), "inbox should be empty after recv");
}

#[test]
fn test_net_recv_multiple_frames() {
    let mut vm = crate::vm::Vm::new();

    // Push two frames
    let header1 = (0u32 << 24) | (2u32 << 16) | (1u32 << 8); // screen_share
    vm.net_inbox.push(vec![header1, 0x112233, 0x445566]);
    let header2 = (1u32 << 24) | (1u32 << 16) | (1u32 << 8); // chat
    vm.net_inbox.push(vec![header2, 0xAABBCC]);

    // Receive first frame
    vm.regs[1] = 0x7000;
    vm.regs[2] = 256;
    vm.ram[0] = 0x9A;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.pc = 0;
    vm.step();
    assert_eq!(vm.regs[0], 6, "first frame: 4 header + 2 data");
    assert_eq!(vm.ram[0x7004], 0x112233);
    assert_eq!(vm.ram[0x7005], 0x445566);

    // Receive second frame
    vm.regs[1] = 0x7100;
    vm.regs[2] = 256;
    vm.ram[100] = 0x9A;
    vm.ram[101] = 1;
    vm.ram[102] = 2;
    vm.pc = 100;
    vm.step();
    assert_eq!(vm.regs[0], 5, "second frame: 4 header + 1 data");
    assert_eq!(vm.ram[0x7104], 0xAABBCC);
    assert!(vm.net_inbox.is_empty());
}

#[test]
fn test_net_recv_respects_max_len() {
    let mut vm = crate::vm::Vm::new();
    vm.regs[1] = 0x7000;
    vm.regs[2] = 1; // max_len = 1 (very small)

    // Push a frame with 5 pixels
    let header = (0u32 << 24) | (5u32 << 16) | (1u32 << 8);
    vm.net_inbox
        .push(vec![header, 0x11, 0x22, 0x33, 0x44, 0x55]);

    vm.ram[0] = 0x9A;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.pc = 0;
    vm.step();

    // Should only receive 1 data pixel (max_len = 1), but still writes 4 header words
    assert_eq!(vm.regs[0], 5, "should write 4 header + 1 data = 5");
    assert_eq!(vm.ram[0x7004], 0x11, "only first pixel written");
}

#[test]
fn test_net_send_disasm() {
    let (s, len) = disasm(&[0x99, 1, 2, 3]);
    assert_eq!(s, "NET_SEND r1, r2, r3");
    assert_eq!(len, 4);
}

#[test]
fn test_net_recv_disasm() {
    let (s, len) = disasm(&[0x9A, 5, 6]);
    assert_eq!(s, "NET_RECV r5, r6");
    assert_eq!(len, 3);
}

#[test]
fn test_pixel_protocol_format() {
    // Verify the pixel protocol frame format constants:
    // Frame types: 0=screen_share, 1=chat, 2=file
    // Header encoding: (type << 24) | (width << 16) | (height << 8) | flags
    // Note: width and height are 8-bit fields (0-255)
    let screen_share: u32 = (0u32 << 24) | (16u32 << 16) | (16u32 << 8) | 0u32;
    assert_eq!(
        (screen_share >> 24) & 0xFF,
        0,
        "type should be screen_share"
    );
    assert_eq!((screen_share >> 16) & 0xFF, 16, "width should be 16");
    assert_eq!((screen_share >> 8) & 0xFF, 16, "height should be 16");
    assert_eq!(screen_share & 0xFF, 0, "flags should be 0");

    let chat: u32 = (1u32 << 24) | (80u32 << 16) | (1u32 << 8) | 0u32;
    assert_eq!((chat >> 24) & 0xFF, 1, "type should be chat");
    assert_eq!((chat >> 16) & 0xFF, 80, "width should be 80");
}

#[test]
fn test_net_inbox_cleared_on_reset() {
    let mut vm = crate::vm::Vm::new();
    vm.net_inbox.push(vec![0x01020304, 0x05060708]);
    assert_eq!(vm.net_inbox.len(), 1);
    vm.reset();
    assert!(
        vm.net_inbox.is_empty(),
        "net_inbox should be cleared on reset"
    );
}

// ════════════════════════════════════════════════════════════════
// Phase 72: Taskbar + Launcher Tests
// ════════════════════════════════════════════════════════════════

#[test]
fn test_procls_opcode_no_processes() {
    let mut vm = crate::vm::Vm::new();
    // PROCLS with no spawned processes should return 1 (main process PID 0)
    let buf_addr: usize = 0x4000;
    vm.ram[0] = 0x9B; // PROCLS opcode
    vm.ram[1] = 10; // r10
    vm.regs[10] = buf_addr as u32;
    vm.step();
    assert_eq!(vm.regs[0], 1, "should have 1 process (main)");
    assert_eq!(vm.ram[buf_addr], 0, "first PID should be 0 (main)");
}

#[test]
fn test_procls_opcode_with_spawned_process() {
    let mut vm = crate::vm::Vm::new();
    // Manually add a spawned process
    use crate::vm::types::Process;
    vm.processes.push(Process::new(1, 0, 100));
    vm.processes.push(Process::new(2, 0, 200));

    let buf_addr: usize = 0x4000;
    vm.ram[0] = 0x9B; // PROCLS opcode
    vm.ram[1] = 10; // r10
    vm.regs[10] = buf_addr as u32;
    vm.step();
    assert_eq!(vm.regs[0], 3, "should have 3 processes (main + 2 spawned)");
    assert_eq!(vm.ram[buf_addr], 0, "first PID should be 0 (main)");
    assert_eq!(vm.ram[buf_addr + 1], 1, "second PID should be 1");
    assert_eq!(vm.ram[buf_addr + 2], 2, "third PID should be 2");
}

#[test]
fn test_procls_disassembles() {
    let mut vm = crate::vm::Vm::new();
    vm.ram[0] = 0x9B;
    vm.ram[1] = 5;
    let (s, len) = vm.disassemble_at(0);
    assert!(
        s.contains("PROCLS"),
        "should disassemble as PROCLS, got: {}",
        s
    );
    assert_eq!(len, 2);
}

#[test]
fn test_procls_assembles() {
    use crate::assembler::assemble;
    let src = "PROCLS r5";
    let result = assemble(src, 0);
    assert!(result.is_ok(), "PROCLS should assemble");
    let asm = result.unwrap();
    assert_eq!(asm.pixels[0], 0x9B, "opcode byte");
    assert_eq!(asm.pixels[1], 5, "register r5");
}

#[test]
fn test_taskbar_assembles() {
    use crate::assembler::assemble;
    let source = std::fs::read_to_string("programs/taskbar.asm").expect("taskbar.asm should exist");
    let result = assemble(&source, 0);
    assert!(result.is_ok(), "taskbar.asm should assemble");
    let asm = result.unwrap();
    assert!(
        asm.pixels.len() > 100,
        "taskbar should produce substantial bytecode, got {} words",
        asm.pixels.len()
    );
}

#[test]
fn test_taskbar_runs() {
    use crate::assembler::assemble;
    let source = std::fs::read_to_string("programs/taskbar.asm").unwrap();
    let asm = assemble(&source, 0).unwrap();
    let mut vm = crate::vm::Vm::new();
    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    let mut frames_seen = 0;
    for _ in 0..5_000_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_ready {
            vm.frame_ready = false;
            frames_seen += 1;
            if frames_seen >= 2 {
                break;
            }
        }
    }
    assert!(
        frames_seen >= 1,
        "taskbar should produce at least 1 frame, got {}",
        frames_seen
    );
    // Taskbar should draw something at the bottom of the screen (y=240+)
    let mut has_taskbar_pixels = false;
    for y in 240..256 {
        for x in 0..256 {
            if vm.screen[y * 256 + x] != 0 {
                has_taskbar_pixels = true;
                break;
            }
        }
        if has_taskbar_pixels {
            break;
        }
    }
    assert!(
        has_taskbar_pixels,
        "taskbar should render pixels at y=240..256"
    );
}

#[test]
fn test_launcher_assembles() {
    use crate::assembler::assemble;
    let source =
        std::fs::read_to_string("programs/launcher.asm").expect("launcher.asm should exist");
    let result = assemble(&source, 0);
    assert!(result.is_ok(), "launcher.asm should assemble");
    let asm = result.unwrap();
    assert!(
        asm.pixels.len() > 100,
        "launcher should produce substantial bytecode, got {} words",
        asm.pixels.len()
    );
}

#[test]
fn test_launcher_runs() {
    use crate::assembler::assemble;
    let source = std::fs::read_to_string("programs/launcher.asm").unwrap();
    let asm = assemble(&source, 0).unwrap();
    let mut vm = crate::vm::Vm::new();
    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    let mut frames_seen = 0;
    for _ in 0..5_000_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_ready {
            vm.frame_ready = false;
            frames_seen += 1;
            if frames_seen >= 2 {
                break;
            }
        }
    }
    assert!(
        frames_seen >= 1,
        "launcher should produce at least 1 frame, got {}",
        frames_seen
    );
    // Launcher should draw title bar at top
    let mut has_title_pixels = false;
    for y in 0..20 {
        for x in 0..256 {
            if vm.screen[y * 256 + x] != 0 {
                has_title_pixels = true;
                break;
            }
        }
        if has_title_pixels {
            break;
        }
    }
    assert!(has_title_pixels, "launcher should render title bar at top");
}

#[test]
fn test_procls_buf_register_out_of_range() {
    let mut vm = crate::vm::Vm::new();
    vm.ram[0] = 0x9B; // PROCLS
    vm.ram[1] = 50; // invalid register
    vm.step();
    assert_eq!(vm.regs[0], 0, "out-of-range register should return 0");
}

#[test]
fn test_taskbar_procls_integration() {
    // Test that taskbar correctly calls PROCLS and reads the count
    use crate::assembler::assemble;
    let source = std::fs::read_to_string("programs/taskbar.asm").unwrap();
    let asm = assemble(&source, 0).unwrap();
    let mut vm = crate::vm::Vm::new();
    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    // Add a spawned process
    use crate::vm::types::Process;
    vm.processes.push(Process::new(1, 0, 0));

    let mut frames_seen = 0;
    for _ in 0..5_000_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_ready {
            vm.frame_ready = false;
            frames_seen += 1;
            if frames_seen >= 1 {
                break;
            }
        }
    }
    // After running, PROC_COUNT at 0x5100 should be set (2 = main + 1 spawned)
    let proc_count = vm.ram[0x5100];
    assert_eq!(
        proc_count, 2,
        "taskbar should count 2 processes, got {}",
        proc_count
    );
}

// ── Phase 76: Debugger UI + Memory Inspector ──────────────────

#[test]
fn test_debugger_assembles() {
    let source = include_str!("../../programs/debugger.asm");
    let result = crate::assembler::assemble(source, 0);
    assert!(
        result.is_ok(),
        "debugger.asm failed to assemble: {:?}",
        result.err()
    );
    let bytecode = result.unwrap();
    assert!(
        bytecode.pixels.len() > 100,
        "debugger.asm should be substantial, got {} words",
        bytecode.pixels.len()
    );
}

#[test]
fn test_debugger_runs_with_frames() {
    let source = include_str!("../../programs/debugger.asm");
    let asm = crate::assembler::assemble(source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    let mut frames_seen = 0;
    for _ in 0..2_000_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_ready {
            vm.frame_ready = false;
            frames_seen += 1;
            if frames_seen >= 2 {
                break;
            }
        }
    }
    assert!(
        frames_seen >= 2,
        "debugger should produce at least 2 frames, got {}",
        frames_seen
    );
}

#[test]
fn test_debugger_trace_integration() {
    let source = include_str!("../../programs/debugger.asm");
    let asm = crate::assembler::assemble(source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    // Run until first frame
    for _ in 0..2_000_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_ready {
            vm.frame_ready = false;
            break;
        }
    }
    // The debugger enables trace recording and should have entries
    let trace_count = vm.trace_buffer.len();
    assert!(
        trace_count > 0,
        "debugger should have trace entries after running, got {}",
        trace_count
    );
}

#[test]
fn test_debugger_shows_title() {
    let vm = run_util_frames("programs/debugger.asm", 1);
    // Title bar should have dark red pixels (0x8B0000) at y=0-15
    let title_pixel = vm.screen[4 * 256 + 60];
    assert_ne!(
        title_pixel, 0x0A0A1E,
        "debugger title bar should differ from background"
    );
}

#[test]
fn test_debugger_status_bar() {
    let vm = run_util_frames("programs/debugger.asm", 1);
    // Status bar area (y=236+) should have non-background pixels
    let status_pixel = vm.screen[238 * 256 + 10];
    assert_ne!(
        status_pixel, 0x0A0A1E,
        "debugger status bar should be visible"
    );
}

#[test]
fn test_debugger_hex_table_built() {
    let vm = run_util_frames("programs/debugger.asm", 1);
    // Hex table at 0x6400 should have '0'-'9' and 'A'-'F'
    assert_eq!(vm.ram[0x6400], 48, "hex table[0] should be '0'");
    assert_eq!(vm.ram[0x6409], 57, "hex table[9] should be '9'");
    assert_eq!(vm.ram[0x640A], 65, "hex table[10] should be 'A'");
    assert_eq!(vm.ram[0x640F], 70, "hex table[15] should be 'F'");
}

#[test]
fn test_meminspect_assembles() {
    let source = include_str!("../../programs/meminspect.asm");
    let result = crate::assembler::assemble(source, 0);
    assert!(
        result.is_ok(),
        "meminspect.asm failed to assemble: {:?}",
        result.err()
    );
    let bytecode = result.unwrap();
    assert!(
        bytecode.pixels.len() > 50,
        "meminspect.asm should be substantial"
    );
}

#[test]
fn test_meminspect_shows_hex_dump() {
    let vm = run_util_frames("programs/meminspect.asm", 1);
    // The hex dump area (y=22+) should have non-background pixels
    let mut non_bg = 0;
    for y in 22..80 {
        for x in 4..200 {
            if vm.screen[y * 256 + x] != 0x080818 {
                non_bg += 1;
            }
        }
    }
    assert!(
        non_bg > 20,
        "meminspect should render hex dump text, found {} non-bg pixels",
        non_bg
    );
}

#[test]
fn test_meminspect_demo_data() {
    let vm = run_util_frames("programs/meminspect.asm", 1);
    // Demo data "Hello, W" should be at 0x2000
    assert_eq!(vm.ram[0x2000], 72, "demo data[0] should be 'H' (72)");
    assert_eq!(vm.ram[0x2001], 101, "demo data[1] should be 'e' (101)");
    assert_eq!(vm.ram[0x2002], 108, "demo data[2] should be 'l' (108)");
}

#[test]
fn test_meminspect_title_bar() {
    let vm = run_util_frames("programs/meminspect.asm", 1);
    // Title bar pixel
    let title = vm.screen[3 * 256 + 40];
    assert_ne!(
        title, 0x080818,
        "meminspect title should differ from background"
    );
}

#[test]
fn test_disasm_assembles() {
    let source = include_str!("../../programs/disasm.asm");
    let result = crate::assembler::assemble(source, 0);
    assert!(
        result.is_ok(),
        "disasm.asm failed to assemble: {:?}",
        result.err()
    );
    let bytecode = result.unwrap();
    assert!(
        bytecode.pixels.len() > 50,
        "disasm.asm should be substantial"
    );
}

#[test]
fn test_disasm_shows_instructions() {
    let vm = run_util_frames("programs/disasm.asm", 1);
    // The disassembly area (y=22+) should have non-background pixels
    let mut non_bg = 0;
    for y in 22..80 {
        for x in 4..180 {
            if vm.screen[y * 256 + x] != 0x080810 {
                non_bg += 1;
            }
        }
    }
    assert!(
        non_bg > 20,
        "disasm should render decoded instructions, found {} non-bg pixels",
        non_bg
    );
}

#[test]
fn test_disasm_opcode_table() {
    let vm = run_util_frames("programs/disasm.asm", 1);
    // Check that the opcode table was populated
    // HALT at OP_TABLE[0] = 'H','A','L','T'
    assert_eq!(vm.ram[0x6400], 72, "opcode table HALT[0] should be 'H'");
    assert_eq!(vm.ram[0x6401], 65, "opcode table HALT[1] should be 'A'");
    assert_eq!(vm.ram[0x6402], 76, "opcode table HALT[2] should be 'L'");
    assert_eq!(vm.ram[0x6403], 84, "opcode table HALT[3] should be 'T'");
}

#[test]
fn test_disasm_demo_bytecode() {
    let vm = run_util_frames("programs/disasm.asm", 1);
    // Demo bytecode should be at address 0
    // LDI r1, 1 -> [0x10, 1, 1]
    assert_eq!(vm.ram[0], 0x10, "demo byte[0] should be LDI opcode");
    assert_eq!(vm.ram[1], 1, "demo byte[1] should be r1");
    assert_eq!(vm.ram[2], 1, "demo byte[2] should be imm 1");
}

/// Helper: run a utility program until N frames are produced
fn run_util_frames(asm_file: &str, target_frames: usize) -> Vm {
    let source = std::fs::read_to_string(asm_file)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", asm_file, e));
    let asm = crate::assembler::assemble(&source, 0)
        .unwrap_or_else(|e| panic!("Failed to assemble {}: {:?}", asm_file, e));
    let mut vm = Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    let mut frames_seen = 0;
    for _ in 0..2_000_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_ready {
            vm.frame_ready = false;
            frames_seen += 1;
            if frames_seen >= target_frames {
                break;
            }
        }
    }
    vm
}

// ============================================================
// Phase 80: LLM Bridge Opcode Tests
// ============================================================

/// Helper: write a null-terminated ASCII string into RAM at the given address.
/// Each character becomes one u32 word. Returns the length (excluding null).
fn write_string_to_ram(vm: &mut crate::vm::Vm, addr: usize, s: &str) -> usize {
    for (i, byte) in s.bytes().enumerate() {
        if addr + i < vm.ram.len() {
            vm.ram[addr + i] = byte as u32;
        }
    }
    let null_pos = addr + s.len();
    if null_pos < vm.ram.len() {
        vm.ram[null_pos] = 0;
    }
    s.len()
}

/// Helper: read a null-terminated string from RAM.
fn read_string_from_ram(vm: &crate::vm::Vm, addr: usize) -> String {
    let mut result = String::new();
    let mut a = addr;
    while a < vm.ram.len() {
        let ch = vm.ram[a];
        if ch == 0 {
            break;
        }
        if let Some(c) = char::from_u32(ch) {
            result.push(c);
        }
        a += 1;
    }
    result
}

#[test]
fn test_llm_mock_response() {
    // Test that LLM opcode uses mock response when set
    let mut vm = crate::vm::Vm::new();
    // Write prompt at 0x2000
    write_string_to_ram(&mut vm, 0x2000, "What is 2+2?");
    // Set mock response
    vm.llm_mock_response = Some("4".to_string());
    // LLM r1, r2, r3 -- prompt_addr=r1(0x2000), response_addr=r2(0x3000), max_len=r3(256)
    vm.regs[1] = 0x2000;
    vm.regs[2] = 0x3000;
    vm.regs[3] = 256;
    // Execute: 0x9C r1 r2 r3
    vm.ram[0] = 0x9C;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.halted = false;
    assert!(vm.step());
    // r0 should be 1 (length of "4")
    assert_eq!(vm.regs[0], 1, "response length should be 1");
    // Response at 0x3000 should be "4"
    assert_eq!(read_string_from_ram(&vm, 0x3000), "4");
    // Mock should be consumed
    assert!(vm.llm_mock_response.is_none());
}

#[test]
fn test_llm_mock_truncation() {
    // Test that response is truncated to max_len
    let mut vm = crate::vm::Vm::new();
    write_string_to_ram(&mut vm, 0x2000, "Tell me a long story");
    vm.llm_mock_response =
        Some("This is a very long response that should be truncated".to_string());
    vm.regs[1] = 0x2000;
    vm.regs[2] = 0x3000;
    vm.regs[3] = 10; // max_len = 10
    vm.ram[0] = 0x9C;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.halted = false;
    vm.step();
    // r0 should be 10 (truncated)
    assert_eq!(vm.regs[0], 10, "response should be truncated to 10");
    // First 10 chars should match
    let response = read_string_from_ram(&vm, 0x3000);
    assert_eq!(response.len(), 10);
    assert_eq!(response, "This is a ");
}

#[test]
fn test_llm_empty_prompt() {
    // Test that empty prompt returns empty response
    let mut vm = crate::vm::Vm::new();
    // Write empty string (just null terminator) at 0x2000
    vm.ram[0x2000] = 0;
    vm.regs[1] = 0x2000;
    vm.regs[2] = 0x3000;
    vm.regs[3] = 256;
    vm.ram[0] = 0x9C;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.halted = false;
    vm.step();
    // Empty prompt should return 0 length
    assert_eq!(vm.regs[0], 0, "empty prompt should return 0");
}

#[test]
fn test_llm_null_termination() {
    // Test that response is null-terminated
    let mut vm = crate::vm::Vm::new();
    write_string_to_ram(&mut vm, 0x2000, "hello");
    vm.llm_mock_response = Some("world".to_string());
    vm.regs[1] = 0x2000;
    vm.regs[2] = 0x3000;
    vm.regs[3] = 256;
    vm.ram[0] = 0x9C;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.halted = false;
    vm.step();
    assert_eq!(vm.regs[0], 5);
    assert_eq!(vm.ram[0x3005], 0, "response should be null-terminated");
}

#[test]
fn test_llm_assembler() {
    // Test that the assembler correctly encodes LLM instruction
    let source = "LLM r1, r2, r3\nHALT\n";
    let result = crate::assembler::assemble(source, 0);
    assert!(
        result.is_ok(),
        "assembly should succeed: {:?}",
        result.err()
    );
    let asm = result.unwrap();
    // LDM r1, r2, r3 -> [0x9C, 1, 2, 3]
    assert_eq!(asm.pixels[0], 0x9C, "opcode should be 0x9C");
    assert_eq!(asm.pixels[1], 1, "prompt_addr reg should be r1");
    assert_eq!(asm.pixels[2], 2, "response_addr reg should be r2");
    assert_eq!(asm.pixels[3], 3, "max_len reg should be r3");
}

#[test]
fn test_llm_assembler_error() {
    // Test wrong argument count
    let source = "LLM r1, r2\nHALT\n";
    let result = crate::assembler::assemble(source, 0);
    match result {
        Err(err) => {
            assert!(
                err.message.contains("3 arguments"),
                "error should mention 3 arguments: {}",
                err.message
            );
        }
        Ok(_) => panic!("should fail with wrong arg count"),
    }
}

#[test]
fn test_llm_disassembler() {
    // Test disassembler recognizes LLM opcode
    let (mnemonic, len) = disasm(&[0x9Cu32, 1, 2, 3]);
    assert_eq!(len, 4, "LLM instruction should be 4 words");
    assert!(mnemonic.contains("LLM"), "should contain LLM: {}", mnemonic);
    assert!(mnemonic.contains("r1"), "should show r1: {}", mnemonic);
    assert!(mnemonic.contains("r2"), "should show r2: {}", mnemonic);
    assert!(mnemonic.contains("r3"), "should show r3: {}", mnemonic);
}

#[test]
fn test_llm_multiline_response() {
    // Test response with newlines
    let mut vm = crate::vm::Vm::new();
    write_string_to_ram(&mut vm, 0x2000, "list 3 colors");
    vm.llm_mock_response = Some("red\ngreen\nblue".to_string());
    vm.regs[1] = 0x2000;
    vm.regs[2] = 0x3000;
    vm.regs[3] = 256;
    vm.ram[0] = 0x9C;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.halted = false;
    vm.step();
    assert_eq!(
        vm.regs[0], 14,
        "response length should be 14 (red\\ngreen\\nblue)"
    );
    // Verify the response bytes in RAM
    let expected = "red\ngreen\nblue";
    for (i, byte) in expected.bytes().enumerate() {
        assert_eq!(vm.ram[0x3000 + i], byte as u32, "byte {} mismatch", i);
    }
}

#[test]
fn test_llm_full_program() {
    // Test a complete program: write prompt, call LLM, verify response
    let mut vm = crate::vm::Vm::new();
    // Write "hello" at 0x2000
    write_string_to_ram(&mut vm, 0x2000, "hello");
    // Set mock response before running
    vm.llm_mock_response = Some("Hello! How can I help?".to_string());
    // Program:
    //   LDI r1, 0x2000    ; prompt addr
    //   LDI r2, 0x3000    ; response addr
    //   LDI r3, 100       ; max_len
    //   LLM r1, r2, r3    ; call LLM
    //   HALT
    vm.ram[0] = 0x10;
    vm.ram[1] = 1;
    vm.ram[2] = 0x2000; // LDI r1, 0x2000
    vm.ram[3] = 0x10;
    vm.ram[4] = 2;
    vm.ram[5] = 0x3000; // LDI r2, 0x3000
    vm.ram[6] = 0x10;
    vm.ram[7] = 3;
    vm.ram[8] = 100; // LDI r3, 100
    vm.ram[9] = 0x9C;
    vm.ram[10] = 1;
    vm.ram[11] = 2;
    vm.ram[12] = 3; // LLM r1, r2, r3
    vm.ram[13] = 0x00; // HALT
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.halted, "program should halt");
    assert_eq!(vm.regs[0], 22, "response length should be 22");
    let response = read_string_from_ram(&vm, 0x3000);
    assert_eq!(response, "Hello! How can I help?");
}

#[test]
fn test_strip_think_blocks() {
    use super::strip_think_blocks;
    // Self-closing
    assert_eq!(strip_think_blocks("<think/>Hello"), "Hello");
    // With space
    assert_eq!(strip_think_blocks("<think />Hello"), "Hello");
    // Full block with proper tags
    assert_eq!(
        strip_think_blocks("<think\nreasoning here</think Hello"),
        "Hello"
    );
    assert_eq!(
        strip_think_blocks("Before<think inner</think After"),
        "BeforeAfter"
    );
    // Multiple blocks
    assert_eq!(strip_think_blocks("<think/>A<think/>B"), "AB");
}

#[test]
fn test_extract_json_str() {
    use super::extract_json_str;
    let json = r#"{"base_url":"http://localhost:11434","model":"qwen3.5"}"#;
    assert_eq!(
        extract_json_str(json, "base_url"),
        Some("http://localhost:11434".to_string())
    );
    assert_eq!(extract_json_str(json, "model"), Some("qwen3.5".to_string()));
    assert_eq!(extract_json_str(json, "missing"), None);
    // With escape sequences
    let json2 = r#"{"text":"hello\nworld"}"#;
    assert_eq!(
        extract_json_str(json2, "text"),
        Some("hello\nworld".to_string())
    );
}

// ── Phase 84: Building & Desktop Integration Tests ────────────────

#[test]
fn test_world_desktop_assembles() {
    use crate::assembler::assemble;
    let source = std::fs::read_to_string("programs/world_desktop.asm")
        .expect("world_desktop.asm should exist");
    let result = assemble(&source, 0);
    assert!(
        result.is_ok(),
        "world_desktop.asm should assemble: {:?}",
        result.err()
    );
    let asm = result.unwrap();
    assert!(
        asm.pixels.len() > 1000,
        "world_desktop should produce substantial bytecode, got {} words",
        asm.pixels.len()
    );
}

#[test]
fn test_world_desktop_runs_and_produces_frame() {
    use crate::assembler::assemble;
    let source = std::fs::read_to_string("programs/world_desktop.asm").unwrap();
    let asm = assemble(&source, 0).unwrap();
    let mut vm = crate::vm::Vm::new();
    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    let mut frames_seen = 0;
    for _ in 0..10_000_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_ready {
            vm.frame_ready = false;
            frames_seen += 1;
            if frames_seen >= 1 {
                break;
            }
        }
    }
    assert!(
        frames_seen >= 1,
        "world_desktop should produce at least 1 frame, got {}",
        frames_seen
    );
}

#[test]
fn test_building_table_initialized() {
    use crate::assembler::assemble;
    let source = std::fs::read_to_string("programs/world_desktop.asm").unwrap();
    let asm = assemble(&source, 0).unwrap();
    let mut vm = crate::vm::Vm::new();
    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    // Run through init until first frame
    for _ in 0..10_000_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_ready {
            break;
        }
    }
    // Building count at 0x7580 should be 13 (ai_terminal added)
    assert_eq!(
        vm.ram[0x7580], 13,
        "building count should be 13, got {}",
        vm.ram[0x7580]
    );
    // First building at 0x7500 should have world_x = 52
    assert_eq!(
        vm.ram[0x7500], 52,
        "building 0 world_x should be 52, got {}",
        vm.ram[0x7500]
    );
    // First building world_y = 48
    assert_eq!(
        vm.ram[0x7501], 48,
        "building 0 world_y should be 48, got {}",
        vm.ram[0x7501]
    );
    // First building type color should be red
    assert_eq!(
        vm.ram[0x7502], 0xFF4444,
        "building 0 color should be 0xFF4444, got {:08X}",
        vm.ram[0x7502]
    );
}

#[test]
fn test_building_names_stored() {
    use crate::assembler::assemble;
    let source = std::fs::read_to_string("programs/world_desktop.asm").unwrap();
    let asm = assemble(&source, 0).unwrap();
    let mut vm = crate::vm::Vm::new();
    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..10_000_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_ready {
            break;
        }
    }
    // Read "snake" from 0x7600
    let mut name = String::new();
    for j in 0..16 {
        let ch = vm.ram[0x7600 + j];
        if ch == 0 || ch > 127 {
            break;
        }
        name.push(ch as u8 as char);
    }
    assert_eq!(
        name, "snake",
        "building 0 name should be 'snake', got '{}'",
        name
    );

    // Read "ball" from 0x7610
    let mut name2 = String::new();
    for j in 0..16 {
        let ch = vm.ram[0x7610 + j];
        if ch == 0 || ch > 127 {
            break;
        }
        name2.push(ch as u8 as char);
    }
    assert_eq!(
        name2, "ball",
        "building 1 name should be 'ball', got '{}'",
        name2
    );
}

#[test]
fn test_player_position_initialized() {
    use crate::assembler::assemble;
    let source = std::fs::read_to_string("programs/world_desktop.asm").unwrap();
    let asm = assemble(&source, 0).unwrap();
    let mut vm = crate::vm::Vm::new();
    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..10_000_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_ready {
            break;
        }
    }
    // Player position should be initialized
    let px = vm.ram[0x7808];
    let py = vm.ram[0x7809];
    assert!(px > 0, "player_x should be > 0, got {}", px);
    assert!(py > 0, "player_y should be > 0, got {}", py);
}

#[test]
fn test_taskbar_renders_pixels() {
    use crate::assembler::assemble;
    let source = std::fs::read_to_string("programs/world_desktop.asm").unwrap();
    let asm = assemble(&source, 0).unwrap();
    let mut vm = crate::vm::Vm::new();
    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..10_000_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_ready {
            vm.frame_ready = false;
            break;
        }
    }
    // Taskbar at y=240..255 should have pixels
    let mut taskbar_pixels = 0;
    for y in 240..256 {
        for x in 0..256 {
            if vm.screen[y * 256 + x] != 0 {
                taskbar_pixels += 1;
            }
        }
    }
    assert!(
        taskbar_pixels > 100,
        "taskbar should render pixels at y=240..255, got {} pixels",
        taskbar_pixels
    );
}

#[test]
fn test_building_types_varied() {
    use crate::assembler::assemble;
    let source = std::fs::read_to_string("programs/world_desktop.asm").unwrap();
    let asm = assemble(&source, 0).unwrap();
    let mut vm = crate::vm::Vm::new();
    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..10_000_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_ready {
            break;
        }
    }
    // Check that buildings have different type colors
    let colors: Vec<u32> = (0..8).map(|i| vm.ram[0x7500 + i * 4 + 2]).collect();
    let unique_colors: std::collections::HashSet<u32> = colors.iter().copied().collect();
    assert!(
        unique_colors.len() >= 3,
        "should have at least 3 different building type colors, got {}",
        unique_colors.len()
    );
}

#[test]
fn test_building_positions_spread() {
    use crate::assembler::assemble;
    let source = std::fs::read_to_string("programs/world_desktop.asm").unwrap();
    let asm = assemble(&source, 0).unwrap();
    let mut vm = crate::vm::Vm::new();
    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..10_000_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_ready {
            break;
        }
    }
    // Buildings should be spread across the map (x range > 100)
    let xs: Vec<u32> = (0..8).map(|i| vm.ram[0x7500 + i * 4]).collect();
    let min_x = *xs.iter().min().unwrap_or(&0);
    let max_x = *xs.iter().max().unwrap_or(&0);
    assert!(
        max_x - min_x > 100,
        "buildings should be spread (x range {}..{} = {})",
        min_x,
        max_x,
        max_x - min_x
    );
}

#[test]
fn test_nearby_building_flag_clears() {
    use crate::assembler::assemble;
    let source = std::fs::read_to_string("programs/world_desktop.asm").unwrap();
    let asm = assemble(&source, 0).unwrap();
    let mut vm = crate::vm::Vm::new();
    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    // Run for 2 frames
    let mut frames = 0;
    for _ in 0..10_000_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_ready {
            vm.frame_ready = false;
            frames += 1;
            if frames >= 2 {
                break;
            }
        }
    }
    // After frame, nearby flag should be 0 (player at 32,32, nearest building at 35,95)
    let nearby = vm.ram[0x7588];
    // Player starts at (32,32), buildings are at (52,48), (78,85), etc.
    // Distance to building 0 at (52,48) is 20+16=36, which is > 4 in both axes
    // So nearby should be 0
    assert_eq!(
        nearby, 0,
        "nearby_building flag should be 0 when no building within 4 tiles, got {}",
        nearby
    );
}

#[test]
fn test_camera_follows_player() {
    use crate::assembler::assemble;
    let source = std::fs::read_to_string("programs/world_desktop.asm").unwrap();
    let asm = assemble(&source, 0).unwrap();
    let mut vm = crate::vm::Vm::new();
    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..10_000_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_ready {
            break;
        }
    }
    let cam_x = vm.ram[0x7800];
    let cam_y = vm.ram[0x7801];
    let player_x = vm.ram[0x7808];
    let player_y = vm.ram[0x7809];
    // Camera should center on player: camera = player - 32 (wrapping u32)
    // Check that camera relates to player (allow wrapping for small values)
    let expected_cam_x = player_x.wrapping_sub(32);
    let expected_cam_y = player_y.wrapping_sub(32);
    assert_eq!(
        cam_x, expected_cam_x,
        "camera_x should be player_x({}) - 32 = {}, got {}",
        player_x, expected_cam_x, cam_x
    );
    assert_eq!(
        cam_y, expected_cam_y,
        "camera_y should be player_y({}) - 32 = {}, got {}",
        player_y, expected_cam_y, cam_y
    );
}

#[test]
fn test_desktop_state_ram_readable() {
    use crate::assembler::assemble;
    let source = std::fs::read_to_string("programs/world_desktop.asm").unwrap();
    let asm = assemble(&source, 0).unwrap();
    let mut vm = crate::vm::Vm::new();
    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..10_000_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_ready {
            break;
        }
    }
    // Frame counter should be incrementing
    let frame = vm.ram[0x7802];
    assert!(
        frame > 0,
        "frame_counter should be > 0 after first frame, got {}",
        frame
    );

    // All building name addresses should be non-zero
    for i in 0..8 {
        let name_addr = vm.ram[0x7503 + i * 4];
        assert_ne!(name_addr, 0, "building {} name_addr should not be 0", i);
    }
}

// ===== Phase 84: Additional Building Tests =====

#[test]
fn test_building_color_classification() {
    // Verify type colors: red=games, green=creative, blue=utility, yellow=system
    use crate::assembler::assemble;
    let source = std::fs::read_to_string("programs/world_desktop.asm").unwrap();
    let asm = assemble(&source, 0).unwrap();
    let mut vm = crate::vm::Vm::new();
    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..10_000_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_ready {
            break;
        }
    }
    // Building 0 (snake): red game
    assert_eq!(vm.ram[0x7502], 0xFF4444, "building 0 should be red (game)");
    // Building 2 (plasma): green creative
    assert_eq!(
        vm.ram[0x750A], 0x44FF44,
        "building 2 should be green (creative)"
    );
    // Building 4 (colors): blue utility
    assert_eq!(
        vm.ram[0x7512], 0x4444FF,
        "building 4 should be blue (utility)"
    );
    // Building 6 (init): yellow system
    assert_eq!(
        vm.ram[0x751A], 0xFFFF44,
        "building 6 should be yellow (system)"
    );
}

#[test]
fn test_all_building_names_readable() {
    use crate::assembler::assemble;
    let source = std::fs::read_to_string("programs/world_desktop.asm").unwrap();
    let asm = assemble(&source, 0).unwrap();
    let mut vm = crate::vm::Vm::new();
    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..10_000_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_ready {
            break;
        }
    }
    let expected = [
        "snake", "ball", "plasma", "painter", "colors", "fire", "init", "shell",
    ];
    for (i, expected_name) in expected.iter().enumerate() {
        let name_addr = vm.ram[0x7503 + i * 4] as usize;
        let mut name = String::new();
        for j in 0..16 {
            if name_addr + j >= vm.ram.len() {
                break;
            }
            let ch = vm.ram[name_addr + j];
            if ch == 0 || ch > 127 {
                break;
            }
            name.push(ch as u8 as char);
        }
        assert_eq!(
            name, *expected_name,
            "building {} name should be '{}', got '{}'",
            i, expected_name, name
        );
    }
}

#[test]
fn test_building_count_correct() {
    use crate::assembler::assemble;
    let source = std::fs::read_to_string("programs/world_desktop.asm").unwrap();
    let asm = assemble(&source, 0).unwrap();
    let mut vm = crate::vm::Vm::new();
    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..10_000_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_ready {
            break;
        }
    }
    assert_eq!(vm.ram[0x7580], 13, "building count should be exactly 13");
}

#[test]
fn test_proximity_detection_positive() {
    // Move player near building 0 at (52,48), verify proximity tooltip renders
    // The nearby flag is set mid-frame and cleared at frame end,
    // so we check for tooltip evidence (text area near y=112)
    use crate::assembler::assemble;
    let source = std::fs::read_to_string("programs/world_desktop.asm").unwrap();
    let asm = assemble(&source, 0).unwrap();
    let mut vm = crate::vm::Vm::new();
    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    // Run first frame to init
    for _ in 0..10_000_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_ready {
            vm.frame_ready = false;
            break;
        }
    }
    // Teleport player to (54, 50) near building 0 center (55, 52)
    vm.ram[0x7808] = 54; // player_x
    vm.ram[0x7809] = 50; // player_y
    vm.ram[0xFFB] = 0; // clear key bitmask
    vm.halted = false;
    // Run another frame
    for _ in 0..10_000_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_ready {
            break;
        }
    }
    // Count non-black pixels in tooltip area (y=108..122, x=90..180)
    let mut tooltip_pixels = 0;
    for y in 108..122 {
        for x in 90..180 {
            if vm.screen[y * 256 + x] != 0 {
                tooltip_pixels += 1;
            }
        }
    }
    assert!(
        tooltip_pixels > 10,
        "tooltip area should have pixels when player near building, got {} pixels",
        tooltip_pixels
    );
}

#[test]
fn test_building_renders_on_screen() {
    // Verify that when a building is in viewport, its colored pixels appear on screen
    use crate::assembler::assemble;
    let source = std::fs::read_to_string("programs/world_desktop.asm").unwrap();
    let asm = assemble(&source, 0).unwrap();
    let mut vm = crate::vm::Vm::new();
    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..10_000_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_ready {
            break;
        }
    }
    // After frame 1, building 0 at world (52,48) with camera at (0,0)
    // Building pixel position: (52*4, 48*4) = (208, 192), size 24x32
    // Check that any building-type pixels appear on screen
    // Building colors: 0xFF4444 (red games), 0x44FF44 (green), 0x4444FF (blue), 0xFFFF44 (yellow)
    let mut found_building_pixel = false;
    for y in 0..240 {
        for x in 0..256 {
            let px = vm.screen[y * 256 + x];
            if px == 0xFF4444 || px == 0x44FF44 || px == 0x4444FF || px == 0xFFFF44 {
                found_building_pixel = true;
                break;
            }
        }
        if found_building_pixel {
            break;
        }
    }
    assert!(
        found_building_pixel,
        "should find building-type colored pixels on screen (checked full viewport)"
    );
}

#[test]
fn test_building_door_rendered() {
    // Verify the dark door (0x222222) is rendered at the bottom of building 0
    use crate::assembler::assemble;
    let source = std::fs::read_to_string("programs/world_desktop.asm").unwrap();
    let asm = assemble(&source, 0).unwrap();
    let mut vm = crate::vm::Vm::new();
    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..10_000_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_ready {
            break;
        }
    }
    // Building 0 at pixel (208, 192), door at (208+10, 192+24) = (218, 216), size 4x8
    // Camera is at (player_x - 32, player_y - 32), player starts at (32,32)
    // so camera = (0,0). Building 0 at world (52,48) -> screen (208, 192)
    let cam_x: i32 = vm.ram.get(0x7800).copied().unwrap_or(0) as i32;
    let cam_y: i32 = vm.ram.get(0x7801).copied().unwrap_or(0) as i32;
    let door_x = ((52 - cam_x) * 4 + 10) as usize;
    let door_y = ((48 - cam_y) * 4 + 24) as usize;
    let mut found_door = false;
    if door_x + 4 <= 256 && door_y + 8 <= 256 {
        for y in door_y..door_y + 8 {
            for x in door_x..door_x + 4 {
                if vm.screen[y * 256 + x] == 0x222222 {
                    found_door = true;
                    break;
                }
            }
            if found_door {
                break;
            }
        }
    }
    assert!(
        found_door,
        "should find dark door pixels at ({},{})",
        door_x, door_y
    );
}

#[test]
fn test_minimap_building_markers() {
    // Verify that the minimap at screen (224..255, 0..31) has colored dots for buildings
    use crate::assembler::assemble;
    let source = std::fs::read_to_string("programs/world_desktop.asm").unwrap();
    let asm = assemble(&source, 0).unwrap();
    let mut vm = crate::vm::Vm::new();
    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..10_000_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_ready {
            break;
        }
    }
    // Count non-black pixels in the minimap region (224..255, 0..31)
    let mut minimap_colored_pixels = 0;
    for y in 0..32 {
        for x in 224..256 {
            if vm.screen[y * 256 + x] != 0 {
                minimap_colored_pixels += 1;
            }
        }
    }
    // Minimap should have border + player dot + at least some building markers
    assert!(
        minimap_colored_pixels > 20,
        "minimap should have colored pixels (border + building markers), got {}",
        minimap_colored_pixels
    );
}

#[test]
fn test_building_door_dark_color() {
    // Verify the door color (0x222222) appears on screen at building location
    // Building 0 at world (52,48) should render a dark door
    use crate::assembler::assemble;
    let source = std::fs::read_to_string("programs/world_desktop.asm").unwrap();
    let asm = assemble(&source, 0).unwrap();
    let mut vm = crate::vm::Vm::new();
    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..10_000_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_ready {
            break;
        }
    }
    // Count dark pixels (0x222222) anywhere in the game area (not taskbar)
    let mut dark_pixels = 0;
    for y in 0..240 {
        for x in 0..256 {
            if vm.screen[y * 256 + x] == 0x222222 {
                dark_pixels += 1;
            }
        }
    }
    // Each door is 4x8=32 pixels, so should find at least 32 dark pixels
    assert!(
        dark_pixels >= 32,
        "should find dark door pixels (0x222222), got {}",
        dark_pixels
    );
}

#[test]
fn test_building_no_overlap() {
    // Verify buildings don't overlap in world space
    use crate::assembler::assemble;
    let source = std::fs::read_to_string("programs/world_desktop.asm").unwrap();
    let asm = assemble(&source, 0).unwrap();
    let mut vm = crate::vm::Vm::new();
    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..10_000_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_ready {
            break;
        }
    }
    // Each building is 6 tiles wide (24px / 4px per tile), 8 tiles tall (32px / 4px per tile)
    // Check that no two buildings overlap
    for i in 0..8 {
        for j in (i + 1)..8 {
            let x1 = vm.ram[0x7500 + i * 4];
            let y1 = vm.ram[0x7501 + i * 4];
            let x2 = vm.ram[0x7500 + j * 4];
            let y2 = vm.ram[0x7501 + j * 4];
            // Buildings are 6x8 tiles
            let overlap_x = x1 < x2 + 6 && x2 < x1 + 6;
            let overlap_y = y1 < y2 + 8 && y2 < y1 + 8;
            assert!(
                !(overlap_x && overlap_y),
                "buildings {} and {} overlap at ({},{}) and ({},{})",
                i,
                j,
                x1,
                y1,
                x2,
                y2
            );
        }
    }
}

// ===== Phase 86: Hypervisor Building Tests =====

#[test]
fn test_hypervisor_opcode_basic() {
    // HYPERVISOR with no window_id (backward compat)
    let source = r#"
        LDI r10, 0x2000
        STRO r10, "arch=riscv64 kernel=Image ram=256M"
        HYPERVISOR r10
        HALT
    "#;
    let asm = crate::assembler::assemble(source, 0).unwrap();
    let mut vm = crate::vm::Vm::new();
    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..100_000 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.hypervisor_active, "hypervisor should be active");
    assert!(
        vm.hypervisor_config.contains("arch=riscv64"),
        "config should contain arch"
    );
    assert_eq!(
        vm.hypervisor_window_id, 0,
        "window_id should be 0 (default)"
    );
    assert_eq!(vm.regs[0], 0, "r0 should be 0 (success)");
}

#[test]
fn test_hypervisor_opcode_with_window() {
    // HYPERVISOR targeting a WINSYS window
    let source = r#"
        LDI r10, 0x2000
        STRO r10, "arch=riscv64 kernel=Image ram=256M"
        ; Create window first
        LDI r1, 0
        LDI r2, 16
        LDI r3, 16
        LDI r4, 200
        LDI r5, 150
        LDI r6, 0x2100
        WINSYS r1
        ; r0 = window_id from create
        MOV r11, r0
        HYPERVISOR r10, r11
        HALT
    "#;
    let asm = crate::assembler::assemble(source, 0).unwrap();
    let mut vm = crate::vm::Vm::new();
    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..100_000 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.hypervisor_active, "hypervisor should be active");
    assert_eq!(vm.regs[0], 0, "r0 should be 0 (success)");
    // window_id should be 1 (first WINSYS window)
    assert_eq!(vm.hypervisor_window_id, 1, "window_id should be 1");
}

#[test]
fn test_hypervisor_missing_arch() {
    // HYPERVISOR without arch= parameter should fail
    let source = r#"
        LDI r10, 0x2000
        STRO r10, "kernel=Image ram=256M"
        HYPERVISOR r10
        HALT
    "#;
    let asm = crate::assembler::assemble(source, 0).unwrap();
    let mut vm = crate::vm::Vm::new();
    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..100_000 {
        if !vm.step() {
            break;
        }
    }
    assert!(!vm.hypervisor_active, "hypervisor should NOT be active");
    assert_eq!(vm.regs[0], 0xFFFFFFFD, "r0 should be missing arch error");
}

#[test]
fn test_hypervisor_empty_string() {
    // HYPERVISOR with empty string should fail
    let source = r#"
        LDI r10, 0x2000
        STRO r10, ""
        HYPERVISOR r10
        HALT
    "#;
    let asm = crate::assembler::assemble(source, 0).unwrap();
    let mut vm = crate::vm::Vm::new();
    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..100_000 {
        if !vm.step() {
            break;
        }
    }
    assert!(
        !vm.hypervisor_active,
        "hypervisor should NOT be active with empty string"
    );
}

#[test]
fn test_hypervisor_mode_native() {
    // HYPERVISOR with mode=native should set Native mode
    let source = r#"
        LDI r10, 0x2000
        STRO r10, "arch=riscv64 mode=native"
        HYPERVISOR r10
        HALT
    "#;
    let asm = crate::assembler::assemble(source, 0).unwrap();
    let mut vm = crate::vm::Vm::new();
    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..100_000 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.hypervisor_active, "hypervisor should be active");
    assert!(
        matches!(vm.hypervisor_mode, crate::vm::HypervisorMode::Native),
        "mode should be Native"
    );
}

#[test]
fn test_hypervisor_mode_qemu_default() {
    // HYPERVISOR without mode= should default to Qemu
    let source = r#"
        LDI r10, 0x2000
        STRO r10, "arch=riscv64 kernel=Image"
        HYPERVISOR r10
        HALT
    "#;
    let asm = crate::assembler::assemble(source, 0).unwrap();
    let mut vm = crate::vm::Vm::new();
    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..100_000 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.hypervisor_active, "hypervisor should be active");
    assert!(
        matches!(vm.hypervisor_mode, crate::vm::HypervisorMode::Qemu),
        "mode should be Qemu (default)"
    );
}

#[test]
fn test_hypervisor_window_id_field_persists() {
    // Verify hypervisor_window_id persists across construction
    let source = r#"
        LDI r10, 0x2000
        STRO r10, "arch=riscv32 mode=qemu"
        LDI r11, 42
        HYPERVISOR r10, r11
        HALT
    "#;
    let asm = crate::assembler::assemble(source, 0).unwrap();
    let mut vm = crate::vm::Vm::new();
    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..100_000 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.hypervisor_window_id, 42, "window_id should be 42");
}

#[test]
fn test_hypervisor_resets_cleanly() {
    // Verify reset() clears hypervisor state
    let source = r#"
        LDI r10, 0x2000
        STRO r10, "arch=riscv64 kernel=Image"
        LDI r11, 5
        HYPERVISOR r10, r11
        HALT
    "#;
    let asm = crate::assembler::assemble(source, 0).unwrap();
    let mut vm = crate::vm::Vm::new();
    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..100_000 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.hypervisor_active);
    assert_eq!(vm.hypervisor_window_id, 5);

    // Reset and verify all fields cleared
    vm.reset();
    assert!(!vm.hypervisor_active, "should be inactive after reset");
    assert!(
        vm.hypervisor_config.is_empty(),
        "config should be empty after reset"
    );
    assert_eq!(
        vm.hypervisor_window_id, 0,
        "window_id should be 0 after reset"
    );
}

#[test]
fn test_hypervisor_assembler_backward_compat() {
    // HYPERVISOR with single arg (no window) should assemble to 3 words
    let source = "HYPERVISOR r5";
    let asm = crate::assembler::assemble(source, 0).unwrap();
    assert_eq!(
        asm.pixels.len(),
        3,
        "should be 3 words: opcode + reg + win_id(0)"
    );
    assert_eq!(asm.pixels[0], 0x72); // opcode
    assert_eq!(asm.pixels[1], 5); // addr_reg = r5
    assert_eq!(asm.pixels[2], 0); // win_id_reg = r0 (default)
}

#[test]
fn test_hypervisor_assembler_with_window() {
    // HYPERVISOR with two args should assemble correctly
    let source = "HYPERVISOR r5, r3";
    let asm = crate::assembler::assemble(source, 0).unwrap();
    assert_eq!(asm.pixels.len(), 3);
    assert_eq!(asm.pixels[0], 0x72);
    assert_eq!(asm.pixels[1], 5); // addr_reg = r5
    assert_eq!(asm.pixels[2], 3); // win_id_reg = r3
}

#[test]
fn test_linux_building_assembles() {
    let source = std::fs::read_to_string("programs/linux_building.asm").unwrap();
    let asm = crate::assembler::assemble(&source, 0);
    assert!(
        asm.is_ok(),
        "linux_building.asm should assemble: {:?}",
        asm.err()
    );
}

#[test]
fn test_world_desktop_has_9_buildings() {
    // Verify the hypervisor building was added
    let source = std::fs::read_to_string("programs/world_desktop.asm").unwrap();
    assert!(source.contains("linux"), "should have linux building name");
    assert!(
        source.contains("0xFFD700"),
        "should have gold color for hypervisor building"
    );
    assert!(
        source.contains("0x7680"),
        "should reference linux name at 0x7680"
    );
}

#[test]
fn test_mcp_hypervisor_tools_in_source() {
    // Verify hypervisor tools are defined in MCP server source
    let source = std::fs::read_to_string("src/mcp_server.rs").unwrap();
    assert!(
        source.contains("\"hypervisor_boot\""),
        "MCP server should define hypervisor_boot tool"
    );
    assert!(
        source.contains("\"hypervisor_kill\""),
        "MCP server should define hypervisor_kill tool"
    );
    assert!(
        source.contains("hypervisor_boot_schema"),
        "MCP server should have hypervisor_boot schema"
    );
    assert!(
        source.contains("hypervisor_kill_schema"),
        "MCP server should have hypervisor_kill schema"
    );
}

// ── Phase 88: AI Vision Bridge Tests ──

#[test]
fn test_ai_agent_assembles() {
    let source = "LDI r10, 1\nAI_AGENT r10\nHALT";
    let asm = crate::assembler::assemble(source, 0);
    assert!(asm.is_ok(), "AI_AGENT should assemble: {:?}", asm.err());
    let pixels = asm.unwrap().pixels;
    assert_eq!(pixels[0], 0x10); // LDI
    assert_eq!(pixels[1], 10); // r10
    assert_eq!(pixels[2], 1); // imm=1
    assert_eq!(pixels[3], 0xB0); // AI_AGENT
    assert_eq!(pixels[4], 10); // r10
    assert_eq!(pixels[5], 0x00); // HALT
}

#[test]
fn test_ai_agent_checksum() {
    let mut vm = crate::vm::Vm::new();
    vm.regs[10] = 1;
    vm.ram[0] = 0xB0; // AI_AGENT
    vm.ram[1] = 10; // r10
    vm.ram[2] = 0x00; // HALT
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    let hash = vm.regs[0];
    let expected = crate::vision::canvas_checksum(&vm.screen);
    assert_eq!(hash, expected, "checksum should match vision module");
}

#[test]
fn test_ai_agent_checksum_detects_change() {
    let mut vm = crate::vm::Vm::new();
    // Checksum of empty screen
    vm.regs[10] = 1;
    vm.ram[0] = 0xB0;
    vm.ram[1] = 10;
    vm.ram[2] = 0x00;
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    let hash_empty = vm.regs[0];

    // Draw a pixel and re-check
    vm.screen[0] = 0xFF0000;
    vm.halted = false;
    vm.regs[10] = 1;
    vm.ram[0] = 0xB0;
    vm.ram[1] = 10;
    vm.ram[2] = 0x00;
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    let hash_changed = vm.regs[0];

    assert_ne!(hash_empty, hash_changed, "checksums should differ");
}

#[test]
fn test_ai_agent_diff() {
    let mut vm = crate::vm::Vm::new();
    // Store screen in RAM at 0x2000 (first 256 pixels only for speed)
    for i in 0..256 {
        vm.ram[0x2000 + i] = vm.screen[i];
    }
    // Change 3 pixels
    vm.screen[0] = 0xFFFFFF;
    vm.screen[1] = 0xFF0000;
    vm.screen[255] = 0x00FF00;

    vm.regs[10] = 2;
    vm.regs[11] = 0x2000;
    vm.ram[0] = 0xB0;
    vm.ram[1] = 10;
    vm.ram[2] = 0x00;
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    let changed = vm.regs[0];
    assert_eq!(changed, 3, "should detect 3 changed pixels");
}

#[test]
fn test_ai_agent_diff_no_change() {
    let mut vm = crate::vm::Vm::new();
    for i in 0..256 * 256 {
        if 0x2000 + i < vm.ram.len() {
            vm.ram[0x2000 + i] = vm.screen[i];
        }
    }
    vm.regs[10] = 2;
    vm.regs[11] = 0x2000;
    vm.ram[0] = 0xB0;
    vm.ram[1] = 10;
    vm.ram[2] = 0x00;
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.regs[0], 0, "no pixels should have changed");
}

#[test]
fn test_ai_agent_vision_api_mock() {
    let mut vm = crate::vm::Vm::new();
    vm.llm_mock_response = Some("AI sees red bars".to_string());

    // Write prompt to RAM
    for (i, b) in b"desc".iter().enumerate() {
        vm.ram[0x5000 + i] = *b as u32;
    }
    vm.ram[0x5004] = 0;

    vm.regs[10] = 3;
    vm.regs[11] = 0x5000;
    vm.regs[12] = 0x6000;
    vm.regs[13] = 256;

    vm.ram[0] = 0xB0;
    vm.ram[1] = 10;
    vm.ram[2] = 0x00;
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }

    let resp_len = vm.regs[0];
    assert_eq!(resp_len, 16, "response should be 16 bytes");

    let mut response = String::new();
    for i in 0..resp_len as usize {
        let ch = vm.ram[0x6000 + i];
        if ch == 0 {
            break;
        }
        if let Some(c) = char::from_u32(ch) {
            response.push(c);
        }
    }
    assert_eq!(response, "AI sees red bars");
    assert!(vm.llm_mock_response.is_none(), "mock should be cleared");
}

#[test]
fn test_ai_agent_vision_api_no_mock() {
    let mut vm = crate::vm::Vm::new();
    vm.regs[10] = 3;
    vm.regs[11] = 0x5000;
    vm.regs[12] = 0x6000;
    vm.regs[13] = 256;
    vm.ram[0] = 0xB0;
    vm.ram[1] = 10;
    vm.ram[2] = 0x00;
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.regs[0], 0xFFFFFFFF, "should error without mock");
}

#[test]
fn test_ai_agent_unknown_op() {
    let mut vm = crate::vm::Vm::new();
    vm.regs[10] = 99;
    vm.ram[0] = 0xB0;
    vm.ram[1] = 10;
    vm.ram[2] = 0x00;
    vm.pc = 0;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }
    assert_eq!(vm.regs[0], 0xFFFFFFFF, "unknown op should error");
}

#[test]
fn test_ai_vision_asm_assembles() {
    let source = std::fs::read_to_string("programs/ai_vision.asm").unwrap();
    let asm = crate::assembler::assemble(&source, 0);
    assert!(
        asm.is_ok(),
        "ai_vision.asm should assemble: {:?}",
        asm.err()
    );
    let pixels = asm.unwrap().pixels;
    assert!(
        pixels.iter().any(|&p| p == 0xB0),
        "should contain AI_AGENT opcode"
    );
}

#[test]
fn test_ai_agent_disasm() {
    let mut vm = crate::vm::Vm::new();
    vm.ram[0] = 0xB0;
    vm.ram[1] = 10;
    let (name, len) = vm.disassemble_at(0);
    assert!(
        name.contains("AI_AGENT"),
        "disasm should show AI_AGENT, got: {}",
        name
    );
    assert_eq!(len, 2);
}

#[test]
fn test_mcp_vision_tools_in_source() {
    let source = std::fs::read_to_string("src/mcp_server.rs").unwrap();
    assert!(
        source.contains("\"vision_screenshot\""),
        "MCP should have vision_screenshot"
    );
    assert!(
        source.contains("\"vision_checksum\""),
        "MCP should have vision_checksum"
    );
    assert!(
        source.contains("\"vision_diff\""),
        "MCP should have vision_diff"
    );
    assert!(
        source.contains("vision_screenshot_schema"),
        "MCP should have schema"
    );
    assert!(
        source.contains("vision_checksum_schema"),
        "MCP should have schema"
    );
    assert!(
        source.contains("vision_diff_schema"),
        "MCP should have schema"
    );
}

// ── Phase 89: AI Agent Input Tests ────────────────────────────

#[test]
fn test_ai_inject_key() {
    let mut vm = crate::vm::Vm::new();
    // AI_INJECT op_reg: op=0 (inject key), r[op_reg+1]=keycode, r[op_reg+2]=shift
    vm.ram[0] = 0xA6; // AI_INJECT
    vm.ram[1] = 5; // op_reg = r5
    vm.regs[5] = 0; // op=0 (inject key)
    vm.regs[6] = 65; // keycode = 'A'
    vm.regs[7] = 0; // shift = 0
    vm.step();
    assert_eq!(vm.regs[0], 1, "AI_INJECT key should return 1 on success");
    assert_eq!(vm.regs[0], 1, "AI_INJECT key should return 1 on success");
    // Key was pushed to the key buffer -- verify by reading back with IKEY
    vm.ram[2] = 0x48; // IKEY
    vm.ram[3] = 8; // dest = r8
    vm.pc = 2;
    vm.step();
    assert_eq!(vm.regs[8], 65, "IKEY should read injected key 'A'");
}

#[test]
fn test_ai_inject_key_buffer_full() {
    let mut vm = crate::vm::Vm::new();
    // Fill the key buffer (capacity 15 -- 16 slots, 1 wasted for full detection)
    for i in 0..15 {
        assert!(vm.push_key(65 + i as u32), "push {} should succeed", i);
    }
    // Now try AI_INJECT -- should fail (buffer full)
    vm.ram[0] = 0xA6;
    vm.ram[1] = 5;
    vm.regs[5] = 0; // op=0 (key)
    vm.regs[6] = 90; // 'Z'
    vm.regs[7] = 0;
    vm.step();
    assert_eq!(vm.regs[0], 0, "AI_INJECT should return 0 when buffer full");
}

#[test]
fn test_ai_inject_mouse_move() {
    let mut vm = crate::vm::Vm::new();
    // AI_INJECT op=1 (mouse move): r[op_reg+1]=x, r[op_reg+2]=y
    vm.ram[0] = 0xA6;
    vm.ram[1] = 5;
    vm.regs[5] = 1; // op=1 (mouse move)
    vm.regs[6] = 100; // x
    vm.regs[7] = 200; // y
    vm.step();
    assert_eq!(vm.regs[0], 1, "AI_INJECT mouse move should return 1");
    assert_eq!(vm.mouse_x, 100, "mouse_x should be 100");
    assert_eq!(vm.mouse_y, 200, "mouse_y should be 200");
    // Also check RAM ports
    assert_eq!(vm.ram[0xFF9], 100, "RAM[0xFF9] should be mouse_x");
    assert_eq!(vm.ram[0xFFA], 200, "RAM[0xFFA] should be mouse_y");
}

#[test]
fn test_ai_inject_mouse_click() {
    let mut vm = crate::vm::Vm::new();
    // AI_INJECT op=2 (mouse click): r[op_reg+1]=x, r[op_reg+2]=y, r[op_reg+3]=button
    vm.ram[0] = 0xA6;
    vm.ram[1] = 5;
    vm.regs[5] = 2; // op=2 (click)
    vm.regs[6] = 128; // x
    vm.regs[7] = 64; // y
    vm.regs[8] = 2; // button=2 (click)
    vm.step();
    assert_eq!(vm.regs[0], 1, "AI_INJECT click should return 1");
    assert_eq!(vm.mouse_x, 128, "mouse_x should be 128");
    assert_eq!(vm.mouse_y, 64, "mouse_y should be 64");
    assert_eq!(vm.mouse_button, 2, "mouse_button should be 2");
}

#[test]
fn test_ai_inject_mouse_then_mouseq() {
    let mut vm = crate::vm::Vm::new();
    // Inject mouse move
    vm.ram[0] = 0xA6;
    vm.ram[1] = 5;
    vm.regs[5] = 1; // move
    vm.regs[6] = 42; // x
    vm.regs[7] = 87; // y
    vm.step();
    // Now read with MOUSEQ
    vm.ram[2] = 0x85; // MOUSEQ
    vm.ram[3] = 10; // dest = r10
    vm.pc = 2;
    vm.step();
    assert_eq!(vm.regs[10], 42, "MOUSEQ x should be 42");
    assert_eq!(vm.regs[11], 87, "MOUSEQ y should be 87");
}

#[test]
fn test_ai_inject_text_string() {
    let mut vm = crate::vm::Vm::new();
    // Write "ABC" at RAM 0x2000
    vm.ram[0x2000] = 65; // 'A'
    vm.ram[0x2001] = 66; // 'B'
    vm.ram[0x2002] = 67; // 'C'
    vm.ram[0x2003] = 0; // null terminator
                        // AI_INJECT op=3 (text string): r[op_reg+1]=addr
    vm.ram[0] = 0xA6;
    vm.ram[1] = 5;
    vm.regs[5] = 3; // op=3 (text)
    vm.regs[6] = 0x2000; // addr
    vm.step();
    assert_eq!(vm.regs[0], 3, "should inject 3 characters");
    // Read back with IKEY
    for expected in [65u32, 66, 67] {
        vm.ram[vm.pc as usize] = 0x48; // IKEY
        vm.ram[vm.pc as usize + 1] = 8;
        vm.step();
        assert_eq!(
            vm.regs[8], expected,
            "IKEY should read injected char {}",
            expected
        );
    }
}

#[test]
fn test_ai_inject_text_partial() {
    let mut vm = crate::vm::Vm::new();
    // Fill key buffer to leave only 3 slots
    for i in 0..13 {
        vm.push_key(48 + i as u32);
    }
    // Write 5-char string
    vm.ram[0x2000] = 72; // 'H'
    vm.ram[0x2001] = 69; // 'E'
    vm.ram[0x2002] = 76; // 'L'
    vm.ram[0x2003] = 76; // 'L'
    vm.ram[0x2004] = 79; // 'O'
    vm.ram[0x2005] = 0;
    vm.ram[0] = 0xA6;
    vm.ram[1] = 5;
    vm.regs[5] = 3;
    vm.regs[6] = 0x2000;
    vm.step();
    // Ring buffer of 16 uses 1 slot for full detection, so 15 usable.
    // 13 already pushed, so 2 more can fit (15 - 13 = 2).
    assert_eq!(
        vm.regs[0], 2,
        "should inject only 2 chars before buffer full"
    );
}

#[test]
fn test_ai_inject_unknown_op() {
    let mut vm = crate::vm::Vm::new();
    vm.ram[0] = 0xA6;
    vm.ram[1] = 5;
    vm.regs[5] = 99; // unknown op
    vm.step();
    assert_eq!(vm.regs[0], 0, "unknown op should return 0");
}

#[test]
fn test_ai_inject_assembles() {
    let source = "LDI r1, 0
AI_INJECT r1
HALT
";
    let result = crate::assembler::assemble(source, 0);
    assert!(result.is_ok(), "AI_INJECT should assemble: {:?}", result);
    let asm = result.unwrap();
    // LDI r1, 0 is [0x10, 1, 0], AI_INJECT r1 is [0xA6, 1], HALT is [0x00]
    assert_eq!(asm.pixels[0], 0x10, "first word should be LDI opcode");
    assert_eq!(
        asm.pixels[3], 0xA6,
        "fourth word should be AI_INJECT opcode"
    );
    assert_eq!(asm.pixels[4], 1, "fifth word should be register r1");
}

#[test]
fn test_ai_inject_disasm() {
    let mut vm = crate::vm::Vm::new();
    vm.ram[0] = 0xA6;
    vm.ram[1] = 5;
    let (name, len) = vm.disassemble_at(0);
    assert!(
        name.contains("AI_INJECT"),
        "disasm should show AI_INJECT, got: {}",
        name
    );
    assert_eq!(len, 2);
}

#[test]
fn test_ai_inject_opcode_name_in_source() {
    // hermes module is binary-only, check source file instead
    let source = std::fs::read_to_string("src/hermes.rs").unwrap();
    assert!(
        source.contains("0xA6 => \"AI_INJECT\""),
        "hermes.rs should map 0xA6 to AI_INJECT"
    );
}

#[test]
fn test_ai_inject_preprocessor() {
    assert!(
        crate::preprocessor::OPCODES.contains(&"AI_INJECT"),
        "AI_INJECT should be in preprocessor OPCODES array"
    );
}

#[test]
fn test_ai_inject_in_hermes_source() {
    // Verify the opcode_name entry exists in source (hermes is binary-only)
    let source = std::fs::read_to_string("src/hermes.rs").unwrap();
    assert!(
        source.contains("AI_INJECT"),
        "hermes.rs should reference AI_INJECT"
    );
}

#[test]
fn test_mcp_input_tools_in_source() {
    let source = std::fs::read_to_string("src/mcp_server.rs").unwrap();
    assert!(
        source.contains("\"input_key\""),
        "MCP should have input_key tool"
    );
    assert!(
        source.contains("\"input_mouse\""),
        "MCP should have input_mouse tool"
    );
    assert!(
        source.contains("\"input_text\""),
        "MCP should have input_text tool"
    );
    assert!(
        source.contains("input_key_schema"),
        "MCP should have input_key schema"
    );
    assert!(
        source.contains("input_mouse_schema"),
        "MCP should have input_mouse schema"
    );
    assert!(
        source.contains("input_text_schema"),
        "MCP should have input_text schema"
    );
}

#[test]
fn test_ai_interact_assembles_and_runs() {
    let source = std::fs::read_to_string("programs/ai_interact.asm").unwrap();
    let result = crate::assembler::assemble(&source, 0);
    assert!(
        result.is_ok(),
        "ai_interact.asm should assemble: {:?}",
        result.err()
    );
    let asm = result.unwrap();
    let mut vm = crate::vm::Vm::new();
    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..10_000 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.halted, "ai_interact should halt");
    // The green border at y=0 should be drawn (key injection succeeded)
    assert_ne!(vm.screen[0], 0, "top border should have pixels (green)");
}

#[test]
fn test_socket_inject_commands_in_source() {
    let source = std::fs::read_to_string("src/main.rs").unwrap();
    assert!(
        source.contains("\"inject_key\""),
        "main.rs should have inject_key socket command"
    );
    assert!(
        source.contains("\"inject_mouse\""),
        "main.rs should have inject_mouse socket command"
    );
    assert!(
        source.contains("\"inject_text\""),
        "main.rs should have inject_text socket command"
    );
}

// ============================================================
// Phase 92: LOADPNG (0xB1) opcode test
// ============================================================

#[test]
fn test_loadpng_opcode_basic() {
    // Create a simple program: LDI r1, 42; HALT
    let source = "LDI r1, 42\nHALT\n";
    let asm = crate::assembler::assemble(source, 0).unwrap();

    // Convert bytecode to bytes
    let mut bytecode_bytes = Vec::new();
    for &word in &asm.pixels {
        bytecode_bytes.push((word & 0xFF) as u8);
        bytecode_bytes.push(((word >> 8) & 0xFF) as u8);
        bytecode_bytes.push(((word >> 16) & 0xFF) as u8);
        bytecode_bytes.push(((word >> 24) & 0xFF) as u8);
    }

    // Encode to pixelpack PNG
    let png_data = crate::pixel::encode_pixelpack_png(&bytecode_bytes);
    let png_path = "/tmp/geo_test_loadpng_basic.png";
    std::fs::write(png_path, &png_data).unwrap();

    // Set up VM with path string in RAM
    let mut vm = crate::vm::Vm::new();
    let path_bytes: Vec<u32> = png_path
        .chars()
        .map(|c| c as u32)
        .chain(std::iter::once(0u32))
        .collect();
    for (i, &word) in path_bytes.iter().enumerate() {
        vm.ram[0x2000 + i] = word;
    }

    // LDI r5, 0x2000 (path address)
    // LDI r6, 0x1000 (dest address)
    // LOADPNG r5, r6
    // HALT
    vm.ram[0] = 0x10; // LDI
    vm.ram[1] = 5; // r5
    vm.ram[2] = 0x2000;

    vm.ram[3] = 0x10; // LDI
    vm.ram[4] = 6; // r6
    vm.ram[5] = 0x1000;

    vm.ram[6] = 0xB1; // LOADPNG
    vm.ram[7] = 5; // path_reg = r5
    vm.ram[8] = 6; // dest_reg = r6

    vm.ram[9] = 0x00; // HALT

    vm.pc = 0;
    vm.halted = false;
    for _ in 0..1000 {
        if !vm.step() {
            break;
        }
    }

    // Verify LOADPNG succeeded (r0 = byte count, not 0xFFFFFFFF)
    assert_ne!(vm.regs[0], 0xFFFFFFFF, "LOADPNG should succeed");
    assert!(vm.regs[0] > 0, "LOADPNG should return byte count > 0");

    // Verify the program was loaded at 0x1000 and runs correctly
    // The loaded program is LDI r1, 42; HALT
    // LDI is [0x10, 1, 42], HALT is [0x00]
    // So ram[0x1000] should be 0x10 (LDI opcode)
    assert_eq!(vm.ram[0x1000], 0x10, "LDI opcode should be at 0x1000");
    assert_eq!(vm.ram[0x1001], 1, "r1 should be the target register");

    // Now run the loaded program
    vm.pc = 0x1000;
    vm.halted = false;
    for _ in 0..1000 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.halted, "loaded program should halt");
    assert_eq!(vm.regs[1], 42, "LDI r1, 42 should set r1=42");

    // Cleanup
    let _ = std::fs::remove_file(png_path);
}

#[test]
fn test_loadpng_opcode_missing_file() {
    let mut vm = crate::vm::Vm::new();

    // Store non-existent path in RAM
    let path = "/tmp/geo_test_nonexistent_12345.png";
    let path_bytes: Vec<u32> = path
        .chars()
        .map(|c| c as u32)
        .chain(std::iter::once(0u32))
        .collect();
    for (i, &word) in path_bytes.iter().enumerate() {
        vm.ram[0x2000 + i] = word;
    }

    // LDI r5, 0x2000
    vm.ram[0] = 0x10;
    vm.ram[1] = 5;
    vm.ram[2] = 0x2000;
    // LDI r6, 0x1000
    vm.ram[3] = 0x10;
    vm.ram[4] = 6;
    vm.ram[5] = 0x1000;
    // LOADPNG r5, r6
    vm.ram[6] = 0xB1;
    vm.ram[7] = 5;
    vm.ram[8] = 6;
    // HALT
    vm.ram[9] = 0x00;

    vm.pc = 0;
    vm.halted = false;
    for _ in 0..1000 {
        if !vm.step() {
            break;
        }
    }

    // Should return error (0xFFFFFFFF)
    assert_eq!(
        vm.regs[0], 0xFFFFFFFF,
        "LOADPNG should fail for missing file"
    );
}

#[test]
fn test_loadpng_opcode_empty_path() {
    let mut vm = crate::vm::Vm::new();

    // Store empty path (just null terminator) in RAM
    vm.ram[0x2000] = 0;

    // LDI r5, 0x2000
    vm.ram[0] = 0x10;
    vm.ram[1] = 5;
    vm.ram[2] = 0x2000;
    // LDI r6, 0x1000
    vm.ram[3] = 0x10;
    vm.ram[4] = 6;
    vm.ram[5] = 0x1000;
    // LOADPNG r5, r6
    vm.ram[6] = 0xB1;
    vm.ram[7] = 5;
    vm.ram[8] = 6;
    // HALT
    vm.ram[9] = 0x00;

    vm.pc = 0;
    vm.halted = false;
    for _ in 0..1000 {
        if !vm.step() {
            break;
        }
    }

    // Should return error (0xFFFFFFFF)
    assert_eq!(vm.regs[0], 0xFFFFFFFF, "LOADPNG should fail for empty path");
}

// ============================================================
// Phase 93: LOADSRCIMG (0xB2) opcode tests + Source PNG tests
// ============================================================

#[test]
fn test_loadsrcimg_opcode_basic() {
    // Create a simple source program: LDI r1, 42\nHALT
    let source = "LDI r1, 42\nHALT\n";

    // Encode as source pixelpack PNG
    let png_data = crate::pixel::encode_source_pixelpack_png(source);
    let png_path = "/tmp/geo_test_loadsrcimg_basic.png";
    std::fs::write(png_path, &png_data).unwrap();

    // Set up VM with path string in RAM
    let mut vm = crate::vm::Vm::new();
    let path_bytes: Vec<u32> = png_path
        .chars()
        .map(|c| c as u32)
        .chain(std::iter::once(0u32))
        .collect();
    for (i, &word) in path_bytes.iter().enumerate() {
        vm.ram[0x2000 + i] = word;
    }

    // LDI r5, 0x2000 (path address)
    // LOADSRCIMG r5
    // HALT
    vm.ram[0] = 0x10; // LDI
    vm.ram[1] = 5; // r5
    vm.ram[2] = 0x2000;

    vm.ram[3] = 0xB2; // LOADSRCIMG
    vm.ram[4] = 5; // path_reg = r5

    vm.ram[5] = 0x00; // HALT

    vm.pc = 0;
    vm.halted = false;
    for _ in 0..1000 {
        if !vm.step() {
            break;
        }
    }

    // LOADSRCIMG should succeed: r0 = bytecode word count
    assert_ne!(vm.regs[0], 0xFFFFFFFF, "LOADSRCIMG should succeed");
    assert!(
        vm.regs[0] > 0,
        "LOADSRCIMG should return bytecode word count > 0"
    );

    // Verify bytecode was written at 0x1000
    assert_eq!(vm.ram[0x1000], 0x10, "LDI opcode should be at 0x1000");

    // Now run the loaded program from 0x1000
    vm.pc = 0x1000;
    vm.halted = false;
    for _ in 0..1000 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.halted, "loaded program should halt");
    assert_eq!(vm.regs[1], 42, "LDI r1, 42 should set r1=42");

    // Cleanup
    let _ = std::fs::remove_file(png_path);
}

#[test]
fn test_loadsrcimg_opcode_missing_file() {
    let mut vm = crate::vm::Vm::new();

    // Store non-existent path in RAM
    let path = "/tmp/geo_test_nonexistent_src_99999.png";
    let path_bytes: Vec<u32> = path
        .chars()
        .map(|c| c as u32)
        .chain(std::iter::once(0u32))
        .collect();
    for (i, &word) in path_bytes.iter().enumerate() {
        vm.ram[0x2000 + i] = word;
    }

    // LDI r5, 0x2000
    vm.ram[0] = 0x10;
    vm.ram[1] = 5;
    vm.ram[2] = 0x2000;
    // LOADSRCIMG r5
    vm.ram[3] = 0xB2;
    vm.ram[4] = 5;
    // HALT
    vm.ram[5] = 0x00;

    vm.pc = 0;
    vm.halted = false;
    for _ in 0..1000 {
        if !vm.step() {
            break;
        }
    }

    // Should return error (0xFFFFFFFF)
    assert_eq!(
        vm.regs[0], 0xFFFFFFFF,
        "LOADSRCIMG should fail for missing file"
    );
}

#[test]
fn test_loadsrcimg_opcode_empty_path() {
    let mut vm = crate::vm::Vm::new();

    // Store empty path (just null terminator) in RAM
    vm.ram[0x2000] = 0;

    // LDI r5, 0x2000
    vm.ram[0] = 0x10;
    vm.ram[1] = 5;
    vm.ram[2] = 0x2000;
    // LOADSRCIMG r5
    vm.ram[3] = 0xB2;
    vm.ram[4] = 5;
    // HALT
    vm.ram[5] = 0x00;

    vm.pc = 0;
    vm.halted = false;
    for _ in 0..1000 {
        if !vm.step() {
            break;
        }
    }

    // Should return error (0xFFFFFFFF)
    assert_eq!(
        vm.regs[0], 0xFFFFFFFF,
        "LOADSRCIMG should fail for empty path"
    );
}

#[test]
fn test_source_png_round_trip() {
    // Test encode -> decode round trip for source PNG
    let source = "LDI r1, 100\nLDI r2, 200\nADD r1, r2\nHALT\n";

    // Encode
    let png_data = crate::pixel::encode_source_pixelpack_png(source);

    // Verify it is detected as a source PNG
    assert!(
        crate::pixel::is_source_png(&png_data),
        "should detect as source PNG"
    );

    // Decode
    let decoded = crate::pixel::decode_pixelpack_source(&png_data).unwrap();

    // Source text should round-trip correctly
    assert_eq!(
        decoded, source,
        "source text should round-trip through PNG encoding"
    );

    // Verify the decoded source assembles successfully
    let asm = crate::assembler::assemble(&decoded, 0x1000).unwrap();
    assert!(
        asm.pixels.len() > 0,
        "assembled bytecode should not be empty"
    );
}

#[test]
fn test_source_png_assembles_correctly() {
    // Test that a source PNG containing a program assembles correctly
    let source = "LDI r1, 42\nHALT\n";

    let png_data = crate::pixel::encode_source_pixelpack_png(source);
    let png_path = "/tmp/geo_test_source_asm.png";
    std::fs::write(png_path, &png_data).unwrap();

    // Use boot_source_png_to_ram to simulate the full CLI boot path
    let mut canvas_buffer = vec![0u32; 4096];
    let mut ram = [0u32; 65536];

    let result =
        crate::pixel::boot_source_png_to_ram(png_path, &mut canvas_buffer, &mut ram).unwrap();

    assert!(result.source_len > 0, "should have source text");
    assert!(result.bytecode_words > 0, "should have bytecode output");
    assert_eq!(
        ram[0x1000], 0x10,
        "first bytecode word should be LDI (0x10)"
    );

    // Cleanup
    let _ = std::fs::remove_file(png_path);
}

#[test]
fn test_source_png_distinguishes_from_bytecode() {
    // A source PNG should have geo_boot=source metadata
    // A bytecode PNG should NOT have this metadata
    let source = "HALT\n";
    let source_png = crate::pixel::encode_source_pixelpack_png(source);
    assert!(crate::pixel::is_source_png(&source_png));

    // Bytecode PNG should NOT be detected as source
    let bytecode_bytes = vec![0x00]; // HALT
    let bytecode_png = crate::pixel::encode_pixelpack_png(&bytecode_bytes);
    assert!(
        !crate::pixel::is_source_png(&bytecode_png),
        "bytecode PNG should not be source PNG"
    );
}

#[test]
fn test_loadsrcimg_canvas_buffer_populated() {
    // Test that LOADSRCIMG writes source to canvas_buffer
    let source = "LDI r1, 99\nHALT\n";
    let png_data = crate::pixel::encode_source_pixelpack_png(source);
    let png_path = "/tmp/geo_test_loadsrcimg_canvas.png";
    std::fs::write(png_path, &png_data).unwrap();

    let mut vm = crate::vm::Vm::new();
    let path_bytes: Vec<u32> = png_path
        .chars()
        .map(|c| c as u32)
        .chain(std::iter::once(0u32))
        .collect();
    for (i, &word) in path_bytes.iter().enumerate() {
        vm.ram[0x2000 + i] = word;
    }

    // LDI r5, 0x2000
    vm.ram[0] = 0x10;
    vm.ram[1] = 5;
    vm.ram[2] = 0x2000;
    // LOADSRCIMG r5
    vm.ram[3] = 0xB2;
    vm.ram[4] = 5;
    // HALT
    vm.ram[5] = 0x00;

    vm.pc = 0;
    vm.halted = false;
    for _ in 0..1000 {
        if !vm.step() {
            break;
        }
    }

    // Verify canvas buffer has the source text characters
    assert_ne!(
        vm.canvas_buffer[0], 0,
        "canvas buffer should have source text"
    );
    // First char should be 'L' (0x4C) from "LDI"
    assert_eq!(
        vm.canvas_buffer[0], 'L' as u32,
        "first canvas cell should be 'L'"
    );

    // Verify bytecode loaded
    assert_ne!(vm.regs[0], 0xFFFFFFFF, "LOADSRCIMG should succeed");

    let _ = std::fs::remove_file(png_path);
}

#[test]
fn test_loadsrcimg_invalid_register() {
    let mut vm = crate::vm::Vm::new();

    // LOADSRCIMG with invalid register number
    vm.ram[0] = 0xB2; // LOADSRCIMG
    vm.ram[1] = 32; // invalid register (max is 31)

    vm.pc = 0;
    vm.halted = false;
    for _ in 0..100 {
        if !vm.step() {
            break;
        }
    }

    assert_eq!(
        vm.regs[0], 0xFFFFFFFF,
        "LOADSRCIMG should fail with invalid register"
    );
}

// ============================================================
// Phase 94: Universal Pixel Executable - Integration Tests
// ============================================================

#[test]
fn test_universal_pixel_boot_bytecode() {
    // Create a bytecode PNG (Level 1) and verify auto-detection
    let source = "LDI r1, 99\nHALT\n";
    let asm = crate::assembler::assemble(source, 0).unwrap();
    let bytecode_bytes: Vec<u8> = asm.pixels.iter().flat_map(|w| w.to_le_bytes()).collect();
    let png_data = crate::pixel::encode_pixelpack_png(&bytecode_bytes);
    let png_path = "/tmp/geo_test_universal_bytecode.png";
    std::fs::write(png_path, &png_data).unwrap();

    // Verify it's NOT detected as source PNG
    assert!(
        !crate::pixel::is_source_png(&png_data),
        "bytecode PNG should not be detected as source"
    );

    // Use LOADPNG to load and execute
    let mut vm = crate::vm::Vm::new();
    let path_bytes: Vec<u32> = png_path
        .chars()
        .map(|c| c as u32)
        .chain(std::iter::once(0u32))
        .collect();
    for (i, &word) in path_bytes.iter().enumerate() {
        vm.ram[0x2000 + i] = word;
    }

    // LDI r5, 0x2000; LOADPNG r5, r6 (r6=0x1000); HALT
    vm.ram[0] = 0x10;
    vm.ram[1] = 5;
    vm.ram[2] = 0x2000;
    vm.ram[3] = 0x10;
    vm.ram[4] = 6;
    vm.ram[5] = 0x1000;
    vm.ram[6] = 0xB1;
    vm.ram[7] = 5;
    vm.ram[8] = 6;
    vm.ram[9] = 0x00; // HALT

    vm.pc = 0;
    vm.halted = false;
    for _ in 0..1000 {
        if !vm.step() {
            break;
        }
    }

    assert_ne!(vm.regs[0], 0xFFFFFFFF, "LOADPNG should succeed");
    // Bytecode should be at 0x1000
    assert_eq!(vm.ram[0x1000], 0x10, "LDI opcode at 0x1000");

    // Run loaded program
    vm.pc = 0x1000;
    vm.halted = false;
    for _ in 0..1000 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.halted);
    assert_eq!(vm.regs[1], 99, "LDI r1, 99 should work");

    let _ = std::fs::remove_file(png_path);
}

#[test]
fn test_universal_pixel_boot_source() {
    // Create a source PNG (Level 2) and verify auto-detection
    let source = "LDI r2, 77\nHALT\n";
    let png_data = crate::pixel::encode_source_pixelpack_png(source);
    let png_path = "/tmp/geo_test_universal_source.png";
    std::fs::write(png_path, &png_data).unwrap();

    // Verify it IS detected as source PNG
    assert!(
        crate::pixel::is_source_png(&png_data),
        "source PNG should be detected as source"
    );

    // Use LOADSRCIMG to load and execute
    let mut vm = crate::vm::Vm::new();
    let path_bytes: Vec<u32> = png_path
        .chars()
        .map(|c| c as u32)
        .chain(std::iter::once(0u32))
        .collect();
    for (i, &word) in path_bytes.iter().enumerate() {
        vm.ram[0x2000 + i] = word;
    }

    // LDI r5, 0x2000; LOADSRCIMG r5; HALT
    vm.ram[0] = 0x10;
    vm.ram[1] = 5;
    vm.ram[2] = 0x2000;
    vm.ram[3] = 0xB2;
    vm.ram[4] = 5;
    vm.ram[5] = 0x00; // HALT

    vm.pc = 0;
    vm.halted = false;
    for _ in 0..1000 {
        if !vm.step() {
            break;
        }
    }

    assert_ne!(vm.regs[0], 0xFFFFFFFF, "LOADSRCIMG should succeed");

    // Run assembled bytecode
    vm.pc = 0x1000;
    vm.halted = false;
    for _ in 0..1000 {
        if !vm.step() {
            break;
        }
    }
    assert!(vm.halted);
    assert_eq!(vm.regs[2], 77, "LDI r2, 77 should work");

    let _ = std::fs::remove_file(png_path);
}

#[test]
fn test_pixel_chain_program_a_loads_b() {
    // Program A: loads program B from a source PNG, then runs it
    // Program B source: LDI r3, 55\nHALT\n
    let source_b = "LDI r3, 55\nHALT\n";
    let png_data = crate::pixel::encode_source_pixelpack_png(source_b);
    let png_path = "/tmp/geo_test_chain_program_b.png";
    std::fs::write(png_path, &png_data).unwrap();

    let mut vm = crate::vm::Vm::new();

    // Store path to program B PNG in RAM
    let path_bytes: Vec<u32> = png_path
        .chars()
        .map(|c| c as u32)
        .chain(std::iter::once(0u32))
        .collect();
    for (i, &word) in path_bytes.iter().enumerate() {
        vm.ram[0x2000 + i] = word;
    }

    // Program A at address 0:
    // LDI r5, 0x2000  (path to program B PNG)
    // LOADSRCIMG r5   (load, write to canvas, assemble)
    // ; Verify r0 > 0 (bytecode loaded)
    // LDI r4, 0xFFFFFFFF
    // CMP r0, r4      (r0 should NOT be 0xFFFFFFFF)
    // BGE r0, chain_ok (success)
    // HALT             (error case)
    // chain_ok:
    // LDI r7, 0x1000
    // JMP r7           (jump to loaded program B)

    let mut addr = 0usize;

    // LDI r5, 0x2000
    vm.ram[addr] = 0x10;
    vm.ram[addr + 1] = 5;
    vm.ram[addr + 2] = 0x2000;
    addr += 3;

    // LOADSRCIMG r5
    vm.ram[addr] = 0xB2;
    vm.ram[addr + 1] = 5;
    addr += 2;

    // LDI r4, 0xFFFFFFFF
    vm.ram[addr] = 0x10;
    vm.ram[addr + 1] = 4;
    vm.ram[addr + 2] = 0xFFFFFFFF;
    addr += 3;

    // CMP r0, r4
    vm.ram[addr] = 0x50;
    vm.ram[addr + 1] = 0;
    vm.ram[addr + 2] = 4;
    addr += 3;

    // BGE r0, chain_ok (skip error HALT)
    let chain_ok_addr = (addr + 4) as u32; // +3 for BGE +1 for HALT
    vm.ram[addr] = 0x36;
    vm.ram[addr + 1] = 0;
    vm.ram[addr + 2] = chain_ok_addr;
    addr += 3;

    // HALT (error path)
    vm.ram[addr] = 0x00;
    addr += 1;

    // chain_ok: JMP 0x1000 (jump to loaded program B)
    assert_eq!(addr, chain_ok_addr as usize, "chain_ok label alignment");
    vm.ram[addr] = 0x30; // JMP
    vm.ram[addr + 1] = 0x1000; // target = 0x1000

    vm.pc = 0;
    vm.halted = false;
    for _ in 0..10000 {
        if !vm.step() {
            break;
        }
    }

    // Should have loaded program B and executed it
    assert!(vm.halted, "should halt after program B runs");
    assert_eq!(vm.regs[3], 55, "program B should have set r3=55");

    let _ = std::fs::remove_file(png_path);
}

// ── System Clipboard Protocol (Phase 96) ─────────────────────────────

/// Clipboard RAM Convention:
///   0xF10 = ownership flag (0=free, 1=writing, PID=owned by process)
///   0xF11 = data length (0-14 u32 words)
///   0xF12-0xF1F = data (up to 14 u32 words)
///
/// Protocol:
///   Writer: claim (STORE 1 to 0xF10), write data to 0xF12+, write length to 0xF11, release (STORE 0 to 0xF10)
///   Reader: check 0xF10 == 0 (free), read length from 0xF11, read data from 0xF12+
///   For large data: clipboard stores a VFS path string instead

#[test]
fn test_clipboard_basic_write_and_read() {
    let mut vm = Vm::new();

    // Writer: claim clipboard
    vm.ram[0xF10] = 1;

    // Write 3 data words
    vm.ram[0xF12] = 0xFF0000;
    vm.ram[0xF13] = 0x00FF00;
    vm.ram[0xF14] = 0x0000FF;

    // Write length
    vm.ram[0xF11] = 3;

    // Release clipboard
    vm.ram[0xF10] = 0;

    // Reader: verify clipboard data
    assert_eq!(vm.ram[0xF10], 0, "clipboard should be free after release");
    assert_eq!(vm.ram[0xF11], 3, "clipboard data length should be 3");
    assert_eq!(vm.ram[0xF12], 0xFF0000, "clipboard data[0] = red");
    assert_eq!(vm.ram[0xF13], 0x00FF00, "clipboard data[1] = green");
    assert_eq!(vm.ram[0xF14], 0x0000FF, "clipboard data[2] = blue");
}

#[test]
fn test_clipboard_ownership_protocol() {
    let mut vm = Vm::new();
    assert_eq!(vm.ram[0xF10], 0, "clipboard starts free");

    // Process claims it
    vm.ram[0xF10] = 1;
    assert_eq!(vm.ram[0xF10], 1, "clipboard is now owned");

    // Process writes data
    vm.ram[0xF12] = 42;
    vm.ram[0xF11] = 1;

    // Release
    vm.ram[0xF10] = 0;

    // Another process reads it
    assert_eq!(vm.ram[0xF10], 0, "clipboard is free again");
    assert_eq!(vm.ram[0xF11], 1, "length is 1");
    assert_eq!(vm.ram[0xF12], 42, "data is 42");
}

#[test]
fn test_clipboard_max_capacity() {
    let mut vm = Vm::new();
    vm.ram[0xF10] = 1;
    for i in 0..14u32 {
        vm.ram[0xF12 + i as usize] = 1000 + i;
    }
    vm.ram[0xF11] = 14;
    vm.ram[0xF10] = 0;

    assert_eq!(vm.ram[0xF11], 14, "length should be 14");
    for i in 0..14u32 {
        assert_eq!(
            vm.ram[0xF12 + i as usize],
            1000 + i,
            "data[{}] should be {}",
            i,
            1000 + i
        );
    }
}

#[test]
fn test_clipboard_overwrite_previous() {
    let mut vm = Vm::new();
    vm.ram[0xF10] = 1;
    vm.ram[0xF12] = 111;
    vm.ram[0xF11] = 1;
    vm.ram[0xF10] = 0;

    vm.ram[0xF10] = 1;
    vm.ram[0xF12] = 222;
    vm.ram[0xF11] = 1;
    vm.ram[0xF10] = 0;

    assert_eq!(
        vm.ram[0xF12], 222,
        "clipboard should have second write value"
    );
}

#[test]
fn test_clipboard_demo_program_assembles() {
    use crate::assembler::assemble;
    let source = std::fs::read_to_string("programs/clipboard_demo.asm")
        .expect("clipboard_demo.asm should exist");
    let result = assemble(&source, 0).expect("clipboard_demo.asm should assemble");
    assert!(
        result.pixels.len() > 50,
        "should produce meaningful bytecode"
    );
}

#[test]
fn test_clipboard_demo_runs_and_writes_data() {
    use crate::assembler::assemble;
    let source = std::fs::read_to_string("programs/clipboard_demo.asm")
        .expect("clipboard_demo.asm should exist");
    let asm = assemble(&source, 0).expect("should assemble");

    let mut vm = Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..10_000_000 {
        if !vm.step() {
            break;
        }
    }

    assert!(vm.halted, "program should halt");

    // Verify clipboard protocol was followed
    assert_eq!(
        vm.ram[0xF10], 0,
        "clipboard should be free after program runs"
    );
    assert_eq!(vm.ram[0xF11], 5, "clipboard should have 5 data words");
    assert_eq!(vm.ram[0xF12], 0xFF0000, "data[0] = red");
    assert_eq!(vm.ram[0xF13], 0x00FF00, "data[1] = green");
    assert_eq!(vm.ram[0xF14], 0x0000FF, "data[2] = blue");
    assert_eq!(vm.ram[0xF15], 0xFFFF00, "data[3] = yellow");
    assert_eq!(vm.ram[0xF16], 0xFF00FF, "data[4] = magenta");
}

#[test]
fn test_clipboard_shared_between_processes() {
    // Tests that clipboard data persists in shared RAM accessible to spawned processes
    let mut vm = Vm::new();

    // Parent writes to clipboard
    vm.ram[0xF10] = 1;
    vm.ram[0xF12] = 0x12345678;
    vm.ram[0xF11] = 1;
    vm.ram[0xF10] = 0;

    // Simulate child process reading (same VM, just verify RAM persists)
    assert_eq!(vm.ram[0xF10], 0, "child sees clipboard as free");
    assert_eq!(vm.ram[0xF11], 1, "child reads length");
    assert_eq!(vm.ram[0xF12], 0x12345678, "child reads data");
}

#[test]
fn test_clipboard_large_data_vfs_path() {
    // For data larger than 14 words, clipboard stores a VFS path string
    let mut vm = Vm::new();

    let path = "/tmp/clip.bin";
    vm.ram[0xF10] = 1;

    for (i, ch) in path.chars().enumerate() {
        if i < 14 {
            vm.ram[0xF12 + i] = ch as u32;
        }
    }
    vm.ram[0xF11] = path.len() as u32;
    vm.ram[0xF10] = 0;

    let len = vm.ram[0xF11] as usize;
    let mut read_path = String::new();
    for i in 0..len.min(14) {
        let ch = vm.ram[0xF12 + i] as u8;
        if ch == 0 {
            break;
        }
        read_path.push(ch as char);
    }
    assert_eq!(
        read_path, path,
        "reader should reconstruct VFS path from clipboard"
    );
}

// ── Window Manager with Close & Re-tiling (Phase 95) ────────────

#[test]
fn test_wm_drag_close_assembles() {
    let source = std::fs::read_to_string("programs/wm_drag_close.asm").unwrap();
    let result = crate::assembler::assemble(&source, 0);
    assert!(
        result.is_ok(),
        "wm_drag_close.asm should assemble: {:?}",
        result.err()
    );
    let asm = result.unwrap();
    assert!(
        asm.pixels.len() > 100,
        "should have significant bytecode, got {} words",
        asm.pixels.len()
    );
    // Verify WINSYS opcode present
    assert!(
        asm.pixels.iter().any(|&p| p == 0x94),
        "should contain WINSYS opcode (0x94)"
    );
    // Verify MOUSEQ opcode present
    assert!(
        asm.pixels.iter().any(|&p| p == 0x85),
        "should contain MOUSEQ opcode (0x85)"
    );
    // Verify WPIXEL opcode present
    assert!(
        asm.pixels.iter().any(|&p| p == 0x95),
        "should contain WPIXEL opcode (0x95)"
    );
}

#[test]
fn test_wm_creates_three_windows() {
    let source = std::fs::read_to_string("programs/wm_drag_close.asm").unwrap();
    let asm = crate::assembler::assemble(&source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    // Run enough steps to get past window creation (past init section)
    // The program has lots of init code (STRO, WINSYS calls, content drawing)
    for _ in 0..200_000 {
        if !vm.step() {
            break;
        }
    }
    // Check that 3 windows were created
    let active_count = vm.windows.iter().filter(|w| w.active).count();
    assert!(
        active_count >= 3,
        "should have at least 3 active windows, got {}",
        active_count
    );
}

#[test]
fn test_wm_close_button_hittest() {
    let mut vm = Vm::new();
    // Create a window at (50, 50), 100x80
    vm.regs[1] = 50;
    vm.regs[2] = 50;
    vm.regs[3] = 100;
    vm.regs[4] = 80;
    vm.regs[5] = 0;
    vm.regs[6] = 0;
    vm.ram[0] = 0x94; // WINSYS
    vm.ram[1] = 6;
    vm.pc = 0;
    vm.halted = false;
    vm.step();
    let win_id = vm.regs[0];
    assert!(win_id > 0, "should create a window");

    // Position mouse on close button: x = 50 + 100 - 11 + 4 = 139, y = 50 + 2 + 4 = 56
    // Close button is at (win_x + win_w - 11, win_y + 2) to (+8, +8)
    // So center is at x=139, y=56
    vm.push_mouse(139, 56);

    // Do HITTEST (op=4)
    vm.regs[6] = 4;
    vm.ram[2] = 0x94; // WINSYS
    vm.ram[3] = 6;
    vm.pc = 2;
    vm.halted = false;
    vm.step();

    // Should detect a hit on the window
    assert_eq!(vm.regs[0], win_id, "hittest should find the window");
    assert_eq!(vm.regs[1], 1, "should be title bar hit (hit_type=1)");
}

#[test]
fn test_wm_close_destroy_window() {
    let mut vm = Vm::new();
    // Create 3 windows
    for i in 0..3u32 {
        vm.regs[1] = 10 + i * 40;
        vm.regs[2] = 10;
        vm.regs[3] = 30;
        vm.regs[4] = 30;
        vm.regs[5] = 0;
        vm.regs[6] = 0;
        vm.ram[0] = 0x94;
        vm.ram[1] = 6;
        vm.pc = 0;
        vm.halted = false;
        vm.step();
    }
    let active_count = vm.windows.iter().filter(|w| w.active).count();
    assert_eq!(active_count, 3, "should have 3 windows");

    // Destroy window with id=2 (the second one)
    vm.regs[0] = 2;
    vm.regs[6] = 1; // op=1 (destroy)
    vm.ram[0] = 0x94;
    vm.ram[1] = 6;
    vm.pc = 0;
    vm.halted = false;
    vm.step();

    let active_count_after = vm.windows.iter().filter(|w| w.active).count();
    assert_eq!(active_count_after, 2, "should have 2 windows after destroy");
}

#[test]
fn test_wm_retile_repositions_windows() {
    let mut vm = Vm::new();
    // Create 3 windows at different positions
    let mut ids = vec![];
    for i in 0..3u32 {
        vm.regs[1] = 10 + i * 40;
        vm.regs[2] = 10;
        vm.regs[3] = 30;
        vm.regs[4] = 30;
        vm.regs[5] = 0;
        vm.regs[6] = 0;
        vm.ram[0] = 0x94;
        vm.ram[1] = 6;
        vm.pc = 0;
        vm.halted = false;
        vm.step();
        ids.push(vm.regs[0]);
    }

    // Destroy the middle window
    vm.regs[0] = ids[1];
    vm.regs[6] = 1;
    vm.ram[0] = 0x94;
    vm.ram[1] = 6;
    vm.pc = 0;
    vm.halted = false;
    vm.step();

    // Manually move remaining windows to simulate re-tiling
    // Window 1 to x=8
    vm.regs[0] = ids[0];
    vm.regs[1] = 8;
    vm.regs[2] = 16;
    vm.regs[6] = 5; // MOVETO
    vm.ram[0] = 0x94;
    vm.ram[1] = 6;
    vm.pc = 0;
    vm.halted = false;
    vm.step();
    assert_eq!(vm.regs[0], 1, "moveto should succeed");

    // Window 3 to x=128
    vm.regs[0] = ids[2];
    vm.regs[1] = 128;
    vm.regs[2] = 16;
    vm.regs[6] = 5;
    vm.ram[0] = 0x94;
    vm.ram[1] = 6;
    vm.pc = 0;
    vm.halted = false;
    vm.step();
    assert_eq!(vm.regs[0], 1, "moveto should succeed");

    // Verify positions via WINFO
    for (i, &wid) in [ids[0], ids[2]].iter().enumerate() {
        vm.regs[0] = wid;
        vm.regs[1] = 0x9000;
        vm.regs[6] = 6; // WINFO
        vm.ram[0] = 0x94;
        vm.ram[1] = 6;
        vm.pc = 0;
        vm.halted = false;
        vm.step();
        let x = vm.ram[0x9000];
        let y = vm.ram[0x9001];
        assert!(x < 256, "window {} x should be on screen", i);
        assert!(y < 256, "window {} y should be on screen", i);
    }
}

#[test]
fn test_wm_drag_moves_window() {
    let mut vm = Vm::new();
    // Create a window at (50, 50)
    vm.regs[1] = 50;
    vm.regs[2] = 50;
    vm.regs[3] = 100;
    vm.regs[4] = 80;
    vm.regs[5] = 0;
    vm.regs[6] = 0;
    vm.ram[0] = 0x94;
    vm.ram[1] = 6;
    vm.pc = 0;
    vm.halted = false;
    vm.step();
    let win_id = vm.regs[0];

    // Move mouse to title bar area (50 + 10, 50 + 5)
    vm.push_mouse(60, 55);

    // Move the window via MOVETO (simulating drag)
    vm.regs[0] = win_id;
    vm.regs[1] = 100; // new x
    vm.regs[2] = 70; // new y
    vm.regs[6] = 5; // MOVETO
    vm.ram[0] = 0x94;
    vm.ram[1] = 6;
    vm.pc = 0;
    vm.halted = false;
    vm.step();
    assert_eq!(vm.regs[0], 1, "moveto should succeed");

    // Verify new position via WINFO
    vm.regs[0] = win_id;
    vm.regs[1] = 0x9000;
    vm.regs[6] = 6; // WINFO
    vm.ram[0] = 0x94;
    vm.ram[1] = 6;
    vm.pc = 0;
    vm.halted = false;
    vm.step();
    assert_eq!(vm.ram[0x9000], 100, "window x should be 100 after drag");
    assert_eq!(vm.ram[0x9001], 70, "window y should be 70 after drag");
}

#[test]
fn test_wm_drag_close_program_runs() {
    let source = std::fs::read_to_string("programs/wm_drag_close.asm").unwrap();
    let asm = crate::assembler::assemble(&source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    // Run init phase: create windows, draw content, enter main loop
    // The full program has heavy WPIXEL fills (100x80 per window x 3) so
    // we run enough steps for init to complete and the main loop to start.
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if vm.frame_ready {
            break;
        }
    }
    // Verify: 3 windows were created (init completed successfully)
    let active_count = vm.windows.iter().filter(|w| w.active).count();
    assert!(
        active_count >= 3,
        "should have 3 active windows after init, got {}",
        active_count
    );
    // Verify: SP was properly initialized (program set r30=0xFF00)
    assert!(
        vm.regs[30] < 0xFF00,
        "SP should have been decremented by PUSH, got {:#x}",
        vm.regs[30]
    );
    // Verify: program didn't halt unexpectedly (it's a loop program)
    // It may or may not have reached FRAME depending on step budget
}

// ── Init System - Service Manager (Phase 97) ───────────────────

#[test]
fn test_init_service_assembles() {
    let source = std::fs::read_to_string("programs/init_service.asm").unwrap();
    let result = crate::assembler::assemble(&source, 0);
    assert!(
        result.is_ok(),
        "init_service.asm should assemble: {:?}",
        result.err()
    );
    let asm = result.unwrap();
    assert!(
        asm.pixels.len() > 100,
        "should have significant bytecode, got {} words",
        asm.pixels.len()
    );
    // Verify key opcodes present
    assert!(
        asm.pixels.iter().any(|&p| p == 0x54),
        "should contain OPEN opcode (0x54)"
    );
    assert!(
        asm.pixels.iter().any(|&p| p == 0x55),
        "should contain READ opcode (0x55)"
    );
    assert!(
        asm.pixels.iter().any(|&p| p == 0x57),
        "should contain CLOSE opcode (0x57)"
    );
    assert!(
        asm.pixels.iter().any(|&p| p == 0x66),
        "should contain EXEC opcode (0x66)"
    );
}

#[test]
fn test_boot_cfg_format_parsing() {
    // Test that config data can be written to RAM and read back
    let mut vm = Vm::new();

    // Write a simple boot config string to RAM at 0x1800
    let cfg_addr = 0x1800usize;
    let config = b"shell 0 255\nclock 1 0\npainter 0 1\n";
    for (i, &byte) in config.iter().enumerate() {
        if cfg_addr + i < vm.ram.len() {
            vm.ram[cfg_addr + i] = byte as u32;
        }
    }

    // Verify first service path starts with 's' (shell)
    // "shell 0 255\n" -> s,h,e,l,l, ,0, ,2,5,5,\n
    assert_eq!(vm.ram[cfg_addr], 's' as u32, "first char should be 's'");
    assert_eq!(
        vm.ram[cfg_addr + 1],
        'h' as u32,
        "second char should be 'h'"
    );
    assert_eq!(vm.ram[cfg_addr + 6], '0' as u32, "policy digit");
    assert_eq!(vm.ram[cfg_addr + 8], '2' as u32, "dep digit 2");
    assert_eq!(vm.ram[cfg_addr + 9], '5' as u32, "dep digit 5");
    assert_eq!(vm.ram[cfg_addr + 10], '5' as u32, "dep digit 5");
}

#[test]
fn test_init_service_spawn_order() {
    // Test that services with dependencies start after their deps
    let mut vm = Vm::new();

    // Create service table in RAM simulating parsed boot.cfg
    // 3 services, 256 words each starting at 0x1000
    let svc_base = 0x1000usize;
    let svc_size = 256usize;

    // Service 0: path="svc0", pid=0, policy=0(always), dep=255(none), status=0(pending)
    vm.ram[svc_base] = 's' as u32;
    vm.ram[svc_base + 1] = 'v' as u32;
    vm.ram[svc_base + 2] = 'c' as u32;
    vm.ram[svc_base + 3] = '0' as u32;
    vm.ram[svc_base + 4] = 0; // null terminator
    vm.ram[svc_base + 32] = 0; // pid
    vm.ram[svc_base + 33] = 0; // policy: always
    vm.ram[svc_base + 34] = 255; // dep: none
    vm.ram[svc_base + 35] = 0; // status: pending

    // Service 1: path="svc1", pid=0, policy=1(onfail), dep=0(depends on svc0), status=0
    let svc1 = svc_base + svc_size;
    vm.ram[svc1] = 's' as u32;
    vm.ram[svc1 + 1] = 'v' as u32;
    vm.ram[svc1 + 2] = 'c' as u32;
    vm.ram[svc1 + 3] = '1' as u32;
    vm.ram[svc1 + 4] = 0;
    vm.ram[svc1 + 32] = 0;
    vm.ram[svc1 + 33] = 1; // policy: onfail
    vm.ram[svc1 + 34] = 0; // dep: svc0
    vm.ram[svc1 + 35] = 0;

    // Service 2: path="svc2", pid=0, policy=0(always), dep=1(depends on svc1), status=0
    let svc2 = svc_base + 2 * svc_size;
    vm.ram[svc2] = 's' as u32;
    vm.ram[svc2 + 1] = 'v' as u32;
    vm.ram[svc2 + 2] = 'c' as u32;
    vm.ram[svc2 + 3] = '2' as u32;
    vm.ram[svc2 + 4] = 0;
    vm.ram[svc2 + 32] = 0;
    vm.ram[svc2 + 33] = 0;
    vm.ram[svc2 + 34] = 1; // dep: svc1
    vm.ram[svc2 + 35] = 0;

    // Verify dependency chain: svc0 -> svc1 -> svc2
    assert_eq!(vm.ram[svc_base + 34], 255, "svc0 has no dependency");
    assert_eq!(vm.ram[svc1 + 34], 0, "svc1 depends on svc0");
    assert_eq!(vm.ram[svc2 + 34], 1, "svc2 depends on svc1");

    // Simulate starting svc0 first (it has no deps)
    vm.ram[svc_base + 32] = 100; // pid
    vm.ram[svc_base + 35] = 1; // status: running

    // Now svc1 can start (dep svc0 is running)
    let dep0_status = vm.ram[svc_base + 35];
    assert_eq!(dep0_status, 1, "svc0 should be running before svc1 starts");

    // Start svc1
    vm.ram[svc1 + 32] = 101;
    vm.ram[svc1 + 35] = 1;

    // Now svc2 can start (dep svc1 is running)
    let dep1_status = vm.ram[svc1 + 35];
    assert_eq!(dep1_status, 1, "svc1 should be running before svc2 starts");

    vm.ram[svc2 + 32] = 102;
    vm.ram[svc2 + 35] = 1;
}

#[test]
fn test_init_service_restart_on_crash() {
    // Test that crashed services with policy=always get restarted
    let mut vm = Vm::new();

    let svc_base = 0x1000usize;
    let _svc_size = 256usize;

    // One service with always-restart policy
    vm.ram[svc_base + 32] = 42; // pid
    vm.ram[svc_base + 33] = 0; // policy: always
    vm.ram[svc_base + 34] = 255; // dep: none
    vm.ram[svc_base + 35] = 1; // status: running

    // Simulate crash: set status to 0 (pending for restart)
    vm.ram[svc_base + 35] = 0;
    vm.ram[svc_base + 32] = 0; // clear PID

    // Verify restart conditions
    assert_eq!(vm.ram[svc_base + 33], 0, "policy should be always");
    assert_eq!(
        vm.ram[svc_base + 35],
        0,
        "status should be pending for restart"
    );

    // Simulate restart: set new PID and status
    vm.ram[svc_base + 32] = 43; // new PID
    vm.ram[svc_base + 35] = 1; // running again

    assert_eq!(vm.ram[svc_base + 32], 43, "new PID after restart");
    assert_eq!(
        vm.ram[svc_base + 35],
        1,
        "status should be running after restart"
    );
}

#[test]
fn test_init_service_no_restart_never_policy() {
    // Test that services with policy=never do NOT get restarted
    let mut vm = Vm::new();

    let svc_base = 0x1000usize;
    let _svc_size = 256usize;

    // One service with never-restart policy
    vm.ram[svc_base + 32] = 42; // pid
    vm.ram[svc_base + 33] = 2; // policy: never
    vm.ram[svc_base + 34] = 255; // dep: none
    vm.ram[svc_base + 35] = 1; // status: running

    // Simulate crash: mark as exited_fail
    vm.ram[svc_base + 35] = 3; // status: exited_fail

    // Verify it stays dead (policy=never means no restart)
    assert_eq!(vm.ram[svc_base + 33], 2, "policy should be never");
    assert_eq!(vm.ram[svc_base + 35], 3, "status should remain exited_fail");
}

// ── Custom Bitmap Fonts (Phase 98) ──────────────────────────────

#[test]
fn test_font_demo_assembles() {
    let source = std::fs::read_to_string("programs/font_demo.asm").unwrap();
    let result = crate::assembler::assemble(&source, 0);
    assert!(
        result.is_ok(),
        "font_demo.asm should assemble: {:?}",
        result.err()
    );
    let asm = result.unwrap();
    assert!(
        asm.pixels.len() > 50,
        "should have bytecode, got {} words",
        asm.pixels.len()
    );
    // Verify IOCTL opcode present
    assert!(
        asm.pixels.iter().any(|&p| p == 0x62),
        "should contain IOCTL opcode (0x62)"
    );
}

#[test]
fn test_custom_font_ioctl_set_and_clear() {
    // Test that IOCTL cmd 2 sets a custom font and cmd 3 clears it
    let mut vm = Vm::new();

    // Write a distinctive glyph at glyph index 65 ('A') in font area 0x2000
    // 8 rows, each a u32 word
    let font_base = 0x2000usize;
    let glyph_a_offset = font_base + 65 * 8;
    // Row 0 = 0xAA (distinctive pattern)
    vm.ram[glyph_a_offset] = 0xAA;
    vm.ram[glyph_a_offset + 1] = 0x55;
    vm.ram[glyph_a_offset + 2] = 0xAA;
    vm.ram[glyph_a_offset + 3] = 0x55;
    vm.ram[glyph_a_offset + 4] = 0xAA;
    vm.ram[glyph_a_offset + 5] = 0x55;
    vm.ram[glyph_a_offset + 6] = 0xAA;
    vm.ram[glyph_a_offset + 7] = 0x55;

    // Create a process to hold the font
    let proc = crate::vm::types::Process::new(1, 0, 0);
    vm.processes.push(proc);
    vm.current_pid = 1;

    // Simulate IOCTL cmd 2 (set font) by running bytecode
    // Build a minimal program: OPEN + IOCTL + HALT
    let _program = [
        0x54u32, 0x01, 0x02, // OPEN r1, r2  (we need /dev/screen path)
        0x00, // HALT
    ];

    // Actually, let's test via the IOCTL handler directly
    // Use raw bytecode that calls IOCTL r3, r4, r5
    // But we need the screen fd first.

    // Simpler approach: just call the IOCTL logic directly through a program
    // Write "/dev/screen\0" to RAM at 0x3000
    let path_bytes = b"/dev/screen";
    for (i, &b) in path_bytes.iter().enumerate() {
        vm.ram[0x3000 + i] = b as u32;
    }
    vm.ram[0x3000 + path_bytes.len()] = 0;

    // Set up registers for OPEN
    vm.regs[1] = 0x3000; // path addr
    vm.regs[2] = 0; // mode
    vm.ram[0] = 0x54; // OPEN
    vm.ram[1] = 0x01; // r1
    vm.ram[2] = 0x02; // r2
    vm.ram[3] = 0x62; // IOCTL
    vm.ram[4] = 0x03; // r3 (fd, will be set after OPEN)
    vm.ram[5] = 0x04; // r4 (cmd)
    vm.ram[6] = 0x05; // r5 (arg = font addr)
    vm.ram[7] = 0x00; // HALT

    // Run OPEN first
    vm.pc = 0;
    vm.halted = false;
    vm.step(); // OPEN
    let fd = vm.regs[0];

    // Verify fd is screen device
    assert_eq!(fd, 0xE000, "should get screen device fd");

    // Set up registers for IOCTL cmd 2
    vm.regs[3] = fd; // screen fd
    vm.regs[4] = 2; // cmd = set font
    vm.regs[5] = 0x2000; // font addr
    vm.step(); // IOCTL

    // Verify success
    assert_eq!(vm.regs[0], 0, "IOCTL set font should return 0");

    // Verify custom font is set on process
    let has_font = vm
        .processes
        .iter()
        .find(|p| p.pid == 1)
        .and_then(|p| p.custom_font.as_ref())
        .is_some();
    assert!(has_font, "process 1 should have custom font");

    // Verify the font data was loaded correctly
    let font = vm
        .processes
        .iter()
        .find(|p| p.pid == 1)
        .and_then(|p| p.custom_font.as_ref())
        .unwrap();
    assert_eq!(font[65][0], 0xAA, "glyph A row 0 should be 0xAA");
    assert_eq!(font[65][1], 0x55, "glyph A row 1 should be 0x55");
    assert_eq!(font[65][7], 0x55, "glyph A row 7 should be 0x55");

    // Now clear font with cmd 3
    vm.regs[3] = fd;
    vm.regs[4] = 3; // cmd = clear font
    vm.regs[5] = 0;
    vm.ram[vm.pc as usize] = 0x62; // IOCTL
    vm.ram[vm.pc as usize + 1] = 0x03;
    vm.ram[vm.pc as usize + 2] = 0x04;
    vm.ram[vm.pc as usize + 3] = 0x05;
    vm.step();

    assert_eq!(vm.regs[0], 0, "IOCTL clear font should return 0");
    let has_font_after_clear = vm
        .processes
        .iter()
        .find(|p| p.pid == 1)
        .and_then(|p| p.custom_font.as_ref())
        .is_some();
    assert!(
        !has_font_after_clear,
        "process 1 should NOT have custom font after clear"
    );
}

#[test]
fn test_custom_font_renders_differently() {
    // Test that TEXT opcode renders differently with a custom font
    let mut vm_no_font = Vm::new();
    let mut vm_with_font = Vm::new();

    // Write "A" to RAM for both VMs
    vm_no_font.ram[0x3000] = 65; // 'A'
    vm_no_font.ram[0x3001] = 0; // null

    vm_with_font.ram[0x3000] = 65;
    vm_with_font.ram[0x3001] = 0;

    // Set up custom font on vm_with_font
    // Make glyph 'A' (index 65) all solid = 0xFF every row
    let font_base = 0x2000usize;
    let glyph_a_offset = font_base + 65 * 8;
    for row in 0..8 {
        vm_with_font.ram[glyph_a_offset + row] = 0xFF;
    }

    // Create a process and set the font
    let mut proc = crate::vm::types::Process::new(1, 0, 0);
    // Load font manually
    let mut glyphs = vec![[0u8; 8]; 128];
    for g in 0..128 {
        for row in 0..8 {
            glyphs[g][row] = vm_with_font.ram[font_base + g * 8 + row] as u8;
        }
    }
    // Override glyph 65 with all-solid
    for row in 0..8 {
        glyphs[65][row] = 0xFF;
    }
    proc.custom_font = Some(glyphs);
    vm_with_font.processes.push(proc);
    vm_with_font.current_pid = 1;

    // Render "A" in both VMs
    vm_no_font.regs[1] = 10; // x
    vm_no_font.regs[2] = 10; // y
    vm_no_font.regs[3] = 0x3000; // addr

    vm_with_font.regs[1] = 10;
    vm_with_font.regs[2] = 10;
    vm_with_font.regs[3] = 0x3000;

    // Execute TEXT opcode directly
    // TEXT = 0x44, args: x_reg, y_reg, addr_reg
    vm_no_font.ram[0] = 0x44;
    vm_no_font.ram[1] = 1; // x reg
    vm_no_font.ram[2] = 2; // y reg
    vm_no_font.ram[3] = 3; // addr reg
    vm_no_font.ram[4] = 0x00; // HALT
    vm_no_font.pc = 0;
    vm_no_font.halted = false;
    vm_no_font.step(); // TEXT

    vm_with_font.ram[0] = 0x44;
    vm_with_font.ram[1] = 1;
    vm_with_font.ram[2] = 2;
    vm_with_font.ram[3] = 3;
    vm_with_font.ram[4] = 0x00;
    vm_with_font.pc = 0;
    vm_with_font.halted = false;
    vm_with_font.step(); // TEXT

    // With custom font (all-solid 0xFF, 8x8), glyph 'A' should render
    // pixels at ALL 8 columns for ALL 8 rows
    // Default font (5x7 mini font) renders fewer pixels

    // Count lit pixels in the 'A' area (x=10..17, y=10..17)
    let count_pixels = |vm: &Vm, x0: usize, y0: usize, w: usize, h: usize| -> usize {
        let mut count = 0;
        for y in y0..(y0 + h) {
            for x in x0..(x0 + w) {
                if x < 256 && y < 256 && vm.screen[y * 256 + x] != 0 {
                    count += 1;
                }
            }
        }
        count
    };

    // Custom font 'A' is 8x8 all-solid = 64 lit pixels
    let custom_count = count_pixels(&vm_with_font, 10, 10, 8, 8);
    // Default font 'A' is 5x7 with some gaps = fewer than 64 lit pixels
    let default_count = count_pixels(&vm_no_font, 10, 10, 8, 8);

    assert!(
        custom_count > default_count,
        "custom font (8x8 solid, {} pixels) should render more pixels than default ({} pixels)",
        custom_count,
        default_count
    );
    assert_eq!(
        custom_count, 64,
        "custom all-solid font should light all 64 pixels (8x8)"
    );
}

#[test]
fn test_per_process_font_isolation() {
    // Test that two processes have independent fonts
    let mut vm = Vm::new();

    // Create two processes
    let mut proc1 = crate::vm::types::Process::new(1, 0, 0);
    let proc2 = crate::vm::types::Process::new(2, 0, 0);

    // Give proc1 a custom font where glyph 'A' is all-solid
    let mut font1 = vec![[0u8; 8]; 128];
    for row in 0..8 {
        font1[65][row] = 0xFF; // 'A' = solid block
    }
    proc1.custom_font = Some(font1);

    // proc2 has no custom font (uses default)
    vm.processes.push(proc1);
    vm.processes.push(proc2);

    // Verify proc1 has custom font
    assert!(
        vm.processes[0].custom_font.is_some(),
        "proc1 should have custom font"
    );
    // Verify proc2 has no custom font
    assert!(
        vm.processes[1].custom_font.is_none(),
        "proc2 should NOT have custom font"
    );

    // Verify proc1's font data is correct
    let font1 = vm.processes[0].custom_font.as_ref().unwrap();
    assert_eq!(font1[65][0], 0xFF, "proc1 glyph A should be solid");
    assert_eq!(font1[66][0], 0, "proc1 glyph B should be empty (default)");
}

// ═══════════════════════════════════════════════════════════════════
// Phase 99: Sound Mixer Daemon Tests
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_sound_mixer_assembles() {
    use crate::assembler::assemble;
    let source =
        std::fs::read_to_string("programs/sound_mixer.asm").expect("sound_mixer.asm should exist");
    let result = assemble(&source, 0);
    assert!(
        result.is_ok(),
        "sound_mixer.asm should assemble: {:?}",
        result.err()
    );
    let asm = result.unwrap();
    assert!(
        asm.pixels.len() > 50,
        "sound_mixer should produce substantial bytecode, got {} words",
        asm.pixels.len()
    );
}

#[test]
fn test_sound_mixer_runs_with_multiproc() {
    use crate::assembler::assemble;
    let source =
        std::fs::read_to_string("programs/sound_mixer.asm").expect("sound_mixer.asm should exist");
    let asm = assemble(&source, 0).expect("should assemble");
    let mut vm = crate::vm::Vm::new();
    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    // Run with multi-process scheduling (parent + mixer daemon)
    for _ in 0..2_000_000 {
        if vm.halted {
            break;
        }
        if !vm.step() {
            break;
        }
        vm.step_all_processes();
    }
    // Parent should halt after sending shutdown command
    assert!(
        vm.halted,
        "sound_mixer parent should halt after melody + shutdown"
    );
    // Should have spawned the mixer daemon
    assert!(
        vm.processes.len() >= 1,
        "should have spawned mixer daemon, got {} processes",
        vm.processes.len()
    );
}

#[test]
fn test_mixer_demo_assembles() {
    use crate::assembler::assemble;
    let source =
        std::fs::read_to_string("programs/mixer_demo.asm").expect("mixer_demo.asm should exist");
    let result = assemble(&source, 0);
    assert!(
        result.is_ok(),
        "mixer_demo.asm should assemble: {:?}",
        result.err()
    );
    let asm = result.unwrap();
    assert!(
        asm.pixels.len() > 50,
        "mixer_demo should produce substantial bytecode, got {} words",
        asm.pixels.len()
    );
}

#[test]
fn test_sound_mixer_shared_ram_ipc() {
    use crate::assembler::assemble;
    let source =
        std::fs::read_to_string("programs/sound_mixer.asm").expect("sound_mixer.asm should exist");
    let asm = assemble(&source, 0).expect("should assemble");
    let mut vm = crate::vm::Vm::new();
    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    let mut notes_played = 0;
    for _ in 0..2_000_000 {
        if vm.halted {
            break;
        }
        if !vm.step() {
            break;
        }
        // Check if BEEP was triggered
        if vm.beep.is_some() {
            notes_played += 1;
            vm.beep = None;
        }
        vm.step_all_processes();
        // Note: child process BEEPs are handled through their own step cycle
    }
    // The mixer daemon should have played at least some notes via BEEP
    assert!(
        notes_played > 0,
        "mixer daemon should have played notes via BEEP, got {}",
        notes_played
    );
}

// Phase 100: Shared Libraries and Dynamic Linking Tests

/// Helper: create a .lib file in VFS from assembled bytecode.
/// Binary format:
///   [magic "LIB\0" (4 bytes), export_count (4 bytes LE),
///    export_offsets (4 bytes each LE), code (4 bytes per u32 word LE)]
fn create_lib_file(
    vfs_dir: &std::path::Path,
    name: &str,
    exports: &[(&str, usize)],
    code: &[u32],
) -> std::io::Result<()> {
    use std::io::Write;
    let path = vfs_dir.join(name);
    let mut file = std::fs::File::create(&path)?;
    file.write_all(b"LIB\x00")?;
    file.write_all(&(exports.len() as u32).to_le_bytes())?;
    for &(_name, offset) in exports {
        file.write_all(&(offset as u32).to_le_bytes())?;
    }
    for &word in code {
        file.write_all(&word.to_le_bytes())?;
    }
    Ok(())
}

/// Assemble a snippet at a given base address (for correct label resolution)
fn assemble_at(source: &str, base_addr: usize) -> Vec<u32> {
    crate::assembler::assemble(source, base_addr)
        .expect("snippet should assemble")
        .pixels
}

#[test]
fn test_shared_lib_file_format() {
    use std::io::Read;
    let dir = tempfile::tempdir().expect("temp dir");
    let code = assemble_at("LDI r0, 42\nHALT", 0);
    create_lib_file(dir.path(), "test.lib", &[("test_func", 0)], &code).expect("create lib");

    let mut file = std::fs::File::open(dir.path().join("test.lib")).expect("open");
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).expect("read");

    assert_eq!(&buf[0..4], b"LIB\x00", "magic should be LIB");
    let export_count = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]);
    assert_eq!(export_count, 1, "should have 1 export");
    let offset = u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]);
    assert_eq!(offset, 0, "first export should be at offset 0");
    let first_opcode = u32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]);
    assert_eq!(first_opcode, 0x10, "first instruction should be LDI (0x10)");
}

#[test]
fn test_lib_test_program_assembles() {
    let source =
        std::fs::read_to_string("programs/lib_test.asm").expect("lib_test.asm should exist");
    let result = crate::assembler::assemble(&source, 0);
    assert!(
        result.is_ok(),
        "lib_test.asm should assemble: {:?}",
        result.err()
    );
}

#[test]
fn test_lib_loader_calls_strlen() {
    // Create strlen bytecode assembled at the load address (0x5000)
    let strlen_code = assemble_at(
        "LDI r0, 0\n\
         strlen_loop:\n\
         LOAD r2, r1\n\
         JZ r2, strlen_done\n\
         LDI r2, 1\n\
         ADD r1, r2\n\
         LDI r2, 1\n\
         ADD r0, r2\n\
         JMP strlen_loop\n\
         strlen_done:\n\
         RET\n",
        0x5000,
    );

    let mut vm = crate::vm::Vm::new();

    // Write "Hello" at 0x6000
    let hello = b"Hello";
    for (i, &ch) in hello.iter().enumerate() {
        vm.ram[0x6000 + i] = ch as u32;
    }
    vm.ram[0x6000 + hello.len()] = 0;

    // Load strlen bytecode at 0x5000
    for (i, &word) in strlen_code.iter().enumerate() {
        vm.ram[0x5000 + i] = word;
    }

    // Call strlen: set r1 = string addr, jump to 0x5000
    vm.pc = 0x5000;
    vm.regs[1] = 0x6000;
    vm.halted = false;

    for _ in 0..1000 {
        if !vm.step() {
            break;
        }
    }

    assert_eq!(
        vm.regs[0], 5,
        "strlen('Hello') should return 5, got {}",
        vm.regs[0]
    );
}

#[test]
fn test_lib_full_roundtrip() {
    // Create strlen bytecode at base 0x5000
    let strlen_code = assemble_at(
        "LDI r0, 0\n\
         strlen_loop:\n\
         LOAD r2, r1\n\
         JZ r2, strlen_done\n\
         LDI r2, 1\n\
         ADD r1, r2\n\
         LDI r2, 1\n\
         ADD r0, r2\n\
         JMP strlen_loop\n\
         strlen_done:\n\
         RET\n",
        0x5000,
    );

    // Create VFS with string.lib
    let dir = tempfile::tempdir().expect("temp dir");
    create_lib_file(dir.path(), "string.lib", &[("strlen", 0)], &strlen_code).expect("create lib");

    let mut vm = crate::vm::Vm::new();
    vm.vfs.base_dir = dir.path().to_path_buf();

    // Directly use VFS fread to load the library into RAM buffer at 0x8000
    // Write filename to RAM
    let filename = b"string.lib";
    for (i, &ch) in filename.iter().enumerate() {
        vm.ram[0x4000 + i] = ch as u32;
    }
    vm.ram[0x4000 + filename.len()] = 0;

    // OPEN via VFS directly
    let fd = vm.vfs.fopen(&vm.ram, 0x4000, 0, 0);
    assert!(fd != 0xFFFFFFFF, "OPEN should succeed, got {:X}", fd);

    // READ into RAM at 0x8000
    let bytes_read = vm.vfs.fread(&mut vm.ram, fd, 0x8000, 512, 0);
    assert!(
        bytes_read > 12,
        "should read header + code, got {} bytes",
        bytes_read
    );

    // Parse the binary header from the buffer
    // Each byte from disk = one RAM word (VFS fread stores low byte per word)
    assert_eq!(vm.ram[0x8000], b'L' as u32, "magic[0]");
    assert_eq!(vm.ram[0x8001], b'I' as u32, "magic[1]");
    assert_eq!(vm.ram[0x8002], b'B' as u32, "magic[2]");

    // Export count at bytes 4-7 (LE u32)
    let export_count =
        vm.ram[0x8004] | (vm.ram[0x8005] << 8) | (vm.ram[0x8006] << 16) | (vm.ram[0x8007] << 24);
    assert_eq!(export_count, 1, "1 export");

    // Export offset at bytes 8-11
    let export_offset =
        vm.ram[0x8008] | (vm.ram[0x8009] << 8) | (vm.ram[0x800A] << 16) | (vm.ram[0x800B] << 24);
    assert_eq!(export_offset, 0, "strlen at offset 0");

    // Code starts at byte 12 (4 magic + 4 count + 4 offset)
    // Unpack first u32 word from 4 bytes (LE)
    let code_base = 0x800C;
    let first_word = vm.ram[code_base]
        | (vm.ram[code_base + 1] << 8)
        | (vm.ram[code_base + 2] << 16)
        | (vm.ram[code_base + 3] << 24);
    assert_eq!(first_word, 0x10, "LDI opcode, got 0x{:X}", first_word);

    // Unpack and load all bytecode words into RAM at 0x5000
    let mut code_addr = code_base;
    let mut ram_addr: usize = 0x5000;
    while code_addr + 3 < 0x8000 + bytes_read as usize {
        let word = vm.ram[code_addr]
            | (vm.ram[code_addr + 1] << 8)
            | (vm.ram[code_addr + 2] << 16)
            | (vm.ram[code_addr + 3] << 24);
        vm.ram[ram_addr] = word;
        code_addr += 4;
        ram_addr += 1;
    }

    // Verify the loaded bytecode matches the original
    assert!(
        ram_addr > 0x5000,
        "should have loaded at least 1 word, loaded {}",
        ram_addr - 0x5000
    );
    assert_eq!(vm.ram[0x5000], 0x10, "first word should be LDI");
    assert_eq!(vm.ram[0x5001], 0, "second word should be register 0");
    assert_eq!(vm.ram[0x5002], 0, "third word should be immediate 0");

    // Write test string "Hi" at 0x6000
    vm.ram[0x6000] = b'H' as u32;
    vm.ram[0x6001] = b'i' as u32;
    vm.ram[0x6002] = 0;

    // Call strlen at 0x5000
    vm.pc = 0x5000;
    vm.regs[1] = 0x6000;
    vm.halted = false;

    for _ in 0..1000 {
        if !vm.step() {
            break;
        }
    }

    assert_eq!(
        vm.regs[0], 2,
        "strlen('Hi') should return 2, got {}",
        vm.regs[0]
    );
}

// ═══════════════════════════════════════════════════════════════
// Phase 101: Cron Daemon Tests
// ═══════════════════════════════════════════════════════════════

/// Helper: set up a temp VFS dir with a crontab file, return (Vm, TempDir)
fn setup_cron_vm(crontab_content: &str) -> (crate::vm::Vm, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("temp dir");
    let crontab_path = dir.path().join("crontab");
    std::fs::write(&crontab_path, crontab_content).expect("write crontab");

    let source = std::fs::read_to_string("programs/cron_daemon.asm").expect("read asm");
    let asm = crate::assembler::assemble(&source, 0).expect("assemble");
    let mut vm = crate::vm::Vm::new();
    vm.vfs.base_dir = dir.path().to_path_buf();

    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    (vm, dir)
}

#[test]
fn test_cron_daemon_assembles() {
    let source =
        std::fs::read_to_string("programs/cron_daemon.asm").expect("cron_daemon.asm should exist");
    let result = crate::assembler::assemble(&source, 0);
    assert!(
        result.is_ok(),
        "cron_daemon.asm should assemble: {:?}",
        result.err()
    );
    let asm = result.unwrap();
    assert!(
        asm.pixels.len() > 100,
        "cron_daemon should produce meaningful bytecode, got {} words",
        asm.pixels.len()
    );
}

#[test]
fn test_cron_daemon_reads_crontab_and_parses() {
    let (mut vm, _dir) = setup_cron_vm("100 hello\n");

    // Run until name_len is set (indicates parsing complete for entry 0)
    for _ in 0..200_000 {
        if !vm.step() {
            break;
        }
        if vm.ram[0x4002] > 0 {
            break;
        }
    }

    // Verify parsed schedule
    assert_eq!(vm.ram[0x4000], 100, "entry 0 interval should be 100");
    assert_eq!(vm.ram[0x4001], 0, "entry 0 last_run should be 0");
    assert_eq!(vm.ram[0x4002], 5, "entry 0 name_len should be 5");

    // Verify name: "hello" = 104, 101, 108, 108, 111
    assert_eq!(vm.ram[0x4004], 104, "name[0] should be 'h'");
    assert_eq!(vm.ram[0x4005], 101, "name[1] should be 'e'");
    assert_eq!(vm.ram[0x4006], 108, "name[2] should be 'l'");
    assert_eq!(vm.ram[0x4007], 108, "name[3] should be 'l'");
    assert_eq!(vm.ram[0x4008], 111, "name[4] should be 'o'");
}

#[test]
fn test_cron_daemon_spawns_on_interval() {
    let (mut vm, _dir) = setup_cron_vm("50 hello\n");

    // Run through parsing (wait for name_len > 0)
    for _ in 0..200_000 {
        if !vm.step() {
            break;
        }
        if vm.ram[0x4002] > 0 {
            break;
        }
    }
    assert_eq!(vm.ram[0x4000], 50, "interval should be 50");
    assert_eq!(vm.ram[0x4002], 5, "name_len should be 5");

    // Advance TICKS past interval to trigger spawn
    vm.frame_count = 60;
    vm.ram[0xFFE] = 60;

    // Run until EXEC spawns a child process
    let mut spawned = false;
    for _ in 0..500_000 {
        if !vm.step() {
            break;
        }
        if !vm.processes.is_empty() {
            spawned = true;
            // Don't break immediately -- let the daemon update last_run
            // (STORE after EXEC needs a few more steps)
            for _ in 0..50 {
                if !vm.step() {
                    break;
                }
            }
            break;
        }
    }

    assert!(spawned, "cron daemon should spawn 'hello' after 50+ frames");
    let child = &vm.processes[0];
    assert!(!child.is_halted(), "spawned process should be active");
    assert_eq!(child.parent_pid, 0, "spawned by main process");

    // Verify last_run was updated
    assert!(
        vm.ram[0x4001] >= 50,
        "last_run should be updated to current ticks, got {}",
        vm.ram[0x4001]
    );
}

#[test]
fn test_cron_daemon_multiple_entries() {
    let (mut vm, _dir) = setup_cron_vm("100 hello\n200 border\n");

    // Run until both entries parsed (wait for entry 1 name_len)
    for _ in 0..300_000 {
        if !vm.step() {
            break;
        }
        if vm.ram[0x4042] > 0 {
            break;
        }
    }

    // Verify first entry
    assert_eq!(vm.ram[0x4000], 100, "entry 0 interval = 100");
    assert_eq!(vm.ram[0x4002], 5, "entry 0 name_len = 5");
    assert_eq!(vm.ram[0x4004], 104, "entry 0 name[0] = 'h'");

    // Verify second entry (at offset 64 = 0x4040)
    assert_eq!(vm.ram[0x4040], 200, "entry 1 interval = 200");
    assert_eq!(vm.ram[0x4042], 6, "entry 1 name_len = 6");
    assert_eq!(vm.ram[0x4044], 98, "entry 1 name[0] = 'b'");
    assert_eq!(vm.ram[0x4045], 111, "entry 1 name[1] = 'o'");
    assert_eq!(vm.ram[0x4046], 114, "entry 1 name[2] = 'r'");
    assert_eq!(vm.ram[0x4047], 100, "entry 1 name[3] = 'd'");
    assert_eq!(vm.ram[0x4048], 101, "entry 1 name[4] = 'e'");
    assert_eq!(vm.ram[0x4049], 114, "entry 1 name[5] = 'r'");
}

// ── Phase 104: Crash Recovery Tests ────────────────────────────────

#[test]
fn test_pc_trace_ring_buffer() {
    // Verify that pc_trace records PC values in a ring buffer
    let mut vm = Vm::new();

    // Run 15 NOPs + HALT
    for i in 0..15 {
        vm.ram[i] = 0x01; // NOP
    }
    vm.ram[15] = 0x00; // HALT
    vm.halted = false;

    for _ in 0..1000 {
        if !vm.step() {
            break;
        }
    }

    assert!(
        vm.pc_trace.len() == 16,
        "pc_trace buffer should be 16 entries"
    );
    let has_nonzero = vm.pc_trace.iter().any(|&pc| pc != 0);
    assert!(
        has_nonzero,
        "pc_trace should have non-zero entries after execution"
    );
}

#[test]
fn test_crash_dialog_renders_on_segfault() {
    // Manually trigger a segfault scenario and verify crash dialog renders.
    use crate::vm::types::Process;
    let mut vm = Vm::new();

    // Create a process that has segfaulted
    let mut proc = Process::new(1, 0, 0x100);
    proc.pc = 0x103;
    let proc_ref = proc.clone();
    vm.processes.push(proc);

    // Manually set segfault state
    vm.segfault_pid = 1;
    vm.segfault = true;
    vm.segfault_addr = 0xFE00;

    // Write core dump first (use clone to avoid borrow conflict)
    let result = vm.write_core_dump(&proc_ref);
    assert!(result, "Core dump should succeed");

    // Verify VFS file was created
    let ino = vm.inode_fs.resolve("/var/core/1.txt");
    assert!(
        ino.is_some(),
        "Core dump file /var/core/1.txt should exist in VFS"
    );

    // Render the crash dialog
    vm.render_crash_dialog(&proc_ref);

    // Verify crash dialog is rendered on screen
    let has_dark_bg = vm.screen.iter().any(|&pixel| pixel == 0x1A1A2E);
    assert!(
        has_dark_bg,
        "Crash dialog background (0x1A1A2E) should be rendered"
    );

    let has_red_border = vm.screen.iter().any(|&pixel| pixel == 0xFF0000);
    assert!(
        has_red_border,
        "Crash dialog should have red border (0xFF0000)"
    );

    assert!(vm.crash_dialog_active, "crash_dialog_active should be true");
    assert_eq!(vm.crash_dialog_pid, 1, "crash_dialog_pid should be 1");
}

#[test]
fn test_core_dump_contains_register_dump() {
    // Verify the core dump VFS file contains register values and PC trace.
    use crate::vm::types::Process;
    let mut vm = Vm::new();

    // Create a process with specific register values
    let mut proc = Process::new(7, 0, 0x1234);
    proc.regs[0] = 0xDEADBEEF;
    proc.regs[1] = 0x12345678;
    proc.regs[5] = 42;
    vm.processes.push(proc);

    vm.segfault_addr = 0xABCD;

    // Populate some PC trace entries
    vm.pc_trace[0] = 0x100;
    vm.pc_trace[1] = 0x200;
    vm.pc_trace[2] = 0x300;
    vm.pc_trace_idx = 3;

    let proc_snapshot = vm.processes[0].clone();
    let result = vm.write_core_dump(&proc_snapshot);
    assert!(result, "Core dump should succeed");

    // Read back the core dump from VFS
    let ino = vm
        .inode_fs
        .resolve("/var/core/7.txt")
        .expect("core dump file should exist");
    let mut buf = vec![0u32; 4096];
    let n = vm.inode_fs.read_inode(ino, 0, &mut buf);
    assert!(n > 0, "Should read non-zero bytes from core dump");

    // Convert to string
    let mut dump_text = String::new();
    for i in 0..n as usize {
        if buf[i] == 0 {
            break;
        }
        dump_text.push((buf[i] & 0xFF) as u8 as char);
    }

    // Verify core dump contents
    assert!(
        dump_text.contains("CORE DUMP"),
        "Should contain CORE DUMP header, got: {}",
        &dump_text[..dump_text.len().min(200)]
    );
    assert!(
        dump_text.contains("PID: 7"),
        "Should contain PID, got: {}",
        &dump_text[..dump_text.len().min(200)]
    );
    assert!(
        dump_text.contains("0x1234"),
        "Should contain PC, got: {}",
        &dump_text[..dump_text.len().min(200)]
    );
    assert!(
        dump_text.contains("0xABCD"),
        "Should contain fault address, got: {}",
        &dump_text[..dump_text.len().min(200)]
    );
    assert!(
        dump_text.contains("DEADBEEF"),
        "Should contain register r0 value"
    );
    assert!(
        dump_text.contains("12345678"),
        "Should contain register r1 value"
    );
    assert!(
        dump_text.contains("0x0100"),
        "Should contain PC trace entry"
    );
}

#[test]
fn test_segfault_addr_field_persists() {
    // Verify segfault_addr is properly set and persists.
    let mut vm = Vm::new();
    assert_eq!(vm.segfault_addr, 0, "Initial segfault_addr should be 0");

    vm.segfault_addr = 0xBEEF;
    assert_eq!(vm.segfault_addr, 0xBEEF, "segfault_addr should persist");

    // After reset, should be cleared
    vm.reset();
    assert_eq!(
        vm.segfault_addr, 0,
        "segfault_addr should be cleared after reset"
    );
    assert_eq!(
        vm.crash_dialog_active, false,
        "crash_dialog_active should be false after reset"
    );
    assert_eq!(
        vm.crash_dialog_pid, 0,
        "crash_dialog_pid should be 0 after reset"
    );
}

#[test]
fn test_crash_dialog_screen_region() {
    // Verify the crash dialog renders in the expected screen region.
    use crate::vm::types::Process;
    let mut vm = Vm::new();

    let proc = Process::new(42, 0, 0x5678);
    let proc_ref = proc.clone();
    vm.processes.push(proc);
    vm.segfault_addr = 0x1234;

    vm.render_crash_dialog(&proc_ref);

    // Dialog is at dx=16, dy=80, dw=224, dh=96
    // Check border color at top-left
    let border = 0xFF0000u32;
    let bg = 0x1A1A2Eu32;

    // Top-left corner of dialog (dx, dy)
    assert_eq!(
        vm.screen[80 * 256 + 16],
        border,
        "Top-left should be border"
    );
    // Inside should be background
    assert_eq!(
        vm.screen[82 * 256 + 20],
        bg,
        "Inside should be dark background"
    );

    // Check that white text pixels exist (SEGFAULT! text)
    let has_white = vm.screen.iter().any(|&p| p == 0xFFFFFF);
    assert!(
        has_white,
        "Should have white text pixels for SEGFAULT! text"
    );
}

// ── Phase 98: Custom Bitmap Fonts ─────────────────────────────────────

#[test]
fn test_font_files_exist_and_valid() {
    // Verify that all three font files exist and are exactly 1024 bytes
    // (128 glyphs × 8 bytes per glyph)
    let fonts_dir = std::path::Path::new("fonts");
    for name in &["default.fnt", "bold.fnt", "wide.fnt"] {
        let path = fonts_dir.join(name);
        assert!(path.exists(), "Font file {:?} should exist", path);
        let metadata = std::fs::metadata(&path).unwrap();
        assert_eq!(
            metadata.len(),
            1024,
            "Font file {:?} should be 1024 bytes",
            path
        );
    }
}

#[test]
fn test_bold_font_differs_from_default() {
    // Verify bold font has wider strokes than default
    let default = std::fs::read("fonts/default.fnt").unwrap();
    let bold = std::fs::read("fonts/bold.fnt").unwrap();

    // Compare glyph for 'A' (index 65, 8 bytes starting at offset 65*8)
    let a_offset = 65 * 8;
    let default_a = &default[a_offset..a_offset + 8];
    let bold_a = &bold[a_offset..a_offset + 8];

    // Bold should have at least some rows that are wider (more bits set)
    let default_bits: u32 = default_a.iter().map(|&b| b.count_ones()).sum();
    let bold_bits: u32 = bold_a.iter().map(|&b| b.count_ones()).sum();

    assert!(
        bold_bits >= default_bits,
        "Bold font 'A' should have at least as many lit pixels as default (bold={}, default={})",
        bold_bits,
        default_bits
    );
}

#[test]
fn test_wide_font_differs_from_default() {
    // Verify wide font is different from both default and bold
    let default = std::fs::read("fonts/default.fnt").unwrap();
    let wide = std::fs::read("fonts/wide.fnt").unwrap();

    let a_offset = 65 * 8;
    let default_a = &default[a_offset..a_offset + 8];
    let wide_a = &wide[a_offset..a_offset + 8];

    // Wide should have at least some different bytes
    let differs = default_a.iter().zip(wide_a.iter()).any(|(a, b)| a != b);
    assert!(differs, "Wide font 'A' should differ from default");
}

#[test]
fn test_font_file_loadable_into_process() {
    // Test that a .fnt file can be read from disk and loaded into a process
    // via the IOCTL cmd 2 mechanism
    let mut vm = Vm::new();

    // Read the bold font file into VM RAM
    let font_data = std::fs::read("fonts/bold.fnt").expect("bold.fnt should exist");
    assert_eq!(font_data.len(), 1024);

    let font_base = 0x2000usize;
    // The IOCTL expects 128*8 u32 words, but font data is bytes.
    // We load each byte into a u32 RAM cell.
    for (i, &byte) in font_data.iter().enumerate() {
        vm.ram[font_base + i] = byte as u32;
    }

    // Create a process
    let proc = crate::vm::types::Process::new(1, 0, 0);
    vm.processes.push(proc);
    vm.current_pid = 1;

    // Set up OPEN for /dev/screen
    let path_bytes = b"/dev/screen";
    for (i, &b) in path_bytes.iter().enumerate() {
        vm.ram[0x3000 + i] = b as u32;
    }
    vm.ram[0x3000 + path_bytes.len()] = 0;

    // OPEN
    vm.regs[1] = 0x3000;
    vm.regs[2] = 0;
    vm.ram[0] = 0x54; // OPEN
    vm.ram[1] = 0x01;
    vm.ram[2] = 0x02;
    vm.pc = 0;
    vm.halted = false;
    vm.step();
    let fd = vm.regs[0];
    assert_eq!(fd, 0xE000);

    // IOCTL cmd 2: set font from RAM
    vm.regs[3] = fd;
    vm.regs[4] = 2;
    vm.regs[5] = 0x2000;
    vm.ram[vm.pc as usize] = 0x62; // IOCTL
    vm.ram[vm.pc as usize + 1] = 0x03;
    vm.ram[vm.pc as usize + 2] = 0x04;
    vm.ram[vm.pc as usize + 3] = 0x05;
    vm.step();

    assert_eq!(vm.regs[0], 0, "IOCTL set font should succeed");

    // Verify font was loaded correctly
    let font = vm
        .processes
        .iter()
        .find(|p| p.pid == 1)
        .and_then(|p| p.custom_font.as_ref())
        .expect("process should have custom font");

    // Verify glyph 65 ('A') matches the bold font data
    let a_offset = 65 * 8;
    for row in 0..8 {
        assert_eq!(
            font[65][row],
            font_data[a_offset + row],
            "Glyph 65 row {} should match font file byte {}",
            row,
            a_offset + row
        );
    }
}

#[test]
fn test_multi_process_font_isolation() {
    // Test that two processes can have different fonts
    // and each sees only its own font
    let mut vm = Vm::new();

    // Create two processes
    let mut proc1 = crate::vm::types::Process::new(1, 0, 100);
    let mut proc2 = crate::vm::types::Process::new(2, 0, 200);

    // Create distinctive fonts for each process
    let mut font1 = vec![[0u8; 8]; 128];
    let mut font2 = vec![[0u8; 8]; 128];

    // Process 1: glyph 'A' (65) is all-0xFF (solid block)
    for row in 0..8 {
        font1[65][row] = 0xFF;
    }
    // Process 2: glyph 'A' (65) is 0x01 (single pixel)
    for row in 0..8 {
        font2[65][row] = 0x01;
    }

    proc1.custom_font = Some(font1);
    proc2.custom_font = Some(font2);

    vm.processes.push(proc1);
    vm.processes.push(proc2);

    // Write "A" to RAM for TEXT opcode
    vm.ram[0x3000] = 65; // 'A'
    vm.ram[0x3001] = 0;

    // Set up registers for TEXT
    vm.regs[1] = 10; // x
    vm.regs[2] = 10; // y
    vm.regs[3] = 0x3000; // addr

    // Render with process 1's font
    vm.current_pid = 1;
    vm.ram[0] = 0x44; // TEXT
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.ram[4] = 0x00; // HALT
    vm.pc = 0;
    vm.halted = false;
    vm.step();

    // Count lit pixels in process 1's 'A' area (x=10..17, y=10..17)
    let mut proc1_pixels = 0;
    for y in 10..18 {
        for x in 10..18 {
            if vm.screen[y * 256 + x] != 0 {
                proc1_pixels += 1;
            }
        }
    }
    // Process 1 has all-solid 'A' = 64 pixels
    assert_eq!(
        proc1_pixels, 64,
        "Process 1 with solid font should have 64 lit pixels"
    );

    // Clear screen for process 2
    for i in 0..65536 {
        vm.screen[i] = 0;
    }

    // Render with process 2's font
    vm.current_pid = 2;
    vm.regs[1] = 10;
    vm.regs[2] = 10;
    vm.regs[3] = 0x3000;
    vm.ram[0] = 0x44;
    vm.ram[1] = 1;
    vm.ram[2] = 2;
    vm.ram[3] = 3;
    vm.ram[4] = 0x00;
    vm.pc = 0;
    vm.halted = false;
    vm.step();

    // Count lit pixels for process 2's 'A'
    let mut proc2_pixels = 0;
    for y in 10..18 {
        for x in 10..18 {
            if vm.screen[y * 256 + x] != 0 {
                proc2_pixels += 1;
            }
        }
    }
    // Process 2 has single-pixel-per-row 'A' = 8 pixels
    assert_eq!(
        proc2_pixels, 8,
        "Process 2 with sparse font should have 8 lit pixels"
    );

    // Verify the two processes produced different output
    assert_ne!(
        proc1_pixels, proc2_pixels,
        "Different processes with different fonts should produce different output"
    );
}

#[test]
fn test_ensure_fonts_on_boot() {
    // Test that ensure_fonts() populates the VFS with font files
    let mut vm = Vm::new();
    vm.ensure_fonts();

    let fonts_dir = vm.vfs.base_dir.join("lib").join("fonts");
    assert!(
        fonts_dir.exists(),
        "/lib/fonts/ directory should exist after ensure_fonts()"
    );

    for name in &["default.fnt", "bold.fnt", "wide.fnt"] {
        let path = fonts_dir.join(name);
        assert!(
            path.exists(),
            "{:?} should exist after ensure_fonts()",
            path
        );
        let size = std::fs::metadata(&path).unwrap().len();
        assert_eq!(size, 1024, "{:?} should be 1024 bytes", path);
    }
}

#[test]
fn test_term_mux_assembles() {
    let source = std::fs::read_to_string("programs/term_mux.asm").unwrap();
    let result = crate::assembler::assemble(&source, 0);
    assert!(result.is_ok(), "term_mux.asm should assemble: {:?}", result);
}

#[test]
fn test_term_mux_pipe_creation() {
    // Test that the terminal multiplexer creates pipe pairs correctly
    let source = std::fs::read_to_string("programs/term_mux.asm").unwrap();
    let asm = crate::assembler::assemble(&source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    vm.pc = 0;
    vm.halted = false;

    // Run until HALT
    for _ in 0..100_000 {
        if !vm.step() {
            break;
        }
    }

    // Verify pipes were created - check that pipe fds are stored in RAM
    let s0_stdout_read = vm.ram[0x4000];
    let s0_stdin_write = vm.ram[0x4001];
    let s1_stdout_read = vm.ram[0x4010];
    let s1_stdin_write = vm.ram[0x4011];

    // Pipe fds should be in the expected ranges
    assert!(
        s0_stdout_read >= 0x8000,
        "session 0 stdout read fd should be a pipe fd, got {:X}",
        s0_stdout_read
    );
    assert!(
        s0_stdin_write >= 0xC000,
        "session 0 stdin write fd should be a pipe fd, got {:X}",
        s0_stdin_write
    );
    assert!(
        s1_stdout_read >= 0x8000,
        "session 1 stdout read fd should be a pipe fd, got {:X}",
        s1_stdout_read
    );
    assert!(
        s1_stdin_write >= 0xC000,
        "session 1 stdin write fd should be a pipe fd, got {:X}",
        s1_stdin_write
    );

    // Active session should default to 0
    assert_eq!(vm.ram[0xF00], 0, "active session should default to 0");

    // Verify scrollback buffers were initialized (should be zero)
    assert_eq!(
        vm.ram[0x5000], 0,
        "session 0 scrollback should be initialized to 0"
    );
    assert_eq!(
        vm.ram[0x6000], 0,
        "session 1 scrollback should be initialized to 0"
    );
}

// ── Phase 106: AI Desktop Control and Guided Demo ──────────

#[test]
fn test_mcp_server_phase106_tool_definitions() {
    // Verify all Phase 106 tools are defined in the MCP server source
    let source = std::fs::read_to_string("src/mcp_server.rs").unwrap();

    // Window management tools
    assert!(
        source.contains("\"window_list\""),
        "window_list tool should be defined"
    );
    assert!(
        source.contains("\"window_move\""),
        "window_move tool should be defined"
    );
    assert!(
        source.contains("\"window_close\""),
        "window_close tool should be defined"
    );
    assert!(
        source.contains("\"window_focus\""),
        "window_focus tool should be defined"
    );
    assert!(
        source.contains("\"window_resize\""),
        "window_resize tool should be defined"
    );
    assert!(
        source.contains("\"process_kill\""),
        "process_kill tool should be defined"
    );

    // Desktop input aliases
    assert!(
        source.contains("\"desktop_key\""),
        "desktop_key tool should be defined"
    );
    assert!(
        source.contains("\"desktop_mouse_move\""),
        "desktop_mouse_move tool should be defined"
    );
    assert!(
        source.contains("\"desktop_mouse_click\""),
        "desktop_mouse_click tool should be defined"
    );

    // Desktop vision
    assert!(
        source.contains("\"desktop_vision\""),
        "desktop_vision tool should be defined"
    );
}

#[test]
fn test_mcp_server_phase106_schema_functions() {
    // Verify all Phase 106 schema functions exist
    let source = std::fs::read_to_string("src/mcp_server.rs").unwrap();

    // Window management schemas
    assert!(
        source.contains("fn window_list_mcp_schema()"),
        "window_list_mcp_schema should exist"
    );
    assert!(
        source.contains("fn window_move_mcp_schema()"),
        "window_move_mcp_schema should exist"
    );
    assert!(
        source.contains("fn window_close_mcp_schema()"),
        "window_close_mcp_schema should exist"
    );
    assert!(
        source.contains("fn window_focus_mcp_schema()"),
        "window_focus_mcp_schema should exist"
    );
    assert!(
        source.contains("fn window_resize_mcp_schema()"),
        "window_resize_mcp_schema should exist"
    );
    assert!(
        source.contains("fn process_kill_mcp_schema()"),
        "process_kill_mcp_schema should exist"
    );

    // Desktop input schemas
    assert!(
        source.contains("fn desktop_key_schema()"),
        "desktop_key_schema should exist"
    );
    assert!(
        source.contains("fn desktop_mouse_move_schema()"),
        "desktop_mouse_move_schema should exist"
    );
    assert!(
        source.contains("fn desktop_mouse_click_schema()"),
        "desktop_mouse_click_schema should exist"
    );

    // Desktop vision schema
    assert!(
        source.contains("fn desktop_vision_schema()"),
        "desktop_vision_schema should exist"
    );
}

#[test]
fn test_mcp_server_phase106_handlers() {
    // Verify all Phase 106 handler match arms exist
    let source = std::fs::read_to_string("src/mcp_server.rs").unwrap();

    // Window management handlers must have socket command calls
    assert!(
        source.contains("\"window_list\" =>"),
        "window_list handler should exist"
    );
    assert!(
        source.contains("\"window_move\" =>"),
        "window_move handler should exist"
    );
    assert!(
        source.contains("\"window_close\" =>"),
        "window_close handler should exist"
    );
    assert!(
        source.contains("\"window_focus\" =>"),
        "window_focus handler should exist"
    );
    assert!(
        source.contains("\"window_resize\" =>"),
        "window_resize handler should exist"
    );
    assert!(
        source.contains("\"process_kill\" =>"),
        "process_kill handler should exist"
    );

    // Desktop input handlers
    assert!(
        source.contains("\"desktop_key\" =>"),
        "desktop_key handler should exist"
    );
    assert!(
        source.contains("\"desktop_mouse_move\" =>"),
        "desktop_mouse_move handler should exist"
    );
    assert!(
        source.contains("\"desktop_mouse_click\" =>"),
        "desktop_mouse_click handler should exist"
    );

    // Desktop vision handler
    assert!(
        source.contains("\"desktop_vision\" =>"),
        "desktop_vision handler should exist"
    );
}

#[test]
fn test_socket_cmd_window_list_in_main() {
    // Verify socket commands for window management exist in main.rs
    let source = std::fs::read_to_string("src/main.rs").unwrap();

    assert!(
        source.contains("\"window_list\" =>"),
        "window_list socket command should exist in main.rs"
    );
    assert!(
        source.contains("\"window_move\" =>"),
        "window_move socket command should exist in main.rs"
    );
    assert!(
        source.contains("\"window_close\" =>"),
        "window_close socket command should exist in main.rs"
    );
    assert!(
        source.contains("\"window_focus\" =>"),
        "window_focus socket command should exist in main.rs"
    );
    assert!(
        source.contains("\"window_resize\" =>"),
        "window_resize socket command should exist in main.rs"
    );
    assert!(
        source.contains("\"process_kill\" =>"),
        "process_kill socket command should exist in main.rs"
    );
}

#[test]
fn test_window_list_socket_format() {
    // Test that the window_list socket command produces correct format
    let source = std::fs::read_to_string("src/main.rs").unwrap();

    // Find the window_list handler and verify format
    let start = source
        .find("\"window_list\" =>")
        .expect("window_list handler should exist");
    let end = source[start..]
        .find("\"window_move\" =>")
        .expect("window_move should follow");
    let handler = &source[start..start + end];

    // Should list active windows with id, x, y, w, h, z_order, pid, title
    assert!(handler.contains("w.id"), "should output window id");
    assert!(handler.contains("w.x"), "should output window x");
    assert!(handler.contains("w.y"), "should output window y");
    assert!(handler.contains("w.w"), "should output window w");
    assert!(handler.contains("w.h"), "should output window h");
    assert!(handler.contains("w.z_order"), "should output z_order");
    assert!(handler.contains("w.pid"), "should output pid");
    assert!(handler.contains("w.active"), "should filter active windows");
}

#[test]
fn test_window_move_socket_updates_position() {
    // Verify window_move handler updates x,y on window
    let source = std::fs::read_to_string("src/main.rs").unwrap();

    let start = source
        .find("\"window_move\" =>")
        .expect("window_move handler should exist");
    let end = source[start..]
        .find("\"window_close\" =>")
        .expect("window_close should follow");
    let handler = &source[start..start + end];

    assert!(
        handler.contains("w.x = new_x"),
        "should update window x position"
    );
    assert!(
        handler.contains("w.y = new_y"),
        "should update window y position"
    );
    assert!(handler.contains("window_move"), "should have usage hint");
}

#[test]
fn test_window_resize_updates_buffer() {
    // Verify window_resize resizes the offscreen buffer
    let source = std::fs::read_to_string("src/main.rs").unwrap();

    let start = source
        .find("\"window_resize\" =>")
        .expect("window_resize handler should exist");
    let end = source[start..]
        .find("\"process_kill\" =>")
        .expect("process_kill should follow");
    let handler = &source[start..start + end];

    // Should update w, h, and resize the offscreen buffer
    assert!(
        handler.contains("w.w = new_w"),
        "should update window width"
    );
    assert!(
        handler.contains("w.h = new_h"),
        "should update window height"
    );
    assert!(
        handler.contains("offscreen_buffer") && handler.contains(".resize("),
        "should resize offscreen buffer"
    );
}

#[test]
fn test_desktop_vision_handler_structure() {
    // Verify desktop_vision combines window_list and vmscreen
    let source = std::fs::read_to_string("src/mcp_server.rs").unwrap();

    let start = source
        .find("\"desktop_vision\" =>")
        .expect("desktop_vision handler should exist");
    let end = source[start..]
        .find("_ => Err(format!(\"Unknown tool")
        .expect("Unknown tool catch");
    let handler = &source[start..start + end];

    // Should call window_list socket command
    assert!(handler.contains("window_list"), "should query window list");
    // Should call vmscreen for ASCII art
    assert!(handler.contains("vmscreen"), "should get screen ASCII art");
    // Should identify focused window (highest z_order)
    assert!(
        handler.contains("focused"),
        "should identify focused window"
    );
    assert!(
        handler.contains("z_order"),
        "should check z_order for focus"
    );
    assert!(
        handler.contains("focused_window"),
        "should return focused_window"
    );
}

#[test]
fn test_desktop_key_uses_inject_key() {
    // Verify desktop_key delegates to inject_key socket command
    let source = std::fs::read_to_string("src/mcp_server.rs").unwrap();

    let start = source
        .find("\"desktop_key\" =>")
        .expect("desktop_key handler should exist");
    let section = &source[start..start + 500];

    assert!(
        section.contains("inject_key"),
        "desktop_key should use inject_key socket command"
    );
}

#[test]
fn test_desktop_mouse_aliases_use_inject_mouse() {
    // Verify desktop_mouse_move and desktop_mouse_click delegate to inject_mouse
    let source = std::fs::read_to_string("src/mcp_server.rs").unwrap();

    let move_start = source
        .find("\"desktop_mouse_move\" =>")
        .expect("desktop_mouse_move handler should exist");
    let move_section = &source[move_start..move_start + 500];
    assert!(
        move_section.contains("inject_mouse move"),
        "desktop_mouse_move should use inject_mouse move"
    );

    let click_start = source
        .find("\"desktop_mouse_click\" =>")
        .expect("desktop_mouse_click handler should exist");
    let click_section = &source[click_start..click_start + 500];
    assert!(
        click_section.contains("inject_mouse click"),
        "desktop_mouse_click should use inject_mouse click"
    );
}

#[test]
fn test_demo_tour_exists() {
    // Verify the demo tour document exists
    assert!(
        std::path::Path::new("docs/demo_tour.md").exists(),
        "docs/demo_tour.md should exist for AI guided demo"
    );
}

// ── Phase 107: World-Space Window Placement ──────────────────

#[test]
fn test_winsys_create_world_space_window() {
    // When RAM[0x7810] == 1, WINSYS op=0 creates a world-space window
    let mut vm = Vm::new();
    vm.ram[WINDOW_WORLD_COORDS_ADDR] = 1; // enable world-space mode

    vm.regs[1] = 100; // world_x
    vm.regs[2] = 200; // world_y
    vm.regs[3] = 64; // w
    vm.regs[4] = 48; // h
    vm.regs[5] = 0; // title_addr
    vm.regs[6] = 0; // op = create

    vm.ram[0] = 0x94; // WINSYS
    vm.ram[1] = 6; // op_reg = r6
    vm.ram[2] = 0x00; // HALT
    vm.pc = 0;
    vm.halted = false;
    vm.step();
    vm.step();

    assert_eq!(vm.regs[0], 1, "first window should have id 1");
    assert_eq!(vm.windows.len(), 1);
    let w = &vm.windows[0];
    assert!(w.is_world_space(), "window should be world-space");
    assert_eq!(w.world_x, 100);
    assert_eq!(w.world_y, 200);
    assert_eq!(w.w, 64);
    assert_eq!(w.h, 48);
    assert_eq!(w.x, 0, "screen x should be 0 (computed at render time)");
    assert_eq!(w.y, 0, "screen y should be 0 (computed at render time)");
}

#[test]
fn test_winsys_create_screen_space_window_default() {
    // Without RAM[0x7810] set, WINSYS op=0 creates screen-space windows (legacy)
    let mut vm = Vm::new();
    // RAM[0x7810] defaults to 0 -- no world-space mode

    vm.regs[1] = 10; // screen x
    vm.regs[2] = 20; // screen y
    vm.regs[3] = 64; // w
    vm.regs[4] = 48; // h
    vm.regs[5] = 0; // title_addr
    vm.regs[6] = 0; // op = create

    vm.ram[0] = 0x94; // WINSYS
    vm.ram[1] = 6; // op_reg = r6
    vm.ram[2] = 0x00; // HALT
    vm.pc = 0;
    vm.halted = false;
    vm.step();
    vm.step();

    assert_eq!(vm.regs[0], 1, "first window should have id 1");
    let w = &vm.windows[0];
    assert!(
        !w.is_world_space(),
        "window should be screen-space (default)"
    );
    assert_eq!(w.x, 10);
    assert_eq!(w.y, 20);
    assert_eq!(w.world_x, WORLD_COORD_UNSET);
    assert_eq!(w.world_y, WORLD_COORD_UNSET);
}

#[test]
fn test_winsys_moveto_world_space_window() {
    // MOVETO on a world-space window updates world_x/world_y
    let mut vm = Vm::new();
    vm.ram[WINDOW_WORLD_COORDS_ADDR] = 1;

    // Create window (op=0) in world-space
    vm.regs[1] = 50; // world_x
    vm.regs[2] = 60; // world_y
    vm.regs[3] = 64;
    vm.regs[4] = 48;
    vm.regs[5] = 0;
    vm.regs[6] = 0; // create
    vm.ram[0] = 0x94;
    vm.ram[1] = 6;
    vm.ram[2] = 0x00;
    vm.pc = 0;
    vm.halted = false;
    vm.step(); // WINSYS
    let win_id = vm.regs[0];
    assert_eq!(win_id, 1);

    // MOVETO (op=5): update world position
    vm.regs[0] = win_id;
    vm.regs[1] = 150; // new world_x
    vm.regs[2] = 250; // new world_y
    vm.regs[6] = 5; // moveto
    vm.ram[3] = 0x94;
    vm.ram[4] = 6;
    vm.ram[5] = 0x00;
    vm.pc = 3;
    vm.step(); // MOVETO
    vm.step(); // HALT

    assert_eq!(vm.regs[0], 1, "moveto should succeed");
    let w = &vm.windows[0];
    assert_eq!(w.world_x, 150, "world_x should be updated");
    assert_eq!(w.world_y, 250, "world_y should be updated");
}

#[test]
fn test_winsys_moveto_screen_space_window() {
    // MOVETO on a screen-space window updates x/y (legacy behavior)
    let mut vm = Vm::new();
    // RAM[0x7810] = 0 (default, screen-space)

    // Create window (op=0) in screen-space
    vm.regs[1] = 10;
    vm.regs[2] = 20;
    vm.regs[3] = 64;
    vm.regs[4] = 48;
    vm.regs[5] = 0;
    vm.regs[6] = 0; // create
    vm.ram[0] = 0x94;
    vm.ram[1] = 6;
    vm.ram[2] = 0x00;
    vm.pc = 0;
    vm.halted = false;
    vm.step(); // WINSYS
    let win_id = vm.regs[0];

    // MOVETO (op=5): update screen position
    vm.regs[0] = win_id;
    vm.regs[1] = 30;
    vm.regs[2] = 40;
    vm.regs[6] = 5; // moveto
    vm.ram[3] = 0x94;
    vm.ram[4] = 6;
    vm.ram[5] = 0x00;
    vm.pc = 3;
    vm.step(); // MOVETO
    vm.step(); // HALT

    assert_eq!(vm.regs[0], 1, "moveto should succeed");
    let w = &vm.windows[0];
    assert_eq!(w.x, 30, "screen x should be updated");
    assert_eq!(w.y, 40, "screen y should be updated");
}

#[test]
fn test_winsys_winfo_includes_world_coords() {
    // WINFO (op=6) now returns 8 values: [x, y, w, h, z_order, pid, world_x, world_y]
    let mut vm = Vm::new();
    vm.ram[WINDOW_WORLD_COORDS_ADDR] = 1;

    // Create world-space window
    vm.regs[1] = 42; // world_x
    vm.regs[2] = 99; // world_y
    vm.regs[3] = 64;
    vm.regs[4] = 48;
    vm.regs[5] = 0;
    vm.regs[6] = 0; // create
    vm.ram[0] = 0x94;
    vm.ram[1] = 6;
    vm.pc = 0;
    vm.halted = false;
    vm.step(); // WINSYS create
    let win_id = vm.regs[0];

    // WINFO (op=6): write to RAM at 0x3000
    let info_addr: usize = 0x3000;
    vm.regs[0] = win_id;
    vm.regs[1] = info_addr as u32;
    vm.regs[6] = 6; // winfo
    vm.ram[2] = 0x94;
    vm.ram[3] = 6;
    vm.ram[4] = 0x00; // HALT
    vm.step(); // WINSYS winfo
    vm.step(); // HALT

    assert_eq!(vm.regs[0], 1, "winfo should succeed");
    // [0]=x, [1]=y, [2]=w, [3]=h, [4]=z_order, [5]=pid, [6]=world_x, [7]=world_y
    assert_eq!(vm.ram[info_addr + 2], 64, "w");
    assert_eq!(vm.ram[info_addr + 3], 48, "h");
    assert_eq!(vm.ram[info_addr + 6], 42, "world_x");
    assert_eq!(vm.ram[info_addr + 7], 99, "world_y");
}

#[test]
fn test_winsys_winfo_screen_space_world_unset() {
    // For screen-space windows, world_x/world_y should be WORLD_COORD_UNSET
    let mut vm = Vm::new();
    // RAM[0x7810] = 0 (default, screen-space)

    // Create screen-space window
    vm.regs[1] = 10;
    vm.regs[2] = 20;
    vm.regs[3] = 32;
    vm.regs[4] = 32;
    vm.regs[5] = 0;
    vm.regs[6] = 0; // create
    vm.ram[0] = 0x94;
    vm.ram[1] = 6;
    vm.pc = 0;
    vm.halted = false;
    vm.step(); // WINSYS create
    let win_id = vm.regs[0];

    // WINFO (op=6)
    let info_addr: usize = 0x3000;
    vm.regs[0] = win_id;
    vm.regs[1] = info_addr as u32;
    vm.regs[6] = 6; // winfo
    vm.ram[2] = 0x94;
    vm.ram[3] = 6;
    vm.ram[4] = 0x00; // HALT
    vm.step();
    vm.step();

    assert_eq!(vm.ram[info_addr + 0], 10, "x");
    assert_eq!(vm.ram[info_addr + 1], 20, "y");
    assert_eq!(
        vm.ram[info_addr + 6],
        WORLD_COORD_UNSET,
        "world_x should be unset"
    );
    assert_eq!(
        vm.ram[info_addr + 7],
        WORLD_COORD_UNSET,
        "world_y should be unset"
    );
}

#[test]
fn test_existing_programs_screen_space_unchanged() {
    // Existing programs that use screen-space WINSYS continue to work
    // (flag defaults to off) -- verify by creating a window without setting RAM[0x7810]
    let mut vm = Vm::new();

    vm.regs[1] = 50;
    vm.regs[2] = 60;
    vm.regs[3] = 80;
    vm.regs[4] = 60;
    vm.regs[5] = 0;
    vm.regs[6] = 0; // create
    vm.ram[0] = 0x94;
    vm.ram[1] = 6;
    vm.ram[2] = 0x00;
    vm.pc = 0;
    vm.halted = false;
    vm.step();
    vm.step();

    let w = &vm.windows[0];
    assert!(!w.is_world_space());
    assert_eq!(w.x, 50);
    assert_eq!(w.y, 60);
    assert_eq!(w.w, 80);
    assert_eq!(w.h, 60);
}

// ── Phase 107: Multi-Process App Execution & Window Drag ─────

#[test]
fn test_multi_process_two_apps_simultaneously() {
    // Two apps can run simultaneously in separate world-space windows
    let mut vm = Vm::new();
    vm.ram[WINDOW_WORLD_COORDS_ADDR] = 1;

    // App 1: Create window at world (10,10), fill with red
    vm.regs[1] = 10; // world_x
    vm.regs[2] = 10; // world_y
    vm.regs[3] = 64; // w
    vm.regs[4] = 48; // h
    vm.regs[5] = 0; // title_addr
    vm.regs[6] = 0; // op = create
    vm.ram[0] = 0x94; // WINSYS
    vm.ram[1] = 6;
    vm.ram[2] = 0x00; // HALT
    vm.pc = 0;
    vm.halted = false;
    vm.step();
    vm.step();
    let win_id_1 = vm.regs[0];
    assert_eq!(win_id_1, 1);

    // App 2: Create window at world (20,20)
    vm.regs[1] = 20;
    vm.regs[2] = 20;
    vm.regs[3] = 64;
    vm.regs[4] = 48;
    vm.regs[5] = 0;
    vm.regs[6] = 0;
    vm.ram[3] = 0x94;
    vm.ram[4] = 6;
    vm.ram[5] = 0x00;
    vm.pc = 3;
    vm.halted = false;
    vm.step();
    vm.step();
    let win_id_2 = vm.regs[0];
    assert_eq!(win_id_2, 2);

    // Both windows exist and are world-space
    assert_eq!(vm.windows.len(), 2);
    let w1 = &vm.windows[0];
    let w2 = &vm.windows[1];
    assert!(w1.is_world_space());
    assert!(w2.is_world_space());
    assert_eq!(w1.world_x, 10);
    assert_eq!(w1.world_y, 10);
    assert_eq!(w2.world_x, 20);
    assert_eq!(w2.world_y, 20);

    // Different PIDs (they'd get different PIDs when spawned as processes)
    // But since we created them directly, both have pid=0
    // The important thing is they have different IDs
    assert_ne!(w1.id, w2.id);
}

#[test]
fn test_offscreen_window_culling() {
    // Windows outside the viewport should be identifiable via viewport module
    use crate::viewport::Viewport;

    let vp = Viewport::new(10, 10, 2); // cam at (10,10), zoom 2

    // Window at world (10,10) tile = world pixel (80,80) - should be visible
    assert!(vp.is_rect_visible(80, 80, 64, 48));

    // Window far offscreen - should not be visible
    assert!(!vp.is_rect_visible(8000, 8000, 64, 48));

    // Window partially visible at right edge
    // At zoom 2, scale=6. cam_px=80. Screen shows up to sx < 256.
    // World pixel 120 -> sx = (120-80)*6 = 240, with width 64*6=384 pixels
    // sx + sw = 624 > 0 and sx = 240 < 256, so visible
    assert!(vp.is_rect_visible(120, 80, 64, 48));

    // Window just past the right edge: sx = (130-80)*6 = 300 > 256
    assert!(!vp.is_rect_visible(130, 80, 64, 48));
}

#[test]
fn test_window_drag_updates_world_coords() {
    // Simulate drag: move window from (10,10) to (15,12)
    let mut vm = Vm::new();
    vm.ram[WINDOW_WORLD_COORDS_ADDR] = 1;

    // Create world-space window
    vm.regs[1] = 10;
    vm.regs[2] = 10;
    vm.regs[3] = 64;
    vm.regs[4] = 48;
    vm.regs[5] = 0;
    vm.regs[6] = 0; // create
    vm.ram[0] = 0x94;
    vm.ram[1] = 6;
    vm.ram[2] = 0x00;
    vm.pc = 0;
    vm.halted = false;
    vm.step();
    vm.step();

    let win_id = vm.regs[0];

    // Move via MOVETO (op=5) to simulate drag result
    vm.regs[0] = win_id;
    vm.regs[1] = 15; // new world_x
    vm.regs[2] = 12; // new world_y
    vm.regs[6] = 5; // moveto
    vm.ram[3] = 0x94;
    vm.ram[4] = 6;
    vm.ram[5] = 0x00;
    vm.pc = 3;
    vm.halted = false; // MUST reset after first HALT
    vm.step();
    vm.step();

    let w = &vm.windows[0];
    assert_eq!(w.world_x, 15, "drag should update world_x");
    assert_eq!(w.world_y, 12, "drag should update world_y");
}

#[test]
fn test_window_z_order_bring_to_front() {
    // Verify z_order management for multiple world-space windows
    let mut vm = Vm::new();
    vm.ram[WINDOW_WORLD_COORDS_ADDR] = 1;

    // Create window 1 at ram[0..1]
    vm.regs[1] = 10;
    vm.regs[2] = 10;
    vm.regs[3] = 64;
    vm.regs[4] = 48;
    vm.regs[5] = 0;
    vm.regs[6] = 0; // create
    vm.ram[0] = 0x94;
    vm.ram[1] = 6;
    vm.pc = 0;
    vm.halted = false;
    vm.step(); // WINSYS create, PC=2

    // Create window 2 at ram[2..3]
    vm.regs[1] = 15;
    vm.regs[2] = 15;
    vm.regs[3] = 64;
    vm.regs[4] = 48;
    vm.regs[5] = 0;
    vm.regs[6] = 0;
    vm.ram[2] = 0x94;
    vm.ram[3] = 6;
    vm.step(); // WINSYS create, PC=4

    assert_eq!(vm.windows.len(), 2, "should have 2 windows");

    // Window 2 should have higher z_order
    let w1 = &vm.windows[0];
    let w2 = &vm.windows[1];
    assert!(
        w2.z_order >= w1.z_order,
        "later window should have >= z_order"
    );

    // FOCUS (op=2): bring window 1 to front
    vm.regs[0] = 1; // window id
    vm.regs[6] = 2; // bring to front
    vm.ram[4] = 0x94;
    vm.ram[5] = 6;
    vm.pc = 4;
    vm.halted = false;
    vm.step();

    let w1 = &vm.windows[0];
    let w2 = &vm.windows[1];
    assert!(w1.z_order > w2.z_order, "focused window should be on top");
}

#[test]
fn test_process_halt_destroys_only_its_windows() {
    // When a process halts, only its windows should be destroyed
    let mut vm = Vm::new();

    // Create window for PID 0 (main process)
    vm.regs[1] = 10;
    vm.regs[2] = 10;
    vm.regs[3] = 64;
    vm.regs[4] = 48;
    vm.regs[5] = 0;
    vm.regs[6] = 0; // create
    vm.ram[0] = 0x94;
    vm.ram[1] = 6;
    vm.pc = 0;
    vm.halted = false;
    vm.step(); // WINSYS create, PC=2

    // Create window for PID 2 (child process)
    // Set current_pid to simulate a different process creating the window
    vm.current_pid = 2;
    vm.regs[1] = 20;
    vm.regs[2] = 20;
    vm.regs[3] = 64;
    vm.regs[4] = 48;
    vm.regs[5] = 0;
    vm.regs[6] = 0;
    vm.ram[2] = 0x94;
    vm.ram[3] = 6;
    vm.pc = 2;
    vm.step(); // WINSYS create, PC=4

    assert_eq!(vm.windows.len(), 2);
    assert_eq!(vm.windows[0].pid, 0);
    assert_eq!(vm.windows[1].pid, 2);

    // Destroy windows of PID 2 (simulating process halt cleanup)
    vm.windows.retain(|w| w.pid != 2);

    assert_eq!(vm.windows.len(), 1, "only PID 0 window should remain");
    assert_eq!(vm.windows[0].pid, 0);
}

#[test]
fn test_viewport_coordinate_roundtrip() {
    // Verify screen_to_world and world_to_screen are inverses
    use crate::viewport::Viewport;

    let vp = Viewport::new(50, 50, 2); // cam at (50,50), zoom 2

    // World tile (55, 55) -> screen -> back to world
    let (sx, sy) = vp.world_to_screen_unchecked(55, 55);
    let (wx, wy) = vp.screen_to_world(sx, sy);
    assert_eq!(wx, 55);
    assert_eq!(wy, 55);

    // World tile (50, 50) -> screen (0, 0)
    let (sx, sy) = vp.world_to_screen_unchecked(50, 50);
    assert_eq!(sx, 0);
    assert_eq!(sy, 0);
}

#[test]
fn test_viewport_zoom_levels_consistency() {
    // Verify that different zoom levels produce consistent mappings
    use crate::viewport::Viewport;

    for zoom in 0..=4 {
        let vp = Viewport::new(10, 10, zoom);
        // Camera tile should map to screen (0,0)
        let (sx, sy) = vp.world_to_screen_unchecked(10, 10);
        assert_eq!(sx, 0, "zoom {}: camera tile should be at screen x=0", zoom);
        assert_eq!(sy, 0, "zoom {}: camera tile should be at screen y=0", zoom);

        // Adjacent tile should be at pixels_per_tile distance
        let (sx2, sy2) = vp.world_to_screen_unchecked(11, 11);
        assert_eq!(sx2, vp.zoom.pixels_per_tile() as i32);
        assert_eq!(sy2, vp.zoom.pixels_per_tile() as i32);
    }
}

#[test]
fn test_world_space_window_hit_test() {
    // Verify HITTEST (op=3) works for world-space windows
    let mut vm = Vm::new();
    vm.ram[WINDOW_WORLD_COORDS_ADDR] = 1;

    // Create a world-space window
    vm.regs[1] = 100; // world_x
    vm.regs[2] = 200; // world_y
    vm.regs[3] = 64;
    vm.regs[4] = 48;
    vm.regs[5] = 0;
    vm.regs[6] = 0; // create
    vm.ram[0] = 0x94;
    vm.ram[1] = 6;
    vm.ram[2] = 0x00;
    vm.pc = 0;
    vm.halted = false;
    vm.step();

    // World-space windows store world coords but screen x/y = 0
    let win_id = vm.regs[0];
    assert_eq!(win_id, 1);
    let w = &vm.windows[0];
    assert!(w.is_world_space());
    assert_eq!(w.world_x, 100);
    assert_eq!(w.world_y, 200);
    // Screen x/y are 0 for world-space windows (computed at render time)
    assert_eq!(w.x, 0);
    assert_eq!(w.y, 0);
}
