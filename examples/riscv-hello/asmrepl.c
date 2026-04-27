/*
 * asmrepl.c -- Bare-metal RISC-V assembly REPL for Geometry OS.
 *
 * Type RISC-V assembly mnemonics (one per line), see them decoded,
 * executed, and register state displayed. Like a simplified r2/gdb
 * session running on bare metal.
 *
 * Supported: LUI, ADDI, ADD, SUB, AND, OR, XOR, SLL, SRL, SRA,
 *            SLT, SLTU, LI (pseudo), MV (pseudo), NOP,
 *            LB, LH, LW, LBU, LHU, SB, SH, SW,
 *            JAL, JALR, BEQ, BNE, BLT, BGE, BLTU, BGEU,
 *            CSRR, CSRW, EBREAK, ECALL, WFI,
 *            REGS (dump all), MEM <addr> (peek), HELP, QUIT
 *
 * Build: cd examples/riscv-hello && ./build.sh asmrepl.c
 * Run:   cargo run --release --example sh_run -- examples/riscv-hello/asmrepl.elf
 */

static inline int sbi_putchar(int ch) {
    register int a0 asm("a0") = ch;
    register int a7 asm("a7") = 1;
    asm volatile("ecall" : "+r"(a0) : "r"(a7) : "memory", "a1");
    return a0;
}

static inline int sbi_getchar(void) {
    register int a0 asm("a0");
    register int a7 asm("a7") = 2;
    asm volatile("ecall" : "=r"(a0) : "r"(a7) : "memory", "a1");
    return a0;
}

static inline void sbi_shutdown(void) {
    register int a7 asm("a7") = 8;
    asm volatile("ecall" : : "r"(a7) : "memory", "a0", "a1");
    __builtin_unreachable();
}

static void puts(const char *s) { while (*s) sbi_putchar(*s++); }
static void putchar(char c) { sbi_putchar(c); }

static int getchar_wait(void) {
    for (volatile int i = 0; i < 1000; i++) {
        int c = sbi_getchar();
        if (c >= 0 && c != 0xFF) return c;
        /* Try UART MMIO */
        volatile unsigned char *lsr = (volatile unsigned char *)0x10000005;
        if (*lsr & 0x01) {
            return *(volatile unsigned char *)0x10000000;
        }
    }
    return -1;
}

static void print_hex(unsigned int v) {
    const char hex[] = "0123456789abcdef";
    puts("0x");
    for (int i = 28; i >= 0; i -= 4)
        putchar(hex[(v >> i) & 0xF]);
}

static void print_dec(int v) {
    if (v < 0) { putchar('-'); v = -v; }
    if (v == 0) { putchar('0'); return; }
    char buf[12]; int i = 0;
    while (v > 0) { buf[i++] = '0' + (v % 10); v /= 10; }
    while (--i >= 0) putchar(buf[i]);
}

static int streq(const char *a, const char *b) {
    while (*a && *b) { if (*a != *b) return 0; a++; b++; }
    return *a == 0 && *b == 0;
}

/* Register names */
static const char *reg_name(int r) {
    static const char *names[] = {
        "zero","ra","sp","gp","tp","t0","t1","t2","s0","s1",
        "a0","a1","a2","a3","a4","a5","a6","a7","s2","s3",
        "s4","s5","s6","s7","s8","s9","s10","s11","t3","t4",
        "t5","t6"
    };
    if (r < 0 || r > 31) return "??";
    return names[r];
}

/* Parse register name -> index. Returns -1 on failure. */
static int parse_reg(const char *s) {
    /* x0-x31 */
    if (s[0] == 'x') {
        int n = 0;
        const char *p = s + 1;
        if (!*p) return -1;
        while (*p) {
            if (*p < '0' || *p > '9') return -1;
            n = n * 10 + (*p - '0');
            p++;
        }
        if (n > 31) return -1;
        return n;
    }
    /* ABI names */
    const char *names[] = {
        "zero","ra","sp","gp","tp","t0","t1","t2","s0","s1",
        "a0","a1","a2","a3","a4","a5","a6","a7","s2","s3",
        "s4","s5","s6","s7","s8","s9","s10","s11","t3","t4",
        "t5","t6"
    };
    /* Also accept fp = s0 */
    if (streq(s, "fp")) return 8;
    for (int i = 0; i < 32; i++) {
        if (streq(s, names[i])) return i;
    }
    return -1;
}

