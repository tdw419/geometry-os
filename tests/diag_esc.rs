use geometry_os::assembler;
use geometry_os::vm::Vm;

#[test]
fn trace_esc_detailed() {
    let source = std::fs::read_to_string("programs/world_desktop.asm").expect("not found");
    let asm = geometry_os::assembler::assemble(&source, 0).expect("assemble");
    let mut vm = geometry_os::vm::Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    vm.key_bitmask = 0;

    // Frame 1
    vm.frame_ready = false;
    let mut steps = 0u32;
    for i in 0..1_500_000u32 {
        if vm.frame_ready {
            steps = i;
            break;
        }
        if !vm.step() {
            steps = i;
            break;
        }
    }
    eprintln!("Frame 1: {} steps, CMD_MODE={}", steps, vm.ram[0x7830]);

    // Push '/' and run
    vm.frame_ready = false;
    vm.push_key(47);
    for i in 0..2_000_000u32 {
        if vm.frame_ready {
            steps = i;
            break;
        }
        if !vm.step() {
            steps = i;
            break;
        }
    }
    eprintln!(
        "Frame 2 (/): {} steps, CMD_MODE={}, CMD_LEN={}",
        steps, vm.ram[0x7830], vm.ram[0x7831]
    );
    eprintln!(
        "  buffer: head={}, tail={}",
        vm.key_buffer_head, vm.key_buffer_tail
    );

    // Push 'A' and run
    vm.frame_ready = false;
    vm.push_key(65);
    eprintln!(
        "  Before frame 3: buffer head={}, tail={}",
        vm.key_buffer_head, vm.key_buffer_tail
    );
    for i in 0..2_000_000u32 {
        if vm.frame_ready {
            steps = i;
            break;
        }
        if !vm.step() {
            steps = i;
            break;
        }
    }
    eprintln!(
        "Frame 3 (A): {} steps, CMD_MODE={}, CMD_LEN={}",
        steps, vm.ram[0x7830], vm.ram[0x7831]
    );
    eprintln!(
        "  buffer: head={}, tail={}",
        vm.key_buffer_head, vm.key_buffer_tail
    );

    // Push ESC and run
    vm.frame_ready = false;
    vm.push_key(27);
    eprintln!(
        "  Before frame 4: buffer head={}, tail={}",
        vm.key_buffer_head, vm.key_buffer_tail
    );
    for i in 0..2_000_000u32 {
        // Trace first 200 steps to see what happens with the key
        if i < 200 {
            let pc_before = vm.pc;
            let key_buf_h = vm.key_buffer_head;
            let key_buf_t = vm.key_buffer_tail;
            let r17_before = vm.regs[17];
            let result = vm.step();
            if !result {
                eprintln!("  Step {}: HALTED! pc was {}", i, pc_before);
                break;
            }
            // Detect IKEY (opcode 0x48) by checking if key_buffer_head changed
            if vm.key_buffer_head != key_buf_h {
                eprintln!(
                    "  Step {}: IKEY consumed key! key_buf was {}->{} val={}",
                    i, key_buf_h, vm.key_buffer_head, vm.regs[17]
                );
            }
            if vm.frame_ready {
                eprintln!("  Step {}: frame_ready at pc={}", i, vm.pc);
                steps = i;
                break;
            }
            if i == 199 {
                eprintln!("  ... continuing regular loop");
            }
            continue;
        }
        if vm.frame_ready {
            steps = i;
            break;
        }
        if !vm.step() {
            steps = i;
            break;
        }
    }
    eprintln!(
        "Frame 4 (ESC): {} steps, CMD_MODE={}, CMD_LEN={}",
        steps, vm.ram[0x7830], vm.ram[0x7831]
    );
    eprintln!(
        "  buffer: head={}, tail={}",
        vm.key_buffer_head, vm.key_buffer_tail
    );

    panic!("DIAGNOSTIC TRACE DONE");
}
