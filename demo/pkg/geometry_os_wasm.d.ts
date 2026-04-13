/* tslint:disable */
/* eslint-disable */

/**
 * The main WASM interface for Geometry OS.
 *
 * Usage from JavaScript:
 * ```js
 * const geo = new GeometryOS(canvas);
 * geo.load("hello.asm");
 * geo.run();
 * ```
 */
export class GeometryOS {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Disassemble instruction at an address. Returns (mnemonic, byte_length).
     */
    disassemble_at(addr: number): any[];
    /**
     * Get the BEEP frequency/duration if pending, returns null if no beep.
     * Returns [freq, duration] as a JsValue array, or null.
     */
    get_beep(): any;
    /**
     * Get the frame count.
     */
    get_frame_count(): number;
    /**
     * Get the current PC.
     */
    get_pc(): number;
    /**
     * Get the current value of a register (0-31).
     */
    get_reg(reg: number): number;
    /**
     * Check if the VM is halted.
     */
    is_halted(): boolean;
    /**
     * Clear the keyboard port (simulates IKEY clearing it).
     */
    key_clear(): void;
    /**
     * Set a key press (for IKEY opcode). The VM reads RAM[0xFFF].
     * key_code should be the ASCII/scan code value.
     */
    key_press(key_code: number): void;
    /**
     * Assemble and load a program from source text.
     * Returns Ok(bytecode_length) on success, Err(error_message) on failure.
     */
    load(source: string): number;
    /**
     * Create a new Geometry OS instance bound to an HTML canvas.
     * The canvas will be scaled up for visibility.
     */
    constructor(canvas_id: string);
    /**
     * Read a word from RAM.
     */
    peek(addr: number): number;
    /**
     * Write a word to RAM.
     */
    poke(addr: number, value: number): void;
    /**
     * Reset the VM to a clean state.
     */
    reset(): void;
    /**
     * Run one VM step. Returns false when halted.
     */
    step(): boolean;
    /**
     * Run the VM until it halts or FRAME fires.
     * Call this in a requestAnimationFrame loop.
     * Returns true if the VM is still running.
     */
    tick(): boolean;
}

/**
 * Assemble source code and return the bytecode length (or error).
 * Convenience function for one-shot assembly without creating a VM.
 */
export function assemble(source: string): number;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_geometryos_free: (a: number, b: number) => void;
    readonly assemble: (a: number, b: number) => [number, number, number];
    readonly geometryos_disassemble_at: (a: number, b: number) => [number, number];
    readonly geometryos_get_beep: (a: number) => any;
    readonly geometryos_get_frame_count: (a: number) => number;
    readonly geometryos_get_pc: (a: number) => number;
    readonly geometryos_get_reg: (a: number, b: number) => number;
    readonly geometryos_is_halted: (a: number) => number;
    readonly geometryos_key_clear: (a: number) => void;
    readonly geometryos_key_press: (a: number, b: number) => void;
    readonly geometryos_load: (a: number, b: number, c: number) => [number, number, number];
    readonly geometryos_new: (a: number, b: number) => [number, number, number];
    readonly geometryos_peek: (a: number, b: number) => number;
    readonly geometryos_poke: (a: number, b: number, c: number) => void;
    readonly geometryos_reset: (a: number) => void;
    readonly geometryos_step: (a: number) => number;
    readonly geometryos_tick: (a: number) => number;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __externref_drop_slice: (a: number, b: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
