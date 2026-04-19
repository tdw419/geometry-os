# Geometry OS Programs

This directory contains assembly programs for the Geometry OS virtual machine. You can load and run these programs from the CLI using `load <name>` and `run`, or by typing them directly onto the canvas in the GUI.

## Games
- **breakout.asm**: Classic brick-breaker game. Move paddle with A/D or arrows, launch ball with W or Space.
- **maze.asm**: Randomly generated maze navigation. WASD to move, R to restart with a new maze.
- **roguelike.asm**: Procedural dungeon crawler with random room placement, L-shaped corridors, WASD movement, and stairs to descend deeper.
- **snake.asm**: Snake game on a 32x32 grid. WASD to control direction.
- **tetris.asm**: Full Tetris implementation. A/D to move, W to rotate, S to soft drop.

## Demos & Animations
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

## System & Tools
- **cat.asm**: Reads "hello.txt" from the virtual filesystem and displays it on screen. Demonstrates `OPEN`, `READ`, `CLOSE`, and `TEXT` opcodes.
- **self_host.asm**: The ultimate VM test. This program contains assembly source in RAM, uses the `ASM` opcode to compile itself into bytecode, and then executes the result.
- **calculator.asm**: Basic add/subtract calculator with text display using `TEXT` and `IKEY`.
- **painter.asm**: Keyboard-controlled drawing tool. WASD to move cursor, Space to paint.
- **blink.asm**: Demonstrates `CMP` and keyboard input by toggling a pixel on/off.

## Technical Tests
- **push_pop_test.asm**: Verifies stack operations using `r30` as the Stack Pointer.
- **shift_test.asm**: Verifies bitwise `SHL` and `SHR` logic.
- **sprint_c_test.asm**: Comprehensive test for `MOD`, `PUSH/POP`, and `BLT/BGE` branch logic.
