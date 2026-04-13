# Signed Arithmetic Reference

Geometry OS uses 32-bit registers (`u32`). All values are stored as raw bits. Whether a value is "signed" or "unsigned" depends on how the program interprets it.

## Two's Complement Convention

- Negative numbers: the high bit (bit 31) is set. `-1` = `0xFFFFFFFF`, `-2` = `0xFFFFFFFE`, etc.
- Range: `-2,147,483,648` (`0x80000000`) to `2,147,483,647` (`0x7FFFFFFF`)
- `NEG r` computes two's complement negation: `r = 0 - r` (wrapping)

## ADD, SUB, MUL — Wrapping Arithmetic

All three use Rust's `wrapping_add`, `wrapping_sub`, `wrapping_mul`. This means:

```
LDI r1, 5
LDI r2, 0xFFFFFFFF    ; -1 as i32
ADD r1, r2            ; r1 = 4 (correct signed subtraction: 5 + (-1) = 4)
```

**Signed multiplication also works correctly** due to two's complement properties:

```
LDI r1, 0xFFFFFFFF    ; -1
LDI r2, 3
MUL r1, r2            ; r1 = 0xFFFFFFFD (-3)
```

Overflow wraps silently (no overflow flag).

## DIV — Unsigned Division

Division uses **unsigned** integer division (`u32 / u32`).

```
LDI r1, 10
LDI r2, 3
DIV r1, r2            ; r1 = 3 (unsigned: 10 / 3 = 3)

LDI r1, 0xFFFFFFFF    ; -1 (4294967295 unsigned)
LDI r2, 2
DIV r1, r2            ; r1 = 2147483647 (WRONG for signed: -1/2 should be 0)
```

**For signed division, use SAR + manual correction**, or avoid negative dividends.

Division by zero is a no-op (the instruction is silently skipped).

## CMP — Signed Comparison

CMP interprets both operands as `i32` (signed) and sets `r0`:

```
CMP rd, rs
  if rd < rs (signed):  r0 = 0xFFFFFFFF (-1)
  if rd = rs:           r0 = 0
  if rd > rs (signed):  r0 = 1
```

Use with BLT/BGE:

```
CMP r1, r2
BLT r0, label    ; branch if r1 < r2 (signed)
BGE r0, label    ; branch if r1 >= r2 (signed)
```

**For unsigned comparison**, use CMP on the values after masking off the sign bit, or compare manually using SUB and checking the borrow.

## SAR vs SHR

- **SHR rd, rs** — Logical shift right. Fills vacated bits with 0.
  ```
  LDI r1, 0xFFFFFFFC    ; -4
  LDI r2, 1
  SHR r1, r2            ; r1 = 0x7FFFFFFE (2147483646)
  ```

- **SAR rd, rs** — Arithmetic shift right. Preserves sign bit.
  ```
  LDI r1, 0xFFFFFFFC    ; -4
  LDI r2, 1
  SAR r1, r2            ; r1 = 0xFFFFFFFE (-2)
  ```

Use SAR for signed division by powers of 2. Use SHR for unsigned bit manipulation.

## Practical Tips

1. **Game physics** (velocity, gravity): Use `i32` semantics. ADD/SUB/MUL/CMP/SAR all work correctly for signed values.
2. **Division by constants**: Prefer SAR over DIV when the divisor is a power of 2 — it handles negatives correctly and is faster.
3. **Random ranges**: RAND returns `u32`. Use `MOD` for unsigned range, then subtract to shift if needed.
4. **Pixel coordinates**: Screen is 256x256, well within `u32` range. No sign issues for coordinates directly, but velocity calculations should use signed arithmetic.
