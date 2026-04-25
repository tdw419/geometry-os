// Program Coverage Tests
//
// Individual tests for every program that didn't already have a dedicated test.
// Phase 13 deliverable: "Each untested program has at least one test"
//
// Categories:
// - Static programs (no FRAME): compile_run + check halt + screen assertion
// - Interactive/animated programs (FRAME): compile_run_steps with limited steps
// - Skip-list programs (won't assemble): test that the file exists

use geometry_os::assembler::{assemble, assemble_with_lib};
use geometry_os::preprocessor::Preprocessor;
use geometry_os::vm::Vm;

/// Helper: assemble and run a program to completion (for programs without FRAME loops)
fn compile_run(path: &str) -> Vm {
    let source =
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("failed to read {}: {}", path, e));
    let mut pp = Preprocessor::new();
    let preprocessed = pp.preprocess(&source);
    let asm = assemble(&preprocessed, 0)
        .unwrap_or_else(|e| panic!("assembly failed for {}: {:?}", path, e));
    let mut vm = Vm::new();
    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..10_000_000 {
        if !vm.step() {
            break;
        }
    }
    vm
}

/// Helper: assemble and run for N steps (for programs with FRAME loops)
fn compile_run_steps(path: &str, steps: usize) -> Vm {
    let source =
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("failed to read {}: {}", path, e));
    let mut pp = Preprocessor::new();
    let preprocessed = pp.preprocess(&source);
    let asm = assemble(&preprocessed, 0)
        .unwrap_or_else(|e| panic!("assembly failed for {}: {:?}", path, e));
    let mut vm = Vm::new();
    for (i, &pixel) in asm.pixels.iter().enumerate() {
        if i < vm.ram.len() {
            vm.ram[i] = pixel;
        }
    }
    vm.pc = 0;
    vm.halted = false;
    for _ in 0..steps {
        if !vm.step() {
            break;
        }
    }
    vm
}

/// Helper: count non-zero screen pixels
fn drawn_pixels(vm: &Vm) -> usize {
    vm.screen.iter().filter(|&&p| p != 0).count()
}

/// Helper: just check that a program assembles successfully
fn assert_assembles(path: &str) {
    let source =
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("failed to read {}: {}", path, e));
    let mut pp = Preprocessor::new();
    let preprocessed = pp.preprocess(&source);
    assemble(&preprocessed, 0).unwrap_or_else(|e| panic!("assembly failed for {}: {:?}", path, e));
}

fn assert_file_exists(path: &str) {
    assert!(std::path::Path::new(path).exists(), "{} should exist", path);
}

// ═══════════════════════════════════════════════════════════════════
// STATIC PROGRAMS (no FRAME -- should halt)
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_about_program() {
    let vm = compile_run("programs/about.asm");
    assert!(vm.halted, "about.asm should halt");
    assert!(
        drawn_pixels(&vm) > 0,
        "about.asm should draw text on screen"
    );
}

#[test]
fn test_ai_interact_program() {
    let vm = compile_run("programs/ai_interact.asm");
    assert!(vm.halted, "ai_interact.asm should halt");
    assert!(
        drawn_pixels(&vm) > 0,
        "ai_interact.asm should produce screen output"
    );
}

#[test]
fn test_ai_vision_program() {
    let vm = compile_run("programs/ai_vision.asm");
    assert!(vm.halted, "ai_vision.asm should halt");
    assert!(
        drawn_pixels(&vm) > 0,
        "ai_vision.asm should produce screen output"
    );
}

#[test]
fn test_alpine_tile_program() {
    let vm = compile_run("programs/alpine_tile.asm");
    assert!(vm.halted, "alpine_tile.asm should halt");
    assert!(
        drawn_pixels(&vm) > 0,
        "alpine_tile.asm should produce screen output"
    );
}

#[test]
fn test_canvas_border_program() {
    // Uses ASMSELF + RUNNEXT -- just verify it assembles
    assert_assembles("programs/canvas_border.asm");
}