/* Parse immediate (decimal or hex with 0x prefix) */
static int parse_imm(const char *s, int *out) {
    *out = 0;
    int neg = 0;
    if (*s == '-') { neg = 1; s++; }
    if (s[0] == '0' && (s[1] == 'x' || s[1] == 'X')) {
        s += 2;
        if (!*s) return 0;
        unsigned int v = 0;
        while (*s) {
            char c = *s++;
            v <<= 4;
            if (c >= '0' && c <= '9') v |= c - '0';
            else if (c >= 'a' && c <= 'f') v |= 10 + c - 'a';
            else if (c >= 'A' && c <= 'F') v |= 10 + c - 'A';
            else return 0;
        }
        *out = neg ? -(int)v : (int)v;
        return 1;
    }
    if (!*s) return 0;
    unsigned int v = 0;
    while (*s) {
        char c = *s++;
        if (c < '0' || c > '9') return 0;
        v = v * 10 + (c - '0');
    }
    *out = neg ? -(int)v : (int)v;
    return 1;
}

/* ---- Register file (shadow copy) ---- */
static unsigned int regs[32];

/* Execute a single instruction against the register file.
 * Returns 0 on success, -1 on error.
 * This is a simplified software interpreter, not using the CPU. */
static int exec_r_type(int rd, int rs1, int rs2, int funct3, int funct7) {
    unsigned int a = regs[rs1];
    unsigned int b = regs[rs2];
    unsigned int result = 0;

    if (funct7 == 0x01) {
        /* M extension (simplified) or shift variants */
        if (funct3 == 0 && funct7 == 0x01) { /* MUL */
            result = a * b;
        } else {
            return -1;
        }
    } else if (funct7 == 0) {
        switch (funct3) {
        case 0: result = a + b; break;  /* ADD */
        case 1: result = a << (b & 31); break; /* SLL */
        case 2: result = ((int)a < (int)b) ? 1 : 0; break; /* SLT */
        case 3: result = (a < b) ? 1 : 0; break; /* SLTU */
        case 4: result = a ^ b; break; /* XOR */
        case 5: result = a >> (b & 31); break; /* SRL */
        case 6: result = a | b; break; /* OR */
        case 7: result = a & b; break; /* AND */
        }
    } else if (funct7 == 0x20) {
        switch (funct3) {
        case 0: result = a - b; break; /* SUB */
        case 5: result = (unsigned int)((int)a >> (b & 31)); break; /* SRA */
        }
    } else {
        return -1;
    }
    if (rd != 0) regs[rd] = result;
    return 0;
}

static int exec_i_type(int rd, int rs1, int imm, int funct3) {
    unsigned int a = regs[rs1];
    unsigned int result = 0;
    volatile unsigned char *mem;

    switch (funct3) {
    case 0: result = a + (unsigned int)imm; break; /* ADDI */
    case 1: result = a << (imm & 31); break; /* SLLI */
    case 2: result = ((int)a < imm) ? 1 : 0; break; /* SLTI */
    case 3: result = (a < (unsigned int)imm) ? 1 : 0; break; /* SLTIU */
    case 4: result = a ^ (unsigned int)imm; break; /* XORI */
    case 5:
        if (imm & 0x400) result = (unsigned int)((int)a >> (imm & 31)); /* SRAI */
        else result = a >> (imm & 31); /* SRLI */
        break;
    case 6: result = a | (unsigned int)imm; break; /* ORI */
    case 7: result = a & (unsigned int)imm; break; /* ANDI */
    /* Load instructions */
    default: return -1;
    }
    if (rd != 0) regs[rd] = result;
    return 0;
}

/* ---- Input buffer ---- */
#define IBUF_SIZE 256
static char ibuf[IBUF_SIZE];
static int ibuf_len;

/* Tokenize into words */
#define MAX_TOKENS 8
static char tokens[MAX_TOKENS][32];
static int ntokens;

static void tokenize(void) {
    ibuf[ibuf_len] = 0;
    ntokens = 0;
    const char *p = ibuf;
    while (*p && ntokens < MAX_TOKENS) {
        while (*p == ' ' || *p == '\t' || *p == ',') p++;
        if (!*p) break;
        int i = 0;
        while (*p && *p != ' ' && *p != '\t' && *p != ',' && i < 31) {
            tokens[ntokens][i++] = *p++;
        }
        tokens[ntokens][i] = 0;
        /* Strip trailing comma */
        if (i > 0 && tokens[ntokens][i-1] == ',') tokens[ntokens][i-1] = 0;
        /* Skip empty tokens */
        if (tokens[ntokens][0] != 0) ntokens++;
    }
}

