#!/usr/bin/env python3
"""
Geometry OS - Visual Standard Library (libgeo.spv)

A collection of common subroutines that any GPU process can link to.
Provides string parsing, vector math, IPC protocols, and memory utilities.

This is the "libc" of Geometry OS.
"""

import struct
from typing import List, Tuple

# Library function IDs
LIBGEO_FUNCTIONS = {
    # String operations
    'STRLEN': 0x100,     # Get string length
    'STRCPY': 0x101,     # Copy string
    'STRCAT': 0x102,     # Concatenate strings
    'STRCMP': 0x103,     # Compare strings
    'ATOI': 0x104,       # String to integer
    'ITOA': 0x105,       # Integer to string

    # Vector math
    'VEC2_ADD': 0x200,   # Add two 2D vectors
    'VEC2_SUB': 0x201,   # Subtract two 2D vectors
    'VEC2_MUL': 0x202,   # Multiply 2D vector by scalar
    'VEC2_DOT': 0x203,   # Dot product
    'VEC3_ADD': 0x210,   # Add two 3D vectors
    'VEC3_CROSS': 0x211, # Cross product
    'MAT4_MUL': 0x220,   # 4x4 matrix multiply

    # IPC utilities
    'IPC_SEND_BLK': 0x300,  # Blocking send
    'IPC_RECV_BLK': 0x301,  # Blocking receive
    'IPC_PEEK': 0x302,      # Non-blocking peek
    'IPC_BROADCAST': 0x303, # Broadcast to all

    # Memory utilities
    'MEMSET': 0x400,     # Set memory block
    'MEMCPY': 0x401,     # Copy memory block
    'MEMCMP': 0x402,     # Compare memory blocks
    'HEAP_ALLOC': 0x410, # Allocate from heap
    'HEAP_FREE': 0x411,  # Free to heap

    # Hilbert curve utilities
    'HILBERT_ENCODE': 0x500,  # (x,y) -> index
    'HILBERT_DECODE': 0x501,  # index -> (x,y)

    # Semantic operations
    'EMBEDDING_COSINE': 0x600,  # Cosine similarity
    'EMBEDDING_ADD': 0x601,     # Add embeddings
    'SEMANTIC_NEAREST': 0x610,  # Find nearest neighbors
}


