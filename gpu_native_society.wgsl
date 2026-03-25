// ============================================================================
// GPU-NATIVE SOCIETY SHADER (v3 - 64-Agent Collective)
// ============================================================================

struct AgentState {
    pc: u32, sp: u32, pos_x: u32, pos_y: u32, vel_x: i32, vel_y: i32,
    color: u32, is_it: u32, halted: u32, step_count: u32, flags: u32, _padding: u32,
    registers: array<u32, 16>, stack: array<u32, 16>,
}

struct Config { width: u32, height: u32, time: f32, frame: u32 }

@group(0) @binding(0) var<storage, read_write> agents: array<AgentState, 64>;
@group(0) @binding(1) var<storage, read> bytecode: array<u32>;
@group(0) @binding(2) var<storage, read_write> framebuffer: array<atomic<u32>>;
@group(0) @binding(3) var<storage, read_write> mailboxes: array<atomic<u32>, 64>;
@group(0) @binding(4) var<uniform> config: Config;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let agent_id = global_id.x;
    if (agent_id >= 64u) { return; }
    
    var state = agents[agent_id];
    if (state.halted != 0u) { return; }
    
    state.step_count = state.step_count + 1u;
    let inst = bytecode[state.pc];
    let opcode = inst & 0xFFu;
    let dest = (inst >> 8u) & 0xFFu;
    let src1 = (inst >> 16u) & 0xFFu;
    let src2 = (inst >> 24u) & 0xFFu;
    let imm  = (inst >> 16u) & 0xFFFFu;
    
    var next_pc = state.pc + 1u;
    
    switch (opcode) {
        case 0u: { } 
        case 1u: { state.halted = 1u; }
        case 2u: { state.registers[dest] = imm; }
        case 3u: { state.registers[dest] = state.registers[src1] + state.registers[src2]; }
        case 4u: { state.registers[dest] = state.registers[src1] - state.registers[src2]; }
        case 5u: { state.registers[dest] = state.registers[src1] * state.registers[src2]; }
        case 7u: { next_pc = imm; }
        case 8u: { if (state.registers[dest] == 0u) { next_pc = imm; } }
        case 10u: { state.registers[14u] = state.pos_x; state.registers[15u] = state.pos_y; }
        case 11u: {
            let dx = i32(state.registers[src1]);
            let dy = i32(state.registers[src2]);
            state.pos_x = u32(i32(state.pos_x) + dx); state.pos_y = u32(i32(state.pos_y) + dy);
            state.pos_x = clamp(state.pos_x, 10u, config.width - 10u); state.pos_y = clamp(state.pos_y, 60u, config.height - 10u);
        }
        case 12u: {
            let idx = state.pos_y * config.width + state.pos_x;
            state.registers[dest] = select(0u, 1u, atomicLoad(&framebuffer[idx]) != 0u);
        }
        case 13u: {
            let idx = state.pos_y * config.width + state.pos_x;
            atomicStore(&framebuffer[idx], state.registers[src1]);
        }
        case 14u: {
            let target_id = state.registers[src1] % 64u;
            atomicStore(&mailboxes[target_id], state.registers[src2]);
        }
        case 15u: {
            state.registers[dest] = atomicExchange(&mailboxes[agent_id], 0u);
        }
        case 16u: {
            let v1 = state.registers[src1]; let v2 = state.registers[src2];
            if (v1 == v2) { state.flags = 1u; } else if (v1 > v2) { state.flags = 2u; } else { state.flags = 4u; }
        }
        case 17u: { state.halted = 2u; }
        default: { }
    }
    
    state.pc = next_pc; agents[agent_id] = state;
}
