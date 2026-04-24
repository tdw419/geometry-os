use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use geometry_os::vm::Vm;

/// Create a VM with a simple LDI loop loaded at address 0
fn vm_with_loop() -> Vm {
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

/// Create a VM with a FILL + PSET program (graphics-heavy)
fn vm_with_graphics() -> Vm {
    let source = r#"
        LDI r0, 0x0000FF
        FILL r0
        LDI r1, 0
        LDI r2, 0
        LDI r3, 0xFF0000
        LDI r4, 1
        LDI r5, 256
    xloop:
        PSET r2, r1, r3
        ADD r2, r4
        CMP r2, r5
        BLT r0, xloop
        ADD r1, r4
        CMP r1, r5
        BLT r0, xloop
        HALT
    "#;
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

/// Create a VM with a RECTF program (fill rectangles)
fn vm_with_rectf() -> Vm {
    let source = r#"
        LDI r0, 0x00FF00
        LDI r1, 10
        LDI r2, 10
        LDI r3, 50
        LDI r4, 50
        RECTF r1, r2, r3, r4, r0
        HALT
    "#;
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

/// Benchmark: VM step throughput (arithmetic loop)
fn bench_vm_step_arithmetic(c: &mut Criterion) {
    let mut group = c.benchmark_group("vm_step_arithmetic");
    group.sample_size(20);

    group.bench_function("1000_iters", |b| {
        b.iter(|| {
            let mut vm = vm_with_loop();
            for _ in 0..10_000 {
                if !vm.step() {
                    break;
                }
            }
            black_box(&vm);
        });
    });

    group.finish();
}

/// Benchmark: VM step throughput (graphics - PSET)
fn bench_vm_step_graphics(c: &mut Criterion) {
    let mut group = c.benchmark_group("vm_step_graphics");
    group.sample_size(10);

    group.bench_function("pset_fill", |b| {
        b.iter(|| {
            let mut vm = vm_with_graphics();
            for _ in 0..2_000_000 {
                if !vm.step() {
                    break;
                }
            }
            black_box(&vm);
        });
    });

    group.finish();
}

/// Benchmark: VM step throughput (RECTF)
fn bench_vm_step_rectf(c: &mut Criterion) {
    let mut group = c.benchmark_group("vm_step_rectf");
    group.sample_size(50);

    group.bench_function("single_rectf", |b| {
        b.iter(|| {
            let mut vm = vm_with_rectf();
            for _ in 0..100 {
                if !vm.step() {
                    break;
                }
            }
            black_box(&vm);
        });
    });

    group.finish();
}

/// Benchmark: Vm::new() construction
fn bench_vm_new(c: &mut Criterion) {
    c.bench_function("vm_new", |b| {
        b.iter(|| {
            let vm = Vm::new();
            black_box(&vm);
        });
    });
}

/// Benchmark: Canvas buffer read/write via LOAD/STORE
fn bench_canvas_buffer(c: &mut Criterion) {
    let mut group = c.benchmark_group("canvas_buffer");
    group.sample_size(50);

    // Write to canvas buffer
    group.bench_function("write_4k_cells", |b| {
        b.iter(|| {
            let mut vm = Vm::new();
            for i in 0..4096 {
                vm.canvas_buffer[i] = black_box(0x41424300 + i as u32);
            }
            black_box(&vm.canvas_buffer);
        });
    });

    // Read from canvas buffer
    group.bench_function("read_4k_cells", |b| {
        let vm = Vm::new();
        b.iter(|| {
            let mut sum: u32 = 0;
            for i in 0..4096 {
                sum = sum.wrapping_add(black_box(vm.canvas_buffer[i]));
            }
            black_box(sum);
        });
    });

    group.finish();
}

/// Benchmark: RAM bulk read/write
fn bench_ram_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("ram_access");
    group.sample_size(20);

    group.bench_function("sequential_write_64k", |b| {
        b.iter(|| {
            let mut vm = Vm::new();
            for i in 0..65536 {
                vm.ram[i] = black_box(i as u32);
            }
            black_box(&vm.ram);
        });
    });

    group.bench_function("sequential_read_64k", |b| {
        let mut vm = Vm::new();
        for i in 0..65536 {
            vm.ram[i] = i as u32;
        }
        b.iter(|| {
            let mut sum: u32 = 0;
            for i in 0..65536 {
                sum = sum.wrapping_add(black_box(vm.ram[i]));
            }
            black_box(sum);
        });
    });

    group.finish();
}

/// Benchmark: Single PSET (pixel set) throughput
fn bench_pset_single(c: &mut Criterion) {
    let source = r#"
        LDI r1, 128
        LDI r2, 128
        LDI r3, 0xFFFFFF
        PSET r1, r2, r3
        HALT
    "#;
    let asm = geometry_os::assembler::assemble(source, 0).unwrap();

    c.bench_function("pset_single", |b| {
        b.iter(|| {
            let mut vm = Vm::new();
            for (i, &w) in asm.pixels.iter().enumerate() {
                if i < vm.ram.len() {
                    vm.ram[i] = w;
                }
            }
            vm.pc = 0;
            vm.halted = false;
            for _ in 0..100 {
                if !vm.step() {
                    break;
                }
            }
            black_box(&vm);
        });
    });
}

/// Benchmark: TEXT opcode throughput
fn bench_text_opcode(c: &mut Criterion) {
    let source = r#"
        LDI r1, 10
        LDI r2, 10
        LDI r3, 0x2000
        LDI r4, 0x48
        STORE r3, r4
        LDI r4, 0x65
        LDI r5, 1
        ADD r3, r5
        STORE r3, r4
        LDI r4, 0x6C
        ADD r3, r5
        STORE r3, r4
        ADD r3, r5
        STORE r3, r4
        LDI r4, 0x6F
        ADD r3, r5
        STORE r3, r4
        LDI r3, 0x2000
        TEXT r1, r2, r3
        HALT
    "#;
    let asm = geometry_os::assembler::assemble(source, 0).unwrap();

    c.bench_function("text_opcode_hello", |b| {
        b.iter(|| {
            let mut vm = Vm::new();
            for (i, &w) in asm.pixels.iter().enumerate() {
                if i < vm.ram.len() {
                    vm.ram[i] = w;
                }
            }
            vm.pc = 0;
            vm.halted = false;
            for _ in 0..1000 {
                if !vm.step() {
                    break;
                }
            }
            black_box(&vm);
        });
    });
}

criterion_group!(
    benches,
    bench_vm_step_arithmetic,
    bench_vm_step_graphics,
    bench_vm_step_rectf,
    bench_vm_new,
    bench_canvas_buffer,
    bench_ram_access,
    bench_pset_single,
    bench_text_opcode,
);
criterion_main!(benches);
