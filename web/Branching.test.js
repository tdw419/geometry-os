/**
 * Tests for Geometry OS Visual Compiler - Branching Opcodes
 *
 * Tests JMP, JZ, JNZ for conditional execution and loops
 */

import { describe, it, expect } from 'vitest';
import { VisualCompiler } from './VisualCompiler.js';

describe('VisualCompiler Branching', () => {
    describe('JNZ opcode (0x45)', () => {
        it('emits OP_JNZ for glyph 0x45', () => {
            const compiler = new VisualCompiler();

            // Push 1, JNZ to label 1
            const grid = [
                { info: { g: 0, b: 1 } },      // Push 1 (true)
                { info: { g: 0x45, b: 1 } },   // JNZ label 1
                { info: { g: 0x46, b: 1 } },   // Label 1
            ];

            const result = compiler.compile(grid, 2);
            const words = new Uint32Array(result.binary);

            // Verify JNZ (200) is in the binary
            const hasJnz = Array.from(words).some(w => (w & 0xFFFF) === 200);
            expect(hasJnz).toBe(true);
        });
    });

    describe('JMP opcode (0x47)', () => {
        it('emits OP_JMP for glyph 0x47', () => {
            const compiler = new VisualCompiler();

            const grid = [
                { info: { g: 0x47, b: 1 } },   // JMP label 1
                { info: { g: 0x46, b: 1 } },   // Label 1
            ];

            const result = compiler.compile(grid, 2);
            const words = new Uint32Array(result.binary);

            // Verify JMP (202) is in the binary
            const hasJmp = Array.from(words).some(w => (w & 0xFFFF) === 202);
            expect(hasJmp).toBe(true);
        });
    });

    describe('JZ opcode (0x48)', () => {
        it('emits OP_JZ for glyph 0x48', () => {
            const compiler = new VisualCompiler();

            const grid = [
                { info: { g: 0, b: 0 } },      // Push 0 (false)
                { info: { g: 0x48, b: 1 } },   // JZ label 1
                { info: { g: 0x46, b: 1 } },   // Label 1
            ];

            const result = compiler.compile(grid, 2);
            const words = new Uint32Array(result.binary);

            // Verify JZ (203) is in the binary
            const hasJz = Array.from(words).some(w => (w & 0xFFFF) === 203);
            expect(hasJz).toBe(true);
        });
    });

    describe('conditional if-then-else', () => {
        it('compiles: if 10 > 5 then push 100 else push 200', () => {
            const compiler = new VisualCompiler();

            // Visual program: if 10 > 5 then push 100 else push 200
            // Using a 4x4 grid with Hilbert curve ordering
            const grid = new Array(16).fill(null).map(() => ({ info: { g: 0, b: 0 } }));

            // Hilbert order for 4x4 (order=2):
            // d=0: (0,0), d=1: (1,0), d=2: (1,1), d=3: (0,1)
            // d=4: (0,2), d=5: (0,3), d=6: (1,3), d=7: (1,2)
            // d=8: (2,2), d=9: (2,3), d=10: (3,3), d=11: (3,2)
            // d=12: (3,1), d=13: (3,0), d=14: (2,0), d=15: (2,1)

            // Position glyphs in Hilbert order
            const program = [
                { g: 0, b: 10 },      // d=0: Push 10
                { g: 0, b: 5 },       // d=1: Push 5
                { g: 0x10, b: 0 },    // d=2: > (10 > 5 = 1.0)
                { g: 0x48, b: 2 },    // d=3: JZ label 2 (skip to ELSE if false)
                { g: 0, b: 100 },     // d=4: Push 100 (THEN branch)
                { g: 0x47, b: 3 },    // d=5: JMP label 3 (skip ELSE)
                { g: 0x46, b: 2 },    // d=6: Label 2 (ELSE)
                { g: 0, b: 200 },     // d=7: Push 200
                { g: 0x46, b: 3 },    // d=8: Label 3 (END)
                { g: 0x72, b: 0 },    // d=9: Store to RAM[0]
            ];

            // Fill grid in Hilbert positions
            const hilbertPositions = [
                [0,0], [1,0], [1,1], [0,1],
                [0,2], [0,3], [1,3], [1,2],
                [2,2], [2,3], [3,3], [3,2],
                [3,1], [3,0], [2,0], [2,1]
            ];

            for (let i = 0; i < program.length; i++) {
                const [x, y] = hilbertPositions[i];
                const idx = y * 4 + x;
                grid[idx] = { info: program[i] };
            }

            const result = compiler.compile(grid, 4);

            // Verify binary contains all expected opcodes
            const words = new Uint32Array(result.binary);
            const opcodes = Array.from(words).map(w => w & 0xFFFF);

            expect(opcodes.includes(43)).toBe(true);   // CONSTANT
            expect(opcodes.includes(182)).toBe(true);  // FGreaterThan
            expect(opcodes.includes(203)).toBe(true);  // JZ
            expect(opcodes.includes(202)).toBe(true);  // JMP
            expect(opcodes.includes(248)).toBe(true);  // LABEL
            expect(opcodes.includes(62)).toBe(true);   // STORE

            // Verify label map has correct labels
            expect(result.labelMap).toBeDefined();
        });
    });
});
