use geometry_os::assembler::assemble;

fn main() {
    let source = std::fs::read_to_string("programs/world_desktop.asm").unwrap();
    let asm = assemble(&source, 0).unwrap();

    // Disassemble properly using instruction widths
    // Find the last FRAME and what follows it
    // The main loop ends with FRAME + JMP main_loop

    // Walk and find all FRAME instructions with their proper context
    let mut pc: usize = 0;
    while pc < asm.pixels.len() {
        let op = asm.pixels[pc];

        if op == 0x02 {
            // Found FRAME. Check what comes after
            let next = if pc + 1 < asm.pixels.len() {
                asm.pixels[pc + 1]
            } else {
                0
            };
            let next2 = if pc + 2 < asm.pixels.len() {
                asm.pixels[pc + 2]
            } else {
                0
            };

            // Is it followed by JMP?
            if next == 0x30 {
                println!("FRAME at {}, followed by JMP to {}", pc, next2);
                if next2 == 2160 {
                    println!("  -> THIS IS THE MAIN LOOP FRAME!");
                }
            } else {
                println!("FRAME at {}, followed by op 0x{:02X}", pc, next);
            }
        }

        let width = match op {
            0x00 | 0x01 | 0x02 | 0x34 | 0x73 | 0x74 => 1,
            0x03
            | 0x10
            | 0x11
            | 0x12
            | 0x15
            | 0x16
            | 0x17
            | 0x20..=0x29
            | 0x2B
            | 0x31
            | 0x32
            | 0x35
            | 0x36
            | 0x4B
            | 0x50
            | 0x51 => 3,
            0x13 | 0x18 | 0x19 | 0x1A | 0x1B | 0x1C | 0x2A | 0x30 | 0x33 | 0x42 | 0x47 | 0x48
            | 0x49 | 0x4D | 0x4E | 0x60 | 0x61 => 2,
            0x40 | 0x41 | 0x44 | 0x4F | 0x6D | 0x75 => 4,
            0x46 => 5,
            0x43 | 0x45 | 0x4A | 0x8C => 6,
            0x4C => 9,
            0x14 => {
                if pc + 2 < asm.pixels.len() {
                    3 + asm.pixels[pc + 2] as usize
                } else {
                    3
                }
            }
            _ => 1,
        };
        if width == 0 {
            break;
        }
        pc += width;
        if pc >= asm.pixels.len() {
            break;
        }
    }
}
