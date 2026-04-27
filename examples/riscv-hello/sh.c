/*
 * sh.c -- Bare-metal interactive mini-shell for Geometry OS RISC-V hypervisor.
 *
 * Built-ins: help, echo, clear, peek, poke, mem, hexdump, regs, shutdown.
 * Uses SBI_CONSOLE_PUTCHAR (a7=1) and SBI_CONSOLE_GETCHAR (a7=2).
 * Loops forever until "shutdown" command.
 *
 * Build: ./build.sh sh.c
 * Run:   cargo run --release --example sh_run
 */

#define UART_BASE  0x10000000
#define UART_THR   (*(volatile unsigned char *)UART_BASE)
#define UART_LSR   (*(volatile unsigned char *)(UART_BASE + 5))
#define UART_LSR_DR  0x01
#define UART_LSR_THRE 0x20

/* SBI console calls */
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

/* UART MMIO -- used as fallback / alternative to SBI */
static inline void uart_putc(char c) {
    while (!(UART_LSR & UART_LSR_THRE))
        ;
    UART_THR = c;
}

static inline int uart_getc(void) {
    if (UART_LSR & UART_LSR_DR)
        return UART_THR;
    return -1;
}

/* Print string via SBI */
static void puts(const char *s) {
    while (*s) {
        sbi_putchar(*s++);
    }
}

/* Print char */
static void putchar(char c) {
    sbi_putchar(c);
}

/* Read one char, blocking */
static int getchar(void) {
    int c;
    /* Try SBI first */
    c = sbi_getchar();
    if (c >= 0 && c != 0xFF) return c;
    /* Fall back to UART MMIO */
    c = uart_getc();
    if (c >= 0) return c;
    return -1;
}

/* Poll for char with tiny delay (busy-wait) */
static int getchar_wait(void) {
    for (volatile int i = 0; i < 1000; i++) {
        int c = getchar();
        if (c >= 0) return c;
    }
    return -1;
}

/* Print unsigned int in decimal */
static void print_uint(unsigned int v) {
    char buf[12];
    int i = 0;
    if (v == 0) { putchar('0'); return; }
    while (v > 0) {
        buf[i++] = '0' + (v % 10);
        v /= 10;
    }
    while (--i >= 0)
        putchar(buf[i]);
}

/* Print unsigned int in hex (0x prefix) */
static void print_hex(unsigned int v) {
    const char hex[] = "0123456789abcdef";
    puts("0x");
    for (int i = 28; i >= 0; i -= 4) {
        putchar(hex[(v >> i) & 0xF]);
    }
}

/* Parse hex string to unsigned int. Returns 1 on success. */
static int parse_hex(const char *s, unsigned int *out) {
    *out = 0;
    if (s[0] == '0' && (s[1] == 'x' || s[1] == 'X'))
        s += 2;
    if (!*s) return 0;
    while (*s) {
        char c = *s++;
        *out <<= 4;
        if (c >= '0' && c <= '9') *out |= c - '0';
        else if (c >= 'a' && c <= 'f') *out |= 10 + c - 'a';
        else if (c >= 'A' && c <= 'F') *out |= 10 + c - 'A';
        else return 0;
    }
    return 1;
}

/* Parse decimal string to unsigned int. Returns 1 on success. */
static int parse_dec(const char *s, unsigned int *out) {
    *out = 0;
    if (!*s) return 0;
    while (*s) {
        char c = *s++;
        if (c < '0' || c > '9') return 0;
        *out = *out * 10 + (c - '0');
    }
    return 1;
}

/* Memory read (volatile) */
static volatile unsigned int *mem_ptr(unsigned int addr) {
    return (volatile unsigned int *)(unsigned long)addr;
}

/* ---- Input buffer ---- */
#define IBUF_SIZE 128
static char ibuf[IBUF_SIZE];
static int ibuf_len = 0;

/* Skip leading whitespace */
static const char *skip_spaces(const char *s) {
    while (*s == ' ' || *s == '\t') s++;
    return s;
}

/* Compare string with prefix */
static int streq(const char *a, const char *b) {
    while (*a && *b) {
        if (*a != *b) return 0;
        a++; b++;
    }
    return *a == 0 && *b == 0;
}

