/**
 * GPU-Native Visual Compiler
 * 
 * This shader runs as a system process (PID 1) that compiles visual
 * programs drawn in the VisualIDE to SPIR-V bytecode.
 * 
 * Memory Layout:
 * - INPUT_BUFFER  @ binding 0: Visual grid (64x64 = 4096 pixels)
 * - OUTPUT_BUFFER @ binding 1: Generated SPIR-V bytecode
 * - SYMBOL_TABLE  @ binding 2: Variable/function names
 * - ERROR_LOG     @ binding 3: Compilation errors
 * - CONTROL       @ binding 4: Control/status registers
 */

// Compiler control registers
struct ControlBlock {
    command: u32,        // 0=idle, 1=compile, 2=get_status
    input_size: u32,     // Size of input in pixels
    output_size: u32,    // Size of generated output
    status: u32,         // 0=idle, 1=compiling, 2=success, 3=error
    error_count: u32,    // Number of errors
    grid_width: u32,     // Grid dimensions
    grid_height: u32,
    _padding: u32,
}

// Pixel from visual grid
struct VisualPixel {
    r: u32,  // Red channel - operand 1 / constant high bits
    g: u32,  // Green channel - opcode
    b: u32,  // Blue channel - operand 2 / constant low bits
    a: u32,  // Alpha - metadata
}

// Compilation token
struct Token {
    opcode: u32,
    operand_a: f32,
    operand_b: f32,
    line: u32,
    column: u32,
}

// Error entry
struct CompileError {
    code: u32,     // Error code
    line: u32,     // Line number
    column: u32,   // Column number
    message: u32,  // Message offset in error string buffer
}

// SPIR-V header constants
const SPIRV_MAGIC: u32 = 0x07230203u;
const SPIRV_VERSION: u32 = 0x00010000u;
const SPIRV_GENERATOR: u32 = 0x474F5300u;  // "GOS\0"

// Opcode definitions (G channel values)
const OP_NOP: u32 = 0u;
const OP_CONSTANT: u32 = 43u;
const OP_FADD: u32 = 129u;
const OP_FSUB: u32 = 130u;
const OP_FMUL: u32 = 133u;
const OP_FDIV: u32 = 134u;
const OP_LOAD: u32 = 61u;
const OP_STORE: u32 = 62u;
const OP_SIN: u32 = 112u;
const OP_COS: u32 = 113u;
const OP_JMP: u32 = 144u;
const OP_JZ: u32 = 145u;
const OP_CALL: u32 = 147u;
const OP_RET: u32 = 148u;
const OP_DUP: u32 = 200u;
const OP_SWAP: u32 = 201u;
const OP_DROP: u32 = 202u;
const OP_PRINT: u32 = 210u;
const OP_HALT: u32 = 253u;

// Glyph to opcode mapping (G channel → instruction)
fn glyph_to_opcode(g: u32) -> u32 {
    switch (g) {
        case 0x6Au: { return OP_FADD; }   // ⊕ addition
        case 0x6Bu: { return OP_FSUB; }   // ⊖ subtraction
        case 0x6Cu: { return OP_FMUL; }   // ⊗ multiplication
        case 0x6Du: { return OP_FDIV; }   // ⊘ division
        case 0x10u: { return OP_STORE; }  // → store
        case 0x11u: { return OP_LOAD; }   // ← load
        case 0x70u: { return OP_SIN; }    // sin
        case 0x71u: { return OP_COS; }    // cos
        case 0x90u: { return OP_JMP; }    // ⤴ jump
        case 0x91u: { return OP_JZ; }     // ⤵ jump if zero
        case 0x93u: { return OP_CALL; }   // ⚙ call
        case 0x94u: { return OP_RET; }    // ↩ return
        case 0x00u: { return OP_NOP; }    // empty
        default: { return OP_CONSTANT; }  // literal value
    }
}

// Hilbert curve encoding for spatial reordering
fn hilbert_encode(x: u32, y: u32, order: u32) -> u32 {
    var d: u32 = 0u;
    var xi = x;
    var yi = y;
    
    for (var s: u32 = 1u << (order - 1u); s > 0u; s = s >> 1u) {
        let rx = select(0u, 1u, (xi & s) != 0u);
        let ry = select(0u, 1u, (yi & s) != 0u);
        d += s * s * ((3u * rx) ^ ry);
        
        if (ry == 0u) {
            if (rx == 1u) {
                xi = s - 1u - xi;
                yi = s - 1u - yi;
            }
            let tmp = xi;
            xi = yi;
            yi = tmp;
        }
    }
    return d;
}

