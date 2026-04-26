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
fn trace_esc_pc() {
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
    eprintln!("Frame 1: {} steps, PC after={}", s, vm.pc);

    // Frame 2: '/'
    vm.push_key(47);
    let s = step_until_frame(&mut vm, 2_000_000);
    eprintln!("Frame 2 (/): {} steps, PC after={}", s, vm.pc);

    // Frame 3: 'A'
    vm.push_key(65);
    vm.frame_ready = false;
    let mut frame3_steps = 0u32;
    for i in 0..2_000_000u32 {
        if vm.frame_ready {
            frame3_steps = i;
            break;
        }
        if !vm.step() {
            frame3_steps = i;
            break;
        }
    }
    eprintln!("Frame 3 (A): {} steps, PC after={}", frame3_steps, vm.pc);
    eprintln!("  CMD_MODE={}, CMD_LEN={}", vm.ram[0x7830], vm.ram[0x7831]);
    eprintln!("  frame_ready={}, halted={}", vm.frame_ready, vm.halted);

    let pc = vm.pc as usize;
    let end = (pc + 10).min(vm.ram.len());
    eprintln!("  RAM at PC[{}..{}]: {:?}", pc, end, &vm.ram[pc..end]);

    // Frame 4: ESC
    vm.push_key(27);
    vm.frame_ready = false;
    let mut frame4_steps = 0u32;
    let mut saw_ikey = false;
    let mut first_ikey_step = 0u32;
    for i in 0..2_000_000u32 {
        if vm.frame_ready {
            frame4_steps = i;
            break;
        }
        let head_before = vm.key_buffer_head;
        if !vm.step() {
            frame4_steps = i;
            break;
        }
        if vm.key_buffer_head != head_before && !saw_ikey {
            saw_ikey = true;
            first_ikey_step = i;
        }
    }
    eprintln!(
        "Frame 4 (ESC): {} steps, saw_ikey={}, first_ikey_step={}",
        frame4_steps, saw_ikey, first_ikey_step
    );
    eprintln!("  CMD_MODE={}, CMD_LEN={}", vm.ram[0x7830], vm.ram[0x7831]);

    panic!("TRACE PC DONE");
}
