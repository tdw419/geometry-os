# geometry_os - infinite_map.asm learnings

## Architecture
- Geometry OS is a Rust codebase: assembler + VM in `src/` (assembler.rs, vm.rs, etc.)
- Programs are `.asm` files in `programs/` -- they assemble to bytecode that runs in the VM
- Tests are Rust integration tests in `tests/program_tests.rs` (255 passing)
- Test pattern: `compile_run("programs/foo.asm")` assembles + runs, then check screen/register state
- No Python anywhere in the project

## infinite_map.asm specifics
- File: `programs/infinite_map.asm` (317 lines)
- Infinite scrolling procedural terrain renderer
- 64x64 tile viewport, 4px tiles = 256x256 pixel display
- Uses two-level hash: coarse hash for biomes (8x8 zones), fine hash for structure placement
- 6 biome types: water (3 subtypes, animated), beach, grass (3 subtypes), forest (2 subtypes), mountain (3 subtypes), snow (3 subtypes)
- Structures: 1/256 tiles get a tree/rock/crystal overlay based on biome
- Controls: arrow keys / WASD via key bitmask at RAM[0xFFB]
- Camera stored at RAM[0x7800..0x7802]
- ~210K instructions/frame (21% of 1M budget)
- Currently NO test exists for this program

## Potential improvements (for daemon tasks)
- Write a test (verify it assembles, runs, produces non-black pixels, camera moves on key input)
- Add diagonal scrolling (currently 4-directional only)
- Add more biomes (desert, swamp, lava fields)
- Optimize render loop (skip off-screen calculations)
- Add animated structures (tree sway, water wave is there but trees are static)
- Add day/night cycle based on camera_x (east = dawn, west = dusk)
- Add player marker / cursor on the map
- Minimap overlay showing camera position in the world
