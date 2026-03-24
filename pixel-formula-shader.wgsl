// ============================================================================
// Pixel Formula Shader — Bytecode Interpreter for GPU
// 
// Compiles formulas on CPU to bytecode, interprets on GPU per-pixel.
// Stack-based VM: operations push/pop from a per-pixel stack.
// ============================================================================

// Bytecode opcodes
const OP_PUSH_X: u32 = 0x01u;     // Push x coordinate (0.0-1.0)
const OP_PUSH_Y: u32 = 0x02u;     // Push y coordinate (0.0-1.0)
const OP_PUSH_T: u32 = 0x03u;     // Push time/frame (0.0-∞)
const OP_PUSH_CONST: u32 = 0x04u; // Push constant (next word)
const OP_ADD: u32 = 0x10u;        // a + b
const OP_SUB: u32 = 0x11u;        // a - b
const OP_MUL: u32 = 0x12u;        // a * b
const OP_DIV: u32 = 0x13u;        // a / b (safe)
const OP_MOD: u32 = 0x14u;        // a % b
const OP_POW: u32 = 0x15u;        // a ^ b
const OP_SIN: u32 = 0x20u;        // sin(a)
const OP_COS: u32 = 0x21u;        // cos(a)
const OP_TAN: u32 = 0x22u;        // tan(a)
const OP_SQRT: u32 = 0x23u;       // sqrt(a)
const OP_ABS: u32 = 0x24u;        // abs(a)
const OP_FLOOR: u32 = 0x25u;      // floor(a)
const OP_CEIL: u32 = 0x26u;       // ceil(a)
const OP_FRACT: u32 = 0x27u;      // fract(a)
const OP_MIN: u32 = 0x28u;        // min(a, b)
const OP_MAX: u32 = 0x29u;        // max(a, b)
const OP_CLAMP: u32 = 0x2Au;      // clamp(a, min, max)
const OP_MIX: u32 = 0x2Bu;        // mix(a, b, t)
const OP_NOISE: u32 = 0x30u;      // noise2D(x, y)
const OP_RGB: u32 = 0xF0u;        // Output RGB (r, g, b on stack)
const OP_HSV: u32 = 0xF1u;        // Output HSV → RGB

// Stack size per pixel
const STACK_SIZE: u32 = 16u;

// Bindings
@group(0) @binding(0) var<storage, read> bytecode: array<u32>;  // Bytecode program
@group(0) @binding(1) var<storage, read> constants: array<f32>; // Constant pool
@group(0) @binding(2) var<storage, read_write> output: array<u32>; // RGBA output
@group(0) @binding(3) var<uniform> config: Config;

struct Config {
    width: u32,
    height: u32,
    bytecode_len: u32,
    time: f32,
}

