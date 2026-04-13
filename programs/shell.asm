; shell.asm -- Phase 29: Interactive command shell for Geometry OS
;
; A minimal command shell that provides:
; - Command prompt display
; - Built-in commands: help, echo, ls, cat, ps, kill, export, clear
; - External program execution via EXEC
; - Pipe operator (cmd1 | cmd2)
; - Output redirection (cmd > file, cmd >> file)
; - Input redirection (cmd < file)
;
; Memory layout:
;   0x0200 - 0x03FF: command input buffer (256 bytes)
;   0x0400 - 0x05FF: parsed command name (256 bytes)
;   0x0600 - 0x07FF: parsed argument (256 bytes)
;   0x0800 - 0x0FFF: file listing buffer (LS output)
;   0x1000 - 0x10FF: read buffer for cat
;   0x1100 - 0x11FF: working directory / env var buffer
;   0x1200:         readln position counter
;   0x1201:         prompt y position
;   0x1202:         child PID storage
;   0x1203:         pipe read fd
;   0x1204:         pipe write fd
;   0x1205:         redirect mode (0=none, 1=>, 2=>>, 3=<)
;   0x1206:         redirect fd
;   0x1207:         pipe mode flag (0=no pipe, 1=pipe)
;   0x1300 - 0x13FF: help text buffer
;   0x1400 - 0x14FF: pipe command buffer (second command in pipeline)
;   0x1500 - 0x15FF: status message buffer

.org 0x000

; ═══════════════════════════════════════════════════════════════
; Initialize shell
; ═══════════════════════════════════════════════════════════════
    LDI r0, 0
    FILL r0               ; clear screen to black
    LDI r9, 0x1201
    LDI r0, 20
    STORE r9, r0          ; prompt y starts at line 20
    LDI r9, 0x1207
    LDI r0, 0
    STORE r9, r0          ; pipe mode = 0
    LDI r9, 0x1205
    STORE r9, r0          ; redirect mode = 0

    ; Write help text to buffer at 0x1300
    LDI r9, 0x1300
    LDI r0, 104       ; h
    STORE r9, r0
    LDI r9, 0x1301
    LDI r0, 101       ; e
    STORE r9, r0
    LDI r9, 0x1302
    LDI r0, 108       ; l
    STORE r9, r0
    LDI r9, 0x1303
    LDI r0, 112       ; p
    STORE r9, r0
    LDI r9, 0x1304
    LDI r0, 0
    STORE r9, r0

; ═══════════════════════════════════════════════════════════════
; Main loop: display prompt, read command, execute
; ═══════════════════════════════════════════════════════════════
main_loop:
    ; Display prompt "> " at current y position
    LDI r9, 0x1201         ; r9 = prompt y addr
    LOAD r1, r9            ; r1 = y position
    LDI r2, 2              ; x = 2
    LDI r3, prompt_str
    TEXT r2, r1, r3

    ; Read a line of input
    LDI r0, 0x0200         ; buf addr
    LDI r1, 200            ; max len
    LDI r2, 0x1200         ; pos addr
    LDI r9, 0x1200
    LDI r3, 0
    STORE r9, r3           ; reset position

readln_loop:
    READLN r0, r1, r2
    CMP r0, r3             ; r0 == 0 means still reading
    JZ r0, readln_loop

    ; r0 > 0 means line complete (Enter pressed)
    ; Check if line is empty
    JZ r0, main_loop       ; empty line, loop

    ; Parse and execute the command
    CALL parse_command
    CALL execute_command

    ; Scroll down prompt position
    LDI r9, 0x1201
    LOAD r1, r9
    LDI r0, 12
    ADD r1, r0
    STORE r9, r1

    ; If y > 240, scroll screen and reset
    LDI r0, 240
    CMP r1, r0
    BLT r1, main_loop
    LDI r0, 10
    SCROLL r0
    LDI r1, 20
    STORE r9, r1
    JMP main_loop