class LibGeoBuilder:
    """Builds the libgeo.spv standard library."""

    def __init__(self):
        self.words = []
        self.function_table = {}  # function_id -> offset

    def emit(self, word):
        """Emit a word to the binary."""
        self.words.append(word)

    def build(self) -> bytes:
        """Build the complete library binary."""
        # Header
        self.emit(0x474F4C47)  # Magic: "GEOC" (Geometry OS Library)
        self.emit(0x00010000)  # Version 1.0
        self.emit(len(LIBGEO_FUNCTIONS))  # Function count

        # Function table (function_id -> offset pairs)
        func_offsets = []
        current_offset = 3 + len(LIBGEO_FUNCTIONS) * 2  # After header and table

        for name, fid in sorted(LIBGEO_FUNCTIONS.items(), key=lambda x: x[1]):
            func_offsets.append((fid, current_offset))
            # Placeholder for function size (will update later)
            current_offset += 50  # Average function size

        # Write function table
        for fid, offset in func_offsets:
            self.emit(fid)
            self.emit(offset)

        # Build each function
        for name, fid in sorted(LIBGEO_FUNCTIONS.items(), key=lambda x: x[1]):
            self._build_function(name, fid)

        return struct.pack(f'<{len(self.words)}I', *self.words)

    def _build_function(self, name: str, func_id: int):
        """Build a single library function."""
        start_offset = len(self.words)

        if name.startswith('VEC2_'):
            self._build_vec2_function(name)
        elif name.startswith('VEC3_'):
            self._build_vec3_function(name)
        elif name.startswith('IPC_'):
            self._build_ipc_function(name)
        elif name.startswith('MEM'):
            self._build_mem_function(name)
        elif name.startswith('HILBERT_'):
            self._build_hilbert_function(name)
        elif name.startswith('EMBEDDING_'):
            self._build_embedding_function(name)
        elif name.startswith('STR'):
            self._build_string_function(name)
        else:
            # Generic placeholder
            self._build_placeholder(name)

    def _build_vec2_function(self, name: str):
        """Build 2D vector math functions."""
        if name == 'VEC2_ADD':
            # Input: stack[sp-2] = v1, stack[sp-1] = v2
            # Output: stack[sp-1] = v1 + v2
            self.emit((2 << 16) | 129)  # FADD
            self.emit(0)  # No extra operands needed
        elif name == 'VEC2_SUB':
            self.emit((2 << 16) | 131)  # FSUB
            self.emit(0)
        elif name == 'VEC2_MUL':
            self.emit((2 << 16) | 133)  # FMUL
            self.emit(0)
        elif name == 'VEC2_DOT':
            # Dot product: v1.x*v2.x + v1.y*v2.y
            self.emit((2 << 16) | 133)  # FMUL (x components)
            self.emit(0)
            self.emit((2 << 16) | 133)  # FMUL (y components)
            self.emit(0)
            self.emit((1 << 16) | 129)  # FADD (sum)
        self.emit((1 << 16) | 253)  # RETURN

    def _build_vec3_function(self, name: str):
        """Build 3D vector math functions."""
        if name == 'VEC3_ADD':
            for _ in range(3):
                self.emit((2 << 16) | 129)  # FADD for each component
                self.emit(0)
        elif name == 'VEC3_CROSS':
            # Cross product: (a.y*b.z - a.z*b.y, a.z*b.x - a.x*b.z, a.x*b.y - a.y*b.x)
            # Simplified placeholder
            for _ in range(6):
                self.emit((2 << 16) | 133)  # FMUL
                self.emit(0)
                self.emit((2 << 16) | 131)  # FSUB
                self.emit(0)
        self.emit((1 << 16) | 253)  # RETURN

    def _build_ipc_function(self, name: str):
        """Build IPC utility functions."""
        if name == 'IPC_SEND_BLK':
            # Wrapper around MSG_SEND (208) with blocking
            self.emit((4 << 16) | 208)  # MSG_SEND
            self.emit(0)  # target_pid (from stack)
            self.emit(0)  # msg_type
            self.emit(0)  # data
        elif name == 'IPC_RECV_BLK':
            self.emit((3 << 16) | 209)  # MSG_RECV
            self.emit(0xFF)  # from any
            self.emit(100)   # timeout
        elif name == 'IPC_PEEK':
            self.emit((2 << 16) | 210)  # MSG_PEEK
            self.emit(0xFF)  # from any
        self.emit((1 << 16) | 253)  # RETURN

    def _build_mem_function(self, name: str):
        """Build memory utility functions."""
        if name == 'MEMSET':
            # Set memory block to value
            self.emit((2 << 16) | 62)  # STORE
            self.emit(0)  # address from stack
        elif name == 'MEMCPY':
            # Copy memory block
            self.emit((4 << 16) | 61)  # LOAD
            self.emit(0)
            self.emit(0)
            self.emit(0)  # src address
            self.emit((2 << 16) | 62)  # STORE
            self.emit(0)  # dst address
        self.emit((1 << 16) | 253)  # RETURN

    def _build_hilbert_function(self, name: str):
        """Build Hilbert curve utilities."""
        if name == 'HILBERT_ENCODE':
            # Convert (x, y) to Hilbert index
            # Simplified: just use x * 32 + y
            self.emit((4 << 16) | 43)  # CONSTANT 32
            self.emit(0)
            self.emit(0)
            self.emit(32)
            self.emit((2 << 16) | 133)  # FMUL (x * 32)
            self.emit(0)
            self.emit((1 << 16) | 129)  # FADD (+ y)
        elif name == 'HILBERT_DECODE':
            # Convert Hilbert index to (x, y)
            self.emit((4 << 16) | 43)  # CONSTANT 32
            self.emit(0)
            self.emit(0)
            self.emit(32)
            self.emit((2 << 16) | 135)  # FDIV (index / 32)
            self.emit(0)
        self.emit((1 << 16) | 253)  # RETURN

    def _build_embedding_function(self, name: str):
        """Build embedding/semantic functions."""
        if name == 'EMBEDDING_COSINE':
            # Cosine similarity between two embeddings
            # dot(a,b) / (||a|| * ||b||)
            # Simplified placeholder
            for _ in range(384):  # 384-dim embeddings
                self.emit((2 << 16) | 133)  # FMUL
                self.emit(0)
                self.emit((1 << 16) | 129)  # FADD
            self.emit((2 << 16) | 135)  # FDIV (normalize)
            self.emit(0)
        self.emit((1 << 16) | 253)  # RETURN

    def _build_string_function(self, name: str):
        """Build string manipulation functions."""
        if name == 'STRLEN':
            # Count characters until null
            # Placeholder - returns stack top value
            self.emit((1 << 16) | 253)  # RETURN
        elif name == 'ATOI':
            # Convert string to integer
            # Placeholder
            self.emit((1 << 16) | 253)  # RETURN
        else:
            self.emit((1 << 16) | 253)  # RETURN

    def _build_placeholder(self, name: str):
        """Build a placeholder function."""
        self.emit((1 << 16) | 253)  # RETURN