/* ---- Commands ---- */

static void cmd_help(void) {
    puts(
        "Commands:\r\n"
        "  help          - show this help\r\n"
        "  echo <text>   - print text\r\n"
        "  clear         - clear screen (ANSI)\r\n"
        "  peek <addr>   - read 32-bit word from address\r\n"
        "  poke <addr> <val> - write 32-bit word to address\r\n"
        "  mem <addr> [n]- dump n words (default 4)\r\n"
        "  hexdump <addr> [n] - dump n bytes in hex+ascii\r\n"
        "  regs          - dump registers (via self-read)\r\n"
        "  ver           - version info\r\n"
        "  shutdown      - halt the VM\r\n"
    );
}

static void cmd_echo(const char *arg) {
    puts(arg);
    puts("\r\n");
}

static void cmd_clear(void) {
    /* ANSI escape: clear screen + home cursor */
    puts("\x1b[2J\x1b[H");
}

static void cmd_peek(const char *arg) {
    unsigned int addr;
    arg = skip_spaces(arg);
    if (!parse_hex(arg, &addr) && !parse_dec(arg, &addr)) {
        puts("Usage: peek <hex_addr>\r\n");
        return;
    }
    puts("[");
    print_hex(addr);
    puts("] = ");
    print_hex(*mem_ptr(addr));
    puts("\r\n");
}

static void cmd_poke(const char *arg) {
    unsigned int addr, val;
    arg = skip_spaces(arg);
    /* Parse address */
    const char *next = arg;
    while (*next && *next != ' ') next++;
    char tmp[32];
    int len = next - arg;
    if (len >= 32) { puts("addr too long\r\n"); return; }
    for (int i = 0; i < len; i++) tmp[i] = arg[i];
    tmp[len] = 0;
    if (!parse_hex(tmp, &addr) && !parse_dec(tmp, &addr)) {
        puts("Usage: poke <hex_addr> <hex_val>\r\n");
        return;
    }
    /* Parse value */
    arg = skip_spaces(next);
    if (!parse_hex(arg, &val) && !parse_dec(arg, &val)) {
        puts("Usage: poke <hex_addr> <hex_val>\r\n");
        return;
    }
    *mem_ptr(addr) = val;
    puts("[");
    print_hex(addr);
    puts("] = ");
    print_hex(val);
    puts("\r\n");
}

static void cmd_mem(const char *arg) {
    unsigned int addr;
    unsigned int count = 4;
    arg = skip_spaces(arg);
    const char *next = arg;
    while (*next && *next != ' ') next++;
    char tmp[32];
    int len = next - arg;
    if (len >= 32) return;
    for (int i = 0; i < len; i++) tmp[i] = arg[i];
    tmp[len] = 0;
    if (!parse_hex(tmp, &addr) && !parse_dec(tmp, &addr)) {
        puts("Usage: mem <hex_addr> [count]\r\n");
        return;
    }
    arg = skip_spaces(next);
    if (*arg) {
        parse_dec(arg, &count);
        if (count > 64) count = 64;
    }
    for (unsigned int i = 0; i < count; i++) {
        print_hex(addr + i * 4);
        puts(": ");
        print_hex(*mem_ptr(addr + i * 4));
        puts("\r\n");
    }
}

static void cmd_hexdump(const char *arg) {
    unsigned int addr;
    unsigned int count = 16;
    arg = skip_spaces(arg);
    const char *next = arg;
    while (*next && *next != ' ') next++;
    char tmp[32];
    int len = next - arg;
    if (len >= 32) return;
    for (int i = 0; i < len; i++) tmp[i] = arg[i];
    tmp[len] = 0;
    if (!parse_hex(tmp, &addr) && !parse_dec(tmp, &addr)) {
        puts("Usage: hexdump <hex_addr> [bytes]\r\n");
        return;
    }
    arg = skip_spaces(next);
    if (*arg) {
        parse_dec(arg, &count);
        if (count > 256) count = 256;
    }
    volatile unsigned char *p = (volatile unsigned char *)(unsigned long)addr;
    for (unsigned int row = 0; row < count; row += 16) {
        print_hex(addr + row);
        puts(": ");
        /* Hex bytes */
        for (unsigned int col = 0; col < 16; col++) {
            if (row + col < count) {
                const char hex[] = "0123456789abcdef";
                unsigned char b = p[row + col];
                putchar(hex[(b >> 4) & 0xF]);
                putchar(hex[b & 0xF]);
            } else {
                putchar(' ');
                putchar(' ');
            }
            putchar(' ');
        }
        puts(" |");
        /* ASCII */
        for (unsigned int col = 0; col < 16 && row + col < count; col++) {
            unsigned char b = p[row + col];
            putchar((b >= 32 && b < 127) ? b : '.');
        }
        puts("|\r\n");
    }
}

