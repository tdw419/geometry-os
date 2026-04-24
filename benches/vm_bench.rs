use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use geometry_os::vm::Vm;

/// Create a VM pre-loaded with bytecode at address 0
fn vm_with_source(source: &str) -> Vm {
    let asm = geometry_os::assembler::assemble(source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &w) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = w;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    vm
}

/// Run VM for N steps, returning instruction count
fn run_steps(vm: &mut Vm, max_steps: usize) -> usize {
    let mut count = 0;
    for _ in 0..max_steps {
        if !vm.step() {
            break;
        }
        count += 1;
    }
    count
}

// ============================================================
// 1. VM Construction
// ============================================================

fn bench_vm_new(c: &mut Criterion) {
    c.bench_function("vm_new", |b| {
        b.iter(|| {
            let vm = Vm::new();
            black_box(&vm);
        });
    });
}

// ============================================================
// 2. Per-instruction benchmarks (arithmetic only, no graphics)
// ============================================================

fn bench_arithmetic_loop(c: &mut Criterion) {
    // 1000 iterations of ADD + CMP + BLT
    let source = r#"
        LDI r1, 0
        LDI r2, 1000
        LDI r7, 1
    loop:
        ADD r1, r7
        CMP r1, r2
        BLT r0, loop
        HALT
    "#;

    let mut group = c.benchmark_group("vm_arithmetic");
    group.throughput(Throughput::Elements(3000)); // ~3 instructions per iter * 1000
    group.bench_function("1000_iters", |b| {
        b.iter_batched(
            || vm_with_source(source),
            |mut vm| {
                let steps = run_steps(&mut vm, 10_000);
                black_box(steps);
            },
            criterion::BatchSize::SmallInput,
        );
    });
    group.finish();
}

// ============================================================
// 3. Memory access patterns (LOAD/STORE in running VM)
// ============================================================