// Decode RGB to operand value
fn decode_operand(r: u32, b: u32) -> f32 {
    // Combine R and B channels to form a float
    let bits = (r << 16u) | (b & 0xFFFFu);
    return bitcast<f32>(bits);
}

// Encode float to SPIR-V constant
fn encode_constant(value: f32, out: ptr<storage, array<u32>>, idx: ptr<function, u32>) {
    // OpConstant: result type, result id, value
    (*out)[(*idx)++] = 0u;  // Will be patched with type id
    (*out)[(*idx)++] = (*idx) - 1u;  // Result ID
    (*out)[(*idx)++] = bitcast<u32>(value);
}

// Emit SPIR-V header
fn emit_header(out: ptr<storage, array<u32>>, idx: ptr<function, u32>, bound: u32) {
    (*out)[(*idx)++] = SPIRV_MAGIC;
    (*out)[(*idx)++] = SPIRV_VERSION;
    (*out)[(*idx)++] = SPIRV_GENERATOR;
    (*out)[(*idx)++] = bound;  // Bound (highest ID + 1)
    (*out)[(*idx)++] = 0u;     // Schema
}

// Emit OpCapability
fn emit_capability(out: ptr<storage, array<u32>>, idx: ptr<function, u32>, cap: u32) {
    (*out)[(*idx)++] = (2u << 16u) | 17u;  // WordCount | OpCapability
    (*out)[(*idx)++] = cap;
}

// Emit OpMemoryModel
fn emit_memory_model(out: ptr<storage, array<u32>>, idx: ptr<function, u32>) {
    (*out)[(*idx)++] = (3u << 16u) | 14u;  // WordCount | OpMemoryModel
    (*out)[(*idx)++] = 0u;  // Logical
    (*out)[(*idx)++] = 1u;  // GLSL450
}

// Emit OpEntryPoint
fn emit_entry_point(out: ptr<storage, array<u32>>, idx: ptr<function, u32>, exec_mode: u32) {
    (*out)[(*idx)++] = (4u << 16u) | 15u;  // WordCount | OpEntryPoint
    (*out)[(*idx)++] = exec_mode;  // GLCompute
    (*out)[(*idx)++] = 1u;  // Entry point ID
    (*out)[(*idx)++] = 0x6E69616Du;  // "main" as u32
}

// Emit type declarations
fn emit_types(out: ptr<storage, array<u32>>, idx: ptr<function, u32>) {
    // OpTypeVoid (id=2)
    (*out)[(*idx)++] = (2u << 16u) | 19u;
    (*out)[(*idx)++] = 2u;
    
    // OpTypeFunction %void (id=3)
    (*out)[(*idx)++] = (3u << 16u) | 33u;
    (*out)[(*idx)++] = 3u;
    (*out)[(*idx)++] = 2u;
    
    // OpTypeFloat 32 (id=4)
    (*out)[(*idx)++] = (3u << 16u) | 22u;
    (*out)[(*idx)++] = 4u;
    (*out)[(*idx)++] = 32u;
    
    // OpTypePointer Uniform %float (id=5)
    (*out)[(*idx)++] = (4u << 16u) | 32u;
    (*out)[(*idx)++] = 5u;
    (*out)[(*idx)++] = 1u;  // Uniform
    (*out)[(*idx)++] = 4u;  // %float
}

// Emit function start
fn emit_function_start(out: ptr<storage, array<u32>>, idx: ptr<function, u32>) {
    // OpFunction %void None %functype
    (*out)[(*idx)++] = (5u << 16u) | 54u;
    (*out)[(*idx)++] = 2u;  // %void
    (*out)[(*idx)++] = 1u;  // %main
    (*out)[(*idx)++] = 0u;  // FunctionControl
    (*out)[(*idx)++] = 3u;  // %functype
    
    // OpLabel (id=10)
    (*out)[(*idx)++] = (2u << 16u) | 248u;
    (*out)[(*idx)++] = 10u;
}

// Emit function end
fn emit_function_end(out: ptr<storage, array<u32>>, idx: ptr<function, u32>) {
    // OpReturn
    (*out)[(*idx)++] = (1u << 16u) | 253u;
    
    // OpFunctionEnd
    (*out)[(*idx)++] = (1u << 16u) | 56u;
}

// Main compilation kernel
@group(0) @binding(0) var<storage, read> input_buffer: array<u32>;
@group(0) @binding(1) var<storage, read_write> output_buffer: array<u32>;
@group(0) @binding(2) var<storage, read_write> symbol_table: array<u32>;
@group(0) @binding(3) var<storage, read_write> error_log: array<u32>;
@group(0) @binding(4) var<storage, read_write> control: ControlBlock;