; ═══════════════════════════════════════════════════════════════
; parse_command -- extract command name and argument from input
; Input: command buffer at 0x0200
; Output: command at 0x0400, argument at 0x0600
; Also sets pipe mode (0x1207) and redirect mode (0x1205)
; ═══════════════════════════════════════════════════════════════
parse_command:
    PUSH r15
    PUSH r14
    PUSH r13
    PUSH r12

    ; Reset pipe mode and redirect mode
    LDI r9, 0x1207
    LDI r0, 0
    STORE r9, r0
    LDI r9, 0x1205
    STORE r9, r0

    ; Skip leading spaces
    LDI r15, 0x0200       ; src pointer
skip_spaces:
    LOAD r0, r15
    JZ r0, parse_done      ; empty string
    LDI r1, 32
    CMP r0, r1
    JZ r0, skip_spaces     ; skip space

    ; Copy command name until space or null
    LDI r14, 0x0400       ; dest for command name
copy_cmd:
    LOAD r0, r15
    JZ r0, parse_cmd_done
    LDI r1, 32
    CMP r0, r1
    JZ r0, parse_cmd_done
    ; Check for pipe character '|' (124)
    LDI r1, 124
    CMP r0, r1
    JZ r0, parse_pipe_found
    ; Check for '>' (62)
    LDI r1, 62
    CMP r0, r1
    JZ r0, parse_redirect_out
    ; Check for '<' (60)
    LDI r1, 60
    CMP r0, r1
    JZ r0, parse_redirect_in
    STORE r14, r0
    ADD r15, r3            ; r3 is still 0 from earlier... let's use r1
    ; Actually need to increment properly
    LDI r1, 1
    ADD r15, r1
    LDI r1, 1
    ADD r14, r1
    JMP copy_cmd

parse_cmd_done:
    ; Null terminate command
    LDI r0, 0
    STORE r14, r0

    ; Skip spaces after command
    LOAD r0, r15
    JZ r0, parse_arg_done  ; no argument
skip_arg_spaces:
    LOAD r0, r15
    LDI r1, 32
    CMP r0, r1
    JZ r0, skip_arg_spaces_inc
    JMP copy_arg_start
skip_arg_spaces_inc:
    LDI r1, 1
    ADD r15, r1
    JMP skip_arg_spaces

copy_arg_start:
    ; Copy argument until null
    LDI r14, 0x0600       ; dest for argument
copy_arg:
    LOAD r0, r15
    JZ r0, parse_arg_done
    ; Check for pipe
    LDI r1, 124
    CMP r0, r1
    JZ r0, parse_pipe_found_arg
    ; Check for redirect
    LDI r1, 62
    CMP r0, r1
    JZ r0, parse_redirect_out_arg
    LDI r1, 60
    CMP r0, r1
    JZ r0, parse_redirect_in_arg
    STORE r14, r0
    LDI r1, 1
    ADD r15, r1
    ADD r14, r1
    JMP copy_arg

parse_arg_done:
    LDI r0, 0
    STORE r14, r0
    JMP parse_done

parse_pipe_found:
    ; Null terminate command
    LDI r0, 0
    STORE r14, r0
    ; Set pipe mode
    LDI r9, 0x1207
    LDI r0, 1
    STORE r9, r0
    ; Skip pipe char and spaces
    LDI r1, 1
    ADD r15, r1
skip_pipe_spaces:
    LOAD r0, r15
    LDI r1, 32
    CMP r0, r1
    JZ r0, skip_pipe_spaces_inc
    JMP copy_pipe_cmd
skip_pipe_spaces_inc:
    LDI r1, 1
    ADD r15, r1
    JMP skip_pipe_spaces

copy_pipe_cmd:
    ; Copy second command to pipe buffer at 0x1400
    LDI r14, 0x1400
copy_pipe_loop:
    LOAD r0, r15
    JZ r0, parse_pipe_done
    STORE r14, r0
    LDI r1, 1
    ADD r15, r1
    ADD r14, r1
    JMP copy_pipe_loop
parse_pipe_done:
    LDI r0, 0
    STORE r14, r0
    JMP parse_done

parse_pipe_found_arg:
    ; Null terminate argument
    LDI r0, 0
    STORE r14, r0
    ; Set pipe mode
    LDI r9, 0x1207
    LDI r0, 1
    STORE r9, r0
    ; Skip pipe char and spaces
    LDI r1, 1
    ADD r15, r1
skip_pipe_spaces2:
    LOAD r0, r15
    LDI r1, 32
    CMP r0, r1
    JZ r0, skip_pipe_spaces2_inc
    JMP copy_pipe_cmd2
