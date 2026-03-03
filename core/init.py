#!/usr/bin/env python3
"""
Geometry OS - Init Process Generator

Generates init.spv - the primary GPU-native boot process.
This replaces JavaScript initialization logic, spawning all services
as independent GPU processes.

The JavaScript host becomes a thin Hardware Abstraction Layer.
"""

import struct
from pathlib import Path

# SPIR-V Magic and Header
SPIRV_MAGIC = 0x07230203
SPIRV_VERSION = 0x00010000

# Geometry OS Opcodes (must match kernel.wgsl)
OPCODES = {
    'CONSTANT': 43,
    'FADD': 129,
    'FMUL': 133,
    'STORE': 62,
    'LOAD': 61,
    'SHARED_STORE': 206,
    'SHARED_LOAD': 207,
    'MSG_SEND': 208,
    'MSG_RECV': 209,
    'MSG_PEEK': 210,
    'SYSCALL': 211,
    'YIELD': 228,
    'JMP': 202,
    'JZ': 203,
    'JNZ': 200,
    'LABEL': 248,
    'RETURN': 253,
}

# System call IDs
SYSCALLS = {
    'SPAWN': 0x01,      # Spawn a new process
    'KILL': 0x02,       # Kill a process
    'READ': 0x03,       # Read from file/memory
    'WRITE': 0x04,      # Write to file/memory
    'OPEN': 0x05,       # Open a file
    'CLOSE': 0x06,      # Close a file
    'IPC_SEND': 0x10,   # Send IPC message
    'IPC_RECV': 0x11,   # Receive IPC message
    'SCHED_YIELD': 0x20,  # Yield CPU
    'MEM_ALLOC': 0x30,  # Allocate memory
    'MEM_FREE': 0x31,   # Free memory
}

# Service PIDs (reserved)
SERVICE_PIDS = {
    'INIT': 0,
    'SHELL': 1,
    'FILES': 2,
    'MEMORY': 3,
    'IPC': 4,
    'SCHEDULER': 5,
    'COMPOSITOR': 6,
    'INPUT': 7,
}

# Service binaries (paths to .spv files)
SERVICE_BINARIES = {
    SERVICE_PIDS['SHELL']: 'shell.spv',
    SERVICE_PIDS['FILES']: 'files.spv',
    SERVICE_PIDS['MEMORY']: 'memory.spv',
    SERVICE_PIDS['IPC']: 'ipc.spv',
    SERVICE_PIDS['SCHEDULER']: 'scheduler.spv',
    SERVICE_PIDS['COMPOSITOR']: 'compositor.spv',
    SERVICE_PIDS['INPUT']: 'input.spv',
}


