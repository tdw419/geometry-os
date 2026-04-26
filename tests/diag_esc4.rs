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
fn trace_esc_v4() {
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

    // Frame 3: 'A' - trace to find what triggers frame_ready
    vm.push_key(65);
    vm.frame_ready = false;
    for i in 0..2_000_000u32 {
        let pc_before = vm.pc;
        let ram_at_pc = vm.ram[pc_before as usize];

        if vm.frame_ready {
            eprintln!("Frame 3: frame_ready already set at step {}!", i);
            break;
        }

        if !vm.step() {
            eprintln!("Frame 3: halted at step {}, pc={}", i, pc_before);
            break;
        }

        if vm.frame_ready {
            eprintln!(
                "Frame 3: frame_ready set at step {}, pc_before={} ram={}",
                i, pc_before, ram_at_pc
            );

            // Disassemble the instruction that caused it
            let opcode = ram_at_pc;
            eprintln!("  Opcode: 0x{:02X}", opcode);
            if opcode == 0x02 {
                eprintln!("  -> FRAME opcode");
            } else if opcode == 0x7C {
                eprintln!("  -> REPLAY opcode!");
            }
            break;
        }
    }

    eprintln!("PC after frame 3: {}", vm.pc);
    eprintln!("CMD_MODE={}, CMD_LEN={}", vm.ram[0x7830], vm.ram[0x7831]);

    panic!("TRACE V4 DONE");
}