// Simple hash for noise
fn hash(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

// 2D value noise
fn noise2D(x: f32, y: f32) -> f32 {
    let i = floor(vec2<f32>(x, y));
    let f = fract(vec2<f32>(x, y));
    let u = f * f * (3.0 - 2.0 * f);
    
    let a = hash(i);
    let b = hash(i + vec2<f32>(1.0, 0.0));
    let c = hash(i + vec2<f32>(0.0, 1.0));
    let d = hash(i + vec2<f32>(1.0, 1.0));
    
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// HSV to RGB conversion
fn hsvToRgb(h: f32, s: f32, v: f32) -> vec3<f32> {
    let i = floor(h * 6.0);
    let f = h * 6.0 - i;
    let p = v * (1.0 - s);
    let q = v * (1.0 - f * s);
    let t = v * (1.0 - (1.0 - f) * s);
    
    let im = i32(i) % 6;
    
    switch im {
        case 0: { return vec3<f32>(v, t, p); }
        case 1: { return vec3<f32>(q, v, p); }
        case 2: { return vec3<f32>(p, v, t); }
        case 3: { return vec3<f32>(p, q, v); }
        case 4: { return vec3<f32>(t, p, v); }
        case 5: { return vec3<f32>(v, p, q); }
        default: { return vec3<f32>(v, t, p); }
    }
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let px = global_id.x;
    let py = global_id.y;
    
    if (px >= config.width || py >= config.height) {
        return;
    }
    
    // Normalized coordinates (0-1)
    let x = f32(px) / f32(config.width);
    let y = f32(py) / f32(config.height);
    let t = config.time;
    
    // Stack for this pixel
    var stack: array<f32, STACK_SIZE>;
    var sp: u32 = 0u;
    
    // Default output (black)
    var r: f32 = 0.0;
    var g: f32 = 0.0;
    var b: f32 = 0.0;
    
    // Interpret bytecode
    var pc: u32 = 0u;
    
    while (pc < config.bytecode_len) {
        let op = bytecode[pc];
        pc += 1u;
        
        switch op {
            case OP_PUSH_X: { 
                stack[sp] = x;
                sp += 1u;
            }
            case OP_PUSH_Y: { 
                stack[sp] = y;
                sp += 1u;
            }
            case OP_PUSH_T: { 
                stack[sp] = t;
                sp += 1u;
            }
            case OP_PUSH_CONST: {
                let idx = bytecode[pc];
                pc += 1u;
                stack[sp] = constants[idx];
                sp += 1u;
            }
            case OP_ADD: {
                sp -= 1u; let b_ = stack[sp];
                sp -= 1u; let a_ = stack[sp];
                stack[sp] = a_ + b_;
                sp += 1u;
            }
            case OP_SUB: {
                sp -= 1u; let b_ = stack[sp];
                sp -= 1u; let a_ = stack[sp];
                stack[sp] = a_ - b_;
                sp += 1u;
            }
            case OP_MUL: {
                sp -= 1u; let b_ = stack[sp];
                sp -= 1u; let a_ = stack[sp];
                stack[sp] = a_ * b_;
                sp += 1u;
            }
            case OP_DIV: {
                sp -= 1u; let b_ = stack[sp];
                sp -= 1u; let a_ = stack[sp];
                stack[sp] = select(a_ / b_, 0.0, abs(b_) < 0.0001);
                sp += 1u;
            }
            case OP_MOD: {
                sp -= 1u; let b_ = stack[sp];
                sp -= 1u; let a_ = stack[sp];
                stack[sp] = a_ - b_ * floor(a_ / b_);
                sp += 1u;
            }
            case OP_POW: {
                sp -= 1u; let b_ = stack[sp];
                sp -= 1u; let a_ = stack[sp];
                stack[sp] = pow(a_, b_);
                sp += 1u;
            }
            case OP_SIN: { 
                sp -= 1u; 
                stack[sp] = sin(stack[sp]);
                sp += 1u;
            }
            case OP_COS: { 
                sp -= 1u; 
                stack[sp] = cos(stack[sp]);
                sp += 1u;
            }
            case OP_TAN: { 
                sp -= 1u; 
                stack[sp] = tan(stack[sp]);
                sp += 1u;
            }
            case OP_SQRT: { 
                sp -= 1u; 
                stack[sp] = sqrt(stack[sp]);
                sp += 1u;
            }
            case OP_ABS: { 
                sp -= 1u; 
                stack[sp] = abs(stack[sp]);
                sp += 1u;
            }
            case OP_FLOOR: { 
                sp -= 1u; 
                stack[sp] = floor(stack[sp]);
                sp += 1u;
            }
            case OP_CEIL: { 
                sp -= 1u; 
                stack[sp] = ceil(stack[sp]);
                sp += 1u;
            }
            case OP_FRACT: { 
                sp -= 1u; 
                stack[sp] = fract(stack[sp]);
                sp += 1u;
            }
            case OP_MIN: {
                sp -= 1u; let b_ = stack[sp];
                sp -= 1u; let a_ = stack[sp];
                stack[sp] = min(a_, b_);
                sp += 1u;
            }
            case OP_MAX: {
                sp -= 1u; let b_ = stack[sp];
                sp -= 1u; let a_ = stack[sp];
                stack[sp] = max(a_, b_);
                sp += 1u;
            }
            case OP_CLAMP: {
                sp -= 1u; let max_ = stack[sp];
                sp -= 1u; let min_ = stack[sp];
                sp -= 1u; let val = stack[sp];
                stack[sp] = clamp(val, min_, max_);
                sp += 1u;
            }
            case OP_MIX: {
                sp -= 1u; let t_ = stack[sp];
                sp -= 1u; let b_ = stack[sp];
                sp -= 1u; let a_ = stack[sp];
                stack[sp] = mix(a_, b_, t_);
                sp += 1u;
            }
            case OP_NOISE: {
                sp -= 1u; let ny = stack[sp];
                sp -= 1u; let nx = stack[sp];
                stack[sp] = noise2D(nx, ny);
                sp += 1u;
            }
            case OP_RGB: {
                sp -= 1u; b = stack[sp];
                sp -= 1u; g = stack[sp];
                sp -= 1u; r = stack[sp];
            }
            case OP_HSV: {
                sp -= 1u; let v = stack[sp];
                sp -= 1u; let s = stack[sp];
                sp -= 1u; let h = stack[sp];
                let rgb = hsvToRgb(h, s, v);
                r = rgb.x;
                g = rgb.y;
                b = rgb.z;
            }
            default: { /* Unknown opcode - skip */ }
        }
    }
    
    // Convert to 0-255 and pack as RGBA
    let ri = u32(clamp(r, 0.0, 1.0) * 255.0);
    let gi = u32(clamp(g, 0.0, 1.0) * 255.0);
    let bi = u32(clamp(b, 0.0, 1.0) * 255.0);
    let rgba = (255u << 24u) | (bi << 16u) | (gi << 8u) | ri;
    
    // Write to output buffer
    let idx = py * config.width + px;
    output[idx] = rgba;
}