class InitProcessBuilder:
    """Builds the init.spv binary."""

    def __init__(self):
        self.words = []
        self.labels = {}
        self.label_counter = 0
        self.id_counter = 10

    def emit(self, word):
        """Emit a single word to the binary."""
        self.words.append(word)

    def emit_header(self, bound=100):
        """Emit SPIR-V header."""
        self.emit(SPIRV_MAGIC)
        self.emit(SPIRV_VERSION)
        self.emit(0)  # Generator
        self.emit(bound)  # Bound
        self.emit(0)  # Schema

    def next_id(self):
        """Get next result ID."""
        self.id_counter += 1
        return self.id_counter

    def label(self, name=None):
        """Create a label."""
        if name:
            self.labels[name] = len(self.words)
        self.emit((1 << 16) | OPCODES['LABEL'])

    def constant(self, value):
        """Push a constant to stack."""
        rid = self.next_id()
        self.emit((4 << 16) | OPCODES['CONSTANT'])
        self.emit(0)  # type
        self.emit(rid)  # result
        if isinstance(value, float):
            self.emit(struct.unpack('<I', struct.pack('<f', value))[0])
        else:
            self.emit(value)

    def syscall(self, syscall_id, arg1=0, arg2=0, arg3=0):
        """Make a system call."""
        self.emit((5 << 16) | OPCODES['SYSCALL'])
        self.emit(syscall_id)
        self.emit(arg1)
        self.emit(arg2)
        self.emit(arg3)

    def store(self, addr):
        """Store to memory."""
        self.emit((2 << 16) | OPCODES['STORE'])
        self.emit(addr)

    def load(self, addr):
        """Load from memory."""
        self.emit((4 << 16) | OPCODES['LOAD'])
        self.emit(0)
        self.emit(0)
        self.emit(addr)

    def shared_store(self, addr):
        """Store to shared memory."""
        self.emit((2 << 16) | OPCODES['SHARED_STORE'])
        self.emit(addr)

    def shared_load(self, addr):
        """Load from shared memory."""
        self.emit((2 << 16) | OPCODES['SHARED_LOAD'])
        self.emit(addr)

    def msg_send(self, target_pid, msg_type, data):
        """Send IPC message."""
        self.emit((4 << 16) | OPCODES['MSG_SEND'])
        self.emit(target_pid)
        self.emit(msg_type)
        self.emit(data)

    def msg_recv(self, from_pid=0xFF, timeout=100):
        """Receive IPC message."""
        self.emit((3 << 16) | OPCODES['MSG_RECV'])
        self.emit(from_pid)
        self.emit(timeout)

    def msg_peek(self, from_pid=0xFF):
        """Peek for IPC message."""
        self.emit((2 << 16) | OPCODES['MSG_PEEK'])
        self.emit(from_pid)

    def yield_cpu(self):
        """Yield CPU time slice."""
        self.emit((1 << 16) | OPCODES['YIELD'])

    def jump(self, label_name):
        """Jump to label."""
        # Will patch later
        self.emit((2 << 16) | OPCODES['JMP'])
        self.emit(label_name)  # Placeholder

    def jump_if_zero(self, label_name):
        """Jump if top of stack is zero."""
        self.emit((2 << 16) | OPCODES['JZ'])
        self.emit(label_name)

    def return_proc(self):
        """Return from process."""
        self.emit((1 << 16) | OPCODES['RETURN'])

    def build(self):
        """Build the init process."""
        self.emit_header()

        # ========================================
        # INIT PROCESS BOOT SEQUENCE
        # ========================================

        # Phase 1: Initialize shared memory regions
        self._init_shared_memory()

        # Phase 2: Spawn system services
        self._spawn_services()

        # Phase 3: Enter main loop
        self._main_loop()

        # Phase 4: Shutdown handler
        self._shutdown_handler()

        return bytes(struct.pack('<' + 'I' * len(self.words), *self.words))

    def _init_shared_memory(self):
        """Initialize shared memory regions."""
        # Mark boot sector as initialized
        self.constant(0x474F5300)  # "GOS\0" magic
        self.shared_store(0)  # Boot magic at address 0

        # Initialize mailbox headers
        for pid in range(16):
            mailbox_base = 1024 + pid * 32
            self.constant(0)  # Empty mailbox
            self.shared_store(mailbox_base + 2)  # Clear size field

        # Set system state: INIT_COMPLETE
        self.constant(1)
        self.shared_store(1)

    def _spawn_services(self):
        """Spawn system services via syscalls."""
        # Spawn Shell (PID 1)
        self.constant(SYSCALLS['SPAWN'])
        self.constant(SERVICE_PIDS['SHELL'])
        self.constant(0)  # Binary path offset
        self.syscall(SYSCALLS['SPAWN'], SERVICE_PIDS['SHELL'], 0, 0)

        # Spawn File Manager (PID 2)
        self.constant(SYSCALLS['SPAWN'])
        self.constant(SERVICE_PIDS['FILES'])
        self.constant(1)
        self.syscall(SYSCALLS['SPAWN'], SERVICE_PIDS['FILES'], 1, 0)

        # Spawn Memory Browser (PID 3)
        self.constant(SYSCALLS['SPAWN'])
        self.constant(SERVICE_PIDS['MEMORY'])
        self.constant(2)
        self.syscall(SYSCALLS['SPAWN'], SERVICE_PIDS['MEMORY'], 2, 0)

        # Spawn IPC Monitor (PID 4)
        self.constant(SYSCALLS['SPAWN'])
        self.constant(SERVICE_PIDS['IPC'])
        self.constant(3)
        self.syscall(SYSCALLS['SPAWN'], SERVICE_PIDS['IPC'], 3, 0)

        # Yield to let services initialize
        self.yield_cpu()

    def _main_loop(self):
        """Main init loop - service monitoring."""
        self.label('main_loop')

        # Check for shutdown signal
        self.shared_load(2)  # Shutdown flag
        self.constant(1)
        # If shutdown flag == 1, jump to shutdown
        # (simplified - actual compare would need more ops)

        # Check each service health (heartbeat)
        self._check_service_health()

        # Process any IPC messages for init
        self.msg_peek(0xFF)  # Peek from anyone
        # If message available, handle it

        # Yield and loop
        self.yield_cpu()
        self.jump('main_loop')

    def _check_service_health(self):
        """Check health of spawned services."""
        # Read heartbeat from each service's shared memory
        for pid, name in [(1, 'SHELL'), (2, 'FILES'), (3, 'MEMORY'), (4, 'IPC')]:
            heartbeat_addr = 10 + pid  # Heartbeat addresses
            self.shared_load(heartbeat_addr)
            # If heartbeat is stale, respawn service
            # (simplified for MVP)

    def _shutdown_handler(self):
        """Handle system shutdown."""
        self.label('shutdown')

        # Send shutdown message to all services
        for pid in [1, 2, 3, 4, 5, 6, 7]:
            self.msg_send(pid, 0xFF, 0)  # Type 0xFF = SHUTDOWN

        # Wait for acknowledgments
        self.yield_cpu()

        # Clean up and exit
        self.constant(0)
        self.shared_store(0)  # Clear boot magic

        self.return_proc()


