"""SPIR-V Encoder for Visual Open Brain memories."""
import struct
import numpy as np
from typing import List, Dict, Any
from .memory_glyph import CATEGORY_OPCODES

class MemorySpirvEncoder:
    """Encodes memory entries into a SPIR-V binary module.

    The generated SPIR-V is a set of constants and memory operations
    that can be executed by the Geometry OS GPU executor.
    """

    def __init__(self):
        self.magic = 0x07230203
        self.version = 0x00010000
        self.generator = 0x00000000
        self.reserved = 0x00000000

    def encode_memories(self, memories: List[Dict[str, Any]]) -> bytes:
        """Convert a list of memories into a SPIR-V binary.

        Each memory is encoded as a series of OpConstant pushes followed
        by a custom OP_STORE_MEMORY instruction (or mapped to existing OpStore).
        """
        words = []
        id_bound = 1

        def next_id():
            nonlocal id_bound
            tid = id_bound
            id_bound += 1
            return tid

        def emit(opcode, operands):
            count = len(operands) + 1
            word0 = (count << 16) | opcode
            words.append(word0)
            for op in operands:
                if isinstance(op, float):
                    words.append(struct.unpack('I', struct.pack('f', op))[0])
                else:
                    words.append(op)

        # 1. SPIR-V Header Opcodes
        float_type_id = next_id()
        void_type_id = next_id()
        func_type_id = next_id()
        main_func_id = next_id()
        entry_label_id = next_id()

        emit(17, [1])  # OpCapability Shader
        emit(14, [0, 1])  # OpMemoryModel Logical GLSL450
        emit(22, [float_type_id, 32])  # OpTypeFloat 32
        emit(19, [void_type_id])  # OpTypeVoid
        emit(33, [func_type_id, void_type_id])  # OpTypeFunction
        emit(54, [void_type_id, main_func_id, 0, func_type_id])  # OpFunction
        emit(248, [entry_label_id])  # OpLabel

        # 2. Encode Memories
        # Layout in RAM:
        # Base Address = Memory ID * 512
        # Offset 0: ID
        # Offset 1: Type Opcode
        # Offset 2: Priority
        # Offset 3-386: Embedding (384 floats)
        
        for memory in memories:
            mem_id = memory.get("id", 0)
            mem_type = memory.get("type", "note")
            priority = memory.get("priority", 0.5)
            embedding = memory.get("embedding", [])
            
            opcode = CATEGORY_OPCODES.get(mem_type, 0x10)
            base_addr = mem_id * 512
            
            # Store ID
            self._emit_store(emit, next_id, float_type_id, base_addr, float(mem_id))
            # Store Type
            self._emit_store(emit, next_id, float_type_id, base_addr + 1, float(opcode))
            # Store Priority
            self._emit_store(emit, next_id, float_type_id, base_addr + 2, priority)
            
            # Store Embedding
            if embedding:
                for i, val in enumerate(embedding[:384]):
                    self._emit_store(emit, next_id, float_type_id, base_addr + 3 + i, float(val))

        # 3. Finalize
        emit(253, [])  # OpReturn
        emit(56, [])  # OpFunctionEnd

        header = [self.magic, self.version, self.generator, id_bound, self.reserved]
        return struct.pack('<' + 'I' * (len(header) + len(words)), *(header + words))

    def _emit_store(self, emit_fn, next_id_fn, type_id, address, value):
        """Emit OpConstant followed by OpStore (mapped to GEO_STORE)."""
        val_id = next_id_fn()
        # OpConstant [Type, Result, Value]
        emit_fn(43, [type_id, val_id, value])
        # OpStore [Pointer, Object] -> We use GeoASM's custom 62u which takes (addr, value_id)
        # In our executor.wgsl, opcode 62u (OP_STORE) expects: [Count|62], Addr, ValueID
        emit_fn(62, [address, val_id])
