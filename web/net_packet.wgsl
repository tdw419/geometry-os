/**
 * Geometry OS Network Packet Processor (WGSL)
 *
 * GPU-side packet processing for TCP/IP-like networking.
 * Processes packets from network memory and routes to destinations.
 */

// Packet header offsets (matches JS implementation)
const HDR_SRC_PORT: u32 = 0u;
const HDR_DST_PORT: u32 = 1u;
const HDR_TYPE: u32 = 2u;
const HDR_FLAGS: u32 = 3u;
const HDR_SEQ_NUM: u32 = 4u;
const HDR_ACK_NUM: u32 = 8u;
const HDR_LENGTH: u32 = 10u;
const HDR_SIZE: u32 = 12u;

// Packet types
const PKT_DATA: u32 = 0x01u;
const PKT_ACK: u32 = 0x02u;
const PKT_NACK: u32 = 0x03u;
const PKT_SYN: u32 = 0x04u;
const PKT_SYN_ACK: u32 = 0x05u;
const PKT_FIN: u32 = 0x06u;
const PKT_RST: u32 = 0x07u;
const PKT_HEARTBEAT: u32 = 0x08u;

// Packet flags
const FLG_URGENT: u32 = 0x01u;
const FLG_BROADCAST: u32 = 0x02u;

// Port definitions
const PORT_IPC: u32 = 5000u;
const PORT_FS: u32 = 5001u;
const PORT_KERNEL: u32 = 5002u;
const PORT_SHELL: u32 = 5003u;

// Network buffer (shared memory region for packets)
@group(0) @binding(0) var<storage, read_write> network_mem: array<u32>;

// Packet queues (per port)
@group(0) @binding(1) var<storage, read_write> ipc_queue: array<u32>;
@group(0) @binding(2) var<storage, read_write> fs_queue: array<u32>;
@group(0) @binding(3) var<storage, read_write> kernel_queue: array<u32>;
@group(0) @binding(4) var<storage, read_write> shell_queue: array<u32>;

// Queue metadata
@group(0) @binding(5) var<storage, read_write> ipc_head: u32;    // Head index
@group(0) @binding(6) var<storage, read_write> ipc_tail: u32;    // Tail index
@group(0) @binding(7) var<storage, read_write> ipc_count: u32;   // Packet count

// Statistics
@group(0) @binding(8) var<storage, read_write> packets_sent: atomic<u32>;
@group(0) @binding(9) var<storage, read_write> packets_recv: atomic<u32>;
@group(0) @binding(10) var<storage, read_write> packets_drop: atomic<u32>;

/**
 * Read packet header from network memory.
 */
fn read_header(addr: u32) -> vec4<u32> {
    let word0 = network_mem[addr + 0u];
    let word1 = network_mem[addr + 1u];
    let word2 = network_mem[addr + 2u];
    
    return vec4<u32>(
        (word0 >> 16) & 0xFFFF,  // src_port
        word0 & 0xFFFF,              // dst_port
        (word1 >> 24) & 0xFF,       // type
        (word1 >> 16) & 0xFF,       // flags
        word1 & 0xFFFF,              // seq_num (lower 16 bits)
        word2 >> 16,               // ack_num
        word2 & 0xFFFF               // length
    );
}

/**
 * Write packet header to network memory.
 */
fn write_header(addr: u32, header: vec4<u32>) {
    network_mem[addr + 0u] = (header.x << 16) | header.y;
    network_mem[addr + 1u] = (header.z << 16) | (header.z & 0xFFFF);
    network_mem[addr + 2u] = header.w;
}

/**
 * Process incoming packets and route to destination queues.
 */
fn process_packet(pkt_addr: u32) {
    let header = read_header(pkt_addr);
    
    // Validate packet
    if (header.y == 0u) {
        return; // Invalid packet
    }
    
    // Get destination queue
    var dest_queue: array<u32>;
    var dest_head: u32;
    var dest_tail: u32;
    var dest_count: u32;
    
    if (header.y == PORT_IPC) {
        dest_queue = ipc_queue;
        dest_head = ipc_head;
        dest_tail = ipc_tail;
        dest_count = ipc_count;
    } else if (header.y == PORT_FS) {
        dest_queue = fs_queue;
        dest_head = fs_head;
        dest_tail = fs_tail;
        dest_count = fs_count;
    } else if (header.y == PORT_KERNEL) {
        dest_queue = kernel_queue;
        dest_head = kernel_head;
        dest_tail = kernel_tail;
        dest_count = kernel_count;
    } else if (header.y == PORT_SHELL) {
        dest_queue = shell_queue;
        dest_head = shell_head;
        dest_tail = shell_tail;
        dest_count = shell_count;
    } else {
        // Unknown port - drop packet
        atomicAdd(&packets_drop, 1u);
        return;
    }
    
    // Check queue capacity
    if (dest_count >= 256u) {
        atomicAdd(&packets_drop, 1u);
        return; // Queue full
    }
    
    // Copy packet to destination queue
    let dest_addr = dest_tail * 64u; // 64 words per packet slot
    for (var i: u32 = 0u; i < 16u; i = i + 1u) {
        dest_queue[dest_addr + i] = network_mem[pkt_addr + i];
    }
    
    // Update tail
    dest_tail = (dest_tail + 1u) % 256u;
    
    // Update count
    atomicAdd(&dest_count, 1u);
    atomicAdd(&packets_recv, 1u);
}

/**
 * Send a packet from a queue.
 */
fn send_packet(port: u32, data: ptr<u32>, len: u32) -> bool {
    // Find queue for source port
    var src_queue: array<u32>;
    var src_head: u32;
    var src_count: u32;
    
    if (port == PORT_IPC) {
        src_queue = ipc_queue;
        src_head = ipc_head;
        src_count = ipc_count;
    } else if (port == PORT_FS) {
        src_queue = fs_queue;
        src_head = fs_head;
        src_count = fs_count;
    } else {
        return false;
    }
    
    // Check if queue has packets
    if (src_count == 0u) {
        return false;
    }
    
    // Copy packet from queue to network memory
    let src_addr = src_head * 64u;
    let pkt_addr = atomicLoad(&packets_sent) * 1024u; // Next available slot
    
    for (var i: u32 = 0u; i < 16u; i = i + 1u) {
        network_mem[pkt_addr + i] = src_queue[src_addr + i];
    }
    
    // Update head
    src_head = (src_head + 1u) % 256u;
    atomicSub(&src_count, 1u);
    
    return true;
}

/**
 * Main compute shader - process network packets.
 */
@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let pkt_idx = global_id.x;
    
    // Calculate packet address
    let pkt_addr = pkt_idx * 16u; // 16 words per packet
    
    // Only process valid packets
    if (pkt_addr < arrayLength(&network_mem) - 16u) {
        // Check if packet has data
        if (network_mem[pkt_addr] != 0u) {
            process_packet(pkt_addr);
        }
    }
}