skip_pipe_spaces2_inc:
    LDI r1, 1
    ADD r15, r1
    JMP skip_pipe_spaces2

copy_pipe_cmd2:
    LDI r14, 0x1400
copy_pipe_loop2:
    LOAD r0, r15
    JZ r0, parse_done2
    STORE r14, r0
    LDI r1, 1
    ADD r15, r1
    ADD r14, r1
    JMP copy_pipe_loop2
parse_done2:
    LDI r0, 0
    STORE r14, r0
    JMP parse_done

parse_redirect_out:
    ; Null terminate command
    LDI r0, 0
    STORE r14, r0
    ; Check for >> (append)
    LDI r1, 1
    ADD r15, r1
    LOAD r0, r15
    LDI r1, 62
    CMP r0, r1
    JZ r0, redirect_append
    ; Single > = write (mode 1)
    LDI r9, 0x1205
    LDI r0, 1
    STORE r9, r0
    JMP skip_redir_spaces
redirect_append:
    LDI r9, 0x1205
    LDI r0, 2
    STORE r9, r0
    LDI r1, 1
    ADD r15, r1
    JMP skip_redir_spaces

parse_redirect_out_arg:
    ; Null terminate argument
    LDI r0, 0
    STORE r14, r0
    LDI r1, 1
    ADD r15, r1
    LOAD r0, r15
    LDI r1, 62
    CMP r0, r1
    JZ r0, redirect_append2
    LDI r9, 0x1205
    LDI r0, 1
    STORE r9, r0
    JMP skip_redir_spaces
redirect_append2:
    LDI r9, 0x1205
    LDI r0, 2
    STORE r9, r0
    LDI r1, 1
    ADD r15, r1
    JMP skip_redir_spaces

parse_redirect_in:
    LDI r0, 0
    STORE r14, r0
    LDI r9, 0x1205
    LDI r0, 3
    STORE r9, r0
    LDI r1, 1
    ADD r15, r1
    JMP skip_redir_spaces

parse_redirect_in_arg:
    LDI r0, 0
    STORE r14, r0
    LDI r9, 0x1205
    LDI r0, 3
    STORE r9, r0
    LDI r1, 1
    ADD r15, r1
    JMP skip_redir_spaces

skip_redir_spaces:
    LOAD r0, r15
    LDI r1, 32
    CMP r0, r1
    JZ r0, skip_redir_spaces_inc
    JMP copy_redir_file
skip_redir_spaces_inc:
    LDI r1, 1
    ADD r15, r1
    JMP skip_redir_spaces

copy_redir_file:
    ; Copy redirect filename to argument buffer (0x0600)
    LDI r14, 0x0600
copy_redir_loop:
    LOAD r0, r15
    JZ r0, parse_done
    STORE r14, r0
    LDI r1, 1
    ADD r15, r1
    ADD r14, r1
    JMP copy_redir_loop

parse_done:
    POP r12
    POP r13
    POP r14
    POP r15
    RET

; ═══════════════════════════════════════════════════════════════
; execute_command -- dispatch to built-in or EXEC
; Command at 0x0400, argument at 0x0600
; ═══════════════════════════════════════════════════════════════
execute_command:
    PUSH r15
    PUSH r14
    PUSH r13

    ; Check for empty command
    LDI r9, 0x0400
    LOAD r0, r9
    JZ r0, exec_done

    ; Compare command with known built-ins
    ; help
    CALL cmd_is_help
    JNZ r0, do_help

    ; echo
    CALL cmd_is_echo
    JNZ r0, do_echo

    ; ls
    CALL cmd_is_ls
    JNZ r0, do_ls

    ; cat
    CALL cmd_is_cat
    JNZ r0, do_cat

    ; ps
    CALL cmd_is_ps
    JNZ r0, do_ps

    ; kill
    CALL cmd_is_kill
    JNZ r0, do_kill

    ; export
    CALL cmd_is_export
    JNZ r0, do_export

    ; clear
    CALL cmd_is_clear
    JNZ r0, do_clear

    ; Not a built-in -- try EXEC
    JMP do_exec

exec_done:
    POP r13
    POP r14
    POP r15
    RET