/* ---- Process one line ---- */

static void do_help(void) {
    puts(
        "RISC-V ASM REPL commands:\r\n"
        "  <opcode> <args>  - execute instruction, show result\r\n"
        "  regs             - dump all 32 registers\r\n"
        "  mem <addr>       - read 32-bit word from address\r\n"
        "  dump <addr> <n>  - hex dump n bytes\r\n"
        "  reset            - zero all registers\r\n"
        "  help             - this help\r\n"
        "  quit             - exit\r\n"
        "\r\n"
        "Opcodes: ADD SUB AND OR XOR SLL SRL SRA SLT SLTU\r\n"
        "         ADDI ANDI ORI XORI SLTI SLTIU SLLI SRLI SRAI\r\n"
        "         LUI LW SW LB SB LH SH\r\n"
        "         LI (pseudo) MV (pseudo) NOP\r\n"
        "         CSRR <csr> CSRW <csr> <val>\r\n"
        "Registers: x0-x31 or ABI names (zero ra sp gp tp t0-t6 s0-s11 a0-a7)\r\n"
    );
}

static void do_regs(void) {
    for (int i = 0; i < 32; i++) {
        puts("  ");
        const char *n = reg_name(i);
        puts(n);
        /* Pad to 4 chars */
        int len = 0; while (n[len]) len++;
        for (int p = len; p < 4; p++) putchar(' ');
        putchar(' ');
        print_hex(regs[i]);
        if ((i + 1) % 4 == 0) puts("\r\n");
    }
}

static void do_mem(void) {
    if (ntokens < 2) { puts("Usage: mem <addr>\r\n"); return; }
    int addr;
    if (!parse_imm(tokens[1], &addr)) { puts("Bad address\r\n"); return; }
    volatile unsigned int *p = (volatile unsigned int *)(unsigned long)addr;
    puts("[");
    print_hex((unsigned int)addr);
    puts("] = ");
    print_hex(*p);
    puts("\r\n");
}

static void do_dump(void) {
    if (ntokens < 2) { puts("Usage: dump <addr> [n]\r\n"); return; }
    int addr, count = 16;
    if (!parse_imm(tokens[1], &addr)) { puts("Bad address\r\n"); return; }
    if (ntokens >= 3 && !parse_imm(tokens[2], &count)) count = 16;
    if (count < 0) count = 16;
    if (count > 256) count = 256;

    volatile unsigned char *p = (volatile unsigned char *)(unsigned long)addr;
    const char hex[] = "0123456789abcdef";
    for (int row = 0; row < count; row += 16) {
        print_hex((unsigned int)(addr + row));
        puts(": ");
        for (int col = 0; col < 16; col++) {
            if (row + col < count) {
                unsigned char b = p[row + col];
                putchar(hex[(b >> 4) & 0xF]);
                putchar(hex[b & 0xF]);
            } else { putchar(' '); putchar(' '); }
            putchar(' ');
        }
        puts(" |");
        for (int col = 0; col < 16 && row + col < count; col++) {
            unsigned char b = p[row + col];
            putchar((b >= 32 && b < 127) ? b : '.');
        }
        puts("|\r\n");
    }
}

/* Parse and execute an instruction.
 * Returns 1 if executed, 0 if not recognized, -1 on error. */