static void cmd_regs(void) {
    /* Read common CSRs via inline asm */
    unsigned int mhartid, mstatus, mepc, mcause;
    asm volatile("csrr %0, mhartid" : "=r"(mhartid));
    asm volatile("csrr %0, mstatus" : "=r"(mstatus));
    asm volatile("csrr %0, mepc" : "=r"(mepc));
    asm volatile("csrr %0, mcause" : "=r"(mcause));
    puts("mhartid: "); print_hex(mhartid); puts("\r\n");
    puts("mstatus: "); print_hex(mstatus); puts("\r\n");
    puts("mepc:    "); print_hex(mepc);    puts("\r\n");
    puts("mcause:  "); print_hex(mcause);  puts("\r\n");
}

static void cmd_ver(void) {
    puts("GeOS Mini-Shell v0.1 (bare-metal RV32IMAC)\r\n");
    puts("Geometry OS RISC-V Hypervisor\r\n");
}

/* ---- Main REPL ---- */

static void process_command(void) {
    /* Null-terminate */
    ibuf[ibuf_len] = 0;

    const char *cmd = skip_spaces(ibuf);
    if (!*cmd) return;

    /* Dispatch */
    if (streq(cmd, "help")) {
        cmd_help();
    } else if (streq(cmd, "clear")) {
        cmd_clear();
    } else if (streq(cmd, "regs")) {
        cmd_regs();
    } else if (streq(cmd, "ver") || streq(cmd, "version")) {
        cmd_ver();
    } else if (streq(cmd, "shutdown") || streq(cmd, "exit") || streq(cmd, "quit")) {
        puts("Goodbye!\r\n");
        sbi_shutdown();
    } else {
        /* Commands with arguments */
        const char *arg = cmd;
        while (*arg && *arg != ' ') arg++;
        char name[16];
        int nlen = arg - cmd;
        if (nlen >= 16) nlen = 15;
        for (int i = 0; i < nlen; i++) name[i] = cmd[i];
        name[nlen] = 0;
        arg = skip_spaces(arg);

        if (streq(name, "echo"))
            cmd_echo(arg);
        else if (streq(name, "peek"))
            cmd_peek(arg);
        else if (streq(name, "poke"))
            cmd_poke(arg);
        else if (streq(name, "mem"))
            cmd_mem(arg);
        else if (streq(name, "hexdump"))
            cmd_hexdump(arg);
        else {
            puts("unknown: ");
            puts(cmd);
            puts("\r\n");
        }
    }
}

void c_start(void) {
    /* Small delay for UART init */
    for (volatile int i = 0; i < 50000; i++);

    puts("\r\n=== GeOS Mini-Shell ===\r\n");
    puts("Type 'help' for commands.\r\n\r\n");

    for (;;) {
        puts("geos> ");

        ibuf_len = 0;
        int collecting = 1;
        while (collecting) {
            int c = getchar_wait();
            if (c < 0) continue;

            if (c == '\r' || c == '\n') {
                /* Enter */
                putchar('\r');
                putchar('\n');
                collecting = 0;
            } else if (c == 127 || c == 8) {
                /* Backspace */
                if (ibuf_len > 0) {
                    ibuf_len--;
                    putchar(8);
                    putchar(' ');
                    putchar(8);
                }
            } else if (c == 3) {
                /* Ctrl-C: cancel line */
                puts("^C\r\n");
                ibuf_len = 0;
                collecting = 0;
            } else if (c >= 32 && c < 127 && ibuf_len < IBUF_SIZE - 1) {
                /* Printable char */
                ibuf[ibuf_len++] = c;
                putchar(c);
            }
        }

        process_command();
    }
}