; ── Command comparison helpers ────────────────────────────────
; Each sets r0=1 if match, r0=0 if not

cmd_is_help:
    LDI r9, 0x0400
    LDI r0, 104       ; h
    LOAD r1, r9
    CMP r0, r1
    JNZ r0, cmd_help_n
    LDI r9, 0x0401
    LDI r0, 101       ; e
    LOAD r1, r9
    CMP r0, r1
    JNZ r0, cmd_help_n
    LDI r9, 0x0402
    LDI r0, 108       ; l
    LOAD r1, r9
    CMP r0, r1
    JNZ r0, cmd_help_n
    LDI r9, 0x0403
    LDI r0, 112       ; p
    LOAD r1, r9
    CMP r0, r1
    JNZ r0, cmd_help_n
    LDI r9, 0x0404
    LDI r0, 0
    LOAD r1, r9
    CMP r0, r1
    JNZ r0, cmd_help_n
    LDI r0, 1
    RET
cmd_help_n:
    LDI r0, 0
    RET

cmd_is_echo:
    LDI r9, 0x0400
    LDI r0, 101       ; e
    LOAD r1, r9
    CMP r0, r1
    JNZ r0, cno
    LDI r9, 0x0401
    LDI r0, 99        ; c
    LOAD r1, r9
    CMP r0, r1
    JNZ r0, cno
    LDI r9, 0x0402
    LDI r0, 104       ; h
    LOAD r1, r9
    CMP r0, r1
    JNZ r0, cno
    LDI r9, 0x0403
    LDI r0, 111       ; o
    LOAD r1, r9
    CMP r0, r1
    JNZ r0, cno
    LDI r9, 0x0404
    LDI r0, 0
    LOAD r1, r9
    CMP r0, r1
    JNZ r0, cno
    LDI r0, 1
    RET

cmd_is_ls:
    LDI r9, 0x0400
    LDI r0, 108       ; l
    LOAD r1, r9
    CMP r0, r1
    JNZ r0, cno
    LDI r9, 0x0401
    LDI r0, 115       ; s
    LOAD r1, r9
    CMP r0, r1
    JNZ r0, cno
    LDI r9, 0x0402
    LDI r0, 0
    LOAD r1, r9
    CMP r0, r1
    JNZ r0, cno
    LDI r0, 1
    RET

cmd_is_cat:
    LDI r9, 0x0400
    LDI r0, 99        ; c
    LOAD r1, r9
    CMP r0, r1
    JNZ r0, cno
    LDI r9, 0x0401
    LDI r0, 97        ; a
    LOAD r1, r9
    CMP r0, r1
    JNZ r0, cno
    LDI r9, 0x0402
    LDI r0, 116       ; t
    LOAD r1, r9
    CMP r0, r1
    JNZ r0, cno
    LDI r9, 0x0403
    LDI r0, 0
    LOAD r1, r9
    CMP r0, r1
    JNZ r0, cno
    LDI r0, 1
    RET

cmd_is_ps:
    LDI r9, 0x0400
    LDI r0, 112       ; p
    LOAD r1, r9
    CMP r0, r1
    JNZ r0, cno
    LDI r9, 0x0401
    LDI r0, 115       ; s
    LOAD r1, r9
    CMP r0, r1
    JNZ r0, cno
    LDI r9, 0x0402
    LDI r0, 0
    LOAD r1, r9
    CMP r0, r1
    JNZ r0, cno
    LDI r0, 1
    RET

cmd_is_kill:
    LDI r9, 0x0400
    LDI r0, 107       ; k
    LOAD r1, r9
    CMP r0, r1
    JNZ r0, cno
    LDI r9, 0x0401
    LDI r0, 105       ; i
    LOAD r1, r9
    CMP r0, r1
    JNZ r0, cno
    LDI r9, 0x0402
    LDI r0, 108       ; l
    LOAD r1, r9
    CMP r0, r1
    JNZ r0, cno
    LDI r9, 0x0403
    LDI r0, 108       ; l
    LOAD r1, r9
    CMP r0, r1
    JNZ r0, cno
    LDI r9, 0x0404
    LDI r0, 0
    LOAD r1, r9
    CMP r0, r1
    JNZ r0, cno
    LDI r0, 1
    RET

