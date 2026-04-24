use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use geometry_os::assembler::assemble;

/// Small program (hello world size)
const SMALL_PROGRAM: &str = r#"
LDI r1, 10
LDI r2, 20
ADD r1, r2
HALT
"#;

/// Medium program (with labels and jumps)
const MEDIUM_PROGRAM: &str = r#"
LDI r10, 0
LDI r1, 1
LDI r5, 256
loop:
  ADD r10, r1
  CMP r10, r5
  BLT r0, loop
done:
HALT
"#;

/// Large program (many instructions, simulating a game init + render loop)
const LARGE_PROGRAM: &str = r#"
; Initialize sine table
LDI r16, 0x2000
LDI r15, 256
LDI r7, 1
LDI r10, 0
build_sine:
  LDI r18, 128
  CMP r10, r18
  BGE r0, above_128
  MOV r17, r18
  SUB r17, r10
  JMP do_square
above_128:
  MOV r17, r10
  SUB r17, r18
do_square:
  MUL r17, r17
  LDI r18, 127
  MUL r17, r18
  LDI r18, 16384
  DIV r17, r18
  LDI r18, 255
  SUB r18, r17
  MOV r19, r16
  ADD r19, r10
  STORE r19, r18
  ADD r10, r7
  CMP r10, r15
  BLT r0, build_sine
HALT
"#;

/// Benchmark: Assembler throughput for small programs
fn bench_assemble_small(c: &mut Criterion) {
    let mut group = c.benchmark_group("assembler");
    group.throughput(Throughput::Elements(4));
    group.bench_function("small_4_lines", |b| {
        b.iter(|| {
            let result = assemble(black_box(SMALL_PROGRAM), 0);
            black_box(&result);
        });
    });
    group.finish();
}

/// Benchmark: Assembler throughput for medium programs
fn bench_assemble_medium(c: &mut Criterion) {
    let mut group = c.benchmark_group("assembler");
    group.throughput(Throughput::Elements(10));
    group.bench_function("medium_10_lines", |b| {
        b.iter(|| {
            let result = assemble(black_box(MEDIUM_PROGRAM), 0);
            black_box(&result);
        });
    });
    group.finish();
}

/// Benchmark: Assembler throughput for large programs
fn bench_assemble_large(c: &mut Criterion) {
    let mut group = c.benchmark_group("assembler");
    group.throughput(Throughput::Elements(30));
    group.bench_function("large_30_lines", |b| {
        b.iter(|| {
            let result = assemble(black_box(LARGE_PROGRAM), 0);
            black_box(&result);
        });
    });
    group.finish();
}

/// Benchmark: Assembler with many labels (two-pass resolution cost)
fn bench_assemble_many_labels(c: &mut Criterion) {
    let mut source = String::new();
    for i in 0..100 {
        source.push_str(&format!("label_{}:\n  LDI r1, {}\n  ADD r1, r1\n", i, i));
    }
    source.push_str("HALT\n");

    let mut group = c.benchmark_group("assembler");
    group.throughput(Throughput::Elements(300));
    group.bench_function("100_labels_300_lines", |b| {
        b.iter(|| {
            let result = assemble(black_box(&source), 0);
            black_box(&result);
        });
    });
    group.finish();
}

/// Benchmark: Assembler with .org directives (multi-segment)
fn bench_assemble_org(c: &mut Criterion) {
    let source = r#"
        LDI r1, child
        SPAWN r1
        LDI r2, 1000
    loop:
        ADD r1, r2
        CMP r1, r2
        BLT r0, loop
        HALT
    .org 0x400
    child:
        LDI r10, 0
    cloop:
        ADD r10, r1
        JMP cloop
    "#;

    let mut group = c.benchmark_group("assembler");
    group.throughput(Throughput::Elements(14));
    group.bench_function("with_org_directive", |b| {
        b.iter(|| {
            let result = assemble(black_box(source), 0);
            black_box(&result);
        });
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_assemble_small,
    bench_assemble_medium,
    bench_assemble_large,
    bench_assemble_many_labels,
    bench_assemble_org,
);
criterion_main!(benches);