static int exec_insn(void) {
    if (ntokens == 0) return 0;
    const char *op = tokens[0];

    /* Uppercase comparison for opcode */
    /* NOP */
    if (streq(op, "nop")) {
        /* NOP = ADDI x0, x0, 0 */
        return 1;
    }

    /* MV rd, rs1 */
    if (streq(op, "mv") || streq(op, "MV")) {
        if (ntokens < 3) { puts("Usage: mv rd, rs1\r\n"); return -1; }
        int rd = parse_reg(tokens[1]);
        int rs1 = parse_reg(tokens[2]);
        if (rd < 0 || rs1 < 0) { puts("Bad register\r\n"); return -1; }
        unsigned int before = regs[rd];
        if (rd != 0) regs[rd] = regs[rs1];
        if (rd != 0) {
            puts("  "); puts(reg_name(rd)); puts(" = ");
            print_hex(regs[rd]);
            puts(" (was "); print_hex(before); puts(")\r\n");
        }
        return 1;
    }

    /* LI rd, imm */
    if (streq(op, "li") || streq(op, "LI")) {
        if (ntokens < 3) { puts("Usage: li rd, imm\r\n"); return -1; }
        int rd = parse_reg(tokens[1]);
        int imm;
        if (rd < 0) { puts("Bad register\r\n"); return -1; }
        if (!parse_imm(tokens[2], &imm)) { puts("Bad immediate\r\n"); return -1; }
        unsigned int before = regs[rd];
        if (rd != 0) regs[rd] = (unsigned int)imm;
        if (rd != 0) {
            puts("  "); puts(reg_name(rd)); puts(" = ");
            print_hex(regs[rd]);
            puts(" (was "); print_hex(before); puts(")\r\n");
        }
        return 1;
    }

    /* R-type: ADD SUB AND OR XOR SLL SRL SRA SLT SLTU MUL */
    {
        int funct3 = -1, funct7 = 0;
        int is_r = 1;
        if (streq(op, "add") || streq(op, "ADD")) { funct3 = 0; funct7 = 0; }
        else if (streq(op, "sub") || streq(op, "SUB")) { funct3 = 0; funct7 = 0x20; }
        else if (streq(op, "sll") || streq(op, "SLL")) { funct3 = 1; funct7 = 0; }
        else if (streq(op, "slt") || streq(op, "SLT")) { funct3 = 2; funct7 = 0; }
        else if (streq(op, "sltu") || streq(op, "SLTU")) { funct3 = 3; funct7 = 0; }
        else if (streq(op, "xor") || streq(op, "XOR")) { funct3 = 4; funct7 = 0; }
        else if (streq(op, "srl") || streq(op, "SRL")) { funct3 = 5; funct7 = 0; }
        else if (streq(op, "sra") || streq(op, "SRA")) { funct3 = 5; funct7 = 0x20; }
        else if (streq(op, "or") || streq(op, "OR")) { funct3 = 6; funct7 = 0; }
        else if (streq(op, "and") || streq(op, "AND")) { funct3 = 7; funct7 = 0; }
        else if (streq(op, "mul") || streq(op, "MUL")) { funct3 = 0; funct7 = 1; }
        else { is_r = 0; }

        if (is_r) {
            if (ntokens < 4) { puts("Usage: op rd, rs1, rs2\r\n"); return -1; }
            int rd = parse_reg(tokens[1]);
            int rs1 = parse_reg(tokens[2]);
            int rs2 = parse_reg(tokens[3]);
            if (rd < 0 || rs1 < 0 || rs2 < 0) { puts("Bad register\r\n"); return -1; }
            unsigned int before = regs[rd];
            if (exec_r_type(rd, rs1, rs2, funct3, funct7) < 0) {
                puts("Execution error\r\n"); return -1;
            }
            if (rd != 0) {
                puts("  "); puts(reg_name(rd)); puts(" = ");
                print_hex(regs[rd]);
                puts(" (was "); print_hex(before); puts(")\r\n");
            } else {
                puts("  (x0 always 0)\r\n");
            }
            return 1;
        }
    }

    /* I-type: ADDI ANDI ORI XORI SLTI SLTIU SLLI SRLI SRAI */
    {
        int funct3 = -1;
        int is_i = 1;
        if (streq(op, "addi") || streq(op, "ADDI")) funct3 = 0;
        else if (streq(op, "slli") || streq(op, "SLLI")) funct3 = 1;
        else if (streq(op, "slti") || streq(op, "SLTI")) funct3 = 2;
        else if (streq(op, "sltiu") || streq(op, "SLTIU")) funct3 = 3;
        else if (streq(op, "xori") || streq(op, "XORI")) funct3 = 4;
        else if (streq(op, "srli") || streq(op, "SRLI")) funct3 = 5;
        else if (streq(op, "srai") || streq(op, "SRAI")) funct3 = 5;
        else if (streq(op, "ori") || streq(op, "ORI")) funct3 = 6;
        else if (streq(op, "andi") || streq(op, "ANDI")) funct3 = 7;
        else { is_i = 0; }

        if (is_i) {
            if (ntokens < 4) { puts("Usage: op rd, rs1, imm\r\n"); return -1; }
            int rd = parse_reg(tokens[1]);
            int rs1 = parse_reg(tokens[2]);
            int imm;
            if (rd < 0 || rs1 < 0) { puts("Bad register\r\n"); return -1; }
            if (!parse_imm(tokens[3], &imm)) { puts("Bad immediate\r\n"); return -1; }
            unsigned int before = regs[rd];
            /* SRAI needs special funct7 in imm field */
            int f3 = funct3;
            int actual_imm = imm;
            if ((streq(op, "srai") || streq(op, "SRAI"))) {
                actual_imm = (0x400) | (imm & 31);
            }
            if (exec_i_type(rd, rs1, actual_imm, f3) < 0) {
                puts("Execution error\r\n"); return -1;
            }
            if (rd != 0) {
                puts("  "); puts(reg_name(rd)); puts(" = ");
                print_hex(regs[rd]);
                puts(" (was "); print_hex(before); puts(")\r\n");
            } else {
                puts("  (x0 always 0)\r\n");
            }
            return 1;
        }
    }

    /* LUI rd, imm */
    if (streq(op, "lui") || streq(op, "LUI")) {
        if (ntokens < 3) { puts("Usage: lui rd, imm\r\n"); return -1; }
        int rd = parse_reg(tokens[1]);
        int imm;
        if (rd < 0) { puts("Bad register\r\n"); return -1; }
        if (!parse_imm(tokens[2], &imm)) { puts("Bad immediate\r\n"); return -1; }
        unsigned int before = regs[rd];
        if (rd != 0) regs[rd] = (unsigned int)imm << 12;
        if (rd != 0) {
            puts("  "); puts(reg_name(rd)); puts(" = ");
            print_hex(regs[rd]);
            puts(" (was "); print_hex(before); puts(")\r\n");
        }
        return 1;
    }

    /* LW rd, offset(rs1) */
    if (streq(op, "lw") || streq(op, "LW")) {
        if (ntokens < 3) { puts("Usage: lw rd, offset(rs1)\r\n"); return -1; }
        int rd = parse_reg(tokens[1]);
        /* Parse offset(rs1) */
        int offset = 0, rs1 = 0;
        /* Simple parse: look for ( in tokens[2] */
        char *paren = 0;
        for (char *p = tokens[2]; *p; p++) if (*p == '(') { paren = p; break; }
        if (paren) {
            *paren = 0;
            parse_imm(tokens[2], &offset);
            /* Parse register inside parens */
            char *reg_start = paren + 1;
            char *reg_end = reg_start;
            while (*reg_end && *reg_end != ')') reg_end++;
            *reg_end = 0;
            rs1 = parse_reg(reg_start);
        } else {
            rs1 = parse_reg(tokens[2]);
        }
        if (rd < 0 || rs1 < 0) { puts("Bad register\r\n"); return -1; }
        unsigned int addr = regs[rs1] + (unsigned int)offset;
        volatile unsigned int *p = (volatile unsigned int *)(unsigned long)addr;
        unsigned int before = regs[rd];
        if (rd != 0) regs[rd] = *p;
        if (rd != 0) {
            puts("  "); puts(reg_name(rd)); puts(" = ");
            print_hex(regs[rd]);
            puts(" ["); print_hex(addr); puts("]");
            puts(" (was "); print_hex(before); puts(")\r\n");
        }
        return 1;
    }

    /* SW rs2, offset(rs1) */
    if (streq(op, "sw") || streq(op, "SW")) {
        if (ntokens < 3) { puts("Usage: sw rs2, offset(rs1)\r\n"); return -1; }
        int rs2 = parse_reg(tokens[1]);
        int offset = 0, rs1 = 0;
        char *paren = 0;
        for (char *p = tokens[2]; *p; p++) if (*p == '(') { paren = p; break; }
        if (paren) {
            *paren = 0;
            parse_imm(tokens[2], &offset);
            char *reg_start = paren + 1;
            char *reg_end = reg_start;
            while (*reg_end && *reg_end != ')') reg_end++;
            *reg_end = 0;
            rs1 = parse_reg(reg_start);
        } else {
            rs1 = parse_reg(tokens[2]);
        }
        if (rs1 < 0 || rs2 < 0) { puts("Bad register\r\n"); return -1; }
        unsigned int addr = regs[rs1] + (unsigned int)offset;
        volatile unsigned int *p = (volatile unsigned int *)(unsigned long)addr;
        *p = regs[rs2];
        puts("  ["); print_hex(addr); puts("] = ");
        print_hex(regs[rs2]); puts("\r\n");
        return 1;
    }

    /* CSRR rd, csr */
    if (streq(op, "csrr") || streq(op, "CSRR")) {
        if (ntokens < 3) { puts("Usage: csrr rd, csr_name\r\n"); return -1; }
        int rd = parse_reg(tokens[1]);
        unsigned int val = 0;
        /* Read a few common CSRs */
        if (streq(tokens[2], "mhartid") || streq(tokens[2], "MHARTID")) {
            asm volatile("csrr %0, mhartid" : "=r"(val));
        } else if (streq(tokens[2], "mstatus") || streq(tokens[2], "MSTATUS")) {
            asm volatile("csrr %0, mstatus" : "=r"(val));
        } else if (streq(tokens[2], "misa") || streq(tokens[2], "MISA")) {
            asm volatile("csrr %0, misa" : "=r"(val));
        } else if (streq(tokens[2], "mie") || streq(tokens[2], "MIE")) {
            asm volatile("csrr %0, mie" : "=r"(val));
        } else if (streq(tokens[2], "mtvec") || streq(tokens[2], "MTVEC")) {
            asm volatile("csrr %0, mtvec" : "=r"(val));
        } else if (streq(tokens[2], "mepc") || streq(tokens[2], "MEPC")) {
            asm volatile("csrr %0, mepc" : "=r"(val));
        } else if (streq(tokens[2], "cycle") || streq(tokens[2], "CYCLE")) {
            asm volatile("csrr %0, cycle" : "=r"(val));
        } else {
            puts("Unknown CSR. Try: mhartid mstatus misa mie mtvec mepc cycle\r\n");
            return -1;
        }
        if (rd != 0) regs[rd] = val;
        puts("  "); puts(reg_name(rd)); puts(" = ");
        print_hex(val); puts("\r\n");
        return 1;
    }

    /* CSRW csr, rs1 */
    if (streq(op, "csrw") || streq(op, "CSRW")) {
        if (ntokens < 3) { puts("Usage: csrw csr_name, rs1\r\n"); return -1; }
        int rs1 = parse_reg(tokens[2]);
        if (rs1 < 0) { puts("Bad register\r\n"); return -1; }
        unsigned int val = regs[rs1];
        if (streq(tokens[1], "mstatus")) {
            asm volatile("csrw mstatus, %0" : : "r"(val));
        } else if (streq(tokens[1], "mie")) {
            asm volatile("csrw mie, %0" : : "r"(val));
        } else {
            puts("Write to that CSR not supported\r\n"); return -1;
        }
        puts("  "); puts(tokens[1]); puts(" <- ");
        print_hex(val); puts("\r\n");
        return 1;
    }

    return 0; /* not recognized */
}

