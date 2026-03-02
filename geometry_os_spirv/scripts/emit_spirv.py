import struct
from typing import List, Dict, Any, Optional

class SPIRVEmitter:
    """Low-level SPIR-V binary builder for Geometry OS."""

    def __init__(self):
        self.words = []
        self.id_bound = 1
        self.labels: Dict[str, int] = {}  # Label name -> ID mapping
        self.label_targets: Dict[int, int] = {}  # Label ID -> target address
        self.pending_branches: List[tuple] = []  # (word_index, label_name)

    def next_id(self):
        tid = self.id_bound
        self.id_bound += 1
        return tid

    def emit(self, opcode, *operands):
        # SPIR-V Opcode word: [WordCount | Opcode]
        word0 = (len(operands) + 1) << 16 | opcode
        self.words.append(word0)
        for op in operands:
            if isinstance(op, float):
                # Encode float as 32-bit uint
                self.words.append(struct.unpack('I', struct.pack('f', op))[0])
            else:
                self.words.append(op)

    def create_label(self, name: str) -> int:
        """Create a new label and return its ID."""
        label_id = self.next_id()
        self.labels[name] = label_id
        return label_id

    def emit_label(self, name: str):
        """Emit a label at the current position."""
        label_id = self.create_label(name)
        self.emit(OP_LABEL, label_id)
        return label_id

    def emit_branch(self, condition_id: Optional[int], target_label: str):
        """Emit a conditional or unconditional branch."""
        target_id = self.labels.get(target_label)
        if target_id is None:
            # Forward reference - will be resolved later
            target_id = 0
            self.pending_branches.append((len(self.words), target_label))

        if condition_id is not None:
            self.emit(OP_BRANCH_CONDITIONAL, condition_id, target_id, target_id)
        else:
            self.emit(OP_BRANCH, target_id)

    def finalize(self):
        """Prepend header and return binary bytes."""
        # Resolve pending branch targets
        for word_idx, label_name in self.pending_branches:
            if label_name in self.labels:
                # Update the branch target in the word stream
                # This is simplified - real implementation needs proper offset calculation
                pass

        # Header: Magic, Version (1.0), Generator (0), Bound, Reserved (0)
        header = [0x07230203, 0x00010000, 0, self.id_bound, 0]
        return struct.pack('<' + 'I' * (len(header) + len(self.words)), *(header + self.words))


# =============================================================================
# SPIR-V Core Opcode Constants
# =============================================================================

# Capabilities and Memory Model
OP_CAPABILITY = 17
OP_MEMORY_MODEL = 14
OP_ENTRY_POINT = 15
OP_EXECUTION_MODE = 16

# Types
OP_TYPE_VOID = 19
OP_TYPE_BOOL = 20
OP_TYPE_INT = 21
OP_TYPE_FLOAT = 22
OP_TYPE_VECTOR = 23
OP_TYPE_MATRIX = 24
OP_TYPE_IMAGE = 25
OP_TYPE_SAMPLER = 26
OP_TYPE_SAMPLED_IMAGE = 27
OP_TYPE_ARRAY = 28
OP_TYPE_RUNTIME_ARRAY = 29
OP_TYPE_STRUCT = 30
OP_TYPE_OPAQUE = 31
OP_TYPE_POINTER = 32
OP_TYPE_FUNCTION = 33

# Constants
OP_CONSTANT_TRUE = 41
OP_CONSTANT_FALSE = 42
OP_CONSTANT = 43
OP_CONSTANT_COMPOSITE = 44
OP_CONSTANT_SAMPLER = 45
OP_CONSTANT_NULL = 46

# Memory Instructions
OP_VARIABLE = 59
OP_IMAGE_TEXEL_POINTER = 60
OP_LOAD = 61
OP_STORE = 62
OP_COPY_MEMORY = 63
OP_COPY_MEMORY_SIZED = 64
OP_ACCESS_CHAIN = 65
OP_IN_BOUNDS_ACCESS_CHAIN = 66
OP_PTR_ACCESS_CHAIN = 67
OP_ARRAY_LENGTH = 68
OP_GENERIC_PTR_MEM_SEMANTICS = 69

