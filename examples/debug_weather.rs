use geometry_os::assembler::assemble;
use geometry_os::vm::Vm;

fn main() {
    let source = std::fs::read_to_string("programs/living_map.asm").unwrap();
    let asm = assemble(&source, 0).unwrap();
    let mut vm = Vm::new();
    for (i, &v) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() { vm.ram[i] = v; }
    }
    vm.pc = 0;
    vm.halted = false;
    
    // Force rain
    vm.ram[0x7809] = 1;
    vm.ram[0x780A] = 100;
    
    // Check initial rain particles
    println!("=== Before running ===");
    println!("weather_state: {}", vm.ram[0x7809]);
    println!("weather_timer: {}", vm.ram[0x780A]);
    for i in 0..5 {
        let x = vm.ram[0x7010 + i*2];
        let y = vm.ram[0x7010 + i*2 + 1];
        println!("rain[{}]: x={}, y={}", i, x, y);
    }
    
    // Run one frame
    for _ in 0..3_000_000 {
        if vm.halted { break; }
        if !vm.step() { break; }
        if vm.frame_ready {
            vm.frame_ready = false;
            break;
        }
    }
    
    println!("\n=== After 1 frame ===");
    println!("weather_state: {}", vm.ram[0x7809]);
    println!("weather_timer: {}", vm.ram[0x780A]);
    println!("halted: {}", vm.halted);
    for i in 0..5 {
        let x = vm.ram[0x7010 + i*2];
        let y = vm.ram[0x7010 + i*2 + 1];
        println!("rain[{}]: x={}, y={}", i, x, y);
    }
    
    // Count blue pixels
    let rain_color = 0x4488FFu32;
    let count = vm.screen.iter().filter(|&&p| p == rain_color).count();
    println!("\nrain pixels (0x4488FF): {}", count);
    
    // Check non-black
    let non_black = vm.screen.iter().filter(|&&p| p != 0).count();
    println!("non-black pixels: {}", non_black);
}
