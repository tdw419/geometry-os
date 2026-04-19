// Verify what the DTB memory node says vs what memblock has
use geometry_os::riscv::dtb;
use geometry_os::riscv::RiscvVm;

fn main() {
    let config = dtb::DtbConfig {
        ram_base: 0,
        ram_size: 256 * 1024 * 1024,
        ..Default::default()
    };
    let blob = dtb::generate_dtb(&config);
    eprintln!("DTB size: {} bytes", blob.len());

    // Parse DTB to find memory node
    // FDT magic is at offset 0
    let magic = u32::from_be_bytes([blob[0], blob[1], blob[2], blob[3]]);
    eprintln!("Magic: 0x{:08X} (expect 0xD00DFEED)", magic);

    // Find "memory" string and the node before it
    let memory_str = b"memory@";
    for i in 0..blob.len() - 8 {
        if &blob[i..i + 8] == memory_str {
            eprintln!("Found 'memory@' at offset {}", i);
            // Read the following property - should be "reg" with address/size
            break;
        }
    }

    // Find the reg property (typically right after the node token)
    // Look for "reg" followed by address/size pairs
    let reg_str = b"reg";
    for i in 0..blob.len() - 3 {
        if &blob[i..i + 3] == reg_str {
            // Check if followed by 0x00 (null terminator)
            if i + 3 < blob.len() && blob[i + 3] == 0 {
                eprintln!("Found 'reg' at offset {}", i);
                // After the null terminator, there's a 4-byte length, then data
                let len_offset = i + 4;
                let prop_len = u32::from_be_bytes([
                    blob[len_offset],
                    blob[len_offset + 1],
                    blob[len_offset + 2],
                    blob[len_offset + 3],
                ]) as usize;
                eprintln!("  Property length: {} bytes", prop_len);

                // Read address cells and size cells
                // For 32-bit RISC-V: address_cells=1, size_cells=1
                // So each is 4 bytes
                let data_start = len_offset + 4;
                if prop_len >= 8 {
                    let addr = u32::from_be_bytes([
                        blob[data_start],
                        blob[data_start + 1],
                        blob[data_start + 2],
                        blob[data_start + 3],
                    ]);
                    let size = u32::from_be_bytes([
                        blob[data_start + 4],
                        blob[data_start + 5],
                        blob[data_start + 6],
                        blob[data_start + 7],
                    ]);
                    eprintln!(
                        "  Memory: base=0x{:08X} size=0x{:08X} ({}MB)",
                        addr,
                        size,
                        size / (1024 * 1024)
                    );
                }
                break;
            }
        }
    }

    // Also check bootargs
    let chosen_str = b"chosen";
    for i in 0..blob.len() - 6 {
        if &blob[i..i + 6] == chosen_str && (i + 6 >= blob.len() || blob[i + 6] == 0) {
            eprintln!("Found 'chosen' at offset {}", i);
            // Look for bootargs after this
            let bootargs_str = b"bootargs";
            for j in i..blob.len() - 8 {
                if &blob[j..j + 8] == bootargs_str && blob[j + 8] == 0 {
                    let len_offset = j + 9;
                    let prop_len = u32::from_be_bytes([
                        blob[len_offset],
                        blob[len_offset + 1],
                        blob[len_offset + 2],
                        blob[len_offset + 3],
                    ]) as usize;
                    let data_start = len_offset + 4;
                    let bootargs =
                        String::from_utf8_lossy(&blob[data_start..data_start + prop_len]);
                    eprintln!("  bootargs: '{}'", bootargs);
                    break;
                }
            }
            break;
        }
    }
}
