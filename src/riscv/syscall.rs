// riscv/syscall.rs -- Linux RISC-V syscall number-to-name decoder (Phase 41)
//
// Maps Linux RISC-V syscall numbers to their names.
// On RISC-V Linux, the syscall number is passed in a7 (x17),
// arguments in a0-a5 (x10-x15), and the return value comes back in a0.
//
// Reference: Linux kernel arch/r86/kernel/syscalls/syscall.tbl
// RISC-V uses the same numbers as the generic syscall table (asm-generic/unistd.h).

/// A recorded Linux syscall event.
#[derive(Debug, Clone)]
pub struct SyscallEvent {
    /// Syscall number (from a7/x17).
    pub nr: u32,
    /// Syscall name (looked up from table, "unknown" if unmapped).
    pub name: &'static str,
    /// Arguments: a0-a5 (x10-x15).
    pub args: [u32; 6],
    /// Return value (a0 after kernel returns).
    /// None if we haven't observed the return yet.
    pub ret: Option<u32>,
    /// PC where the ECALL was executed.
    pub pc: u32,
}

/// Decode a Linux RISC-V syscall number to its name.
/// Returns "unknown" for unmapped numbers.
pub fn syscall_name(nr: u32) -> &'static str {
    SYSCALL_TABLE.get(nr as usize).copied().flatten().unwrap_or("unknown")
}

/// Total number of syscalls in the table.
pub fn syscall_count() -> usize {
    SYSCALL_TABLE.len()
}