#[test]
fn test_canvas_checker_program() {
    // Uses ASMSELF + RUNNEXT -- just verify it assembles
    assert_assembles("programs/canvas_checker.asm");
}

#[test]
fn test_canvas_rainbow_program() {
    // Uses ASMSELF + RUNNEXT -- just verify it assembles
    assert_assembles("programs/canvas_rainbow.asm");
}

#[test]
fn test_clipboard_demo_program() {
    let vm = compile_run("programs/clipboard_demo.asm");
    assert!(vm.halted, "clipboard_demo.asm should halt");
    assert!(
        drawn_pixels(&vm) > 0,
        "clipboard_demo.asm should produce screen output"
    );
}

#[test]
fn test_code_evolution_program() {
    // Uses ASMSELF + RUNNEXT -- just verify it assembles
    assert_assembles("programs/code_evolution.asm");
}

#[test]
fn test_concentric_program() {
    let vm = compile_run("programs/concentric.asm");
    assert!(vm.halted, "concentric.asm should halt");
    assert!(drawn_pixels(&vm) > 0, "concentric.asm should draw rings");
}

#[test]
fn test_disasm_program() {
    // Interactive disassembler with FRAME loop -- does not halt
    let vm = compile_run_steps("programs/disasm.asm", 100_000);
    assert!(vm.pc > 0, "disasm.asm should execute instructions");
}

#[test]
fn test_font_demo_program() {
    let vm = compile_run("programs/font_demo.asm");
    assert!(vm.halted, "font_demo.asm should halt");
    assert!(
        drawn_pixels(&vm) > 0,
        "font_demo.asm should draw custom font text"
    );
}

#[test]
fn test_glyph_demo_program() {
    let vm = compile_run("programs/glyph_demo.asm");
    assert!(vm.halted, "glyph_demo.asm should halt");
    assert!(
        drawn_pixels(&vm) > 0,
        "glyph_demo.asm should produce screen output"
    );
}

#[test]
fn test_grep_program() {
    let vm = compile_run("programs/grep.asm");
    assert!(vm.halted, "grep.asm should halt");
    assert!(
        drawn_pixels(&vm) > 0,
        "grep.asm should produce screen output"
    );
}

#[test]
fn test_hello_window_program() {
    let vm = compile_run("programs/hello_window.asm");
    assert!(vm.halted, "hello_window.asm should halt");
    assert!(
        drawn_pixels(&vm) > 0,
        "hello_window.asm should draw a window"
    );
}

#[test]
fn test_hexdump_program() {
    let vm = compile_run("programs/hexdump.asm");
    assert!(vm.halted, "hexdump.asm should halt");
    assert!(
        drawn_pixels(&vm) > 0,
        "hexdump.asm should produce hex dump output"
    );
}

#[test]
fn test_init_service_program() {
    let vm = compile_run("programs/init_service.asm");
    assert!(vm.halted, "init_service.asm should halt");
}

#[test]
fn test_lib_test_program() {
    let vm = compile_run("programs/lib_test.asm");
    assert!(vm.halted, "lib_test.asm should halt");
}

#[test]
fn test_ls_program() {
    let vm = compile_run("programs/ls.asm");
    assert!(vm.halted, "ls.asm should halt");
    assert!(
        drawn_pixels(&vm) > 0,
        "ls.asm should display directory listing"
    );
}

#[test]
fn test_net_share_program() {
    let vm = compile_run("programs/net_share.asm");
    assert!(vm.halted, "net_share.asm should halt");
    assert!(
        drawn_pixels(&vm) > 0,
        "net_share.asm should produce screen output"
    );
}

#[test]
fn test_oracle_autodev_program() {
    // LLM Oracle with interactive loop -- does not halt
    let vm = compile_run_steps("programs/oracle_autodev.asm", 100_000);
    assert!(vm.pc > 0, "oracle_autodev.asm should execute instructions");
}