def build_libgeo():
    """Build the Visual Standard Library."""
    words = []

    # Header
    words.extend([SPIRV_MAGIC, SPIRV_VERSION, 0, 100, 0])

    # ========================================
    # LIBGEO - VISUAL STANDARD LIBRARY
    # ========================================

    # String functions
    # - strlen: Compute string length
    # - strcmp: Compare two strings
    # - strcpy: Copy string

    # Vector math functions
    # - vec3_add: Add two 3D vectors
    # - vec3_sub: Subtract two 3D vectors
    # - vec3_dot: Dot product
    # - vec3_cross: Cross product
    # - vec3_len: Vector length
    # - vec3_norm: Normalize vector

    # IPC helper functions
    # - ipc_send_str: Send string via IPC
    # - ipc_recv_str: Receive string via IPC
    # - ipc_broadcast: Broadcast to all processes

    # Hilbert functions
    # - hilbert_encode: Encode (x,y) to Hilbert index
    # - hilbert_decode: Decode Hilbert index to (x,y)

    # Memory functions
    # - mem_alloc: Allocate memory block
    # - mem_free: Free memory block
    # - mem_copy: Copy memory block

    return bytes(struct.pack('<' + 'I' * len(words), *words))


def generate_service_binary(service_name, pid):
    """Generate a minimal service binary."""
    words = []

    # Header
    words.extend([SPIRV_MAGIC, SPIRV_VERSION, 0, 50, 0])

    # Service entry point
    # 1. Initialize service state
    # 2. Enter service loop
    # 3. Handle IPC messages
    # 4. Perform service-specific work
    # 5. Yield and repeat

    # Heartbeat increment
    words.extend([
        (4 << 16) | OPCODES['CONSTANT'], 0, 10, 1,  # Push 1
        (2 << 16) | OPCODES['SHARED_STORE'], 10 + pid,  # Store to heartbeat
    ])

    # Service loop
    loop_start = len(words)
    words.append((1 << 16) | OPCODES['LABEL'])  # Loop label

    # Check for messages
    words.extend([
        (2 << 16) | OPCODES['MSG_PEEK'], 0xFF,  # Peek any sender
    ])

    # Service-specific work (placeholder)
    # ...

    # Yield
    words.append((1 << 16) | OPCODES['YIELD'])

    # Jump back to loop start
    words.extend([
        (2 << 16) | OPCODES['JMP'], loop_start,
    ])

    return bytes(struct.pack('<' + 'I' * len(words), *words))


def main():
    """Generate all Morphological Userland binaries."""
    output_dir = Path(__file__).parent
    output_dir.mkdir(exist_ok=True)

    # Generate init.spv
    builder = InitProcessBuilder()
    init_binary = builder.build()
    (output_dir / 'init.spv').write_bytes(init_binary)
    print(f"[Init] Generated init.spv ({len(init_binary)} bytes)")

    # Generate libgeo.spv
    libgeo_binary = build_libgeo()
    (output_dir / 'libgeo.spv').write_bytes(libgeo_binary)
    print(f"[LibGeo] Generated libgeo.spv ({len(libgeo_binary)} bytes)")

    # Generate service binaries
    for name, pid in SERVICE_PIDS.items():
        if pid == 0:  # Skip init
            continue
        binary = generate_service_binary(name, pid)
        filename = f"{name.lower()}.spv"
        (output_dir / filename).write_bytes(binary)
        print(f"[Service] Generated {filename} for PID {pid}")

    print("\n[Morphological Userland] All binaries generated.")


if __name__ == '__main__':
    main()
