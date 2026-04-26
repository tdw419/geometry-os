use geometry_os::assembler;
use geometry_os::vm::Vm;

fn step_until_frame(vm: &mut Vm, max: u32) -> u32 {
    vm.frame_ready = false;
    for i in 0..max {
        if vm.frame_ready {
            return i;
        }
        if !vm.step() {
            return i;
        }
    }
    max
}

#[test]
fn trace_disasm() {
    let source = std::fs::read_to_string("programs/world_desktop.asm").unwrap();
    let asm = assembler::assemble(&source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    vm.key_bitmask = 0;

    // Frame 1
    step_until_frame(&mut vm, 1_500_000);
    // Frame 2: '/'
    vm.push_key(47);
    step_until_frame(&mut vm, 2_000_000);

    // Frame 3: 'A' - trace with disassembly
    vm.push_key(65);
    vm.frame_ready = false;

    let mut history: Vec<(u32, u32, String)> = vec![];
    for i in 0..2_000_000u32 {
        let pc_before = vm.pc;
        let (disasm, _) = vm.disassemble_at(pc_before);

        if vm.frame_ready {
            break;
        }
        if !vm.step() {
            break;
        }

        history.push((i, pc_before, disasm));
        if history.len() > 100 {
            history.remove(0);
        }

        if vm.frame_ready {
            eprintln!("=== Last 100 instructions before FRAME (step {}) ===", i);
            for (step, pc, d) in &history {
                eprintln!("  step={} pc={} {}", step, pc, d);
            }
            break;
        }
    }

    panic!("DISASM DONE");
}
