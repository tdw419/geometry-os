import struct

class SPIRVEmitter:
    """Low-level SPIR-V binary builder for Geometry OS."""
    
    def __init__(self):
        self.words = []
        self.id_bound = 1
        
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

    def finalize(self):
        """Prepend header and return binary bytes."""
        # Header: Magic, Version (1.0), Generator (0), Bound, Reserved (0)
        header = [0x07230203, 0x00010000, 0, self.id_bound, 0]
        return struct.pack('<' + 'I' * (len(header) + len(self.words)), *(header + self.words))

# SPIR-V Opcode Constants
OP_CAPABILITY = 17
OP_MEMORY_MODEL = 14
OP_TYPE_FLOAT = 22
OP_TYPE_VOID = 19
OP_TYPE_FUNCTION = 33
OP_FUNCTION = 54
OP_LABEL = 248
OP_RETURN = 253
OP_FUNCTION_END = 56
OP_CONSTANT = 43
OP_FADD = 129
OP_FSUB = 131
OP_FMUL = 133
OP_FNEGATE = 127
OP_LOAD = 61
OP_STORE = 62
OP_VARIABLE = 59
OP_DECORATE = 71