fn bench_memory_access(c: &mut Criterion) {
    // Sequential STORE: write 256 values
    let store_source = r#"
        LDI r1, 0x2000    ; base address
        LDI r2, 0
        LDI r3, 256
        LDI r4, 1
        LDI r5, 42
    loop:
        STORE r1, r5
        ADD r1, r4
        ADD r2, r4
        CMP r2, r3
        BLT r0, loop
        HALT
    "#;

    // Sequential LOAD: read 256 values
    let load_source = r#"
        LDI r1, 0x2000
        LDI r2, 0
        LDI r3, 256
        LDI r4, 1
        LDI r6, 0         ; accumulator
    loop:
        LOAD r5, r1
        ADD r6, r5
        ADD r1, r4
        ADD r2, r4
        CMP r2, r3
        BLT r0, loop
        HALT
    "#;

    let mut group = c.benchmark_group("vm_memory");
    group.throughput(Throughput::Elements(256));

    group.bench_function("store_256_seq", |b| {
        b.iter_batched(
            || vm_with_source(store_source),
            |mut vm| {
                let steps = run_steps(&mut vm, 10_000);
                black_box(steps);
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.bench_function("load_256_seq", |b| {
        b.iter_batched(
            || vm_with_source(load_source),
            |mut vm| {
                let steps = run_steps(&mut vm, 10_000);
                black_box(steps);
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

// ============================================================
// 4. Graphics opcodes (PSET, RECTF, FILL)
// ============================================================

fn bench_graphics(c: &mut Criterion) {
    // PSET: single pixel
    let pset_source = r#"
        LDI r1, 128
        LDI r2, 128
        LDI r3, 0xFFFFFF
        PSET r1, r2, r3
        HALT
    "#;

    // FILL: fill entire screen
    let fill_source = r#"
        LDI r0, 0x00FF00
        FILL r0
        HALT
    "#;

    // RECTF: fill a 50x50 rectangle
    let rectf_source = r#"
        LDI r0, 0xFF0000
        LDI r1, 10
        LDI r2, 10
        LDI r3, 50
        LDI r4, 50
        RECTF r1, r2, r3, r4, r0
        HALT
    "#;

    let mut group = c.benchmark_group("vm_graphics");
    group.throughput(Throughput::Elements(1));

    group.bench_function("pset_single", |b| {
        b.iter_batched(
            || vm_with_source(pset_source),
            |mut vm| {
                run_steps(&mut vm, 100);
                black_box(&vm);
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.throughput(Throughput::Elements(65536)); // 256*256 pixels
    group.bench_function("fill_screen", |b| {
        b.iter_batched(
            || vm_with_source(fill_source),
            |mut vm| {
                run_steps(&mut vm, 100);
                black_box(&vm);
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.throughput(Throughput::Elements(2500)); // 50*50 pixels
    group.bench_function("rectf_50x50", |b| {
        b.iter_batched(
            || vm_with_source(rectf_source),
            |mut vm| {
                run_steps(&mut vm, 100);
                black_box(&vm);
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

// ============================================================
// 5. Canvas buffer RAM interception (LOAD/STORE at 0x8000)
// ============================================================

fn bench_canvas_intercept(c: &mut Criterion) {
    // Store to canvas buffer range (0x8000-0x8FFF)
    let canvas_write = r#"
        LDI r1, 0x8000
        LDI r2, 0
        LDI r3, 256
        LDI r4, 1
        LDI r5, 65         ; 'A'
    loop:
        STORE r1, r5
        ADD r1, r4
        ADD r2, r4
        CMP r2, r3
        BLT r0, loop
        HALT
    "#;

    // Load from canvas buffer range
    let canvas_read = r#"
        LDI r1, 0x8000
        LDI r2, 0
        LDI r3, 256
        LDI r4, 1
        LDI r6, 0
    loop:
        LOAD r5, r1
        ADD r6, r5
        ADD r1, r4
        ADD r2, r4
        CMP r2, r3
        BLT r0, loop
        HALT
    "#;

    let mut group = c.benchmark_group("vm_canvas_intercept");
    group.throughput(Throughput::Elements(256));

    group.bench_function("write_256_canvas", |b| {
        b.iter_batched(
            || vm_with_source(canvas_write),
            |mut vm| {
                run_steps(&mut vm, 10_000);
                black_box(&vm);
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.bench_function("read_256_canvas", |b| {
        b.iter_batched(
            || vm_with_source(canvas_read),
            |mut vm| {
                run_steps(&mut vm, 10_000);
                black_box(&vm);
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

// ============================================================
// 6. Branch prediction pattern (tight JMP loop)
// ============================================================

fn bench_branch(c: &mut Criterion) {
    // Unconditional JMP loop (count via JNZ)
    let jmp_loop = r#"
        LDI r1, 1000
        LDI r7, 1
    loop:
        SUB r1, r7
        JNZ r1, loop
        HALT
    "#;

    // CMP + BLT loop
    let cmp_blt_loop = r#"
        LDI r1, 0
        LDI r2, 1000
        LDI r7, 1
    loop:
        ADD r1, r7
        CMP r1, r2
        BLT r0, loop
        HALT
    "#;

    let mut group = c.benchmark_group("vm_branch");
    group.throughput(Throughput::Elements(1000));

    group.bench_function("jmp_loop_1000", |b| {
        b.iter_batched(
            || vm_with_source(jmp_loop),
            |mut vm| {
                run_steps(&mut vm, 10_000);
                black_box(&vm);
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.bench_function("cmp_blt_loop_1000", |b| {
        b.iter_batched(
            || vm_with_source(cmp_blt_loop),
            |mut vm| {
                run_steps(&mut vm, 10_000);
                black_box(&vm);
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

// ============================================================
// 7. CALL/RET (subroutine overhead)
// ============================================================

fn bench_call_ret(c: &mut Criterion) {
    let source = r#"
        LDI r1, 1000
        LDI r7, 1
        LDI r30, 0xFF00
    loop:
        CALL helper
        SUB r1, r7
        JNZ r1, loop
        HALT
    helper:
        LDI r10, 1
        ADD r10, r10
        RET
    "#;

    let mut group = c.benchmark_group("vm_call_ret");
    group.throughput(Throughput::Elements(1000));

    group.bench_function("call_ret_1000", |b| {
        b.iter_batched(
            || vm_with_source(source),
            |mut vm| {
                run_steps(&mut vm, 100_000);
                black_box(&vm);
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

// ============================================================
// 8. MUL/DIV heavy (compute-intensive)
// ============================================================

fn bench_muldiv(c: &mut Criterion) {
    let source = r#"
        LDI r1, 100
        LDI r2, 37
        LDI r3, 13
        LDI r7, 1
        LDI r4, 0
        LDI r5, 1000
    loop:
        MUL r4, r2
        DIV r4, r3
        ADD r1, r7
        CMP r1, r5
        BLT r0, loop
        HALT
    "#;

    let mut group = c.benchmark_group("vm_muldiv");
    group.throughput(Throughput::Elements(900)); // ~3 compute + 3 loop per iter * 900 iters

    group.bench_function("mul_div_900", |b| {
        b.iter_batched(
            || vm_with_source(source),
            |mut vm| {
                run_steps(&mut vm, 100_000);
                black_box(&vm);
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

// ============================================================
// 9. SPAWN + KILL (process management overhead)
// ============================================================

fn bench_spawn_kill(c: &mut Criterion) {
    let source = r#"
        LDI r1, child
        SPAWN r1
        KILL r1
        HALT
    .org 0x400
    child:
        LDI r10, 1
        ADD r10, r10
        JMP child
    "#;

    let mut group = c.benchmark_group("vm_process");
    group.throughput(Throughput::Elements(1));

    group.bench_function("spawn_kill_single", |b| {
        b.iter_batched(
            || vm_with_source(source),
            |mut vm| {
                run_steps(&mut vm, 10_000);
                black_box(&vm);
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

// ============================================================
// 10. Preprocessor + assemble pipeline
// ============================================================

fn bench_preprocessor(c: &mut Criterion) {
    let source_with_vars = r#"
        VAR count 0x7000
        VAR limit 0x7001
        SET count, 0
        SET limit, 1000
    loop:
        LDI r1, 1
        GET r2, count
        ADD r2, r1
        LDI r3, count
        STORE r3, r2
        GET r4, limit
        CMP r2, r4
        BLT r0, loop
        HALT
    "#;

    let mut group = c.benchmark_group("preprocessor");
    group.throughput(Throughput::Elements(15)); // ~15 lines

    group.bench_function("assemble_with_vars", |b| {
        b.iter(|| {
            let mut pp = geometry_os::preprocessor::Preprocessor::new();
            let preprocessed = pp.preprocess(black_box(source_with_vars));
            let result = geometry_os::assembler::assemble(&preprocessed, 0);
            black_box(&result);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_vm_new,
    bench_arithmetic_loop,
    bench_memory_access,
    bench_graphics,
    bench_canvas_intercept,
    bench_branch,
    bench_call_ret,
    bench_muldiv,
    bench_spawn_kill,
    bench_preprocessor,
);
criterion_main!(benches);