# Function Instructions
OP_FUNCTION = 54
OP_FUNCTION_PARAMETER = 55
OP_FUNCTION_CALL = 57
OP_FUNCTION_END = 56

# Control Flow
OP_LABEL = 248
OP_BRANCH = 249
OP_BRANCH_CONDITIONAL = 250
OP_SWITCH = 251
OP_KILL = 252
OP_RETURN = 253
OP_RETURN_VALUE = 254
OP_UNREACHABLE = 255
OP_SELECTION_MERGE = 246
OP_LOOP_MERGE = 245

# Arithmetic (Float)
OP_FADD = 129
OP_FSUB = 131
OP_FMUL = 133
OP_FDIV = 136
OP_FNEGATE = 127
OP_FMOD = 141
OP_FREM = 228

# Arithmetic (Integer)
OP_IADD = 128
OP_ISUB = 130
OP_IMUL = 132
OP_SDIV = 134
OP_UDIV = 135
OP_SNEGATE = 126
OP_SMOD = 139
OP_UMOD = 140

# Bitwise Operations
OP_SHIFT_RIGHT_LOGICAL = 194
OP_SHIFT_RIGHT_ARITHMETIC = 195
OP_SHIFT_LEFT_LOGICAL = 196
OP_BITWISE_OR = 197
OP_BITWISE_XOR = 198
OP_BITWISE_AND = 199
OP_NOT = 204

# Logical Operations
OP_LOGICAL_EQUAL = 166
OP_LOGICAL_NOT_EQUAL = 167
OP_LOGICAL_OR = 168
OP_LOGICAL_AND = 169
OP_LOGICAL_NOT = 170

# Comparison (Float)
OP_FORD_EQUAL = 179
OP_FORD_NOT_EQUAL = 180
OP_FORD_LESS_THAN = 181
OP_FORD_GREATER_THAN = 182
OP_FORD_LESS_THAN_EQUAL = 183
OP_FORD_GREATER_THAN_EQUAL = 184

# Comparison (Integer - Signed)
OP_SCONVERT = 125
OP_I_EQUAL = 161
OP_INOT_EQUAL = 162
OP_SLESS_THAN = 163
OP_SGREATER_THAN = 164
OP_SLESS_THAN_EQUAL = 165
OP_SGREATER_THAN_EQUAL = 166

# Extended Instructions (GLSLstd450)
OP_EXT_INST = 80
OP_GLSL_STD_450 = 0  # Extended instruction set import

# GLSLstd450 Extended Instruction IDs
GLSL_SIN = 13
GLSL_COS = 14
GLSL_TAN = 15
GLSL_ASIN = 16
GLSL_ACOS = 17
GLSL_ATAN = 18
GLSL_SINH = 19
GLSL_COSH = 20
GLSL_TANH = 21
GLSL_ATAN2 = 25
GLSL_POW = 26
GLSL_EXP = 27
GLSL_LOG = 28
GLSL_SQRT = 31
GLSL_FABS = 4
GLSL_FMIN = 37
GLSL_FMAX = 40
GLSL_FLOOR = 8
GLSL_CEIL = 9
GLSL_FRACT = 10
GLSL_ROUND = 11
GLSL_ROUND_EVEN = 12

# Decorations
OP_DECORATE = 71
OP_MEMBER_DECORATE = 72

# =============================================================================
# Geometry OS GeoASM Opcode Constants
# =============================================================================
# These are the G channel values used in visual glyphs to encode instructions.
# G < 0x80: Constant push (B channel is the value)
# G >= 0x80: Instruction execution

# --- Arithmetic Operations (0x6A - 0x7F) ---
GEO_FADD = 0x6A        # Float addition
GEO_FSUB = 0x6B        # Float subtraction
GEO_FMUL = 0x6C        # Float multiplication
GEO_FDIV = 0x6D        # Float division
GEO_FMOD = 0x6E        # Float modulo
GEO_FNEG = 0x6F        # Float negation

# --- Comparison Operations (0x10 - 0x1F, 0xB0 - 0xBF) ---
GEO_GT = 0x10          # Greater than
GEO_LT = 0x11          # Less than
GEO_EQ = 0xB0          # Equal
GEO_NEQ = 0xB1         # Not equal
GEO_GTE = 0xB2         # Greater than or equal
GEO_LTE = 0xB3         # Less than or equal

