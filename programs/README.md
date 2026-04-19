# Geometry OS Programs

This directory contains assembly programs for the Geometry OS virtual machine. You can load and run these programs from the CLI using `load <name>` and `run`, or by typing them directly onto the canvas in the GUI.

## Games
- **breakout.asm**: Classic brick-breaker game. Move paddle with A/D or arrows, launch ball with W or Space.
- **maze.asm**: Randomly generated maze navigation. WASD to move, R to restart with a new maze.
- **roguelike.asm**: Procedural dungeon crawler with random room placement, L-shaped corridors, WASD movement, and stairs to descend deeper. Combat, enemies, items, and time-travel undo via FORK opcode.
- **snake.asm**: Snake game on a 32x32 grid. WASD to control direction.
- **tetris.asm**: Full Tetris implementation. A/D to move, W to rotate, S to soft drop.

## Demos & Animations
- **mandelbrot.asm**: Mandelbrot set fractal renderer using fixed-point (8.8) arithmetic. Full 256x256 rendering with psychedelic color palette based on escape-time coloring.
- **sprite_demo.asm**: Interactive 8x8 pixel-art character. Demonstrates `SPRITE` transparency, gravity, and floor collision. WASD to move/jump.
- **ball.asm**: Bouncing ball with keyboard interaction (WASD). Demonstrates `CIRCLE` and `BEEP`.
- **fire.asm**: Procedural scrolling fire animation using `SCROLL` and `FRAME`.
- **rainbow.asm**: Diagonal rainbow pattern using `MOD` and a double loop.
- **rings.asm**: Concentric colored rings emanating from the center using Manhattan distance.
- **hello.asm**: "Hello, World!" string built in RAM and rendered using the `TEXT` opcode.
- **circles.asm**: Concentric circles with cycling colors.
- **lines.asm**: A starburst pattern demonstrating the Bresenham `LINE` opcode.
- **scroll_demo.asm**: Simple demonstration of the `SCROLL` hardware capability.
- **gradient.asm**: Horizontal color gradient using `PSET`.
- **checkerboard.asm**: 8x8 alternating black and white squares.
- **colors.asm**: Fills the screen with multiple horizontal color bands.
- **nested_rects.asm**: Concentric colored rectangles using `RECTF`.
- **stripes.asm**: Alternating horizontal red and blue stripes.
- **diagonal.asm**: Draws a single diagonal line from (0,0) to (255,255).
- **fill_screen.asm**: Basic test that fills the entire screen with a solid color.
- **border.asm**: Draws a colored border (green) around all four screen edges using `RECTF`.
- **particles.asm**: 100 colored pixels that drift, bounce off screen edges, and fade over time. Proves `FRAME` + pixel write performance for real-time effects.
- **plasma.asm**: Classic plasma demo effect. Smoothly animated color waves across 256x256 using a precomputed sine table and phase-shifted lookups.
- **starfield.asm**: 3D starfield zoom effect. Stars stream toward the viewer from a central vanishing point using perspective projection.
- **wirecube.asm**: 3D rotating wireframe cube with perspective projection and depth-based edge coloring. Uses parabolic sin/cos approximation for rotation math.
- **peek_bounce.asm**: `PEEK`-based collision demo. A white ball bounces off drawn obstacles by reading pixel colors from the screen buffer directly. No RAM-based collision map -- the screen IS the state.

## World & Terrain
- **infinite_map.asm**: Infinite scrolling procedural terrain (v10). Arrow keys/WASD scroll through procedurally generated biomes (water, sand, grass, forest, snow, lava) using two-level hash. Includes day/night cycle, weather (rain, snow, lightning), elevation contours, water reflections, tree sprites, and mini-map overlay.
- **infinite_map_pxpk.asm**: Pixelpack seed-driven infinite terrain. Evolution of infinite_map.asm using table-driven expansion instead of cascading CMP/BLT for biome color selection.
- **living_map.asm**: Stateful infinite world extending infinite_map.asm with sparse state overlay (footstep trails), wandering creatures with AI, and living weather system.
- **game_of_life.asm**: Conway's Game of Life on a 64x64 grid (4x4 pixel cells). Uses `PEEK` to read neighbors and RAM back buffer. The screen IS the cellular automaton -- pure pixel-driven-pixels.

