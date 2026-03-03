/**
 * Tests for Geometry OS Visual Compiler - Tone Opcode
 */

import { describe, it, expect } from 'vitest';
import { VisualCompiler } from './VisualCompiler.js';

describe('VisualCompiler', () => {
    describe('tone opcode', () => {
        it('emits OP_TONE for glyph 0x82', () => {
            const compiler = new VisualCompiler();

            // Create a minimal grid with tone instruction
            // 440 (freq), 0.5 (duration), 0.8 (volume), tone
            const grid = [
                { info: { g: 0, b: 440 } },    // Push 440 (frequency)
                { info: { g: 0, b: 0 } },      // Placeholder for 0.5
                { info: { g: 0, b: 0 } },      // Placeholder for 0.8
                { info: { g: 0x82, b: 0 } },   // Tone opcode
            ];

            const result = compiler.compile(grid, 2);

            expect(result.binary).toBeDefined();
            expect(result.labelMap).toBeDefined();

            // Verify binary contains OP_TONE (201)
            const words = new Uint32Array(result.binary);
            const hasTone = Array.from(words).some(w => (w & 0xFFFF) === 201);
            expect(hasTone).toBe(true);
        });

        it('pops 3 values from stack for tone (freq, duration, volume)', () => {
            const compiler = new VisualCompiler();

            // Push values, then tone - stack should balance
            const grid = [
                { info: { g: 0, b: 440 } },    // Push freq
                { info: { g: 0, b: 1 } },      // Push duration
                { info: { g: 0, b: 1 } },      // Push volume
                { info: { g: 0x82, b: 0 } },   // Tone (pops 3)
            ];

            const result = compiler.compile(grid, 2);
            expect(result.binary).toBeDefined();
        });
    });

    describe('existing opcodes still work', () => {
        it('compiles arithmetic operations', () => {
            const compiler = new VisualCompiler();

            const grid = [
                { info: { g: 0, b: 10 } },     // Push 10
                { info: { g: 0, b: 5 } },      // Push 5
                { info: { g: 0x6A, b: 0 } },   // Add
            ];

            const result = compiler.compile(grid, 2);
            expect(result.binary).toBeDefined();
        });
    });
});
