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
fn trace_esc_v2() {
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
    let s = step_until_frame(&mut vm, 1_500_000);
    eprintln!("Frame 1: {} steps", s);

    // Frame 2: '/'
    vm.push_key(47);
    let s = step_until_frame(&mut vm, 2_000_000);
    eprintln!("Frame 2 (/): {} steps, CMD_MODE={}", s, vm.ram[0x7830]);

    // Frame 3: 'A'
    vm.push_key(65);
    let s = step_until_frame(&mut vm, 2_000_000);
    eprintln!(
        "Frame 3 (A): {} steps, CMD_MODE={}, CMD_LEN={}",
        s, vm.ram[0x7830], vm.ram[0x7831]
    );

    // Frame 4: ESC - detailed trace
    vm.push_key(27);
    vm.frame_ready = false;
    eprintln!("=== Frame 4 detailed trace ===");
    eprintln!("CMD_MODE at start of frame 4: {}", vm.ram[0x7830]);
    eprintln!(
        "Buffer: head={}, tail={}",
        vm.key_buffer_head, vm.key_buffer_tail
    );

    let mut found_ikey = false;
    let mut ikey_step = 0u32;
    let mut ikey_val = 0u32;

    for i in 0..2_000_000u32 {
        if vm.frame_ready {
            eprintln!("FRAME reached at step {}", i);
            break;
        }
        let buf_head_before = vm.key_buffer_head;

        if !vm.step() {
            eprintln!("HALTED at step {}, pc was {}", i, vm.pc);
            break;
        }

        // Detect IKEY by key_buffer_head change
        if vm.key_buffer_head != buf_head_before && !found_ikey {
            found_ikey = true;
            ikey_step = i;
            ikey_val = vm.regs[17];
            eprintln!("Step {}: IKEY consumed key! r17={}", i, vm.regs[17]);
        }
    }

    eprintln!("CMD_MODE at end: {}", vm.ram[0x7830]);
    eprintln!("CMD_LEN at end: {}", vm.ram[0x7831]);
    eprintln!(
        "found_ikey={} at step {} val={}",
        found_ikey, ikey_step, ikey_val
    );

    panic!("TRACE V2 DONE");
}
