"""
Test for Visual RAM (LOAD 204 / STORE 205 opcodes)

This test generates a SPIR-V binary that:
1. Pushes 42.0 onto the stack
2. Stores it to visual_ram[0] (STORE opcode 205)
3. Loads from visual_ram[0] (LOAD opcode 204)
4. Pushes 8.0
5. Adds (42.0 + 8.0 = 50.0)
6. Returns result

Expected: visual_ram[0] = 42.0, stack[0] = 50.0
"""

import sys
import struct
from pathlib import Path

# Add project root to path
sys.path.insert(0, str(Path(__file__).parent.parent))
from geometry_os_spirv.scripts.emit_spirv import (
    SPIRVEmitter,
    OP_CAPABILITY,
    OP_MEMORY_MODEL,
    OP_TYPE_FLOAT,
    OP_TYPE_VOID,
    OP_TYPE_FUNCTION,
    OP_FUNCTION,
    OP_LABEL,
    OP_CONSTANT,
    OP_FADD,
    OP_RETURN,
    OP_FUNCTION_END,
    GEO_VISUAL_LOAD,
    GEO_VISUAL_STORE,
)


def generate_visual_ram_test_program():
    """Generate SPIR-V binary for visual RAM test."""
    emitter = SPIRVEmitter()

    # Setup types
    float_id = emitter.next_id()
    void_id = emitter.next_id()
    func_type_id = emitter.next_id()
    main_func_id = emitter.next_id()
    label_id = emitter.next_id()

    # Header
    emitter.emit(OP_CAPABILITY, 1)  # Shader
    emitter.emit(OP_MEMORY_MODEL, 0, 1)  # Logical, GLSL450

    # Types
    emitter.emit(OP_TYPE_FLOAT, float_id, 32)
    emitter.emit(OP_TYPE_VOID, void_id)
    emitter.emit(OP_TYPE_FUNCTION, func_type_id, void_id)

    # Main function
    emitter.emit(OP_FUNCTION, void_id, main_func_id, 0, func_type_id)
    emitter.emit(OP_LABEL, label_id)

    # Program: 42.0 -> STORE 0 -> LOAD 0 -> 8.0 -> ADD -> result
    # 1. Push 42.0
    const_42_id = emitter.next_id()
    emitter.emit(OP_CONSTANT, float_id, const_42_id, 42.0)

    # 2. STORE to visual_ram[0] (custom opcode 205)
    # Format: [count|opcode, address]
    emitter.emit(GEO_VISUAL_STORE, 0)

    # 3. LOAD from visual_ram[0] (custom opcode 204)
    # Format: [count|opcode, address]
    load_result_id = emitter.next_id()
    emitter.emit(GEO_VISUAL_LOAD, load_result_id, 0)

    # 4. Push 8.0
    const_8_id = emitter.next_id()
    emitter.emit(OP_CONSTANT, float_id, const_8_id, 8.0)

    # 5. ADD
    add_result_id = emitter.next_id()
    emitter.emit(OP_FADD, float_id, add_result_id, load_result_id, const_8_id)

    # Finalize
    emitter.emit(OP_RETURN)
    emitter.emit(OP_FUNCTION_END)

    return emitter.finalize()


def generate_raw_spirv_test():
    """
    Generate raw SPIR-V binary for the WebGPU interpreter.
    The interpreter expects raw opcodes with word counts.
    """
    words = []

    # SPIR-V Header: Magic, Version, Generator, Bound, Reserved
    header = [0x07230203, 0x00010000, 0, 100, 0]
    words.extend(header)

    # OpConstant: Push 42.0 (opcode 43)
    # Word format: [wordCount | opcode, type, resultId, value]
    # 42.0 as float bits
    val_42 = struct.unpack('I', struct.pack('f', 42.0))[0]
    words.append((4 << 16) | 43)  # 4 words, opcode 43
    words.append(1)  # float type id
    words.append(2)  # result id
    words.append(val_42)  # 42.0

    # STORE to visual_ram[0] (custom opcode 205)
    # Word format: [wordCount | opcode, address]
    words.append((2 << 16) | 205)  # 2 words, opcode 205
    words.append(0)  # address 0

    # LOAD from visual_ram[0] (custom opcode 204)
    # Word format: [wordCount | opcode, address]
    words.append((2 << 16) | 204)  # 2 words, opcode 204
    words.append(0)  # address 0

    # OpConstant: Push 8.0 (opcode 43)
    val_8 = struct.unpack('I', struct.pack('f', 8.0))[0]
    words.append((4 << 16) | 43)  # 4 words, opcode 43
    words.append(1)  # float type id
    words.append(4)  # result id
    words.append(val_8)  # 8.0

    # FADD (opcode 129)
    # Word format: [wordCount | opcode, type, resultId, operand1, operand2]
    words.append((5 << 16) | 129)  # 5 words, opcode 129
    words.append(1)  # float type id
    words.append(5)  # result id
    words.append(3)  # loaded value
    words.append(4)  # 8.0

    # OpReturn (opcode 253)
    words.append((1 << 16) | 253)

    return struct.pack('<' + 'I' * len(words), *words)


if __name__ == "__main__":
    # Generate the test binary
    binary = generate_raw_spirv_test()

    output_path = Path(__file__).parent.parent / "test_visual_ram.spv"
    with open(output_path, "wb") as f:
        f.write(binary)

    print(f"Generated test program: {output_path}")
    print(f"Size: {len(binary)} bytes")

    # Print hex dump for debugging
    print("\nHex dump:")
    for i, word in enumerate(struct.unpack('<' + 'I' * (len(binary) // 4), binary)):
        print(f"  Word {i:2d}: 0x{word:08X}")
