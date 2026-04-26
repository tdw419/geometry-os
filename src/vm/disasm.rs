use super::types::*;
use super::Vm;

impl Vm {
    /// Disassemble one instruction starting at `addr` in RAM.
    /// Returns (mnemonic_string, instruction_length_in_words).
    /// Does not mutate VM state.
    pub fn disassemble_at(&self, addr: u32) -> (String, usize) {
        let a = addr as usize;
        if a >= self.ram.len() {
            return ("???".to_string(), 1);
        }
        let op = self.ram[a];
        let ram = |i: usize| -> u32 {
            if i < self.ram.len() {
                self.ram[i]
            } else {
                0
            }
        };
        let reg = |v: u32| -> String { format!("r{}", v) };
        match op {
            0x00 => ("HALT".into(), 1),
            0x01 => ("NOP".into(), 1),
            0x02 => ("FRAME".into(), 1),
            0x03 => {
                let fr = ram(a + 1);
                let dr = ram(a + 2);
                (format!("BEEP {}, {}", reg(fr), reg(dr)), 3)
            }
            0x04 => {
                let dr = ram(a + 1);
                let sr = ram(a + 2);
                let lr = ram(a + 3);
                (format!("MEMCPY {}, {}, {}", reg(dr), reg(sr), reg(lr)), 4)
            }
            0x10 => {
                let r = ram(a + 1);
                let imm = ram(a + 2);
                (format!("LDI {}, 0x{:X}", reg(r), imm), 3)
            }
            0x11 => {
                let r = ram(a + 1);
                let ar = ram(a + 2);
                (format!("LOAD {}, [{}]", reg(r), reg(ar)), 3)
            }
            0x12 => {
                let ar = ram(a + 1);
                let r = ram(a + 2);
                (format!("STORE [{}], {}", reg(ar), reg(r)), 3)
            }
            0x13 => {
                let x = ram(a + 1);
                let y = ram(a + 2);
                let count = ram(a + 3) as usize;
                (
                    format!(
                        "TEXTI {}, {}, \"{}\"",
                        x,
                        y,
                        (4..4 + count.min(32))
                            .map(|i| (ram(a + i) & 0xFF) as u8 as char)
                            .collect::<String>()
                    ),
                    4 + count,
                )
            }
            0x14 => {
                let ar = ram(a + 1);
                let count = ram(a + 2) as usize;
                (
                    format!(
                        "STRO {}, \"{}\"",
                        reg(ar),
                        (3..3 + count.min(32))
                            .map(|i| (ram(a + i) & 0xFF) as u8 as char)
                            .collect::<String>()
                    ),
                    3 + count,
                )
            }
            0x15 => {
                let rd = ram(a + 1);
                let imm = ram(a + 2);
                (format!("CMPI {}, {}", reg(rd), imm), 3)
            }
            0x16 => {
                let rd = ram(a + 1);
                let off = ram(a + 2);
                (format!("LOADS {}, {}", reg(rd), off as i32), 3)
            }
            0x17 => {
                let off = ram(a + 1);
                let rs = ram(a + 2);
                (format!("STORES {}, {}", off as i32, reg(rs)), 3)
            }
            0x18 => {
                let rd = ram(a + 1);
                let imm = ram(a + 2);
                (format!("SHLI {}, {}", reg(rd), imm), 3)
            }
            0x19 => {
                let rd = ram(a + 1);
                let imm = ram(a + 2);
                (format!("SHRI {}, {}", reg(rd), imm), 3)
            }
            0x1A => {
                let rd = ram(a + 1);
                let imm = ram(a + 2);
                (format!("SARI {}, {}", reg(rd), imm), 3)
            }
            0x1B => {
                let rd = ram(a + 1);
                let imm = ram(a + 2);
                (format!("ADDI {}, {}", reg(rd), imm), 3)
            }
            0x1C => {
                let rd = ram(a + 1);
                let imm = ram(a + 2);
                (format!("SUBI {}, {}", reg(rd), imm), 3)
            }
            0x1D => {
                let rd = ram(a + 1);
                let imm = ram(a + 2);
                (format!("ANDI {}, {}", reg(rd), imm), 3)
            }
            0x1E => {
                let rd = ram(a + 1);
                let imm = ram(a + 2);
                (format!("ORI {}, {}", reg(rd), imm), 3)
            }
            0x1F => {
                let rd = ram(a + 1);
                let imm = ram(a + 2);
                (format!("XORI {}, {}", reg(rd), imm), 3)
            }
            0x20 => {
                let rd = ram(a + 1);
                let rs = ram(a + 2);
                (format!("ADD {}, {}", reg(rd), reg(rs)), 3)
            }
            0x21 => {
                let rd = ram(a + 1);
                let rs = ram(a + 2);
                (format!("SUB {}, {}", reg(rd), reg(rs)), 3)
            }
            0x22 => {
                let rd = ram(a + 1);
                let rs = ram(a + 2);
                (format!("MUL {}, {}", reg(rd), reg(rs)), 3)
            }
            0x23 => {
                let rd = ram(a + 1);
                let rs = ram(a + 2);
                (format!("DIV {}, {}", reg(rd), reg(rs)), 3)
            }
            0x24 => {
                let rd = ram(a + 1);
                let rs = ram(a + 2);
                (format!("AND {}, {}", reg(rd), reg(rs)), 3)
            }
            0x25 => {
                let rd = ram(a + 1);
                let rs = ram(a + 2);
                (format!("OR {}, {}", reg(rd), reg(rs)), 3)
            }
            0x26 => {
                let rd = ram(a + 1);
                let rs = ram(a + 2);
                (format!("XOR {}, {}", reg(rd), reg(rs)), 3)
            }
            0x27 => {
                let rd = ram(a + 1);
                let rs = ram(a + 2);
                (format!("SHL {}, {}", reg(rd), reg(rs)), 3)
            }
            0x28 => {
                let rd = ram(a + 1);
                let rs = ram(a + 2);
                (format!("SHR {}, {}", reg(rd), reg(rs)), 3)
            }
            0x29 => {
                let rd = ram(a + 1);
                let rs = ram(a + 2);
                (format!("MOD {}, {}", reg(rd), reg(rs)), 3)
            }
            0x2A => {
                let rd = ram(a + 1);
                (format!("NEG {}", reg(rd)), 2)
            }
            0x2B => {
                let rd = ram(a + 1);
                let rs = ram(a + 2);
                (format!("SAR {}, {}", reg(rd), reg(rs)), 3)
            }
            0x30 => {
                let addr2 = ram(a + 1);
                (format!("JMP 0x{:04X}", addr2), 2)
            }
            0x31 => {
                let r = ram(a + 1);
                let addr2 = ram(a + 2);
                (format!("JZ {}, 0x{:04X}", reg(r), addr2), 3)
            }
            0x32 => {
                let r = ram(a + 1);
                let addr2 = ram(a + 2);
                (format!("JNZ {}, 0x{:04X}", reg(r), addr2), 3)
            }
            0x33 => {
                let addr2 = ram(a + 1);
                (format!("CALL 0x{:04X}", addr2), 2)
            }
            0x34 => ("RET".into(), 1),
            0x35 => {
                let r = ram(a + 1);
                let addr2 = ram(a + 2);
                (format!("BLT {}, 0x{:04X}", reg(r), addr2), 3)
            }
            0x36 => {
                let r = ram(a + 1);
                let addr2 = ram(a + 2);
                (format!("BGE {}, 0x{:04X}", reg(r), addr2), 3)
            }
            0x37 => {
                let xr = ram(a + 1);
                let yr = ram(a + 2);
                let wr = ram(a + 3);
                let hr = ram(a + 4);
                let id = ram(a + 5);
                (
                    format!(
                        "HITSET {}, {}, {}, {}, {}",
                        reg(xr),
                        reg(yr),
                        reg(wr),
                        reg(hr),
                        id
                    ),
                    6,
                )
            }
            0x38 => {
                let rd = ram(a + 1);
                (format!("HITQ {}", reg(rd)), 2)
            }
            0x40 => {
                let xr = ram(a + 1);
                let yr = ram(a + 2);
                let cr = ram(a + 3);
                (format!("PSET {}, {}, {}", reg(xr), reg(yr), reg(cr)), 4)
            }
            0x41 => {
                let x = ram(a + 1);
                let y = ram(a + 2);
                let c = ram(a + 3);
                (format!("PSETI {}, {}, 0x{:X}", x, y, c), 4)
            }
            0x42 => {
                let cr = ram(a + 1);
                (format!("FILL {}", reg(cr)), 2)
            }
            0x43 => {
                let xr = ram(a + 1);
                let yr = ram(a + 2);
                let wr = ram(a + 3);
                let hr = ram(a + 4);
                let cr = ram(a + 5);
                (
                    format!(
                        "RECTF {},{},{},{},{}",
                        reg(xr),
                        reg(yr),
                        reg(wr),
                        reg(hr),
                        reg(cr)
                    ),
                    6,
                )
            }
            0x44 => {
                let xr = ram(a + 1);
                let yr = ram(a + 2);
                let ar = ram(a + 3);
                (format!("TEXT {},{},[{}]", reg(xr), reg(yr), reg(ar)), 4)
            }
            0x45 => {
                let x0r = ram(a + 1);
                let y0r = ram(a + 2);
                let x1r = ram(a + 3);
                let y1r = ram(a + 4);
                let cr = ram(a + 5);
                (
                    format!(
                        "LINE {},{},{},{},{}",
                        reg(x0r),
                        reg(y0r),
                        reg(x1r),
                        reg(y1r),
                        reg(cr)
                    ),
                    6,
                )
            }
            0x46 => {
                let xr = ram(a + 1);
                let yr = ram(a + 2);
                let rr = ram(a + 3);
                let cr = ram(a + 4);
                (
                    format!("CIRCLE {},{},{},{}", reg(xr), reg(yr), reg(rr), reg(cr)),
                    5,
                )
            }
            0x47 => {
                let nr = ram(a + 1);
                (format!("SCROLL {}", reg(nr)), 2)
            }
            0x48 => {
                let rd = ram(a + 1);
                (format!("IKEY {}", reg(rd)), 2)
            }
            0x49 => {
                let rd = ram(a + 1);
                (format!("RAND {}", reg(rd)), 2)
            }
            0x4A => {
                let xr = ram(a + 1);
                let yr = ram(a + 2);
                let ar = ram(a + 3);
                let wr = ram(a + 4);
                let hr = ram(a + 5);
                (
                    format!(
                        "SPRITE {}, {}, {}, {}, {}",
                        reg(xr),
                        reg(yr),
                        reg(ar),
                        reg(wr),
                        reg(hr)
                    ),
                    6,
                )
            }
            0x4B => {
                let sr = ram(a + 1);
                let dr = ram(a + 2);
                (format!("ASM {}, {}", reg(sr), reg(dr)), 3)
            }
            0x4C => {
                let xr = ram(a + 1);
                let yr = ram(a + 2);
                let mr = ram(a + 3);
                let tr = ram(a + 4);
                let gwr = ram(a + 5);
                let ghr = ram(a + 6);
                let twr = ram(a + 7);
                let thr = ram(a + 8);
                (
                    format!(
                        "TILEMAP {}, {}, {}, {}, {}, {}, {}, {}",
                        reg(xr),
                        reg(yr),
                        reg(mr),
                        reg(tr),
                        reg(gwr),
                        reg(ghr),
                        reg(twr),
                        reg(thr)
                    ),
                    9,
                )
            }
            0x4D => {
                let ar = ram(a + 1);
                (format!("SPAWN {}", reg(ar)), 2)
            }
            0x4E => {
                let pr = ram(a + 1);
                (format!("KILL {}", reg(pr)), 2)
            }
            0x4F => {
                let rx = ram(a + 1);
                let ry = ram(a + 2);
                let rd = ram(a + 3);
                (format!("PEEK {}, {}, {}", reg(rx), reg(ry), reg(rd)), 4)
            }
            0x50 => {
                let rd = ram(a + 1);
                let rs = ram(a + 2);
                (format!("CMP {}, {}", reg(rd), reg(rs)), 3)
            }
            0x51 => {
                let rd = ram(a + 1);
                let rs = ram(a + 2);
                (format!("MOV {}, {}", reg(rd), reg(rs)), 3)
            }

            0x60 => {
                let r = ram(a + 1);
                (format!("PUSH {}", reg(r)), 2)
            }
            0x61 => {
                let r = ram(a + 1);
                (format!("POP {}", reg(r)), 2)
            }
            0x52 => {
                let n = ram(a + 1);
                (format!("SYSCALL {}", n), 2)
            }
            0x53 => ("RETK".into(), 1),
            0x54 => {
                let pr = ram(a + 1);
                let mr = ram(a + 2);
                (format!("OPEN {}, {}", reg(pr), reg(mr)), 3)
            }
            0x55 => {
                let fr = ram(a + 1);
                let br = ram(a + 2);
                let lr = ram(a + 3);
                (format!("READ {}, {}, {}", reg(fr), reg(br), reg(lr)), 4)
            }
            0x56 => {
                let fr = ram(a + 1);
                let br = ram(a + 2);
                let lr = ram(a + 3);
                (format!("WRITE {}, {}, {}", reg(fr), reg(br), reg(lr)), 4)
            }
            0x57 => {
                let fr = ram(a + 1);
                (format!("CLOSE {}", reg(fr)), 2)
            }
            0x58 => {
                let fr = ram(a + 1);
                let or_ = ram(a + 2);
                let wr = ram(a + 3);
                (format!("SEEK {}, {}, {}", reg(fr), reg(or_), reg(wr)), 4)
            }
            0x59 => {
                let br = ram(a + 1);
                (format!("LS {}", reg(br)), 2)
            }

            0x5A => ("YIELD".into(), 1),
            0x5B => {
                let r = ram(a + 1);
                (format!("SLEEP {}", reg(r)), 2)
            }
            0x5C => {
                let r = ram(a + 1);
                (format!("SETPRIORITY {}", reg(r)), 2)
            }
            0x5D => {
                let rr = ram(a + 1);
                let rw = ram(a + 2);
                (format!("PIPE {}, {}", reg(rr), reg(rw)), 3)
            }
            0x5E => {
                let r = ram(a + 1);
                (format!("MSGSND {}", reg(r)), 2)
            }
            0x5F => ("MSGRCV".into(), 1),
            0x62 => {
                let fd = ram(a + 1);
                let cmd = ram(a + 2);
                let arg = ram(a + 3);
                (format!("IOCTL {}, {}, {}", reg(fd), reg(cmd), reg(arg)), 4)
            }
            0x63 => {
                let kr = ram(a + 1);
                let vr = ram(a + 2);
                (format!("GETENV {}, {}", reg(kr), reg(vr)), 3)
            }
            0x64 => {
                let kr = ram(a + 1);
                let vr = ram(a + 2);
                (format!("SETENV {}, {}", reg(kr), reg(vr)), 3)
            }
            0x65 => ("GETPID".into(), 1),
            0x66 => {
                let r = ram(a + 1);
                (format!("EXEC {}", reg(r)), 2)
            }
            0x67 => {
                let fr = ram(a + 1);
                let sr = ram(a + 2);
                (format!("WRITESTR {}, {}", reg(fr), reg(sr)), 3)
            }
            0x68 => {
                let br = ram(a + 1);
                let mr = ram(a + 2);
                let pr = ram(a + 3);
                (format!("READLN {}, {}, {}", reg(br), reg(mr), reg(pr)), 4)
            }
            0x69 => {
                let pr = ram(a + 1);
                (format!("WAITPID {}", reg(pr)), 2)
            }
            0x6A => {
                let pr = ram(a + 1);
                let sr = ram(a + 2);
                let dr = ram(a + 3);
                (format!("EXECP {}, {}, {}", reg(pr), reg(sr), reg(dr)), 4)
            }
            0x6B => {
                let pr = ram(a + 1);
                (format!("CHDIR {}", reg(pr)), 2)
            }
            0x6C => {
                let br = ram(a + 1);
                (format!("GETCWD {}", reg(br)), 2)
            }
            0x6D => {
                let dr = ram(a + 1);
                let xr = ram(a + 2);
                let yr = ram(a + 3);
                (format!("SCREENP {}, {}, {}", reg(dr), reg(xr), reg(yr)), 4)
            }
            0x6E => ("SHUTDOWN".into(), 1),
            0x6F => {
                let cr = ram(a + 1);
                (format!("EXIT {}", reg(cr)), 2)
            }
            0x70 => {
                let pr = ram(a + 1);
                let sr = ram(a + 2);
                (format!("SIGNAL {}, {}", reg(pr), reg(sr)), 3)
            }
            0x71 => {
                let sr = ram(a + 1);
                let hr = ram(a + 2);
                (format!("SIGSET {}, {}", reg(sr), reg(hr)), 3)
            }

            0x72 => {
                let ar = ram(a + 1);
                let wr = ram(a + 2);
                if wr == 0 {
                    (format!("HYPERVISOR {}", reg(ar)), 3)
                } else {
                    (format!("HYPERVISOR {}, {}", reg(ar), reg(wr)), 3)
                }
            }

            0x73 => ("ASMSELF".into(), 1),
            0x74 => ("RUNNEXT".into(), 1),

            0x75 => {
                let ti = ram(a + 1);
                let oc = ram(a + 2);
                let dc = ram(a + 3) as usize;
                let op_name = match oc {
                    0 => "ADD",
                    1 => "SUB",
                    2 => "MUL",
                    3 => "DIV",
                    4 => "AND",
                    5 => "OR",
                    6 => "XOR",
                    7 => "NOT",
                    8 => "COPY",
                    9 => "MAX",
                    10 => "MIN",
                    11 => "MOD",
                    12 => "SHL",
                    13 => "SHR",
                    _ => "???",
                };
                let total = 4 + dc.min(MAX_FORMULA_DEPS);
                (format!("FORMULA {}, {}, {}", ti, op_name, dc), total)
            }
            0x76 => ("FORMULACLEAR".into(), 1),
            0x77 => {
                let ti = ram(a + 1);
                (format!("FORMULAREM {}", ti), 2)
            }
            0x78 => {
                let pr = ram(a + 1);
                (format!("FMKDIR [{}]", reg(pr)), 2)
            }
            0x79 => {
                let ir = ram(a + 1);
                let br = ram(a + 2);
                (format!("FSTAT {}, [{}]", reg(ir), reg(br)), 3)
            }
            0x7A => {
                let pr = ram(a + 1);
                (format!("FUNLINK [{}]", reg(pr)), 2)
            }
            0x7B => {
                let mr = ram(a + 1);
                (format!("SNAP_TRACE {}", reg(mr)), 2)
            }
            0x7C => {
                let fr = ram(a + 1);
                (format!("REPLAY {}", reg(fr)), 2)
            }
            0x7D => {
                let mr = ram(a + 1);
                (format!("FORK {}", reg(mr)), 2)
            }
            0x7E => {
                let wr = ram(a + 1);
                let fr = ram(a + 2);
                let dr = ram(a + 3);
                (format!("NOTE {}, {}, {}", reg(wr), reg(fr), reg(dr)), 4)
            }
            0x7F => {
                let ar = ram(a + 1);
                let pr = ram(a + 2);
                let fr = ram(a + 3);
                (format!("CONNECT {}, {}, {}", reg(ar), reg(pr), reg(fr)), 4)
            }
            0x80 => {
                let fr = ram(a + 1);
                let br = ram(a + 2);
                let lr = ram(a + 3);
                let sr = ram(a + 4);
                (
                    format!(
                        "SOCKSEND {}, {}, {}, {}",
                        reg(fr),
                        reg(br),
                        reg(lr),
                        reg(sr)
                    ),
                    5,
                )
            }
            0x81 => {
                let fr = ram(a + 1);
                let br = ram(a + 2);
                let mr = ram(a + 3);
                let rr = ram(a + 4);
                (
                    format!(
                        "SOCKRECV {}, {}, {}, {}",
                        reg(fr),
                        reg(br),
                        reg(mr),
                        reg(rr)
                    ),
                    5,
                )
            }
            0x82 => {
                let fr = ram(a + 1);
                (format!("DISCONNECT {}", reg(fr)), 2)
            }
            0x83 => {
                let mr = ram(a + 1);
                (format!("TRACE_READ {}", reg(mr)), 2)
            }
            0x84 => {
                let mr = ram(a + 1);
                (format!("PIXEL_HISTORY {}", reg(mr)), 2)
            }
            0x85 => {
                let xr = ram(a + 1);
                (format!("MOUSEQ {}", reg(xr)), 2)
            }
            0x86 => (
                format!("STRCMP {}, {}", reg(ram(a + 1)), reg(ram(a + 2))),
                3,
            ),
            0x87 => (format!("ABS {}", reg(ram(a + 1))), 2),
            0x88 => (
                format!(
                    "RECT {}, {}, {}, {}, {}",
                    reg(ram(a + 1)),
                    reg(ram(a + 2)),
                    reg(ram(a + 3)),
                    reg(ram(a + 4)),
                    reg(ram(a + 5))
                ),
                6,
            ),
            0x89 => (format!("MIN {}, {}", reg(ram(a + 1)), reg(ram(a + 2))), 3),
            0x8A => (format!("MAX {}, {}", reg(ram(a + 1)), reg(ram(a + 2))), 3),
            0x8B => (
                format!(
                    "CLAMP {}, {}, {}",
                    reg(ram(a + 1)),
                    reg(ram(a + 2)),
                    reg(ram(a + 3))
                ),
                4,
            ),
            0x8C => (
                format!(
                    "DRAWTEXT {}, {}, {}, {}, {}",
                    reg(ram(a + 1)),
                    reg(ram(a + 2)),
                    reg(ram(a + 3)),
                    reg(ram(a + 4)),
                    reg(ram(a + 5))
                ),
                6,
            ),
            0x8D => (
                format!("BITSET {}, {}", reg(ram(a + 1)), reg(ram(a + 2))),
                3,
            ),
            0x8E => (
                format!("BITCLR {}, {}", reg(ram(a + 1)), reg(ram(a + 2))),
                3,
            ),
            0x8F => (
                format!("BITTEST {}, {}", reg(ram(a + 1)), reg(ram(a + 2))),
                3,
            ),
            0x90 => (format!("NOT {}", reg(ram(a + 1))), 2),
            0x91 => ("INV".to_string(), 1),
            0x92 => (
                format!(
                    "MATVEC {}, {}, {}, {}, {}",
                    reg(ram(a + 1)),
                    reg(ram(a + 2)),
                    reg(ram(a + 3)),
                    reg(ram(a + 4)),
                    reg(ram(a + 5))
                ),
                6,
            ),
            0x93 => (format!("RELU {}", reg(ram(a + 1))), 2),
            0x94 => (format!("WINSYS {}", reg(ram(a + 1))), 2),
            0x95 => (
                format!(
                    "WPIXEL {}, {}, {}, {}",
                    reg(ram(a + 1)),
                    reg(ram(a + 2)),
                    reg(ram(a + 3)),
                    reg(ram(a + 4))
                ),
                5,
            ),
            0x96 => (
                format!(
                    "WREAD {}, {}, {}, {}",
                    reg(ram(a + 1)),
                    reg(ram(a + 2)),
                    reg(ram(a + 3)),
                    reg(ram(a + 4))
                ),
                5,
            ),
            0x97 => (
                format!(
                    "SPRBLT {}, {}, {}, {}",
                    reg(ram(a + 1)),
                    reg(ram(a + 2)),
                    reg(ram(a + 3)),
                    reg(ram(a + 4))
                ),
                5,
            ),

            // SCRSHOT path_addr_reg (0x98) -- save screen to VFS file
            0x98 => (format!("SCRSHOT {}", reg(ram(a + 1))), 2),

            // NET_SEND addr_reg, len_reg, dest_reg (0x99) -- send pixel data
            0x99 => (
                format!(
                    "NET_SEND {}, {}, {}",
                    reg(ram(a + 1)),
                    reg(ram(a + 2)),
                    reg(ram(a + 3))
                ),
                4,
            ),

            // NET_RECV addr_reg, max_len_reg (0x9A) -- receive pixel data
            0x9A => (
                format!("NET_RECV {}, {}", reg(ram(a + 1)), reg(ram(a + 2))),
                3,
            ),

            0x9B => (format!("PROCLS {}", ram(a + 1)), 2),

            // LLM prompt_addr_reg, response_addr_reg, max_len_reg (0x9C)
            // Send prompt to external LLM, write response to RAM.
            0x9C => (
                format!(
                    "LLM {}, {}, {}",
                    reg(ram(a + 1)),
                    reg(ram(a + 2)),
                    reg(ram(a + 3))
                ),
                4,
            ),

            // HTPARSE src_reg, dest_reg, max_lines_reg (0x9D)
            // Parse HTML from RAM into styled lines.
            0x9D => (
                format!(
                    "HTPARSE {}, {}, {}",
                    reg(ram(a + 1)),
                    reg(ram(a + 2)),
                    reg(ram(a + 3))
                ),
                4,
            ),

            // HITCLR (0x9E) -- clear hit-test regions
            0x9E => ("HITCLR".to_string(), 1),

            // ── Phase 87: Multi-Hypervisor Opcodes ──

            // VM_SPAWN (0x9F) -- create background VM
            0x9F => (
                format!("VM_SPAWN {}, {}", reg(ram(a + 1)), reg(ram(a + 2))),
                3,
            ),

            // VM_KILL (0xA0) -- kill background VM
            0xA0 => (format!("VM_KILL {}", reg(ram(a + 1))), 2),

            // VM_STATUS (0xA1) -- query background VM status
            0xA1 => (format!("VM_STATUS {}", reg(ram(a + 1))), 2),

            // VM_PAUSE (0xA2) -- pause background VM
            0xA2 => (format!("VM_PAUSE {}", reg(ram(a + 1))), 2),

            // VM_RESUME (0xA3) -- resume background VM
            0xA3 => (format!("VM_RESUME {}", reg(ram(a + 1))), 2),

            // VM_SET_BUDGET (0xA4) -- set VM time-slice budget
            0xA4 => (
                format!("VM_SET_BUDGET {}, {}", reg(ram(a + 1)), reg(ram(a + 2))),
                3,
            ),

            // VM_LIST (0xA5) -- list background VM IDs to RAM
            0xA5 => (format!("VM_LIST {}", reg(ram(a + 1))), 2),

            // ── Phase 89: AI Agent Input ──

            // AI_INJECT op_reg (0xA6) -- programmatic input injection
            0xA6 => (format!("AI_INJECT {}", reg(ram(a + 1))), 2),

            // ── Phase 102: Permissions and Capability System ──

            // SPAWNC addr_reg, caps_reg (0xA7) -- spawn with capabilities
            0xA7 => (
                format!("SPAWNC {}, {}", reg(ram(a + 1)), reg(ram(a + 2))),
                3,
            ),

            // HERMES prompt_addr_reg, response_addr_reg, max_len_reg (0xA8)
            // Send prompt to Hermes Agent CLI, write response to RAM.
            0xA8 => (
                format!(
                    "HERMES {}, {}, {}",
                    reg(ram(a + 1)),
                    reg(ram(a + 2)),
                    reg(ram(a + 3))
                ),
                4,
            ),

            // PTYOPEN cmd_addr_reg, handle_reg (0xA9) -- spawn host shell in pty
            0xA9 => (
                format!("PTYOPEN {}, {}", reg(ram(a + 1)), reg(ram(a + 2))),
                3,
            ),
            // PTYWRITE handle_reg, buf_reg, len_reg (0xAA) -- write bytes to pty
            0xAA => (
                format!(
                    "PTYWRITE {}, {}, {}",
                    reg(ram(a + 1)),
                    reg(ram(a + 2)),
                    reg(ram(a + 3))
                ),
                4,
            ),
            // PTYREAD handle_reg, buf_reg, max_len_reg (0xAB) -- drain pty output
            0xAB => (
                format!(
                    "PTYREAD {}, {}, {}",
                    reg(ram(a + 1)),
                    reg(ram(a + 2)),
                    reg(ram(a + 3))
                ),
                4,
            ),
            // PTYCLOSE handle_reg (0xAC) -- close pty slot
            0xAC => (format!("PTYCLOSE {}", reg(ram(a + 1))), 2),
            // PTYSIZE handle_reg, rows_reg, cols_reg (0xAD) -- resize pty
            0xAD => (
                format!(
                    "PTYSIZE {}, {}, {}",
                    reg(ram(a + 1)),
                    reg(ram(a + 2)),
                    reg(ram(a + 3))
                ),
                4,
            ),

            // ── Phase 88: AI Vision Bridge ──

            // AI_AGENT op_reg (0xB0) -- AI vision operations
            0xB0 => (format!("AI_AGENT {}", reg(ram(a + 1))), 2),

            // LOADPNG path_reg, dest_reg (0xB1) -- Load pixelpack PNG to RAM
            0xB1 => (
                format!("LOADPNG {}, {}", reg(ram(a + 1)), reg(ram(a + 2))),
                3,
            ),

            // LOADSRCIMG path_reg (0xB2) -- Load pixelpack source PNG to canvas + assemble
            0xB2 => (format!("LOADSRCIMG {}", reg(ram(a + 1))), 2),

            // ASM_RAM src_addr_reg (0xB3) -- Assemble source string from RAM
            0xB3 => (format!("ASM_RAM {}", reg(ram(a + 1))), 2),

            // ── Phase 123: Alpine Linux Live Tile Opcodes ──
            // VM_LIVE_SPAWN config_reg, window_reg (0xB4)
            0xB4 => (
                format!("VM_LIVE_SPAWN {}, {}", reg(ram(a + 1)), reg(ram(a + 2))),
                3,
            ),

            // VM_LIVE_STEP (0xB5) -- Advance live RISC-V VM by one time slice
            0xB5 => ("VM_LIVE_STEP".into(), 1),

            // VM_LIVE_KILL (0xB6) -- Kill the live RISC-V hypervisor
            0xB6 => ("VM_LIVE_KILL".into(), 1),
            // UNLINK (0xB7) -- delete file from VFS
            0xB7 => ("UNLINK".into(), 2),
            // FCOPY (0xB8) -- copy file within VFS
            0xB8 => ("FCOPY".into(), 3),
            // Phase 137: Host Filesystem Bridge
            0xB9 => ("FSOPEN".into(), 3),
            0xBA => ("FSCLOSE".into(), 2),
            0xBB => ("FSREAD".into(), 4),
            0xBC => ("FSWRITE".into(), 4),
            0xBD => ("FSLS".into(), 4),
            0xBE => ("NPROC".into(), 1),
            0xBF => ("PROCINFO".into(), 3),

            // SMALLTEXT x, y, addr, fg, bg (0xD0) -- tiny 3x5 font, 85 cols in 256px
            0xD0 => (
                format!(
                    "SMALLTEXT {}, {}, {}, {}, {}",
                    reg(ram(a + 1)),
                    reg(ram(a + 2)),
                    reg(ram(a + 3)),
                    reg(ram(a + 4)),
                    reg(ram(a + 5))
                ),
                6,
            ),

            // MEDTEXT x, y, addr, fg, bg (0xD1) -- medium 5x7 font, 42 cols in 256px
            0xD1 => (
                format!(
                    "MEDTEXT {}, {}, {}, {}, {}",
                    reg(ram(a + 1)),
                    reg(ram(a + 2)),
                    reg(ram(a + 3)),
                    reg(ram(a + 4)),
                    reg(ram(a + 5))
                ),
                6,
            ),

            _ => (format!("??? (0x{:02X})", op), 1),
        }
    }
}

