"""
Geometry OS SPIR-V Emitter V2 - Produces Valid SPIR-V Binaries

This emitter generates properly structured SPIR-V compute shaders that can
execute on the GPU and output results via storage buffers.

SPIR-V Structure (in order):
1. Magic header
2. Capability (Shader)
3. Extensions (optional)
4. Memory model
5. Entry point declaration
6. Execution modes
7. Debug info (optional)
8. Annotations (decorations)
9. Types and constants (GLOBAL)
10. Functions
"""

import struct
from typing import List, Dict, Any, Optional


class SPIRVEmitterV2:
    """Generates valid SPIR-V binary modules for Geometry OS."""

    # SPIR-V Magic number
    MAGIC = 0x07230203

    # Version 1.0
    VERSION = 0x00010000

    # Generator ID (custom)
    GENERATOR = 0x00100000  # Geometry OS Compiler

    # =============================================================================
    # SPIR-V Core Opcode Constants
    # =============================================================================

    # Capabilities
    OP_CAPABILITY = 17
    CAP_SHADER = 1

    # Memory Model
    OP_MEMORY_MODEL = 14
    LOGICAL = 0
    GLSL450 = 1

    # Entry Point
    OP_ENTRY_POINT = 15
    OP_EXECUTION_MODE = 16
    EXEC_MODE_LOCAL_SIZE = 17

    # Source Extension
    OP_SOURCE_EXTENSION = 11

    # Decorations
    OP_DECORATE = 71
    OP_MEMBER_DECORATE = 72
    DEC_BLOCK = 2
    DEC_BUFFER_BLOCK = 3
    DEC_DESCRIPTOR_SET = 34
    DEC_BINDING = 33
    DEC_OFFSET = 35

    # Types
    OP_TYPE_VOID = 19
    OP_TYPE_BOOL = 20
    OP_TYPE_INT = 21
    OP_TYPE_FLOAT = 22
    OP_TYPE_VECTOR = 23
    OP_TYPE_ARRAY = 28
    OP_TYPE_STRUCT = 30
    OP_TYPE_POINTER = 32
    OP_TYPE_FUNCTION = 33

    # Storage Classes
    STORAGE_CLASS_UNIFORM = 2
    STORAGE_CLASS_UNIFORM_CONSTANT = 0
    STORAGE_CLASS_INPUT = 1
    STORAGE_CLASS_OUTPUT = 3
    STORAGE_CLASS_PRIVATE = 5
    STORAGE_CLASS_FUNCTION = 7

    # Constants
    OP_CONSTANT = 43
    OP_CONSTANT_COMPOSITE = 44

    # Variables
    OP_VARIABLE = 59
    OP_ACCESS_CHAIN = 65

    # Functions
    OP_FUNCTION = 54
    OP_FUNCTION_PARAMETER = 55
    OP_FUNCTION_CALL = 57
    OP_FUNCTION_END = 56

    # Control Flow
    OP_LABEL = 248
    OP_BRANCH = 249
    OP_RETURN = 253

    # Memory Operations
    OP_LOAD = 61
    OP_STORE = 62

    # Arithmetic (Float)
    OP_FADD = 129
    OP_FSUB = 131
    OP_FMUL = 133
    OP_FDIV = 136

    # Extended Instructions
    OP_EXT_INST_IMPORT = 11
    OP_EXT_INST = 80
    GLSL_STD_450 = "GLSL.std.450"

    def __init__(self):
        self.words: List[int] = []
        self.id_bound = 1
        self.id_map: Dict[str, int] = {}

        # Sections for proper ordering
        self.capabilities: List[int] = []
        self.extensions: List[str] = []
        self.entry_points: List[tuple] = []
        self.execution_modes: List[tuple] = []
        self.decorations: List[tuple] = []
        self.types_constants: List[int] = []
        self.global_variables: List[tuple] = []  # Module-scope variables
        self.functions: List[int] = []

        # Track GLSLstd450 import
        self.glsl_ext_id: Optional[int] = None

    def next_id(self) -> int:
        """Allocate and return the next Result ID."""
        tid = self.id_bound
        self.id_bound += 1
        return tid

    def get_or_create_id(self, name: str, creator=None) -> int:
        """Get existing ID or create new one."""
        if name in self.id_map:
            return self.id_map[name]
        if creator:
            id = creator()
            self.id_map[name] = id
            return id
        id = self.next_id()
        self.id_map[name] = id
        return id

    def emit_word(self, word: int):
        """Add a single word to the current section."""
        self.words.append(word)

    def emit_instruction(self, opcode: int, *operands):
        """Emit a complete instruction with word count."""
        word_count = len(operands) + 1
        word0 = (word_count << 16) | opcode
        self.words.append(word0)
        for op in operands:
            if isinstance(op, float):
                # Encode float as 32-bit uint
                self.words.append(struct.unpack('<I', struct.pack('<f', op))[0])
            elif isinstance(op, str):
                # Encode string (null-padded to word boundary)
                encoded = op.encode('utf-8') + b'\x00'
                while len(encoded) % 4 != 0:
                    encoded += b'\x00'
                for i in range(0, len(encoded), 4):
                    self.words.append(struct.unpack('<I', encoded[i:i+4])[0])
            else:
                self.words.append(op)

    # =========================================================================
    # Section Builders
    # =========================================================================

    def add_capability(self, cap: int):
        """Add a capability requirement."""
        self.capabilities.append(cap)

    def add_extension(self, ext: str):
        """Add an extension requirement."""
        self.extensions.append(ext)

    def set_entry_point(self, entry_id: int, exec_model: int, name: str, *interfaces):
        """Set the shader entry point."""
        self.entry_points.append((exec_model, entry_id, name, interfaces))

    def add_execution_mode(self, entry_id: int, mode: int, *args):
        """Add an execution mode for an entry point."""
        self.execution_modes.append((entry_id, mode, args))

    def add_decoration(self, target_id: int, decoration: int, *args):
        """Add a decoration to an ID."""
        self.decorations.append((self.OP_DECORATE, target_id, decoration, args))

    def add_member_decoration(self, struct_type_id: int, member_index: int, decoration: int, *args):
        """Add a member decoration to a struct."""
        self.decorations.append((self.OP_MEMBER_DECORATE, struct_type_id, member_index, decoration, args))

    # =========================================================================
    # Type Builders
    # =========================================================================

    def declare_void_type(self) -> int:
        """Declare void type, return its ID."""
        id = self.next_id()
        self.types_constants.append((self.OP_TYPE_VOID, id))
        return id

    def declare_float_type(self, width: int = 32) -> int:
        """Declare float type, return its ID."""
        id = self.next_id()
        self.types_constants.append((self.OP_TYPE_FLOAT, id, width))
        return id

    def declare_int_type(self, width: int = 32, signed: bool = True) -> int:
        """Declare int type, return its ID."""
        id = self.next_id()
        self.types_constants.append((self.OP_TYPE_INT, id, width, 1 if signed else 0))
        return id

    def declare_pointer_type(self, storage_class: int, pointee_type: int) -> int:
        """Declare pointer type, return its ID."""
        id = self.next_id()
        self.types_constants.append((self.OP_TYPE_POINTER, id, storage_class, pointee_type))
        return id

    def declare_struct_type(self, *member_types) -> int:
        """Declare struct type, return its ID."""
        id = self.next_id()
        self.types_constants.append((self.OP_TYPE_STRUCT, id, *member_types))
        return id

    def declare_function_type(self, return_type: int, *param_types) -> int:
        """Declare function type, return its ID."""
        id = self.next_id()
        self.types_constants.append((self.OP_TYPE_FUNCTION, id, return_type, *param_types))
        return id

    def declare_array_type(self, element_type: int, length_id: int) -> int:
        """Declare array type, return its ID."""
        id = self.next_id()
        self.types_constants.append((self.OP_TYPE_ARRAY, id, element_type, length_id))
        return id

    # =========================================================================
    # Constant Builders
    # =========================================================================

    def declare_constant(self, type_id: int, value: float) -> int:
        """Declare a constant, return its ID."""
        id = self.next_id()
        self.types_constants.append((self.OP_CONSTANT, type_id, id, value))
        return id

    def declare_int_constant(self, type_id: int, value: int) -> int:
        """Declare an integer constant, return its ID."""
        id = self.next_id()
        self.types_constants.append((self.OP_CONSTANT, type_id, id, value))
        return id

    # =========================================================================
    # Function Building
    # =========================================================================

    def begin_function(self, result_type: int, function_id: int, function_type: int):
        """Begin a function definition."""
        self.functions.append((self.OP_FUNCTION, result_type, function_id, 0, function_type))

    def emit_label(self, label_id: int):
        """Emit a label."""
        self.functions.append((self.OP_LABEL, label_id))

    def emit_fadd(self, result_type: int, result_id: int, operand1: int, operand2: int):
        """Emit floating-point addition."""
        self.functions.append((self.OP_FADD, result_type, result_id, operand1, operand2))

    def emit_fsub(self, result_type: int, result_id: int, operand1: int, operand2: int):
        """Emit floating-point subtraction."""
        self.functions.append((self.OP_FSUB, result_type, result_id, operand1, operand2))

    def emit_fmul(self, result_type: int, result_id: int, operand1: int, operand2: int):
        """Emit floating-point multiplication."""
        self.functions.append((self.OP_FMUL, result_type, result_id, operand1, operand2))

    def emit_variable(self, result_type: int, result_id: int, storage_class: int):
        """Emit a local variable declaration (inside function)."""
        self.functions.append((self.OP_VARIABLE, result_type, result_id, storage_class))

    def emit_global_variable(self, result_type: int, result_id: int, storage_class: int):
        """Emit a module-scope global variable declaration."""
        self.global_variables.append((self.OP_VARIABLE, result_type, result_id, storage_class))

    def emit_access_chain(self, result_type: int, result_id: int, base_id: int, *indices):
        """Emit an access chain."""
        self.functions.append((self.OP_ACCESS_CHAIN, result_type, result_id, base_id, *indices))

    def emit_load(self, result_type: int, result_id: int, pointer_id: int):
        """Emit a load instruction."""
        self.functions.append((self.OP_LOAD, result_type, result_id, pointer_id))

    def emit_store(self, pointer_id: int, object_id: int):
        """Emit a store instruction."""
        self.functions.append((self.OP_STORE, pointer_id, object_id))

    def emit_return(self):
        """Emit a return instruction."""
        self.functions.append((self.OP_RETURN,))

    def end_function(self):
        """End the current function."""
        self.functions.append((self.OP_FUNCTION_END,))

    # =========================================================================
    # Final Binary Generation
    # =========================================================================

    def finalize(self) -> bytes:
        """Generate the complete SPIR-V binary."""
        words = []

        # 1. Header
        words.extend([self.MAGIC, self.VERSION, self.GENERATOR, self.id_bound, 0])

        # 2. Capabilities
        for cap in self.capabilities:
            wc = 2
            words.append((wc << 16) | self.OP_CAPABILITY)
            words.append(cap)

        # 3. Extensions
        for ext in self.extensions:
            encoded = ext.encode('utf-8') + b'\x00'
            while len(encoded) % 4 != 0:
                encoded += b'\x00'
            string_words = [struct.unpack('<I', encoded[i:i+4])[0]
                          for i in range(0, len(encoded), 4)]
            wc = 1 + len(string_words)
            words.append((wc << 16) | self.OP_SOURCE_EXTENSION)
            words.extend(string_words)

        # 4. Memory Model
        words.append((3 << 16) | self.OP_MEMORY_MODEL)
        words.extend([self.LOGICAL, self.GLSL450])

        # 5. Entry Points
        for exec_model, entry_id, name, interfaces in self.entry_points:
            encoded = name.encode('utf-8') + b'\x00'
            while len(encoded) % 4 != 0:
                encoded += b'\x00'
            string_words = [struct.unpack('<I', encoded[i:i+4])[0]
                          for i in range(0, len(encoded), 4)]
            # Word count = 1 (opcode) + 1 (exec_model) + 1 (entry_id) + string_words + interfaces
            wc = 3 + len(string_words) + len(interfaces)
            words.append((wc << 16) | self.OP_ENTRY_POINT)
            words.append(exec_model)
            words.append(entry_id)
            words.extend(string_words)
            words.extend(interfaces)

        # 6. Execution Modes
        for entry_id, mode, args in self.execution_modes:
            wc = 3 + len(args)
            words.append((wc << 16) | self.OP_EXECUTION_MODE)
            words.append(entry_id)
            words.append(mode)
            words.extend(args)

        # 7. Decorations (both OpDecorate and OpMemberDecorate)
        for dec_instr in self.decorations:
            opcode = dec_instr[0]
            if opcode == self.OP_DECORATE:
                # Format: (OP_DECORATE, target_id, decoration, args)
                target_id = dec_instr[1]
                decoration = dec_instr[2]
                args = dec_instr[3]
                wc = 3 + len(args)
                words.append((wc << 16) | self.OP_DECORATE)
                words.append(target_id)
                words.append(decoration)
                words.extend(args)
            elif opcode == self.OP_MEMBER_DECORATE:
                # Format: (OP_MEMBER_DECORATE, struct_type_id, member_index, decoration, args)
                struct_type_id = dec_instr[1]
                member_index = dec_instr[2]
                decoration = dec_instr[3]
                args = dec_instr[4]
                wc = 4 + len(args)
                words.append((wc << 16) | self.OP_MEMBER_DECORATE)
                words.append(struct_type_id)
                words.append(member_index)
                words.append(decoration)
                words.extend(args)

        # 8. Types and Constants
        for instr in self.types_constants:
            opcode = instr[0]
            operands = instr[1:]
            # Handle float values specially
            processed_ops = []
            for op in operands:
                if isinstance(op, float):
                    processed_ops.append(struct.unpack('<I', struct.pack('<f', op))[0])
                else:
                    processed_ops.append(op)
            wc = 1 + len(processed_ops)
            words.append((wc << 16) | opcode)
            words.extend(processed_ops)

        # 9. Global Variables (module-scope)
        for instr in self.global_variables:
            opcode = instr[0]
            operands = instr[1:]
            wc = 1 + len(operands)
            words.append((wc << 16) | opcode)
            words.extend(operands)

        # 10. Functions
        for instr in self.functions:
            opcode = instr[0]
            operands = instr[1:]
            wc = 1 + len(operands)
            words.append((wc << 16) | opcode)
            words.extend(operands)

        return struct.pack('<' + 'I' * len(words), *words)


