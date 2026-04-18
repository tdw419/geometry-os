// riscv/decode/mod.rs -- RV32I/RV32C instruction decode (Phase 34)
//
// Re-exports from submodules. The public API is:
//   - Operation (enum)
//   - decode(word: u32) -> Operation
//   - decode_c(halfword: u16) -> Operation
//   - is_compressed(halfword: u16) -> bool

mod compressed;
mod operation;
mod rv32i;

pub use compressed::{decode_c, is_compressed};
pub use operation::Operation;
pub use rv32i::decode;

#[cfg(test)]
mod tests;
