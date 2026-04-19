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
                (format!("HYPERVISOR {}", reg(ar)), 2)
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

            _ => (format!("??? (0x{:02X})", op), 1),
        }
    }
}