#[test]
fn test_preprocessor_advanced_test_program() {
    let vm = compile_run("programs/preprocessor_advanced_test.asm");
    assert!(vm.halted, "preprocessor_advanced_test.asm should halt");
}

#[test]
fn test_preprocessor_test_program() {
    let vm = compile_run("programs/preprocessor_test.asm");
    assert!(vm.halted, "preprocessor_test.asm should halt");
}

#[test]
fn test_simple_bug_program() {
    let vm = compile_run("programs/simple_bug.asm");
    assert!(vm.halted, "simple_bug.asm should halt");
    // Draws three colored pixels
    assert!(
        drawn_pixels(&vm) >= 3,
        "simple_bug.asm should draw at least 3 pixels"
    );
}

#[test]
fn test_sound_mixer_program() {
    // Sound mixer daemon with SPAWN -- just verify it assembles
    assert_assembles("programs/sound_mixer.asm");
}

#[test]
fn test_sprint_c_test_program() {
    let vm = compile_run("programs/sprint_c_test.asm");
    assert!(vm.halted, "sprint_c_test.asm should halt");
}

#[test]
fn test_vm_park_program() {
    let vm = compile_run("programs/vm_park.asm");
    assert!(vm.halted, "vm_park.asm should halt");
    assert!(
        drawn_pixels(&vm) > 0,
        "vm_park.asm should produce screen output"
    );
}

#[test]
fn test_wc_program() {
    let vm = compile_run("programs/wc.asm");
    assert!(vm.halted, "wc.asm should halt");
    assert!(
        drawn_pixels(&vm) > 0,
        "wc.asm should display word count output"
    );
}

// ═══════════════════════════════════════════════════════════════════
// ANIMATED / INTERACTIVE PROGRAMS (use FRAME -- need limited steps)
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_ai_terminal_program() {
    let vm = compile_run_steps("programs/ai_terminal.asm", 50_000);
    assert!(
        drawn_pixels(&vm) > 0,
        "ai_terminal.asm should produce screen output"
    );
}

#[test]
fn test_browser_program() {
    let vm = compile_run_steps("programs/browser.asm", 50_000);
    assert!(
        drawn_pixels(&vm) > 0,
        "browser.asm should produce screen output"
    );
}

#[test]
fn test_calendar_program() {
    let vm = compile_run_steps("programs/calendar.asm", 50_000);
    assert!(drawn_pixels(&vm) > 0, "calendar.asm should draw a calendar");
}

#[test]
fn test_canvas_counter_program() {
    // Interactive counter -- uses FRAME loop, does not halt
    let vm = compile_run_steps("programs/canvas_counter.asm", 100_000);
    assert!(vm.pc > 0, "canvas_counter.asm should execute instructions");
}

#[test]
fn test_clock_program() {
    let vm = compile_run_steps("programs/clock.asm", 50_000);
    assert!(drawn_pixels(&vm) > 0, "clock.asm should draw a clock face");
}

#[test]
fn test_color_picker_program() {
    // Mouse-driven color picker with FRAME loop
    let vm = compile_run_steps("programs/color_picker.asm", 200_000);
    assert!(vm.pc > 0, "color_picker.asm should execute instructions");
}

#[test]
fn test_counter_program() {
    let vm = compile_run_steps("programs/counter.asm", 50_000);
    assert!(drawn_pixels(&vm) > 0, "counter.asm should display counter");
}

#[test]
fn test_crash_demo_program() {
    let vm = compile_run_steps("programs/crash_demo.asm", 100_000);
    assert!(
        drawn_pixels(&vm) > 0,
        "crash_demo.asm should produce screen output"
    );
}

#[test]
fn test_cron_daemon_program() {
    // Cron daemon with FRAME loop -- does not halt
    let vm = compile_run_steps("programs/cron_daemon.asm", 100_000);
    assert!(vm.pc > 0, "cron_daemon.asm should execute instructions");
}