# --- Trigonometric Operations (0x70 - 0x7F) ---
GEO_SIN = 0x70         # Sine
GEO_COS = 0x71         # Cosine
GEO_TAN = 0x72         # Tangent
GEO_ASIN = 0x73        # Arc sine
GEO_ACOS = 0x74        # Arc cosine
GEO_ATAN = 0x75        # Arc tangent
GEO_SQRT = 0x76        # Square root
GEO_POW = 0x77         # Power
GEO_ABS = 0x78         # Absolute value
GEO_FLOOR = 0x79       # Floor
GEO_CEIL = 0x7A        # Ceiling

# --- Memory Operations (0x80 - 0x8F) ---
GEO_LOAD = 0x80        # Load from memory address
GEO_STORE = 0x81       # Store to memory address
GEO_ALLOC = 0x82       # Allocate memory block
GEO_FREE = 0x83        # Free memory block
GEO_MEMCPY = 0x84      # Copy memory block
GEO_MEMSET = 0x85      # Set memory block
GEO_LOAD_LOCAL = 0x86  # Load from local memory
GEO_STORE_LOCAL = 0x87 # Store to local memory

# --- Control Flow Operations (0x90 - 0x9F) ---
GEO_JMP = 0x90         # Unconditional jump
GEO_JZ = 0x91          # Jump if zero
GEO_JNZ = 0x92         # Jump if not zero
GEO_CALL = 0x93        # Function call
GEO_RET = 0x94         # Function return
GEO_LOOP = 0x95        # Loop start
GEO_ENDLOOP = 0x96     # Loop end
GEO_BREAK = 0x97       # Break out of loop
GEO_CONTINUE = 0x98    # Continue to next iteration
GEO_FOR = 0x99         # For loop (B channel = iterations)

# --- Logical Operations (0xA0 - 0xAF) ---
GEO_AND = 0xA0         # Bitwise AND
GEO_OR = 0xA1          # Bitwise OR
GEO_XOR = 0xA2         # Bitwise XOR
GEO_NOT = 0xA3         # Bitwise NOT
GEO_SHL = 0xA4         # Shift left
GEO_SHR = 0xA5         # Shift right
GEO_ROL = 0xA6         # Rotate left
GEO_ROR = 0xA7         # Rotate right
GEO_LAND = 0xA8        # Logical AND
GEO_LOR = 0xA9         # Logical OR
GEO_LNOT = 0xAA        # Logical NOT

# --- Integer Operations (0xC0 - 0xCF) ---
GEO_IADD = 0xC0        # Integer addition
GEO_ISUB = 0xC1        # Integer subtraction
GEO_IMUL = 0xC2        # Integer multiplication
GEO_IDIV = 0xC3        # Integer division
GEO_IMOD = 0xC4        # Integer modulo
GEO_INEG = 0xC5        # Integer negation

# --- Stack Operations (0xD0 - 0xDF) ---
GEO_DUP = 0xD0         # Duplicate top of stack
GEO_DROP = 0xD1        # Drop top of stack
GEO_SWAP = 0xD2        # Swap top two elements
GEO_OVER = 0xD3        # Copy second element to top
GEO_ROT = 0xD4         # Rotate third element to top
GEO_PICK = 0xD5        # Pick element at depth B
GEO_DEPTH = 0xD6       # Push stack depth

# --- System Operations (0xE0 - 0xEF) ---
GEO_SYSCALL = 0xE0     # System call (B channel = syscall number)
GEO_HALT = 0xE1        # Halt execution
GEO_NOP = 0xE2         # No operation
GEO_DEBUG = 0xE3       # Debug breakpoint
GEO_YIELD = 0xE4       # Yield to scheduler

# --- I/O Operations (0xF0 - 0xFF) ---
GEO_READ = 0xF0        # Read from I/O port
GEO_WRITE = 0xF1       # Write to I/O port
GEO_IN = 0xF2          # Input from device
GEO_OUT = 0xF3         # Output to device

# --- Visual Memory Operations (Custom 204-205) ---
GEO_VISUAL_LOAD = 204  # Load from secondary visual RAM buffer
GEO_VISUAL_STORE = 205 # Store to secondary visual RAM buffer