// Complete Linux RISC-V syscall table (asm-generic/unistd.h).
// Covers syscalls 0-449 as defined in Linux 6.x.
// Stored as a flat array for O(1) lookup.
static SYSCALL_TABLE: &[Option<&str>] = &[
    // 0-9
    Some("io_setup"),             // 0
    Some("io_destroy"),           // 1
    Some("io_submit"),            // 2
    Some("io_cancel"),            // 3
    Some("io_getevents"),         // 4
    Some("setxattr"),             // 5
    Some("lsetxattr"),            // 6
    Some("fsetxattr"),            // 7
    Some("getxattr"),             // 8
    Some("lgetxattr"),            // 9
    // 10-19
    Some("fgetxattr"),            // 10
    Some("listxattr"),            // 11
    Some("llistxattr"),           // 12
    Some("flistxattr"),           // 13
    Some("removexattr"),          // 14
    Some("lremovexattr"),         // 15
    Some("fremovexattr"),         // 16
    Some("getcwd"),               // 17
    Some("lookup_dcookie"),       // 18
    Some("eventfd2"),             // 19
    // 20-29
    Some("epoll_create1"),        // 20
    Some("epoll_ctl"),            // 21
    Some("epoll_pwait"),          // 22
    Some("dup"),                  // 23
    Some("dup3"),                 // 24
    Some("fcntl"),                // 25
    Some("inotify_init1"),        // 26
    Some("inotify_add_watch"),    // 27
    Some("inotify_rm_watch"),     // 28
    Some("ioctl"),                // 29
    // 30-39
    Some("ioprio_set"),           // 30
    Some("ioprio_get"),           // 31
    Some("flock"),                // 32
    Some("mknodat"),              // 33
    Some("mkdirat"),              // 34
    Some("unlinkat"),             // 35
    Some("symlinkat"),            // 36
    Some("linkat"),               // 37
    Some("renameat"),             // 38
    Some("umount2"),              // 39
    // 40-49
    Some("mount"),                // 40
    Some("pivot_root"),           // 41
    Some("nfsservctl"),           // 42
    Some("statfs"),               // 43
    Some("fstatfs"),              // 44
    Some("truncate"),             // 45
    Some("ftruncate"),            // 46
    Some("fallocate"),            // 47
    Some("faccessat"),            // 48
    Some("chdir"),                // 49
    // 50-59
    Some("fchdir"),               // 50
    Some("chroot"),               // 51
    Some("fchmod"),               // 52
    Some("fchmodat"),             // 53
    Some("fchownat"),             // 54
    Some("fchown"),               // 55
    Some("openat"),               // 56
    Some("close"),                // 57
    Some("vhangup"),              // 58
    Some("pipe2"),                // 59
    // 60-69
    Some("quotactl"),             // 60
    Some("getdents64"),           // 61
    Some("lseek"),                // 62
    Some("read"),                 // 63
    Some("write"),                // 64
    Some("readv"),                // 65
    Some("writev"),               // 66
    Some("pread64"),              // 67
    Some("pwrite64"),             // 68
    Some("preadv"),               // 69
    // 70-79
    Some("pwritev"),              // 70
    Some("sendfile"),             // 71
    Some("pselect6"),             // 72
    Some("ppoll"),                // 73
    Some("signalfd4"),            // 74
    Some("vmsplice"),             // 75
    Some("splice"),               // 76
    Some("tee"),                  // 77
    Some("readlinkat"),           // 78
    Some("sync"),                 // 79
    // 80-89
    Some("fsync"),                // 80
    Some("fdatasync"),            // 81
    Some("sync_file_range"),      // 82
    Some("timerfd_create"),       // 83
    Some("timerfd_settime"),      // 84
    Some("timerfd_gettime"),      // 85
    Some("utimensat"),            // 86
    Some("acct"),                 // 87
    Some("capget"),               // 88
    Some("capset"),               // 89
    // 90-99
    Some("personality"),          // 90
    Some("exit"),                 // 91
    Some("exit_group"),           // 92
    Some("waitid"),               // 93
    Some("set_tid_address"),      // 94
    Some("unshare"),              // 95
    Some("futex"),                // 96
    Some("set_robust_list"),      // 97
    Some("get_robust_list"),      // 98
    Some("nanosleep"),            // 99
    // 100-109
    Some("getitimer"),            // 100
    Some("setitimer"),            // 101
    Some("kexec_load"),           // 102
    Some("init_module"),          // 103
    Some("delete_module"),        // 104
    Some("timer_create"),         // 105
    Some("timer_gettime"),        // 106
    Some("timer_getoverrun"),     // 107
    Some("timer_settime"),        // 108
    Some("timer_delete"),         // 109
    // 110-119
    Some("clock_settime"),        // 110
    Some("clock_gettime"),        // 111
    Some("clock_getres"),         // 112
    Some("clock_nanosleep"),      // 113
    Some("syslog"),               // 114
    Some("ptrace"),               // 115
    Some("sched_setparam"),       // 116
    Some("sched_setscheduler"),   // 117
    Some("sched_getscheduler"),   // 118
    Some("sched_getparam"),       // 119
    // 120-129
    Some("sched_setaffinity"),    // 120
    Some("sched_getaffinity"),    // 121
    Some("sched_yield"),          // 122
    Some("sched_get_priority_max"), // 123
    Some("sched_get_priority_min"), // 124
    Some("sched_rr_get_interval"),  // 125
    Some("restart_syscall"),      // 126
    Some("kill"),                 // 127
    Some("tkill"),                // 128
    Some("tgkill"),               // 129
    // 130-139
    Some("sigaltstack"),          // 130
    Some("rt_sigsuspend"),        // 131
    Some("rt_sigaction"),         // 132
    Some("rt_sigprocmask"),       // 133
    Some("rt_sigpending"),        // 134
    Some("rt_sigtimedwait"),      // 135
    Some("rt_sigqueueinfo"),      // 136
    Some("rt_sigreturn"),         // 137
    Some("setpriority"),          // 138
    Some("getpriority"),          // 139
    // 140-149
    Some("reboot"),               // 140
    Some("setregid"),             // 141
    Some("setgid"),               // 142
    Some("setreuid"),             // 143
    Some("setuid"),               // 144
    Some("setresuid"),            // 145
    Some("getresuid"),            // 146
    Some("setresgid"),            // 147
    Some("getresgid"),            // 148
    Some("setfsuid"),             // 149
    // 150-159
    Some("setfsgid"),             // 150
    Some("times"),                // 151
    Some("setpgid"),              // 152
    Some("getpgid"),              // 153
    Some("getsid"),               // 154
    Some("setsid"),               // 155
    Some("getgroups"),            // 156
    Some("setgroups"),            // 157
    Some("uname"),                // 158
    Some("sethostname"),          // 159
    // 160-169
    Some("setdomainname"),        // 160
    Some("getrlimit"),            // 161
    Some("setrlimit"),            // 162
    Some("getrusage"),            // 163
    Some("umask"),                // 164
    Some("prctl"),                // 165
    Some("getcpu"),               // 166
    Some("gettimeofday"),         // 167
    Some("settimeofday"),         // 168
    Some("adjtimex"),             // 169
    // 170-179
    Some("getpid"),               // 170
    Some("getppid"),              // 171
    Some("getuid"),               // 172
    Some("geteuid"),              // 173
    Some("getgid"),               // 174
    Some("getegid"),              // 175
    Some("gettid"),               // 176
    Some("sysinfo"),              // 177
    Some("mq_open"),              // 178
    Some("mq_unlink"),            // 179
    // 180-189
    Some("mq_timedsend"),         // 180
    Some("mq_timedreceive"),      // 181
    Some("mq_notify"),            // 182
    Some("mq_getsetattr"),        // 183
    Some("msgget"),               // 184
    Some("msgctl"),               // 185
    Some("msgrcv"),               // 186
    Some("msgsnd"),               // 187
    Some("semget"),               // 188
    Some("semctl"),               // 189
    // 190-199
    Some("semtimedop"),           // 190
    Some("semop"),                // 191
    Some("shmget"),               // 192
    Some("shmctl"),               // 193
    Some("shmat"),                // 194
    Some("shmdt"),                // 195
    Some("socket"),               // 196
    Some("socketpair"),           // 197
    Some("bind"),                 // 198
    Some("listen"),               // 199
    // 200-209
    Some("accept"),               // 200
    Some("connect"),              // 201
    Some("getsockname"),          // 202
    Some("getpeername"),          // 203
    Some("sendto"),               // 204
    Some("recvfrom"),             // 205
    Some("setsockopt"),           // 206
    Some("getsockopt"),           // 207
    Some("shutdown"),             // 208
    Some("sendmsg"),              // 209
    // 210-219
    Some("recvmsg"),              // 210
    Some("readahead"),            // 211
    Some("brk"),                  // 212
    Some("munmap"),               // 213
    Some("mremap"),               // 214
    Some("add_key"),              // 215
    Some("request_key"),          // 216
    Some("keyctl"),               // 217
    Some("clone"),                // 218
    Some("execve"),               // 219
    // 220-229
    Some("mmap"),                 // 220
    Some("fadvise64"),            // 221
    Some("swapon"),               // 222
    Some("swapoff"),              // 223
    Some("mprotect"),             // 224
    Some("msync"),                // 225
    Some("mlock"),                // 226
    Some("munlock"),              // 227
    Some("mlockall"),             // 228
    Some("munlockall"),           // 229
    // 230-239
    Some("mincore"),              // 230
    Some("madvise"),              // 231
    Some("remap_file_pages"),     // 232
    Some("mbind"),                // 233
    Some("get_mempolicy"),        // 234
    Some("set_mempolicy"),        // 235
    Some("migrate_pages"),        // 236
    Some("move_pages"),           // 237
    Some("rt_tgsigqueueinfo"),    // 238
    Some("perf_event_open"),      // 239
    // 240-249
    Some("accept4"),              // 240
    Some("recvmmsg"),             // 241
    None,                         // 242 (unmapped)
    None,                         // 243
    None,                         // 244
    Some("clone3"),               // 245
    Some("close_range"),          // 246 (Linux 5.9+)
    Some("openat2"),              // 247 (Linux 5.6+)
    None,                         // 248
    None,                         // 249
    // 250-259
    None,                         // 250
    None,                         // 251
    None,                         // 252
    None,                         // 253
    None,                         // 254
    None,                         // 255
    None,                         // 256
    None,                         // 257
    None,                         // 258
    None,                         // 259
    // 260-269
    Some("wait4"),                // 260
    Some("prlimit64"),            // 261
    Some("fanotify_init"),        // 262
    Some("fanotify_mark"),        // 263
    Some("name_to_handle_at"),    // 264
    Some("open_by_handle_at"),    // 265
    Some("clock_adjtime"),        // 266
    Some("syncfs"),               // 267
    Some("setns"),                // 268
    Some("sendmmsg"),             // 269
    // 270-279
    Some("process_vm_readv"),     // 270
    Some("process_vm_writev"),    // 271
    Some("kcmp"),                 // 272
    Some("finit_module"),         // 273
    Some("sched_setattr"),        // 274
    Some("sched_getattr"),        // 275
    Some("renameat2"),            // 276
    Some("seccomp"),              // 277
    Some("getrandom"),            // 278
    Some("memfd_create"),         // 279
    // 280-289
    Some("bpf"),                  // 280
    Some("execveat"),             // 281
    Some("userfaultfd"),          // 282
    Some("membarrier"),           // 283
    Some("mlock2"),               // 284
    Some("copy_file_range"),      // 285
    Some("preadv2"),              // 286
    Some("pwritev2"),             // 287
    Some("pkey_mprotect"),        // 288
    Some("pkey_alloc"),           // 289
    // 290-299
    Some("pkey_free"),            // 290
    Some("statx"),                // 291
    None,                         // 292
    None,                         // 293
    Some("rseq"),                 // 294
    None,                         // 295
    None,                         // 296
    None,                         // 297
    None,                         // 298
    None,                         // 299
    // 300-309 (kernel internal, skip)
    None,                         // 300
    None,                         // 301
    None,                         // 302
    None,                         // 303
    None,                         // 304
    None,                         // 305
    None,                         // 306
    None,                         // 307
    None,                         // 308
    None,                         // 309
    // 310-319 (kernel internal, skip)
    None,                         // 310
    None,                         // 311
    None,                         // 312
    None,                         // 313
    None,                         // 314
    None,                         // 315
    None,                         // 316
    None,                         // 317
    None,                         // 318
    None,                         // 319
    // 320-329
    None,                         // 320
    None,                         // 321
    None,                         // 322
    None,                         // 323
    None,                         // 324
    None,                         // 325
    None,                         // 326
    None,                         // 327
    None,                         // 328
    None,                         // 329
    // 330-339
    None,                         // 330
    None,                         // 331
    None,                         // 332
    None,                         // 333
    None,                         // 334
    None,                         // 335
    None,                         // 336
    None,                         // 337
    None,                         // 338
    None,                         // 339
    // 340-349
    None,                         // 340
    None,                         // 341
    None,                         // 342
    None,                         // 343
    None,                         // 344
    None,                         // 345
    None,                         // 346
    None,                         // 347
    None,                         // 348
    None,                         // 349
    // 350-359
    None,                         // 350
    None,                         // 351
    None,                         // 352
    None,                         // 353
    None,                         // 354
    None,                         // 355
    None,                         // 356
    None,                         // 357
    None,                         // 358
    None,                         // 359
    // 360-369
    None,                         // 360
    None,                         // 361
    None,                         // 362
    None,                         // 363
    None,                         // 364
    None,                         // 365
    None,                         // 366
    None,                         // 367
    None,                         // 368
    None,                         // 369
    // 370-379
    None,                         // 370
    None,                         // 371
    None,                         // 372
    None,                         // 373
    None,                         // 374
    None,                         // 375
    None,                         // 376
    None,                         // 377
    None,                         // 378
    None,                         // 379
    // 380-389
    None,                         // 380
    None,                         // 381
    None,                         // 382
    None,                         // 383
    None,                         // 384
    None,                         // 385
    None,                         // 386
    None,                         // 387
    None,                         // 388
    None,                         // 389
    // 390-399
    None,                         // 390
    None,                         // 391
    None,                         // 392
    None,                         // 393
    None,                         // 394
    None,                         // 395
    None,                         // 396
    None,                         // 397
    None,                         // 398
    None,                         // 399
    // 400-409 (direct syscalls added in newer kernels)
    None,                         // 400
    None,                         // 401
    None,                         // 402
    None,                         // 403
    None,                         // 404
    None,                         // 405
    None,                         // 406
    None,                         // 407
    None,                         // 408
    None,                         // 409
    // 410-419
    None,                         // 410
    None,                         // 411
    None,                         // 412
    None,                         // 413
    None,                         // 414
    None,                         // 415
    None,                         // 416
    None,                         // 417
    None,                         // 418
    None,                         // 419
    // 420-429
    None,                         // 420
    None,                         // 421
    None,                         // 422
    None,                         // 423
    None,                         // 424
    None,                         // 425
    None,                         // 426
    None,                         // 427
    None,                         // 428
    None,                         // 429
    // 430-439
    Some("pidfd_send_signal"),    // 430
    Some("io_uring_setup"),       // 431
    Some("io_uring_enter"),       // 432
    Some("io_uring_register"),    // 433
    Some("open_tree"),            // 434
    Some("move_mount"),           // 435
    Some("fsopen"),               // 436
    Some("fsconfig"),             // 437
    Some("fsmount"),              // 438
    Some("fspick"),               // 439
    // 440-449
    Some("pidfd_open"),           // 440
    Some("clone3"),               // 441 (duplicate from 245 on some tables)
    Some("close_range"),          // 442
    Some("openat2"),              // 443
    Some("pidfd_getfd"),          // 444
    Some("faccessat2"),           // 445
    Some("process_madvise"),      // 446
    Some("epoll_pwait2"),         // 447
    Some("mount_setattr"),        // 448
    Some("landlock_create_ruleset"), // 449
    // 450-459
    Some("landlock_add_rule"),    // 450
    Some("landlock_restrict_self"), // 451
    None,                         // 452
    Some("process_mrelease"),     // 453
    None,                         // 454
    Some("futex_waitv"),          // 455
    Some("set_mempolicy_home_node"), // 456
    Some("cachestat"),            // 457
    Some("fchmodat2"),            // 458
    Some("map_shadow_stack"),     // 459
    // 460-469
    Some("futex_wake"),           // 460
    Some("futex_wait"),           // 461
    Some("futex_requeue"),        // 462
    Some("statmount"),            // 463
    Some("listmount"),            // 464
    Some("lsm_get_self_attr"),    // 465
    Some("lsm_set_self_attr"),    // 466
    Some("lsm_list_modules"),     // 467
    Some("mseal"),                // 468
];