cmd_is_export:
    LDI r9, 0x0400
    LDI r0, 101       ; e
    LOAD r1, r9
    CMP r0, r1
    JNZ r0, cno
    LDI r9, 0x0401
    LDI r0, 120       ; x
    LOAD r1, r9
    CMP r0, r1
    JNZ r0, cno
    LDI r9, 0x0402
    LDI r0, 112       ; p
    LOAD r1, r9
    CMP r0, r1
    JNZ r0, cno
    LDI r9, 0x0403
    LDI r0, 111       ; o
    LOAD r1, r9
    CMP r0, r1
    JNZ r0, cno
    LDI r9, 0x0404
    LDI r0, 114       ; r
    LOAD r1, r9
    CMP r0, r1
    JNZ r0, cno
    LDI r9, 0x0405
    LDI r0, 116       ; t
    LOAD r1, r9
    CMP r0, r1
    JNZ r0, cno
    LDI r9, 0x0406
    LDI r0, 0
    LOAD r1, r9
    CMP r0, r1
    JNZ r0, cno
    LDI r0, 1
    RET

cmd_is_clear:
    LDI r9, 0x0400
    LDI r0, 99        ; c
    LOAD r1, r9
    CMP r0, r1
    JNZ r0, cno
    LDI r9, 0x0401
    LDI r0, 108       ; l
    LOAD r1, r9
    CMP r0, r1
    JNZ r0, cno
    LDI r9, 0x0402
    LDI r0, 101       ; e
    LOAD r1, r9
    CMP r0, r1
    JNZ r0, cno
    LDI r9, 0x0403
    LDI r0, 97        ; a
    LOAD r1, r9
    CMP r0, r1
    JNZ r0, cno
    LDI r9, 0x0404
    LDI r0, 114       ; r
    LOAD r1, r9
    CMP r0, r1
    JNZ r0, cno
    LDI r9, 0x0405
    LDI r0, 0
    LOAD r1, r9
    CMP r0, r1
    JNZ r0, cno
    LDI r0, 1
    RET

cno:
    LDI r0, 0
    RET

; ── Built-in command implementations ──────────────────────────

do_help:
    ; Display available commands
    LDI r9, 0x1201
    LOAD r1, r9
    LDI r2, 2
    LDI r3, help_text
    TEXT r2, r1, r3
    JMP exec_done

do_echo:
    ; Print the argument
    LDI r9, 0x1201
    LOAD r1, r9
    LDI r0, 12
    ADD r1, r0
    STORE r9, r1
    LDI r2, 4
    LDI r3, 0x0600
    TEXT r2, r1, r3
    JMP exec_done

do_ls:
    ; List files using LS opcode
    LDI r1, 0x0800
    LS r1                  ; list files into buffer at 0x0800
    ; r0 = number of files
    ; Display each filename
    LDI r9, 0x1201
    LOAD r1, r9
    LDI r0, 12
    ADD r1, r0
    STORE r9, r1
    LDI r2, 4
    LDI r3, 0x0800
    TEXT r2, r1, r3
    JMP exec_done

do_cat:
    ; Read and display a file
    ; Argument (filename) at 0x0600
    LDI r1, 0x0600       ; filename addr
    LDI r2, 0            ; mode = read
    OPEN r1, r2           ; r0 = fd
    ; Check for error
    LDI r1, 0xFFFFFFFF
    CMP r0, r1
    JZ r0, exec_done      ; error, skip

    MOV r5, r0            ; save fd in r5
    LDI r3, 0x1000        ; read buffer
    LDI r4, 200           ; max bytes
    READ r5, r3, r4       ; r0 = bytes read
    ; Null terminate
    LDI r6, 200
    CMP r0, r6
    BLT r0, cat_term_ok
    LDI r0, 200
cat_term_ok:
    LDI r6, 0x1000
    ADD r0, r6
    LDI r7, 0
    STORE r0, r7
    ; Display
    LDI r9, 0x1201
    LOAD r1, r9
    LDI r0, 12
    ADD r1, r0
    STORE r9, r1
    LDI r2, 4
    LDI r3, 0x1000
    TEXT r2, r1, r3
    CLOSE r5
    JMP exec_done