def create_compute_shader_with_output(compute_fn) -> bytes:
    """
    Create a valid SPIR-V compute shader with storage buffer output.

    Args:
        compute_fn: Callback function that receives (emitter, float_type, output_ptr)
                   and emits the computation instructions.

    Returns:
        Valid SPIR-V binary bytes.
    """
    emitter = SPIRVEmitterV2()

    # Add required capabilities
    emitter.add_capability(emitter.CAP_SHADER)

    # Declare types
    void_type = emitter.declare_void_type()
    float_type = emitter.declare_float_type(32)
    int_type = emitter.declare_int_type(32, False)  # unsigned int for array size

    # Create output struct: struct Output { float value; };
    output_struct_type = emitter.declare_struct_type(float_type)

    # Pointer to output struct (Uniform storage class for storage buffer)
    output_ptr_type = emitter.declare_pointer_type(emitter.STORAGE_CLASS_UNIFORM, output_struct_type)

    # Function type: void main()
    func_type = emitter.declare_function_type(void_type)

    # Main function ID
    main_id = emitter.next_id()

    # Entry point
    emitter.set_entry_point(main_id, 5, "main")  # 5 = GLCompute

    # Execution mode: local_size_x=1, local_size_y=1, local_size_z=1
    emitter.add_execution_mode(main_id, emitter.EXEC_MODE_LOCAL_SIZE, 1, 1, 1)

    # Decorate output struct as BufferBlock
    emitter.add_decoration(output_struct_type, emitter.DEC_BUFFER_BLOCK)

    # Decorate struct member offset (REQUIRED for BufferBlock)
    emitter.add_member_decoration(output_struct_type, 0, emitter.DEC_OFFSET, 0)

    # Create output variable
    output_var_id = emitter.next_id()
    emitter.add_decoration(output_var_id, emitter.DEC_DESCRIPTOR_SET, 0)
    emitter.add_decoration(output_var_id, emitter.DEC_BINDING, 0)

    # Declare output variable at module scope (REQUIRED for Uniform storage class)
    emitter.emit_global_variable(output_ptr_type, output_var_id, emitter.STORAGE_CLASS_UNIFORM)

    # Begin main function
    emitter.begin_function(void_type, main_id, func_type)

    # Entry label
    label_id = emitter.next_id()
    emitter.emit_label(label_id)

    # Call user compute function to emit computation
    compute_fn(emitter, float_type, output_var_id)

    # Return
    emitter.emit_return()

    # End function
    emitter.end_function()

    return emitter.finalize()


if __name__ == "__main__":
    # Test: Create a simple compute shader that outputs 42.0
    def simple_compute(emitter, float_type, output_var):
        # Create constant 42.0
        const_id = emitter.next_id()
        emitter.types_constants.append((emitter.OP_CONSTANT, float_type, const_id, 42.0))

        # Store to output buffer at index 0
        int_type = emitter.declare_int_type(32, False)
        index_id = emitter.next_id()
        emitter.types_constants.append((emitter.OP_CONSTANT, int_type, index_id, 0))

        # Access chain to get pointer to output[0]
        ptr_type = emitter.declare_pointer_type(emitter.STORAGE_CLASS_UNIFORM, float_type)
        access_id = emitter.next_id()
        emitter.emit_access_chain(ptr_type, access_id, output_var, index_id)

        # Store result
        emitter.emit_store(access_id, const_id)

    binary = create_compute_shader_with_output(simple_compute)
    with open("test_valid.spv", "wb") as f:
        f.write(binary)
    print(f"Generated test_valid.spv ({len(binary)} bytes)")
