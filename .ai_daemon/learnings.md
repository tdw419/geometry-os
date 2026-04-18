# geometry_os - Verified Project State

*Last updated: 2025-04-18. All counts verified by scanning the repo.*

## Architecture
- Geometry OS is a Rust codebase: assembler + VM in `src/` (assembler.rs, vm.rs, etc.)
- Programs are `.asm` files in `programs/` -- they assemble to bytecode that runs in the VM
- Test pattern: `compile_run("programs/foo.asm")` assembles + runs, then check screen/register state
- No Python anywhere in the project

## Test Suite (1093 total, 0 failing, 2 ignored)
- **Lib unit tests** (in src/): 490 passing
- **Integration tests** (tests/program_tests/*.rs): 275 passing
- **RISC-V tests** (tests/riscv_tests/*.rs): 147 passing
- **Signal tests** (tests/signal_tests.rs): 20 passing
- **QEMU boot test**: 1 ignored (requires QEMU installed)
- **Hypervisor QEMU boot**: 1 ignored (requires QEMU installed)
- Tests broken into 14 modules: basic_programs(24), opcodes(35), games(29), devices(29), shell(28), kernel(27), multiprocess(22), self_host(14), hypervisor(13), boot(11), filesystem(10), ipc(15), scheduling(9), vm_state(9)

## Programs: 50 .asm files, 16,826 total lines

### Programs >200 lines (verified line counts)
| Program | Lines | Tests | Notes |
|---------|-------|-------|-------|
| code_evolution.asm | 5,705 | 0 | Largest .asm. Self-modifying evolution demo. UNTESTED. |
| tetris.asm | 1,783 | 2 | Full tetris game |
| shell.asm | 1,248 | 2 | Interactive shell with pipes, redirection |
| infinite_map.asm | 1,153 | 20 | See detailed breakdown below |
| maze.asm | 775 | 3 | Maze generation + solving |
| breakout.asm | 698 | 2 | Breakout game |
| infinite_map_pxpk.asm | 669 | 7 | Pixelpack seed variant of infinite_map |
| living_map.asm | 625 | 5 | Stateful world: footstep trails, wandering creatures |
| snake.asm | 396 | 1 | Snake game |
| calculator.asm | 247 | 2 | Add/subtract with text display |
| game_of_life.asm | 231 | 0 | Conway's Game of Life. UNTESTED. |
| register_dashboard.asm | 208 | 1 | Register visualization |
| window_manager.asm | 205 | 4 | Multi-process window demo |
| stdlib_test.asm | 199 | 0 | Standard library test harness. UNTESTED. |
| device_test.asm | 160 | 0 | Device driver test harness. UNTESTED. |

### 12 Programs with Zero Test Coverage
| Program | Lines | Risk |
|---------|-------|------|
| code_evolution.asm | 5,705 | HIGH -- largest program, no regression protection |
| game_of_life.asm | 231 | MEDIUM |
| stdlib_test.asm | 199 | LOW (test harness itself) |
| device_test.asm | 160 | LOW (test harness itself) |
| multiproc.asm | 110 | MEDIUM -- multi-process demo |
| sprint_c_test.asm | 94 | LOW (test harness) |
| pipe_test.asm | 91 | LOW (test harness) |
| pipe_demo.asm | 61 | LOW |
| preprocessor_advanced_test.asm | 25 | LOW (preprocessor test) |
| canvas_grid_writer.asm | 21 | LOW |
| preprocessor_test.asm | 19 | LOW (preprocessor test) |
| canvas_counter.asm | 17 | LOW |

## infinite_map.asm -- Detailed State (1,153 lines)

### What exists (all verified in code)
- **64x64 tile viewport**, 4px tiles = 256x256 screen
- **21 biomes** across 32 color table entries (0x7A00-0x7A1F):
  deep ocean, shallow water, beach, desert sand, desert dunes, oasis, grass light/dark, swamp light/dark, forest light/dark, mushroom, mountain rock, mountain snow, tundra, lava flowing/cooled, volcanic, snow light/ice/peak, coral, ruins, crystal dark/dense, ash, deadlands light/dark, biolum light/dark, void
- **32-entry pattern table** (0x7900-0x791F): 8 pattern types (horizontal, vertical, center, corner, diagonal\, diagonal/, top edge, dither scatter)
- **BPE/LINEAR color variation**: 16-entry table at 0x7B00 for per-tile multi-channel variation (256 unique color combos)
- **Day/night tint**: camera_x position shifts color warmth (west=cool, east=warm)
- **Player cursor**: pulsing white/yellow crosshair at screen center (127,127)
- **Minimap overlay**: 16x16 top-right corner, 10-category dimmed color map
- **Diagonal scrolling**: key bitmask bits 4-7 allow single-key diagonal movement
- **Animated water shimmer**: biome 0-1 get frame_counter-driven blue channel animation
- **Camera**: stored at RAM[0x7800..0x7802], arrow/WASD/diagonal control via key bitmask at RAM[0xFFB]
- **Instruction budget**: ~410K instructions/frame (41% of 1M budget)

### 20 tests for infinite_map
test_infinite_map_assembles, test_infinite_map_pxpk_assembles, test_infinite_map_pxpk_runs_and_renders, test_infinite_map_pxpk_camera_moves, test_infinite_map_pxpk_pattern_variety, test_infinite_map_pxpk_step_budget, test_infinite_map_pxpk_tint_phase_analysis, test_infinite_map_pxpk_day_night_tint, test_infinite_map_runs_and_renders, test_infinite_map_camera_moves_on_key_input, test_infinite_map_camera_moves_multiple_steps, test_infinite_map_screen_differs_per_camera_position, test_infinite_map_diagonal_keys_move_camera, test_infinite_map_diagonal_accumulates, test_infinite_map_cardinal_and_diagonal_combined, test_infinite_map_render_loop_instruction_count, test_infinite_map_player_cursor_visible, test_infinite_map_player_cursor_pulses, test_infinite_map_minimap_overlay, test_infinite_map_diagonal_scroll

## living_map.asm (625 lines, 5 tests)
- Stateful infinite world built on infinite_map terrain engine
- Footstep trails via ring buffer
- 3 wandering creatures with random walk AI
- Player marker
- Single-process simulation using subroutines (not SPAWN)
- Tests: test_living_map_assembles, test_living_map_runs, test_living_map_draws_terrain, test_living_map_draws_player, test_living_map_footstep_trail

## Real Next-Step Opportunities (not yet implemented)

### High value
1. **Write tests for code_evolution.asm** -- 5,705 lines with zero regression protection. Even a basic assembles+runs+non-black test would catch breakage.
2. **Write tests for game_of_life.asm** -- 231 lines, untested. Grid simulation is easy to assert.
3. **Write tests for multiproc.asm** -- Multi-process demo with no test coverage.
4. **Reduce infinite_map instruction budget** -- Currently 410K/frame (41% of 1M). Table lookups and batched rendering could free headroom for new features.
5. **Fix GUI mode bytecode base offset** -- Assembler resolves labels from address 0 but GUI loads at 0x1000. Programs with backward JMPs break in GUI. Either load at 0 or add base_offset to assembler.

### Medium value
6. **Terrain transitions** -- Biome boundaries are hard edges. A 1-tile gradient between adjacent biomes would look much better.
7. **Weather system** -- Overlay particles (rain in swamps, snow in tundra, ash in volcanic) using the existing accent pixel infrastructure.
8. **Audio landscape** -- Use BEEP opcode tied to biome type (deep hum near lava, wind in tundra).
9. **Save/load world state** -- Camera position persists between sessions via the existing F7 save state.
10. **code_evolution.asm optimization** -- At 5,705 lines it's the largest program. May benefit from table-lookup patterns like infinite_map uses.

### Low priority / polish
11. **Triple-view inspector** -- Show byte value, color block, and glyph for cursor cell (designed but not implemented).
12. **Character literals in assembler** -- Allow `'A'` instead of requiring `65`.
13. **Key buffer for IKEY** -- Currently only one key per frame; rapid typing drops keys.
14. **Fix checkerboard.asm** -- Line 28 has "invalid number: r4" assembly error.
15. **Snake self-collision** -- snake.asm doesn't check if the snake overlaps its own body.
