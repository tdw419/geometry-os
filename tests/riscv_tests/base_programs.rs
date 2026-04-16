use super::*;

// ============================================================
// Multi-instruction programs
// ============================================================

#[test]
fn test_rv32_fibonacci() {
    // Compute fib(10) = 55
    // x1=a=0, x2=b=1, x3=counter=10
    let mut vm = test_vm(&[
        addi(1, 0, 0),  // a = 0
        addi(2, 0, 1),  // b = 1
        addi(3, 0, 10), // counter = 10
        add(4, 1, 2),   // temp = a + b        [inst 3]
        addi(1, 2, 0),  // a = b               [inst 4]
        addi(2, 4, 0),  // b = temp            [inst 5]
        addi(3, 3, -1), // counter--           [inst 6]
        bne(3, 0, -16), // if counter!=0 goto 3 [inst 7] -4 instr * 4 bytes
        ecall(),
    ]);
    run(&mut vm, 200);
    assert_eq!(vm.cpu.x[1], 55, "fib(10) should be 55 (x1=a)");
}

#[test]
fn test_rv32_fibonacci_20_iterations() {
    // Acceptance: Fibonacci(10) = 55, then continue to fib(20) = 6765
    // Phase 34 deliverable: "fibonacci test program that runs 20 iterations"
    //
    // After N iterations: a = fib(N), b = fib(N+1)
    //   fib(10) = 55, fib(20) = 6765
    //
    // x1=a=0, x2=b=1, x3=counter=20, x4=temp
    let mut vm = test_vm(&[
        addi(1, 0, 0),  // a = 0               [inst 0]
        addi(2, 0, 1),  // b = 1               [inst 1]
        addi(3, 0, 20), // counter = 20         [inst 2]
        add(4, 1, 2),   // temp = a + b         [inst 3] loop start
        addi(1, 2, 0),  // a = b               [inst 4]
        addi(2, 4, 0),  // b = temp            [inst 5]
        addi(3, 3, -1), // counter--           [inst 6]
        bne(3, 0, -16), // if counter!=0 goto 3 [inst 7]
        ecall(),        //                      [inst 8]
    ]);
    run(&mut vm, 400);
    assert_eq!(vm.cpu.x[1], 6765, "fib(20) should be 6765 (x1=a)");
}

#[test]
fn test_rv32_add_overflow() {
    let mut vm = test_vm(&[
        lui(1, 0x80000000),
        lui(2, 0x80000000),
        add(3, 1, 2), // wrapping: 0x80000000 + 0x80000000 = 0
        ecall(),
    ]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[3], 0);
}

#[test]
fn test_rv32_sub_underflow() {
    let mut vm = test_vm(&[addi(1, 0, 0), addi(2, 0, 1), sub(3, 1, 2), ecall()]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[3], 0xFFFFFFFF);
}

#[test]
fn test_rv32_loop_count() {
    let mut vm = test_vm(&[
        addi(1, 0, 0),  // counter = 0
        addi(2, 0, 1),  // increment
        addi(3, 0, 10), // limit
        add(1, 1, 2),   // counter++        [inst 3]
        bne(1, 3, -8),  // if counter!=10 goto 3 [inst 4]
        ecall(),
    ]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[1], 10);
}

#[test]
fn test_rv32_jal_function_call() {
    // Call a function that doubles x1
    let mut vm = test_vm(&[
        addi(1, 0, 21), // x1 = 21
        jal(5, 12),     // call function (skip 3 instr), x5 = 0x80000008
        ecall(),        // inst 2: return here, x1 should be 42
        addi(0, 0, 0), // inst 3: padding (NOP)
        add(1, 1, 1),  // inst 4: double x1
        jalr(0, 5, 0), // inst 5: return to x5
    ]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[1], 42);
}

#[test]
fn test_rv32_sum_1_to_10() {
    let mut vm = test_vm(&[
        addi(1, 0, 0),  // sum = 0
        addi(2, 0, 1),  // i = 1
        addi(3, 0, 11), // limit = 11
        add(1, 1, 2),   // sum += i          [inst 3]
        addi(2, 2, 1),  // i++               [inst 4]
        bne(2, 3, -12), // if i!=11 goto 3   [inst 5]
        ecall(),
    ]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[1], 55, "sum 1..10 should be 55");
}

#[test]
fn test_rv32_memory_roundtrip() {
    let mut vm = test_vm(&[
        auipc(5, 0x0),
        addi(5, 5, 200), // base = 0x800000C8
        lui(1, 0xDEADB000),
        ori(1, 1, 0x0EF), // x1 = 0xDEADB0EF (low 12 bits must have bit 11 = 0)
        sw(1, 5, 0),
        addi(2, 0, 0xCA),
        sh(2, 5, 4),     // store half at offset 4
        addi(3, 0, 0x42),
        sb(3, 5, 6),     // store byte at offset 6
        lw(10, 5, 0),    // x10 = 0xDEADBEEF
        lhu(11, 5, 4),   // x11 = 0x00CA
        lbu(12, 5, 6),   // x12 = 0x42
        ecall(),
    ]);
    run(&mut vm, 100);
    assert_eq!(vm.cpu.x[10], 0xDEADB0EF);
    assert_eq!(vm.cpu.x[11], 0x00CA);
    assert_eq!(vm.cpu.x[12], 0x42);
}