def build_libgeo() -> bytes:
    """Build the libgeo.spv standard library."""
    builder = LibGeoBuilder()
    return builder.build()


# Function manifest for documentation
LIBGEO_MANIFEST = """
# libgeo.spv - Geometry OS Standard Library

## String Operations (0x100-0x1FF)
- STRLEN(0x100): Get string length
- STRCPY(0x101): Copy string
- STRCAT(0x102): Concatenate strings
- STRCMP(0x103): Compare strings
- ATOI(0x104): String to integer
- ITOA(0x105): Integer to string

## Vector Math (0x200-0x2FF)
- VEC2_ADD(0x200): Add 2D vectors
- VEC2_SUB(0x201): Subtract 2D vectors
- VEC2_MUL(0x202): Multiply by scalar
- VEC2_DOT(0x203): Dot product
- VEC3_ADD(0x210): Add 3D vectors
- VEC3_CROSS(0x211): Cross product
- MAT4_MUL(0x220): 4x4 matrix multiply

## IPC Utilities (0x300-0x3FF)
- IPC_SEND_BLK(0x300): Blocking send
- IPC_RECV_BLK(0x301): Blocking receive
- IPC_PEEK(0x302): Non-blocking peek
- IPC_BROADCAST(0x303): Broadcast to all

## Memory Utilities (0x400-0x4FF)
- MEMSET(0x400): Set memory block
- MEMCPY(0x401): Copy memory block
- MEMCMP(0x402): Compare memory blocks
- HEAP_ALLOC(0x410): Allocate from heap
- HEAP_FREE(0x411): Free to heap

## Hilbert Curve (0x500-0x5FF)
- HILBERT_ENCODE(0x500): (x,y) -> index
- HILBERT_DECODE(0x501): index -> (x,y)

## Semantic Operations (0x600-0x6FF)
- EMBEDDING_COSINE(0x600): Cosine similarity
- EMBEDDING_ADD(0x601): Add embeddings
- SEMANTIC_NEAREST(0x610): Find nearest neighbors
"""


if __name__ == '__main__':
    import sys
    output_path = sys.argv[1] if len(sys.argv) > 1 else 'libgeo.spv'

    binary = build_libgeo()
    with open(output_path, 'wb') as f:
        f.write(binary)

    print(f"[libgeo] Generated {output_path} ({len(binary)} bytes)")
    print(f"[libgeo] {len(LIBGEO_FUNCTIONS)} functions available")
