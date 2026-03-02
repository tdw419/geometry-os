"""
Geometry OS Visual Compiler V2 (SPIR-V)

Converts a visual program PNG into a VALID SPIR-V compute shader binary.
Uses a Stack-based (Postfix) execution model with storage buffer output.

Key improvements over V1:
- Constants declared globally (before functions)
- Proper entry point and execution mode
- Storage buffer for output (enables reading results from GPU)
- Valid SPIR-V structure that passes spirv-val
"""

import sys
import os
import numpy as np
from PIL import Image
from pathlib import Path

# Add project root to path for core utilities
sys.path.append(str(Path(__file__).parent.parent.parent))
from core.hilbert_util import HilbertCurve
from geometry_os_spirv.scripts.emit_spirv_v2 import SPIRVEmitterV2


# =============================================================================
# GeoASM Opcode Constants (G channel values in glyphs)
# =============================================================================
# G == 0: Constant push (B channel is the value)
# G != 0: Instruction execution

GEO_FADD = 0x6A        # Float addition
GEO_FSUB = 0x6B        # Float subtraction
GEO_FMUL = 0x6C        # Float multiplication
GEO_FDIV = 0x6D        # Float division
GEO_FNEG = 0x6F        # Float negation


class VisualCompilerV2:
    """Compiles visual glyph programs to valid SPIR-V compute shaders."""

    def __init__(self):
        self.emitter = None
        self.float_type = None
        self.int_type = None
        self.void_type = None
        self.output_var = None
        self.output_ptr_type = None

        # Runtime IDs created during compilation
        self.constants = {}  # value -> id mapping
        self.stack = []  # Stack of result IDs

    def compile(self, image_path: str, output_path: str) -> bool:
        """
        Compile a visual program PNG to SPIR-V compute shader.

        Args:
            image_path: Path to input PNG image
            output_path: Path to output .spv file

        Returns:
            True if compilation succeeded
        """
        print(f"[GOS Compiler V2] Compiling {image_path}...")

        # Load image
        img = Image.open(image_path).convert("RGBA")
        data = np.array(img)

        h, w, _ = data.shape
        glyph_size = 16
        grid_size = w // glyph_size

        if grid_size == 0:
            print(f"Error: Image too small. Need at least {glyph_size}x{glyph_size} pixels")
            return False

        # Setup emitter
        self.emitter = SPIRVEmitterV2()
        self._setup_shader_framework()

        # Read and process glyphs in Hilbert order
        order = int(np.log2(grid_size)) if grid_size > 0 else 0
        if order == 0:
            order = 1
        curve = HilbertCurve(order)

        # Collect operations first (to emit constants globally)
        operations = []
        for d in range(grid_size * grid_size):
            gx, gy = curve.d2xy(d)
            px, py = gx * glyph_size, gy * glyph_size

            # Sample the center pixel of the glyph for semantic data
            sample_y = min(py + glyph_size // 2, h - 1)
            sample_x = min(px + glyph_size // 2, w - 1)
            sample = data[sample_y, sample_x]

            r, g, b, a = sample

            if a < 128:
                continue  # Skip empty/transparent pixels

            operations.append((g, b))

        # Emit all constants FIRST (global scope)
        for g, b in operations:
            if g == 0:  # Constant (G == 0 means push B as value)
                val = float(b)
                if val not in self.constants:
                    self.constants[val] = self._emit_constant(val)

        # Emit output variable at MODULE SCOPE (required for Uniform storage class)
        self.emitter.emit_global_variable(self.output_ptr_type, self.output_var, self.emitter.STORAGE_CLASS_UNIFORM)

        # Begin main function
        main_id = self.emitter.next_id()
        func_type = self.emitter.declare_function_type(self.void_type)
        self.emitter.begin_function(self.void_type, main_id, func_type)

        # Entry point
        self.emitter.set_entry_point(main_id, 5, "main")  # 5 = GLCompute
        self.emitter.add_execution_mode(main_id, self.emitter.EXEC_MODE_LOCAL_SIZE, 1, 1, 1)

        # Entry label
        label_id = self.emitter.next_id()
        self.emitter.emit_label(label_id)

        # Execute operations
        self.stack = []
        for g, b in operations:
            if g == 0:
                # Push constant
                val = float(b)
                self.stack.append(self.constants[val])
            else:
                # Execute instruction
                self._execute_instruction(g)

        # Store final result to output buffer
        if self.stack:
            result_id = self.stack[-1]
            self._store_result(result_id)

        # End function
        self.emitter.emit_return()
        self.emitter.end_function()

        # Generate and save binary
        binary = self.emitter.finalize()
        with open(output_path, "wb") as f:
            f.write(binary)

        print(f"Success: Generated {output_path} ({len(binary)} bytes)")
        return True

    def _setup_shader_framework(self):
        """Setup the SPIR-V shader framework with types and output buffer."""
        # Capabilities
        self.emitter.add_capability(self.emitter.CAP_SHADER)

        # Types
        self.void_type = self.emitter.declare_void_type()
        self.float_type = self.emitter.declare_float_type(32)
        self.int_type = self.emitter.declare_int_type(32, False)

        # Output struct: struct Output { float value; };
        output_struct_type = self.emitter.declare_struct_type(self.float_type)

        # Pointer to output struct (Uniform storage class)
        self.output_ptr_type = self.emitter.declare_pointer_type(
            self.emitter.STORAGE_CLASS_UNIFORM, output_struct_type
        )

        # Output variable ID (will be emitted in function)
        self.output_var = self.emitter.next_id()

        # Decorations
        self.emitter.add_decoration(output_struct_type, self.emitter.DEC_BUFFER_BLOCK)
        self.emitter.add_decoration(self.output_var, self.emitter.DEC_DESCRIPTOR_SET, 0)
        self.emitter.add_decoration(self.output_var, self.emitter.DEC_BINDING, 0)

        # Member offset for struct (member 0 at offset 0) - REQUIRED for BufferBlock
        self.emitter.add_member_decoration(output_struct_type, 0, self.emitter.DEC_OFFSET, 0)

    def _emit_constant(self, value: float) -> int:
        """Emit a constant and return its ID."""
        const_id = self.emitter.next_id()
        self.emitter.types_constants.append(
            (self.emitter.OP_CONSTANT, self.float_type, const_id, value)
        )
        return const_id

    def _execute_instruction(self, opcode: int):
        """Execute an instruction, modifying the stack."""
        if opcode == GEO_FADD:
            self._emit_binary_op(self.emitter.OP_FADD)
        elif opcode == GEO_FSUB:
            self._emit_binary_op(self.emitter.OP_FSUB)
        elif opcode == GEO_FMUL:
            self._emit_binary_op(self.emitter.OP_FMUL)
        elif opcode == GEO_FDIV:
            self._emit_binary_op(self.emitter.OP_FDIV)
        elif opcode == GEO_FNEG:
            self._emit_unary_op(self.emitter.OP_FNEG if hasattr(self.emitter, 'OP_FNEG') else 127)
        else:
            print(f"Warning: Unknown opcode 0x{opcode:02X}")

    def _emit_binary_op(self, spirv_opcode: int):
        """Emit a binary operation, popping two values and pushing result."""
        if len(self.stack) >= 2:
            v2 = self.stack.pop()
            v1 = self.stack.pop()
            result_id = self.emitter.next_id()
            self.emitter.functions.append((spirv_opcode, self.float_type, result_id, v1, v2))
            self.stack.append(result_id)

    def _emit_unary_op(self, spirv_opcode: int):
        """Emit a unary operation, popping one value and pushing result."""
        if len(self.stack) >= 1:
            v1 = self.stack.pop()
            result_id = self.emitter.next_id()
            self.emitter.functions.append((spirv_opcode, self.float_type, result_id, v1))
            self.stack.append(result_id)

    def _store_result(self, value_id: int):
        """Store the final result to the output buffer."""
        # Create index constant for access chain
        zero_id = self.emitter.next_id()
        self.emitter.types_constants.append(
            (self.emitter.OP_CONSTANT, self.int_type, zero_id, 0)
        )

        # Pointer to float within struct
        float_ptr_type = self.emitter.declare_pointer_type(
            self.emitter.STORAGE_CLASS_UNIFORM, self.float_type
        )

        # Access chain to get pointer to output.value
        access_id = self.emitter.next_id()
        self.emitter.emit_access_chain(float_ptr_type, access_id, self.output_var, zero_id)

        # Store result
        self.emitter.emit_store(access_id, value_id)


def compile_visual_program(image_path: str, output_path: str) -> bool:
    """
    Compile a visual program PNG to SPIR-V compute shader.

    Args:
        image_path: Path to input PNG image
        output_path: Path to output .spv file

    Returns:
        True if compilation succeeded
    """
    compiler = VisualCompilerV2()
    return compiler.compile(image_path, output_path)


if __name__ == "__main__":
    if len(sys.argv) < 3:
        print("Usage: python3 visual_to_spirv_v2.py <input.png> <output.spv>")
        sys.exit(1)

    success = compile_visual_program(sys.argv[1], sys.argv[2])
    sys.exit(0 if success else 1)
