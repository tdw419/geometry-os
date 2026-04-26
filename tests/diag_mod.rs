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
fn trace_modification() {
    let source = std::fs::read_to_string("programs/world_desktop.asm").unwrap();
    let asm = assembler::assemble(&source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &word) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = word;
        }
    }
    vm.key_bitmask = 0;

    let original_4094 = vm.ram[4094];
    eprintln!("Initial ram[4094] = {}", original_4094);

    // Frame 1
    step_until_frame(&mut vm, 1_500_000);
    eprintln!("After frame 1: ram[4094] = {}", vm.ram[4094]);

    // Frame 2: '/'
    vm.push_key(47);
    step_until_frame(&mut vm, 2_000_000);
    eprintln!("After frame 2: ram[4094] = {}", vm.ram[4094]);

    // Frame 3: 'A' - watch for modification at address 4094
    vm.push_key(65);
    vm.frame_ready = false;
    for i in 0..2_000_000u32 {
        let ram_before = vm.ram[4094];
        let pc = vm.pc;

        if vm.frame_ready {
            break;
        }
        if !vm.step() {
            break;
        }

        if vm.ram[4094] != ram_before {
            eprintln!(
                "Step {}: ram[4094] changed {} -> {} (pc was {})",
                i, ram_before, vm.ram[4094], pc
            );
        }

        if vm.frame_ready {
            eprintln!("Frame 3 done at step {}, ram[4094] = {}", i, vm.ram[4094]);
            break;
        }
    }

    panic!("MOD TRACE DONE");
}