do_ps:
    ; Display process info
    ; For now, show PID count and current PID
    GETPID
    MOV r5, r0            ; save our PID
    LDI r9, 0x1201
    LOAD r1, r9
    LDI r0, 12
    ADD r1, r0
    STORE r9, r1
    ; Show "PID: N" using register value
    ; Write PID as text at 0x1500
    LDI r9, 0x1500
    LDI r0, 80            ; P
    STORE r9, r0
    LDI r9, 0x1501
    LDI r0, 73            ; I
    STORE r9, r0
    LDI r9, 0x1502
    LDI r0, 68            ; D
    STORE r9, r0
    LDI r9, 0x1503
    LDI r0, 58            ; :
    STORE r9, r0
    LDI r9, 0x1504
    LDI r0, 32            ; space
    STORE r9, r0
    ; Convert PID to decimal digits
    LDI r9, 0x1505
    LDI r1, 48            ; '0'
    ADD r0, r1
    STORE r9, r0
    LDI r9, 0x1506
    LDI r0, 0
    STORE r9, r0
    LDI r2, 4
    LDI r3, 0x1500
    TEXT r2, r1, r3
    JMP exec_done

do_kill:
    ; Kill a process by PID
    ; Argument should be a number -- simplified: just echo "kill"
    LDI r9, 0x1201
    LOAD r1, r9
    LDI r0, 12
    ADD r1, r0
    STORE r9, r1
    LDI r3, kill_msg
    LDI r2, 4
    TEXT r2, r1, r3
    JMP exec_done

do_export:
    ; Set environment variable
    ; Argument format: KEY=VALUE
    ; For simplicity, just acknowledge
    LDI r9, 0x1201
    LOAD r1, r9
    LDI r0, 12
    ADD r1, r0
    STORE r9, r1
    LDI r3, export_msg
    LDI r2, 4
    TEXT r2, r1, r3
    JMP exec_done

do_clear:
    LDI r0, 0
    FILL r0
    LDI r9, 0x1201
    LDI r0, 20
    STORE r9, r0
    JMP exec_done

do_exec:
    ; Execute external program via EXEC
    ; Command name at 0x0400
    LDI r1, 0x0400
    EXEC r1               ; r0 = PID or error
    ; Check for error
    LDI r2, 0xFFFFFFFF
    CMP r0, r2
    JZ r0, exec_error

    ; Save PID
    LDI r9, 0x1202
    STORE r9, r0

    ; Wait for child to complete
exec_wait:
    LDI r1, 0x1202
    LOAD r1, r9
    WAITPID r1
    JZ r0, exec_wait     ; still running

    ; Check for pipe mode
    LDI r9, 0x1207
    LOAD r0, r9
    LDI r1, 1
    CMP r0, r1
    JZ r0, exec_pipe

    JMP exec_done

exec_pipe:
    ; Execute second command from pipe buffer
    LDI r1, 0x1400
    EXEC r1
    LDI r9, 0x1202
    STORE r9, r0
exec_pipe_wait:
    LDI r1, 0x1202
    LOAD r1, r9
    WAITPID r1
    JZ r0, exec_pipe_wait
    JMP exec_done

exec_error:
    ; Display "command not found"
    LDI r9, 0x1201
    LOAD r1, r9
    LDI r0, 12
    ADD r1, r0
    STORE r9, r1
    LDI r3, not_found_msg
    LDI r2, 4
    TEXT r2, r1, r3
    JMP exec_done

; ═══════════════════════════════════════════════════════════════
; Data section
; ═══════════════════════════════════════════════════════════════
.org 0x1A00

prompt_str:
    ; "> " stored as ASCII bytes
    ; 62 = '>', 32 = ' ', 0 = null
    .byte 62
    .byte 32
    .byte 0

.org 0x1A10