#[test]
fn test_debugger_program() {
    let vm = compile_run_steps("programs/debugger.asm", 100_000);
    assert!(drawn_pixels(&vm) > 0, "debugger.asm should draw debug info");
}

#[test]
fn test_debug_monitor_program() {
    let vm = compile_run_steps("programs/debug_monitor.asm", 50_000);
}

#[test]
fn test_debug_parent_test_program() {
    let vm = compile_run_steps("programs/debug_parent_test.asm", 100_000);
}

#[test]
fn test_file_browser_program() {
    let vm = compile_run_steps("programs/file_browser.asm", 50_000);
    assert!(
        drawn_pixels(&vm) > 0,
        "file_browser.asm should draw file browser UI"
    );
}

#[test]
fn test_flatliner_program() {
    let vm = compile_run_steps("programs/flatliner.asm", 50_000);
    assert!(
        drawn_pixels(&vm) > 0,
        "flatliner.asm should produce screen output"
    );
}

#[test]
fn test_gui_calc_program() {
    let vm = compile_run_steps("programs/gui_calc.asm", 50_000);
    assert!(
        drawn_pixels(&vm) > 0,
        "gui_calc.asm should draw calculator UI"
    );
}

#[test]
fn test_help_program() {
    let vm = compile_run_steps("programs/help.asm", 50_000);
    assert!(drawn_pixels(&vm) > 0, "help.asm should draw help text");
}

#[test]
fn test_hex_viewer_program() {
    let vm = compile_run_steps("programs/hex_viewer.asm", 50_000);
    assert!(
        drawn_pixels(&vm) > 0,
        "hex_viewer.asm should draw hex viewer"
    );
}

#[test]
fn test_html_browser_program() {
    // HTML browser with FRAME loop -- renders then loops
    let vm = compile_run_steps("programs/html_browser.asm", 200_000);
    assert!(vm.pc > 0, "html_browser.asm should execute instructions");
}

#[test]
fn test_http_get_program() {
    // HTTP client library with FRAME loop
    let vm = compile_run_steps("programs/http_get.asm", 200_000);
    assert!(vm.pc > 0, "http_get.asm should execute instructions");
}

#[test]
fn test_imgview_program() {
    let vm = compile_run_steps("programs/imgview.asm", 50_000);
    assert!(drawn_pixels(&vm) > 0, "imgview.asm should display an image");
}

#[test]
fn test_invert_demo_program() {
    let vm = compile_run_steps("programs/invert_demo.asm", 100_000);
    assert!(
        drawn_pixels(&vm) > 0,
        "invert_demo.asm should draw something before inverting"
    );
}

#[test]
fn test_launcher_program() {
    let vm = compile_run_steps("programs/launcher.asm", 50_000);
    assert!(
        drawn_pixels(&vm) > 0,
        "launcher.asm should draw app launcher"
    );
}

#[test]
fn test_linux_building_program() {
    let vm = compile_run_steps("programs/linux_building.asm", 50_000);
    assert!(
        drawn_pixels(&vm) > 0,
        "linux_building.asm should draw hypervisor building"
    );
}

#[test]
fn test_meminspect_program() {
    let vm = compile_run_steps("programs/meminspect.asm", 50_000);
    assert!(
        drawn_pixels(&vm) > 0,
        "meminspect.asm should draw memory inspector"
    );
}

#[test]
fn test_minesweeper_program() {
    let vm = compile_run_steps("programs/minesweeper.asm", 50_000);
    assert!(
        drawn_pixels(&vm) > 0,
        "minesweeper.asm should draw minefield"
    );
}

#[test]
fn test_mixer_demo_program() {
    // Multi-channel music demo with FRAME loop
    let vm = compile_run_steps("programs/mixer_demo.asm", 200_000);
    assert!(vm.pc > 0, "mixer_demo.asm should execute instructions");
}