/// Probe every opcode byte against a zeroed scratch VM and return the set of
/// opcodes that disassemble to a real mnemonic (everything except `???`).
/// The returned pair is `(opcode_byte, mnemonic)` where mnemonic is the first
/// whitespace-separated token from the disassembler — the bare instruction name
/// with no operands. Used to inject a truthful opcode inventory into LLM
/// system prompts so generated assembly can't invoke opcodes that don't exist.
pub fn valid_opcode_mnemonics() -> Vec<(u8, String)> {
    let mut probe = Vm::new();
    let mut out = Vec::with_capacity(160);
    for op in 0u32..=0xFFu32 {
        for cell in probe.ram.iter_mut().take(16) {
            *cell = 0;
        }
        probe.ram[0] = op;
        let (text, _) = probe.disassemble_at(0);
        if text.starts_with("???") {
            continue;
        }
        let name = text.split_whitespace().next().unwrap_or("").to_string();
        if !name.is_empty() {
            out.push((op as u8, name));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::Vm;

    /// Helper: create a VM and load a single instruction at addr.
    /// `words` are the opcode and its operands in order.
    pub(crate) fn load_instruction(words: &[u32], addr: usize) -> Vm {
        let mut vm = Vm::new();
        for (i, &w) in words.iter().enumerate() {
            vm.ram[addr + i] = w;
        }
        vm
    }

    #[test]
    fn test_halt_disasm() {
        let vm = load_instruction(&[0x00], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "HALT");
        assert_eq!(len, 1);
    }

    #[test]
    fn test_nop_disasm() {
        let vm = load_instruction(&[0x01], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "NOP");
        assert_eq!(len, 1);
    }

    #[test]
    fn test_frame_disasm() {
        let vm = load_instruction(&[0x02], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "FRAME");
        assert_eq!(len, 1);
    }

    #[test]
    fn test_beep_disasm() {
        let vm = load_instruction(&[0x03, 3, 5], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "BEEP r3, r5");
        assert_eq!(len, 3);
    }

    #[test]
    fn test_memcpy_disasm() {
        let vm = load_instruction(&[0x04, 1, 2, 3], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "MEMCPY r1, r2, r3");
        assert_eq!(len, 4);
    }

    #[test]
    fn test_ldi_disasm() {
        let vm = load_instruction(&[0x10, 1, 0xFF], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "LDI r1, 0xFF");
        assert_eq!(len, 3);
    }

    #[test]
    fn test_ldi_large_immediate() {
        let vm = load_instruction(&[0x10, 5, 0x12345], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "LDI r5, 0x12345");
        assert_eq!(len, 3);
    }

    #[test]
    fn test_load_disasm() {
        let vm = load_instruction(&[0x11, 1, 2], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "LOAD r1, [r2]");
        assert_eq!(len, 3);
    }

    #[test]
    fn test_store_disasm() {
        let vm = load_instruction(&[0x12, 1, 2], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "STORE [r1], r2");
        assert_eq!(len, 3);
    }

    #[test]
    fn test_texti_disasm() {
        let vm = load_instruction(
            &[
                0x13,
                10,
                20,
                5,
                b'H' as u32,
                b'i' as u32,
                b'!' as u32,
                b'\0' as u32,
                0,
            ],
            0,
        );
        let (s, len) = vm.disassemble_at(0);
        assert!(s.starts_with("TEXTI 10, 20, \"Hi!"));
        assert_eq!(len, 9); // 4 header + 5 chars (count word at ram[3]=5)
    }

    #[test]
    fn test_stro_disasm() {
        let vm = load_instruction(&[0x14, 3, 2, b'A' as u32, b'B' as u32], 0);
        let (s, len) = vm.disassemble_at(0);
        assert!(s.starts_with("STRO r3, \"AB"));
        assert_eq!(len, 5); // 3 header + 2 chars
    }

    #[test]
    fn test_cmpi_disasm() {
        let vm = load_instruction(&[0x15, 1, 42], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "CMPI r1, 42");
        assert_eq!(len, 3);
    }

    #[test]
    fn test_loads_disasm() {
        let vm = load_instruction(&[0x16, 1, 0xFFFFFFFE], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "LOADS r1, -2");
        assert_eq!(len, 3);
    }

    #[test]
    fn test_stores_disasm() {
        let vm = load_instruction(&[0x17, 5, 2], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "STORES 5, r2");
        assert_eq!(len, 3);
    }

    // -- ALU register-register ops (0x20-0x2B) --
    #[test]
    fn test_alu_binary_ops() {
        let ops = [
            (0x20, "ADD"),
            (0x21, "SUB"),
            (0x22, "MUL"),
            (0x23, "DIV"),
            (0x24, "AND"),
            (0x25, "OR"),
            (0x26, "XOR"),
            (0x27, "SHL"),
            (0x28, "SHR"),
            (0x29, "MOD"),
            (0x2B, "SAR"),
        ];
        for (opcode, name) in ops {
            let vm = load_instruction(&[opcode, 3, 7], 0);
            let (s, len) = vm.disassemble_at(0);
            assert_eq!(s, format!("{} r3, r7", name), "opcode 0x{:02X}", opcode);
            assert_eq!(len, 3);
        }
    }

    #[test]
    fn test_neg_disasm() {
        let vm = load_instruction(&[0x2A, 4], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "NEG r4");
        assert_eq!(len, 2);
    }

    // -- Immediate shift/logic ops (0x18-0x1F) --
    #[test]
    fn test_immediate_ops() {
        let ops = [
            (0x18, "SHLI"),
            (0x19, "SHRI"),
            (0x1A, "SARI"),
            (0x1B, "ADDI"),
            (0x1C, "SUBI"),
            (0x1D, "ANDI"),
            (0x1E, "ORI"),
            (0x1F, "XORI"),
        ];
        for (opcode, name) in ops {
            let vm = load_instruction(&[opcode, 2, 15], 0);
            let (s, len) = vm.disassemble_at(0);
            assert_eq!(s, format!("{} r2, 15", name), "opcode 0x{:02X}", opcode);
            assert_eq!(len, 3);
        }
    }

    // -- Jump ops (0x30-0x36) --
    #[test]
    fn test_jmp_disasm() {
        let vm = load_instruction(&[0x30, 0x1000], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "JMP 0x1000");
        assert_eq!(len, 2);
    }

    #[test]
    fn test_jz_disasm() {
        let vm = load_instruction(&[0x31, 1, 0x200], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "JZ r1, 0x0200");
        assert_eq!(len, 3);
    }

    #[test]
    fn test_jnz_disasm() {
        let vm = load_instruction(&[0x32, 5, 0x50], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "JNZ r5, 0x0050");
        assert_eq!(len, 3);
    }

    #[test]
    fn test_call_disasm() {
        let vm = load_instruction(&[0x33, 0x100], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "CALL 0x0100");
        assert_eq!(len, 2);
    }

    #[test]
    fn test_ret_disasm() {
        let vm = load_instruction(&[0x34], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "RET");
        assert_eq!(len, 1);
    }

    #[test]
    fn test_blt_bge_disasm() {
        let vm = load_instruction(&[0x35, 1, 0x100], 0);
        let (s, _) = vm.disassemble_at(0);
        assert_eq!(s, "BLT r1, 0x0100");

        let vm2 = load_instruction(&[0x36, 2, 0x200], 0);
        let (s, _) = vm2.disassemble_at(0);
        assert_eq!(s, "BGE r2, 0x0200");
    }

    // -- Graphics ops (0x40-0x4A) --
    #[test]
    fn test_pset_disasm() {
        let vm = load_instruction(&[0x40, 10, 20, 0xFF0000], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "PSET r10, r20, r16711680");
        assert_eq!(len, 4);
    }

    #[test]
    fn test_pseti_disasm() {
        let vm = load_instruction(&[0x41, 50, 100, 0x00FF00], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "PSETI 50, 100, 0xFF00");
        assert_eq!(len, 4);
    }

    #[test]
    fn test_fill_disasm() {
        let vm = load_instruction(&[0x42, 3], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "FILL r3");
        assert_eq!(len, 2);
    }

    #[test]
    fn test_rectf_disasm() {
        let vm = load_instruction(&[0x43, 1, 2, 3, 4, 5], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "RECTF r1,r2,r3,r4,r5");
        assert_eq!(len, 6);
    }

    #[test]
    fn test_text_disasm() {
        let vm = load_instruction(&[0x44, 10, 20, 3], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "TEXT r10,r20,[r3]");
        assert_eq!(len, 4);
    }

    #[test]
    fn test_line_disasm() {
        let vm = load_instruction(&[0x45, 1, 2, 3, 4, 5], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "LINE r1,r2,r3,r4,r5");
        assert_eq!(len, 6);
    }

    #[test]
    fn test_circle_disasm() {
        let vm = load_instruction(&[0x46, 64, 64, 50, 0xFF0000], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "CIRCLE r64,r64,r50,r16711680");
        assert_eq!(len, 5);
    }

    #[test]
    fn test_scroll_disasm() {
        let vm = load_instruction(&[0x47, 10], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "SCROLL r10");
        assert_eq!(len, 2);
    }

    #[test]
    fn test_ikey_disasm() {
        let vm = load_instruction(&[0x48, 0], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "IKEY r0");
        assert_eq!(len, 2);
    }

    #[test]
    fn test_rand_disasm() {
        let vm = load_instruction(&[0x49, 7], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "RAND r7");
        assert_eq!(len, 2);
    }

    #[test]
    fn test_sprite_disasm() {
        let vm = load_instruction(&[0x4A, 0, 0, 0x1000, 16, 16], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "SPRITE r0, r0, r4096, r16, r16");
        assert_eq!(len, 6);
    }

    #[test]
    fn test_asm_disasm() {
        let vm = load_instruction(&[0x4B, 1, 2], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "ASM r1, r2");
        assert_eq!(len, 3);
    }

    #[test]
    fn test_tilemap_disasm() {
        let vm = load_instruction(&[0x4C, 0, 0, 1, 2, 8, 8, 16, 16], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "TILEMAP r0, r0, r1, r2, r8, r8, r16, r16");
        assert_eq!(len, 9);
    }

    // -- Process ops (0x4D-0x4F) --
    #[test]
    fn test_spawn_disasm() {
        let vm = load_instruction(&[0x4D, 3], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "SPAWN r3");
        assert_eq!(len, 2);
    }

    #[test]
    fn test_kill_disasm() {
        let vm = load_instruction(&[0x4E, 1], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "KILL r1");
        assert_eq!(len, 2);
    }

    #[test]
    fn test_peek_disasm() {
        let vm = load_instruction(&[0x4F, 10, 20, 5], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "PEEK r10, r20, r5");
        assert_eq!(len, 4);
    }

    // -- Compare/MOV (0x50-0x51) --
    #[test]
    fn test_cmp_disasm() {
        let vm = load_instruction(&[0x50, 1, 2], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "CMP r1, r2");
        assert_eq!(len, 3);
    }

    #[test]
    fn test_mov_disasm() {
        let vm = load_instruction(&[0x51, 3, 7], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "MOV r3, r7");
        assert_eq!(len, 3);
    }

    // -- Stack ops (0x60-0x61) --
    #[test]
    fn test_push_pop_disasm() {
        let vm = load_instruction(&[0x60, 5], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "PUSH r5");
        assert_eq!(len, 2);

        let vm2 = load_instruction(&[0x61, 5], 0);
        let (s, len) = vm2.disassemble_at(0);
        assert_eq!(s, "POP r5");
        assert_eq!(len, 2);
    }

    // -- Syscall/File ops (0x52-0x59) --
    #[test]
    fn test_syscall_disasm() {
        let vm = load_instruction(&[0x52, 1], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "SYSCALL 1");
        assert_eq!(len, 2);
    }

    #[test]
    fn test_retk_disasm() {
        let vm = load_instruction(&[0x53], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "RETK");
        assert_eq!(len, 1);
    }

    #[test]
    fn test_open_disasm() {
        let vm = load_instruction(&[0x54, 1, 2], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "OPEN r1, r2");
        assert_eq!(len, 3);
    }

    #[test]
    fn test_read_disasm() {
        let vm = load_instruction(&[0x55, 1, 2, 3], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "READ r1, r2, r3");
        assert_eq!(len, 4);
    }

    #[test]
    fn test_write_disasm() {
        let vm = load_instruction(&[0x56, 1, 2, 3], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "WRITE r1, r2, r3");
        assert_eq!(len, 4);
    }

    #[test]
    fn test_close_disasm() {
        let vm = load_instruction(&[0x57, 3], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "CLOSE r3");
        assert_eq!(len, 2);
    }

    #[test]
    fn test_seek_disasm() {
        let vm = load_instruction(&[0x58, 1, 2, 3], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "SEEK r1, r2, r3");
        assert_eq!(len, 4);
    }

    #[test]
    fn test_ls_disasm() {
        let vm = load_instruction(&[0x59, 5], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "LS r5");
        assert_eq!(len, 2);
    }

    // -- Scheduler ops (0x5A-0x5F) --
    #[test]
    fn test_yield_disasm() {
        let vm = load_instruction(&[0x5A], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "YIELD");
        assert_eq!(len, 1);
    }

    #[test]
    fn test_sleep_disasm() {
        let vm = load_instruction(&[0x5B, 10], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "SLEEP r10");
        assert_eq!(len, 2);
    }

    #[test]
    fn test_setpriority_disasm() {
        let vm = load_instruction(&[0x5C, 5], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "SETPRIORITY r5");
        assert_eq!(len, 2);
    }

    #[test]
    fn test_pipe_disasm() {
        let vm = load_instruction(&[0x5D, 1, 2], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "PIPE r1, r2");
        assert_eq!(len, 3);
    }

    #[test]
    fn test_msgsnd_msgrcv_disasm() {
        let vm = load_instruction(&[0x5E, 3], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "MSGSND r3");
        assert_eq!(len, 2);

        let vm2 = load_instruction(&[0x5F], 0);
        let (s, len) = vm2.disassemble_at(0);
        assert_eq!(s, "MSGRCV");
        assert_eq!(len, 1);
    }

    // -- IO/Env ops (0x62-0x6F) --
    #[test]
    fn test_ioctl_disasm() {
        let vm = load_instruction(&[0x62, 1, 2, 3], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "IOCTL r1, r2, r3");
        assert_eq!(len, 4);
    }

    #[test]
    fn test_getenv_setenv_disasm() {
        let vm = load_instruction(&[0x63, 1, 2], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "GETENV r1, r2");
        assert_eq!(len, 3);

        let vm2 = load_instruction(&[0x64, 3, 4], 0);
        let (s, len) = vm2.disassemble_at(0);
        assert_eq!(s, "SETENV r3, r4");
        assert_eq!(len, 3);
    }

    #[test]
    fn test_getpid_disasm() {
        let vm = load_instruction(&[0x65], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "GETPID");
        assert_eq!(len, 1);
    }

    #[test]
    fn test_exec_disasm() {
        let vm = load_instruction(&[0x66, 5], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "EXEC r5");
        assert_eq!(len, 2);
    }

    #[test]
    fn test_writestr_readln_disasm() {
        let vm = load_instruction(&[0x67, 1, 2], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "WRITESTR r1, r2");
        assert_eq!(len, 3);

        let vm2 = load_instruction(&[0x68, 1, 2, 3], 0);
        let (s, len) = vm2.disassemble_at(0);
        assert_eq!(s, "READLN r1, r2, r3");
        assert_eq!(len, 4);
    }

    #[test]
    fn test_waitpid_disasm() {
        let vm = load_instruction(&[0x69, 1], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "WAITPID r1");
        assert_eq!(len, 2);
    }

    #[test]
    fn test_execp_disasm() {
        let vm = load_instruction(&[0x6A, 1, 2, 3], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "EXECP r1, r2, r3");
        assert_eq!(len, 4);
    }

    #[test]
    fn test_chdir_getcwd_disasm() {
        let vm = load_instruction(&[0x6B, 5], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "CHDIR r5");
        assert_eq!(len, 2);

        let vm2 = load_instruction(&[0x6C, 3], 0);
        let (s, len) = vm2.disassemble_at(0);
        assert_eq!(s, "GETCWD r3");
        assert_eq!(len, 2);
    }

    #[test]
    fn test_screenp_disasm() {
        let vm = load_instruction(&[0x6D, 1, 2, 3], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "SCREENP r1, r2, r3");
        assert_eq!(len, 4);
    }

    #[test]
    fn test_shutdown_exit_disasm() {
        let vm = load_instruction(&[0x6E], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "SHUTDOWN");
        assert_eq!(len, 1);

        let vm2 = load_instruction(&[0x6F, 0], 0);
        let (s, len) = vm2.disassemble_at(0);
        assert_eq!(s, "EXIT r0");
        assert_eq!(len, 2);
    }

    // -- Signal ops (0x70-0x71) --
    #[test]
    fn test_signal_ops() {
        let vm = load_instruction(&[0x70, 1, 2], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "SIGNAL r1, r2");
        assert_eq!(len, 3);

        let vm2 = load_instruction(&[0x71, 3, 4], 0);
        let (s, len) = vm2.disassemble_at(0);
        assert_eq!(s, "SIGSET r3, r4");
        assert_eq!(len, 3);
    }

    // -- Hypervisor (0x72) --
    #[test]
    fn test_hypervisor_disasm() {
        let vm = load_instruction(&[0x72, 5, 0], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "HYPERVISOR r5");
        assert_eq!(len, 3);
    }

    #[test]
    fn test_hypervisor_disasm_with_window() {
        let vm = load_instruction(&[0x72, 5, 3], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "HYPERVISOR r5, r3");
        assert_eq!(len, 3);
    }

    // -- Assembly/Self ops (0x73-0x74) --
    #[test]
    fn test_asmself_runnext() {
        let vm = load_instruction(&[0x73], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "ASMSELF");
        assert_eq!(len, 1);

        let vm2 = load_instruction(&[0x74], 0);
        let (s, len) = vm2.disassemble_at(0);
        assert_eq!(s, "RUNNEXT");
        assert_eq!(len, 1);
    }

    // -- Formula ops (0x75-0x77) --
    #[test]
    fn test_formula_disasm() {
        let vm = load_instruction(&[0x75, 100, 2, 3, 0, 0], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "FORMULA 100, MUL, 3");
        assert_eq!(len, 7); // 4 header + 3 deps
    }

    #[test]
    fn test_formula_all_ops() {
        let op_names = [
            "ADD", "SUB", "MUL", "DIV", "AND", "OR", "XOR", "NOT", "COPY", "MAX", "MIN", "MOD",
            "SHL", "SHR",
        ];
        for (i, expected) in op_names.iter().enumerate() {
            let vm = load_instruction(&[0x75, 0, i as u32, 1, 0], 0);
            let (s, len) = vm.disassemble_at(0);
            assert_eq!(s, format!("FORMULA 0, {}, 1", expected), "formula op {}", i);
            assert_eq!(len, 5); // 4 + 1 dep
        }
    }

    #[test]
    fn test_formula_unknown_op() {
        let vm = load_instruction(&[0x75, 0, 99, 1, 0], 0);
        let (s, _) = vm.disassemble_at(0);
        assert_eq!(s, "FORMULA 0, ???, 1");
    }

    #[test]
    fn test_formula_max_deps_clamped() {
        let vm = load_instruction(&[0x75, 0, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0], 0);
        let (_s, len) = vm.disassemble_at(0);
        // MAX_FORMULA_DEPS is 8, so dep count should show 20 (the raw value)
        // but length should be clamped to 4 + 8 = 12
        assert_eq!(len, 12);
    }

    #[test]
    fn test_formulaclear_disasm() {
        let vm = load_instruction(&[0x76], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "FORMULACLEAR");
        assert_eq!(len, 1);
    }

    #[test]
    fn test_formularem_disasm() {
        let vm = load_instruction(&[0x77, 42], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "FORMULAREM 42");
        assert_eq!(len, 2);
    }

    // -- VFS ops (0x78-0x7A) --
    #[test]
    fn test_vfs_ops() {
        let vm = load_instruction(&[0x78, 5], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "FMKDIR [r5]");
        assert_eq!(len, 2);

        let vm2 = load_instruction(&[0x79, 1, 2], 0);
        let (s, len) = vm2.disassemble_at(0);
        assert_eq!(s, "FSTAT r1, [r2]");
        assert_eq!(len, 3);

        let vm3 = load_instruction(&[0x7A, 3], 0);
        let (s, len) = vm3.disassemble_at(0);
        assert_eq!(s, "FUNLINK [r3]");
        assert_eq!(len, 2);
    }

    // -- Trace/Debug ops (0x7B-0x7D) --
    #[test]
    fn test_snap_trace_replay_fork() {
        let vm = load_instruction(&[0x7B, 1], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "SNAP_TRACE r1");
        assert_eq!(len, 2);

        let vm2 = load_instruction(&[0x7C, 2], 0);
        let (s, len) = vm2.disassemble_at(0);
        assert_eq!(s, "REPLAY r2");
        assert_eq!(len, 2);

        let vm3 = load_instruction(&[0x7D, 0], 0);
        let (s, len) = vm3.disassemble_at(0);
        assert_eq!(s, "FORK r0");
        assert_eq!(len, 2);
    }

    // -- NOTE (0x7E) --
    #[test]
    fn test_note_disasm() {
        let vm = load_instruction(&[0x7E, 60, 440, 2], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "NOTE r60, r440, r2");
        assert_eq!(len, 4);
    }

    // -- Network ops (0x7F-0x82) --
    #[test]
    fn test_connect_disasm() {
        let vm = load_instruction(&[0x7F, 1, 2, 3], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "CONNECT r1, r2, r3");
        assert_eq!(len, 4);
    }

    #[test]
    fn test_socksend_disasm() {
        let vm = load_instruction(&[0x80, 1, 2, 3, 4], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "SOCKSEND r1, r2, r3, r4");
        assert_eq!(len, 5);
    }

    #[test]
    fn test_sockrecv_disasm() {
        let vm = load_instruction(&[0x81, 1, 2, 3, 4], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "SOCKRECV r1, r2, r3, r4");
        assert_eq!(len, 5);
    }

    #[test]
    fn test_disconnect_disasm() {
        let vm = load_instruction(&[0x82, 0], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "DISCONNECT r0");
        assert_eq!(len, 2);
    }

    // -- Provenance ops (0x83-0x84) --
    #[test]
    fn test_trace_read_disasm() {
        let vm = load_instruction(&[0x83, 5], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "TRACE_READ r5");
        assert_eq!(len, 2);
    }

    #[test]
    fn test_pixel_history_disasm() {
        let vm = load_instruction(&[0x84, 10], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "PIXEL_HISTORY r10");
        assert_eq!(len, 2);
    }

    // -- Edge cases --
    #[test]
    fn test_unknown_opcode() {
        let vm = load_instruction(&[0xFE], 0);
        let (s, len) = vm.disassemble_at(0);
        assert_eq!(s, "??? (0xFE)");
        assert_eq!(len, 1);
    }

    #[test]
    fn test_out_of_bounds_addr() {
        let vm = Vm::new();
        let (s, len) = vm.disassemble_at(999_999);
        assert_eq!(s, "???");
        assert_eq!(len, 1);
    }

    #[test]
    fn test_disasm_does_not_mutate() {
        let mut vm = load_instruction(&[0x10, 1, 42], 0);
        vm.regs[1] = 100; // set a register to check it's not modified
        let _ = vm.disassemble_at(0);
        assert_eq!(vm.regs[1], 100);
        assert_eq!(vm.pc, 0);
        assert_eq!(vm.halted, false);
    }

    #[test]
    fn test_disasm_at_nonzero_addr() {
        let vm = load_instruction(&[0, 0, 0x10, 5, 0xABCD], 0);
        let (s, len) = vm.disassemble_at(2);
        assert_eq!(s, "LDI r5, 0xABCD");
        assert_eq!(len, 3);
    }

    #[test]
    fn test_texti_truncation_at_32_chars() {
        let mut words = vec![0x13, 0, 0, 60]; // 60 chars, but should truncate at 32
        for i in 0..60 {
            words.push((b'A' + (i % 26)) as u32);
        }
        let vm = load_instruction(&words, 0);
        let (s, len) = vm.disassemble_at(0);
        // The string part should be truncated to 32 chars
        let after_quote: String = s.split('"').nth(1).unwrap().to_string();
        assert_eq!(after_quote.len(), 32, "TEXTI should truncate to 32 chars");
        // Length should be 4 + actual count (60), not truncated
        assert_eq!(len, 64); // 4 header + 60 chars
    }

    #[test]
    fn test_stro_truncation_at_32_chars() {
        let mut words = vec![0x14, 3, 50]; // 50 chars, should truncate at 32
        for i in 0..50 {
            words.push((b'0' + (i % 10)) as u32);
        }
        let vm = load_instruction(&words, 0);
        let (s, len) = vm.disassemble_at(0);
        let after_quote: String = s.split('"').nth(1).unwrap().to_string();
        assert_eq!(after_quote.len(), 32, "STRO should truncate to 32 chars");
        assert_eq!(len, 53); // 3 header + 50 chars
    }

    #[test]
    fn test_valid_opcode_mnemonics_covers_core_ops() {
        let pairs = super::valid_opcode_mnemonics();
        let lookup: std::collections::HashMap<u8, String> = pairs.into_iter().collect();
        // Sanity: core opcodes listed by project memory must be present with the right mnemonic.
        assert_eq!(lookup.get(&0x00).map(String::as_str), Some("HALT"));
        assert_eq!(lookup.get(&0x01).map(String::as_str), Some("NOP"));
        assert_eq!(lookup.get(&0x02).map(String::as_str), Some("FRAME"));
        assert_eq!(lookup.get(&0x10).map(String::as_str), Some("LDI"));
        // Total opcode count should match the number of non-??? match arms in disasm.
        // As of phase 108 that was ~150; guard against regression by requiring >= 100.
        assert!(
            lookup.len() >= 100,
            "valid_opcode_mnemonics returned only {} entries",
            lookup.len()
        );
    }

    #[test]
    fn test_valid_opcode_mnemonics_excludes_unknown() {
        let pairs = super::valid_opcode_mnemonics();
        // Verify no mnemonic contains "???" — the probe filters those out.
        for (op, name) in &pairs {
            assert!(
                !name.contains("???"),
                "opcode 0x{:02X} leaked ??? into mnemonic: {}",
                op,
                name
            );
        }
    }
}