## Sound
- **music_demo.asm**: Plays "Mary Had a Little Lamb" using square wave `NOTE` opcode, with `FRAME` sync for visual bars showing each note's frequency.
- **sfx_demo.asm**: Retro sound effects catalog. Plays a sequence of notes in all 5 waveforms (sine, square, triangle, sawtooth, noise) with visual bars.

## System & Tools
- **cat.asm**: Reads "hello.txt" from the virtual filesystem and displays it on screen. Demonstrates `OPEN`, `READ`, `CLOSE`, and `TEXT` opcodes.
- **self_host.asm**: The ultimate VM test. This program contains assembly source in RAM, uses the `ASM` opcode to compile itself into bytecode, and then executes the result.
- **calculator.asm**: Basic add/subtract calculator with text display using `TEXT` and `IKEY`.
- **painter.asm**: Keyboard-controlled drawing tool. WASD to move cursor, Space to paint.
- **blink.asm**: Demonstrates `CMP` and keyboard input by toggling a pixel on/off.
- **shell.asm**: Interactive command shell. Built-in commands: help, echo, ls, cat, ps, kill, export, clear. Supports pipe operator, redirection, and script execution.
- **init.asm**: Init process (PID 1). Started by the boot ROM, reads boot.cfg, spawns the shell and configured services, respawns the shell on crash (supervisor pattern).
- **device_test.asm**: Device driver abstraction demo (Phase 28). Opens all 4 device files, uses `IOCTL` to query them, reads from /dev/keyboard, writes pixels to /dev/screen. Press any key to draw.
- **window_manager.asm**: Geometric Window Manager demo. Demonstrates the Window Bounds Protocol with movable/resizable windows on the canvas.

## IPC & Multiprocess
- **pipe_demo.asm**: Pipe, `MSGSND`/`MSGRCV`, and blocking I/O demo (Phase 27).
- **pipe_test.asm**: Pipe test. Parent creates a pipe, spawns a child, writes 3 color words through the pipe. Child reads them and draws 3 colored rows.
- **multiproc.asm**: `SPATIAL_SPAWN` demo. Primary process draws a bouncing white dot in the left half; spawned process draws a bouncing red dot in the right half. Both share the same screen.

## Self-Modifying & Reactive Canvas
- **self_writer.asm**: A program that writes a NEW program to the canvas, compiles it with `ASMSELF`, and runs it with `RUNNEXT`. Generation 1 sets r1 to 42 and halts.
- **code_evolution.asm**: The crown jewel. A program that writes increasingly complex versions of itself. Each generation writes its successor, compiles, and runs it. At least 3 generations of visible code evolution.
- **canvas_counter.asm**: Increments a digit on the canvas grid each frame. Demonstrates that the grid IS the display.
- **canvas_grid_writer.asm**: Writes "PIXELS DRIVE PIXELS" to the canvas grid using `STORE` to 0x8000+ addresses.
- **evolving_counter.asm**: Pixel-driving-pixels demo. Live frame counter on the canvas grid, reading `TICKS` from RAM[0xFFE] and writing ASCII digits to the grid.
- **register_dashboard.asm**: Phase 50 live register dashboard. Canvas grid shows register values as 4-digit decimals that update in real time.

## Time-Travel Debugger
- **replay_demo.asm**: Phase 38c demo. Uses `SNAP_TRACE` to record, `FRAME` to checkpoint, then `REPLAY` to display frames in reverse.
- **fork_demo.asm**: Phase 38d timeline forking demo. Visual proof that `FORK` save/restore rewinds all RAM and registers while the snapshots Vec survives.

## Preprocessor & Standard Library
- **preprocessor_test.asm**: Tests the assembler preprocessor abstraction layer (`#define`, `.str`, `.lib`).
- **preprocessor_advanced_test.asm**: Tests register safety and `INC`/`DEC` preprocessor macros.
- **stdlib_test.asm**: Tests standard library functions: strlen, strcpy, strcmp, memset, itoa, print_str, print_int. Also tests `.str` directive and `.lib`/`.include` linking.

## Procedural Generation
- **maze_gen.asm**: Static procedural maze generator. Generates a solvable maze on the 256x256 screen using depth-first search with backtracking. 16x16 cell grid with white walls and green entrance/exit markers.

## Technical Tests
- **push_pop_test.asm**: Verifies stack operations using `r30` as the Stack Pointer.
- **shift_test.asm**: Verifies bitwise `SHL` and `SHR` logic.
- **sprint_c_test.asm**: Comprehensive test for `MOD`, `PUSH/POP`, and `BLT/BGE` branch logic.