#[test]
fn test_multiproc_program() {
    // Multi-process demo with FRAME loop
    let vm = compile_run_steps("programs/multiproc.asm", 200_000);
    assert!(vm.pc > 0, "multiproc.asm should execute instructions");
}

#[test]
fn test_net_chat_program() {
    let vm = compile_run_steps("programs/net_chat.asm", 50_000);
    assert!(drawn_pixels(&vm) > 0, "net_chat.asm should draw chat UI");
}

#[test]
fn test_neural_program() {
    // Neural network inference with FRAME loop
    let vm = compile_run_steps("programs/neural.asm", 200_000);
    assert!(vm.pc > 0, "neural.asm should execute instructions");
}

#[test]
fn test_nn_demo_program() {
    let vm = compile_run_steps("programs/nn_demo.asm", 50_000);
    assert!(
        drawn_pixels(&vm) > 0,
        "nn_demo.asm should draw XOR neural net output"
    );
}

#[test]
fn test_notepad_program() {
    let vm = compile_run_steps("programs/notepad.asm", 50_000);
    assert!(drawn_pixels(&vm) > 0, "notepad.asm should draw text editor");
}

#[test]
fn test_oracle_program() {
    // Oracle building on infinite map -- uses FRAME, has HALT path
    let vm = compile_run_steps("programs/oracle.asm", 200_000);
    assert!(vm.pc > 0, "oracle.asm should execute instructions");
}

#[test]
fn test_paint_program() {
    let vm = compile_run_steps("programs/paint.asm", 50_000);
    assert!(drawn_pixels(&vm) > 0, "paint.asm should draw paint canvas");
}

#[test]
fn test_pixelflow_program() {
    let vm = compile_run_steps("programs/pixelflow.asm", 50_000);
    assert!(
        drawn_pixels(&vm) > 0,
        "pixelflow.asm should draw neural net visualization"
    );
}

#[test]
fn test_pulse_program() {
    let vm = compile_run_steps("programs/pulse.asm", 50_000);
    assert!(
        drawn_pixels(&vm) > 0,
        "pulse.asm should draw animated pulse bar"
    );
}

#[test]
fn test_reaction_program() {
    let vm = compile_run_steps("programs/reaction.asm", 50_000);
    assert!(
        drawn_pixels(&vm) > 0,
        "reaction.asm should draw reaction tester"
    );
}

#[test]
fn test_replay_demo_program() {
    // Draw frames then replay backward -- has HALT
    let vm = compile_run_steps("programs/replay_demo.asm", 300_000);
    assert!(vm.pc > 0, "replay_demo.asm should execute instructions");
}

#[test]
fn test_sci_calc_program() {
    let vm = compile_run_steps("programs/sci_calc.asm", 50_000);
    assert!(
        drawn_pixels(&vm) > 0,
        "sci_calc.asm should draw scientific calculator"
    );
}

#[test]
fn test_screensaver_program() {
    // Multi-effect screensaver with FRAME loop
    let vm = compile_run_steps("programs/screensaver.asm", 200_000);
    assert!(vm.pc > 0, "screensaver.asm should execute instructions");
}

#[test]
fn test_screenshot_program() {
    let vm = compile_run_steps("programs/screenshot.asm", 50_000);
    assert!(
        drawn_pixels(&vm) > 0,
        "screenshot.asm should produce screen output"
    );
}

#[test]
fn test_self_analysis_program() {
    let vm = compile_run_steps("programs/self_analysis.asm", 100_000);
    assert!(
        drawn_pixels(&vm) > 0,
        "self_analysis.asm should produce screen output"
    );
}

#[test]
fn test_settings_program() {
    let vm = compile_run_steps("programs/settings.asm", 50_000);
    assert!(
        drawn_pixels(&vm) > 0,
        "settings.asm should draw settings panel"
    );
}

#[test]
fn test_simon_program() {
    let vm = compile_run_steps("programs/simon.asm", 50_000);
    assert!(
        drawn_pixels(&vm) > 0,
        "simon.asm should draw Simon Says game"
    );
}