static void process_line(void) {
    ibuf[ibuf_len] = 0;
    tokenize();

    if (ntokens == 0) return;

    /* Meta commands */
    if (streq(tokens[0], "help") || streq(tokens[0], "HELP")) { do_help(); return; }
    if (streq(tokens[0], "quit") || streq(tokens[0], "QUIT")
        || streq(tokens[0], "exit") || streq(tokens[0], "EXIT")
        || streq(tokens[0], "shutdown")) {
        puts("Bye!\r\n");
        sbi_shutdown();
    }
    if (streq(tokens[0], "regs") || streq(tokens[0], "REGS")) { do_regs(); return; }
    if (streq(tokens[0], "reset") || streq(tokens[0], "RESET")) {
        for (int i = 0; i < 32; i++) regs[i] = 0;
        puts("All registers zeroed.\r\n");
        return;
    }
    if (streq(tokens[0], "mem") || streq(tokens[0], "MEM")) { do_mem(); return; }
    if (streq(tokens[0], "dump") || streq(tokens[0], "DUMP")) { do_dump(); return; }

    /* Try as instruction */
    int r = exec_insn();
    if (r == 0) {
        puts("Unknown: ");
        puts(ibuf);
        puts("\r\nType 'help' for commands.\r\n");
    }
}

void c_start(void) {
    for (volatile int i = 0; i < 50000; i++);

    puts("\r\n=== GeOS ASM REPL ===\r\n");
    puts("RISC-V instruction executor. Type 'help' for commands.\r\n\r\n");

    for (;;) {
        puts("asm> ");

        ibuf_len = 0;
        int collecting = 1;
        while (collecting) {
            int c = getchar_wait();
            if (c < 0) continue;

            if (c == '\r' || c == '\n') {
                putchar('\r'); putchar('\n');
                collecting = 0;
            } else if (c == 127 || c == 8) {
                if (ibuf_len > 0) {
                    ibuf_len--;
                    putchar(8); putchar(' '); putchar(8);
                }
            } else if (c == 3) {
                puts("^C\r\n");
                ibuf_len = 0;
                collecting = 0;
            } else if (c >= 32 && c < 127 && ibuf_len < IBUF_SIZE - 1) {
                ibuf[ibuf_len++] = c;
                putchar(c);
            }
        }

        process_line();
    }
}