@compute @workgroup_size(1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    // Check if compilation requested
    if (control.command != 1u) {
        return;
    }
    
    control.status = 1u;  // Compiling
    control.error_count = 0u;
    
    var out_idx: u32 = 0u;
    var const_id: u32 = 100u;  // Starting ID for constants
    var instr_id: u32 = 200u;  // Starting ID for instructions
    
    // Emit SPIR-V header
    emit_header(&output_buffer, &out_idx, 1000u);
    
    // Emit capabilities
    emit_capability(&output_buffer, &out_idx, 1u);   // Shader
    emit_capability(&output_buffer, &out_idx, 2u);   // Matrix
    emit_capability(&output_buffer, &out_idx, 3u);   // SampledBuffer
    
    // Emit memory model
    emit_memory_model(&output_buffer, &out_idx);
    
    // Emit entry point
    emit_entry_point(&output_buffer, &out_idx, 5u);  // GLCompute
    
    // Emit types
    emit_types(&output_buffer, &out_idx);
    
    // Emit function start
    emit_function_start(&output_buffer, &out_idx);
    
    // Process input grid using Hilbert ordering
    let grid_size = control.grid_width * control.grid_height;
    
    for (var pixel_idx: u32 = 0u; pixel_idx < grid_size; pixel_idx++) {
        // Read pixel from input buffer (4 u32s per pixel: RGBA)
        let base = pixel_idx * 4u;
        let r = input_buffer[base + 0u];
        let g = input_buffer[base + 1u];
        let b = input_buffer[base + 2u];
        let a = input_buffer[base + 3u];
        
        // Convert to opcode
        let opcode = glyph_to_opcode(g);
        
        // Skip NOPs
        if (opcode == OP_NOP) {
            continue;
        }
        
        // Decode operands
        let operand_a = decode_operand(r, b);
        let operand_b = bitcast<f32>(a);
        
        // Emit instruction based on opcode
        switch (opcode) {
            case OP_CONSTANT: {
                // OpConstant
                output_buffer[out_idx++] = (4u << 16u) | 43u;
                output_buffer[out_idx++] = 4u;  // %float
                output_buffer[out_idx++] = const_id++;
                output_buffer[out_idx++] = bitcast<u32>(operand_a);
            }
            case OP_FADD: {
                // OpFAdd
                output_buffer[out_idx++] = (5u << 16u) | 129u;
                output_buffer[out_idx++] = 4u;  // %float
                output_buffer[out_idx++] = instr_id++;
                output_buffer[out_idx++] = const_id - 2u;
                output_buffer[out_idx++] = const_id - 1u;
            }
            case OP_FSUB: {
                // OpFSub
                output_buffer[out_idx++] = (5u << 16u) | 130u;
                output_buffer[out_idx++] = 4u;
                output_buffer[out_idx++] = instr_id++;
                output_buffer[out_idx++] = const_id - 2u;
                output_buffer[out_idx++] = const_id - 1u;
            }
            case OP_FMUL: {
                // OpFMul
                output_buffer[out_idx++] = (5u << 16u) | 133u;
                output_buffer[out_idx++] = 4u;
                output_buffer[out_idx++] = instr_id++;
                output_buffer[out_idx++] = const_id - 2u;
                output_buffer[out_idx++] = const_id - 1u;
            }
            case OP_FDIV: {
                // OpFDiv
                output_buffer[out_idx++] = (5u << 16u) | 134u;
                output_buffer[out_idx++] = 4u;
                output_buffer[out_idx++] = instr_id++;
                output_buffer[out_idx++] = const_id - 2u;
                output_buffer[out_idx++] = const_id - 1u;
            }
            default: {
                // Unknown opcode - log error
                let err_idx = control.error_count * 4u;
                error_log[err_idx + 0u] = 1u;  // Unknown opcode
                error_log[err_idx + 1u] = pixel_idx / control.grid_width;
                error_log[err_idx + 2u] = pixel_idx % control.grid_width;
                error_log[err_idx + 3u] = opcode;
                control.error_count++;
            }
        }
    }
    
    // Emit function end
    emit_function_end(&output_buffer, &out_idx);
    
    // Update control block
    control.output_size = out_idx * 4u;  // Size in bytes
    control.status = if (control.error_count > 0u) { 3u } else { 2u };
    control.command = 0u;  // Done
}