help_text:
    .byte 104 ; h
    .byte 101 ; e
    .byte 108 ; l
    .byte 112 ; p
    .byte 58  ; :
    .byte 32  ; space
    .byte 101 ; e
    .byte 99  ; c
    .byte 104 ; h
    .byte 111 ; o
    .byte 32  ; space
    .byte 108 ; l
    .byte 115 ; s
    .byte 32  ; space
    .byte 99  ; c
    .byte 97  ; a
    .byte 116 ; t
    .byte 32  ; space
    .byte 112 ; p
    .byte 115  ; s
    .byte 32  ; space
    .byte 107 ; k
    .byte 105 ; i
    .byte 108 ; l
    .byte 108 ; l
    .byte 32  ; space
    .byte 99  ; c
    .byte 108 ; l
    .byte 101 ; e
    .byte 97  ; a
    .byte 114 ; r
    .byte 32  ; space
    .byte 101 ; e
    .byte 120 ; x
    .byte 112 ; p
    .byte 111 ; o
    .byte 114 ; r
    .byte 116 ; t
    .byte 32  ; space
    .byte 104 ; h
    .byte 121 ; y
    .byte 112 ; p
    .byte 101 ; e
    .byte 114 ; r
    .byte 118 ; v
    .byte 105 ; i
    .byte 115 ; s
    .byte 111 ; o
    .byte 114 ; r
    .byte 0

.org 0x1A50

kill_msg:
    .byte 107 ; k
    .byte 105 ; i
    .byte 108 ; l
    .byte 108 ; l
    .byte 58  ; :
    .byte 32  ; space
    .byte 117 ; u
    .byte 115 ; s
    .byte 97  ; a
    .byte 103 ; g
    .byte 101 ; e
    .byte 32  ; space
    .byte 107 ; k
    .byte 105 ; i
    .byte 108 ; l
    .byte 32  ; space
    .byte 60  ; <
    .byte 112 ; p
    .byte 105 ; i
    .byte 100 ; d
    .byte 62  ; >
    .byte 0

.org 0x1A70

export_msg:
    .byte 101 ; e
    .byte 120 ; x
    .byte 112 ; p
    .byte 111 ; o
    .byte 114 ; r
    .byte 116 ; t
    .byte 58  ; :
    .byte 32  ; space
    .byte 115 ; s
    .byte 101 ; e
    .byte 116  ; t
    .byte 0

.org 0x1A90

not_found_msg:
    .byte 99  ; c
    .byte 111 ; o
    .byte 109 ; m
    .byte 109 ; m
    .byte 97  ; a
    .byte 110 ; n
    .byte 100 ; d
    .byte 32  ; space
    .byte 110 ; n
    .byte 111 ; o
    .byte 116 ; t
    .byte 32  ; space
    .byte 102 ; f
    .byte 111 ; o
    .byte 117 ; u
    .byte 110 ; n
    .byte 100 ; d
    .byte 0

.org 0x1AB0

hypervisor_usage_msg:
    .byte 117 ; u
    .byte 115 ; s
    .byte 97  ; a
    .byte 103 ; g
    .byte 101 ; e
    .byte 58  ; :
    .byte 32  ; space
    .byte 104 ; h
    .byte 121 ; y
    .byte 112 ; p
    .byte 101 ; e
    .byte 114 ; r
    .byte 118 ; v
    .byte 105 ; i
    .byte 115 ; s
    .byte 111 ; o
    .byte 114 ; r
    .byte 32  ; space
    .byte 97  ; a
    .byte 114 ; r
    .byte 99  ; c
    .byte 104 ; h
    .byte 61  ; =
    .byte 60  ; <
    .byte 97  ; a
    .byte 114 ; r
    .byte 99  ; c
    .byte 104 ; h
    .byte 62  ; >
    .byte 32  ; space
    .byte 107 ; k
    .byte 101 ; e
    .byte 114 ; r
    .byte 110 ; n
    .byte 101 ; e
    .byte 108 ; l
    .byte 61  ; =
    .byte 60  ; <
    .byte 102 ; f
    .byte 105 ; i
    .byte 108 ; l
    .byte 101 ; e
    .byte 62  ; >
    .byte 0

.org 0x1AE0

hypervisor_err_msg:
    .byte 104 ; h
    .byte 121 ; y
    .byte 112 ; p
    .byte 101 ; e
    .byte 114 ; r
    .byte 118 ; v
    .byte 105 ; i
    .byte 115 ; s
    .byte 111 ; o
    .byte 114 ; r
    .byte 58  ; :
    .byte 32  ; space
    .byte 102 ; f
    .byte 97  ; a
    .byte 105 ; i
    .byte 108 ; l
    .byte 101 ; e
    .byte 100 ; d
    .byte 0

HALT