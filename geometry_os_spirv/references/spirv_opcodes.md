# GeoASM Opcode to SPIR-V Mapping (Stack-based)

In the Stack-based model, each glyph carries an **Opcode (G)** and an **Operand (B)**.

## Opcodes (G Channel)

| GeoASM Op (G) | Symbol | SPIR-V Instruction | Opcode (Hex) | Description | Stack Effect |
|---------------|--------|---------------------|--------------|-------------|--------------|
| `0x00` - `0x7F`| `N/A` | `OpConstant`        | `0x2B`       | Push Immediate | ( -> val)   |
| `0x6A`        | `+`    | `OpFAdd`            | `0x81`       | Float Add   | (v1, v2 -> v1+v2) |
| `0x6B`        | `-`    | `OpFSub`            | `0x83`       | Float Subtract| (v1, v2 -> v1-v2) |
| `0x6C`        | `*`    | `OpFMul`            | `0x85`       | Float Multiply| (v1, v2 -> v1*v2) |

## Operands (B Channel)
- **For `G < 0x80`**: The B channel represents a literal **immediate float value** (0-127).
- **For `G >= 0x80`**: The B channel is currently ignored (stack-based flow).

## Compilation flow
1.  Read pixel at `(x, y)`.
2.  If `A < 128`, skip.
3.  If `G < 128`: `emitter.emit(OP_CONSTANT, float_id, next_id, float(B))`.
4.  If `G == 0x6A`: `v2 = pop(); v1 = pop(); res = add(v1, v2); push(res)`.