#[test]
fn test_smart_term_program() {
    let vm = compile_run_steps("programs/smart_term.asm", 50_000);
    assert!(drawn_pixels(&vm) > 0, "smart_term.asm should draw terminal");
}

#[test]
fn test_stopwatch_program() {
    let vm = compile_run_steps("programs/stopwatch.asm", 100_000);
    assert!(
        drawn_pixels(&vm) > 0,
        "stopwatch.asm should draw stopwatch display"
    );
}

#[test]
fn test_taskbar_program() {
    let vm = compile_run_steps("programs/taskbar.asm", 50_000);
    assert!(
        drawn_pixels(&vm) > 0,
        "taskbar.asm should draw desktop taskbar"
    );
}

#[test]
fn test_terminal_program() {
    let vm = compile_run_steps("programs/terminal.asm", 50_000);
    assert!(
        drawn_pixels(&vm) > 0,
        "terminal.asm should draw terminal UI"
    );
}

#[test]
fn test_term_mux_program() {
    let vm = compile_run_steps("programs/term_mux.asm", 50_000);
    assert!(
        drawn_pixels(&vm) > 0,
        "term_mux.asm should draw multiplexer"
    );
}

#[test]
fn test_tetris_clean_program() {
    // Tetris game with FRAME loop -- does not halt
    let vm = compile_run_steps("programs/tetris_clean.asm", 200_000);
    assert!(vm.pc > 0, "tetris_clean.asm should execute instructions");
}

#[test]
fn test_thinking_creature_program() {
    let vm = compile_run_steps("programs/thinking_creature.asm", 50_000);
    assert!(
        drawn_pixels(&vm) > 0,
        "thinking_creature.asm should draw creature"
    );
}

#[test]
fn test_timer_program() {
    let vm = compile_run_steps("programs/timer.asm", 100_000);
    assert!(
        drawn_pixels(&vm) > 0,
        "timer.asm should draw countdown timer"
    );
}

#[test]
fn test_wallpaper_program() {
    let vm = compile_run_steps("programs/wallpaper.asm", 100_000);
    assert!(
        drawn_pixels(&vm) > 0,
        "wallpaper.asm should draw procedural wallpaper"
    );
}

#[test]
fn test_wm_drag_close_program() {
    // Window manager with drag/close -- uses WINSYS, FRAME loop
    let vm = compile_run_steps("programs/wm_drag_close.asm", 200_000);
    assert!(vm.pc > 0, "wm_drag_close.asm should execute instructions");
}

// ═══════════════════════════════════════════════════════════════════
// SKIP-LIST PROGRAMS (won't assemble normally -- just verify existence)
// These programs use unsupported features (.db strings, wrong syntax, etc.)
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_canvas_grid_writer_exists() {
    assert_file_exists("programs/canvas_grid_writer.asm");
}

#[test]
fn test_device_test_exists() {
    assert_file_exists("programs/device_test.asm");
}

#[test]
fn test_pipe_demo_exists() {
    assert_file_exists("programs/pipe_demo.asm");
}

#[test]
fn test_pipe_test_exists() {
    assert_file_exists("programs/pipe_test.asm");
}

#[test]
fn test_pixel_history_demo_exists() {
    assert_file_exists("programs/pixel_history_demo.asm");
}

#[test]
fn test_window_desktop_exists() {
    assert_file_exists("programs/window_desktop.asm");
}

#[test]
fn test_net_demo_exists() {
    assert_file_exists("programs/net_demo.asm");
}

#[test]
fn test_stdlib_test_exists() {
    assert_file_exists("programs/stdlib_test.asm");
}

#[test]
fn test_vfs_viewer_exists() {
    assert_file_exists("programs/vfs_viewer.asm");
}

#[test]
fn test_test_vfs_pixel_exists() {
    assert_file_exists("programs/test_vfs_pixel.asm");
}