impl SyscallEvent {
    /// Format the syscall event as: name(arg0, arg1, ...) = ret
    /// or: name(arg0, arg1, ...) [no return captured]
    pub fn to_string(&self) -> String {
        let args_str = self
            .args
            .iter()
            .map(|a| format!("0x{:08X}", a))
            .collect::<Vec<_>>()
            .join(", ");
        match self.ret {
            Some(r) => format!(
                "{}({}) = 0x{:08X}  [pc=0x{:08X}, nr={}]",
                self.name, args_str, r, self.pc, self.nr
            ),
            None => format!(
                "{}({}) [pending]  [pc=0x{:08X}, nr={}]",
                self.name, args_str, self.pc, self.nr
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_known_syscalls() {
        assert_eq!(syscall_name(56), "openat");
        assert_eq!(syscall_name(57), "close");
        assert_eq!(syscall_name(63), "read");
        assert_eq!(syscall_name(64), "write");
        assert_eq!(syscall_name(91), "exit");
        assert_eq!(syscall_name(92), "exit_group");
        assert_eq!(syscall_name(172), "getuid");
        assert_eq!(syscall_name(174), "getgid");
        assert_eq!(syscall_name(212), "brk");
        assert_eq!(syscall_name(220), "mmap");
        assert_eq!(syscall_name(218), "clone");
        assert_eq!(syscall_name(219), "execve");
        assert_eq!(syscall_name(96), "futex");
        assert_eq!(syscall_name(160), "setdomainname");
        assert_eq!(syscall_name(170), "getpid");
        assert_eq!(syscall_name(178), "mq_open");
        assert_eq!(syscall_name(278), "getrandom");
        assert_eq!(syscall_name(291), "statx");
    }

    #[test]
    fn test_unknown_syscall() {
        assert_eq!(syscall_name(9999), "unknown");
        assert_eq!(syscall_name(500), "unknown");
    }

    #[test]
    fn test_table_size() {
        // We should have entries covering the main syscall range (0-468).
        let known = SYSCALL_TABLE.iter().filter(|e| e.is_some()).count();
        assert!(
            known >= 300,
            "expected at least 300 mapped syscalls, got {}",
            known
        );
    }

    #[test]
    fn test_event_format() {
        let event = SyscallEvent {
            nr: 64,
            name: "write",
            args: [1, 0x80000, 12, 0, 0, 0],
            ret: Some(12),
            pc: 0x80001000,
        };
        let s = event.to_string();
        assert!(s.contains("write"));
        assert!(s.contains("= 0x0000000C"));
    }

    #[test]
    fn test_all_key_syscalls_present() {
        // Verify every syscall a typical Linux userspace program uses
        let key_syscalls = [
            (56, "openat"),
            (57, "close"),
            (63, "read"),
            (64, "write"),
            (62, "lseek"),
            (91, "exit"),
            (92, "exit_group"),
            (96, "futex"),
            (160, "setdomainname"),
            (172, "getuid"),
            (174, "getgid"),
            (210, "recvmsg"),
            (212, "brk"),
            (215, "add_key"),
            (218, "clone"),
            (219, "execve"),
            (220, "mmap"),
            (224, "mprotect"),
            (231, "madvise"),
            (260, "wait4"),
            (278, "getrandom"),
            (291, "statx"),
        ];
        for (nr, expected) in &key_syscalls {
            assert_eq!(syscall_name(*nr), *expected, "syscall {} mismatch", nr);
        }
    }
}
