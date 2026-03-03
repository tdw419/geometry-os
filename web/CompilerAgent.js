/**
 * GPU-Native Compiler Agent
 * 
 * Manages the self-hosting visual compiler that runs as a GPU process.
 * This agent provides the interface between the VisualIDE and the
 * GPU-native compiler (compiler.wgsl).
 * 
 * The compiler enables Geometry OS to compile itself - a key milestone
 * for self-hosting systems.
 */

// Compiler status codes
const COMPILER_STATUS = {
    IDLE: 0,
    COMPILING: 1,
    SUCCESS: 2,
    ERROR: 3
};

// Compiler commands
const COMPILER_COMMAND = {
    NONE: 0,
    COMPILE: 1,
    GET_STATUS: 2,
    RESET: 3
};

// Error codes
const COMPILER_ERROR = {
    NONE: 0,
    UNKNOWN_OPCODE: 1,
    INVALID_OPERAND: 2,
    STACK_OVERFLOW: 3,
    SYNTAX_ERROR: 4
};

// Glyph to opcode mapping (mirrors compiler.wgsl)
const GLYPH_TO_OPCODE = {
    // Arithmetic
    0x6A: 'OP_FADD',   // ⊕ addition
    0x6B: 'OP_FSUB',   // ⊖ subtraction
    0x6C: 'OP_FMUL',   // ⊗ multiplication
    0x6D: 'OP_FDIV',   // ⊘ division
    
    // Memory
    0x10: 'OP_STORE',  // → store
    0x11: 'OP_LOAD',   // ← load
    
    // Math functions
    0x70: 'OP_SIN',    // sin
    0x71: 'OP_COS',    // cos
    
    // Control flow
    0x90: 'OP_JMP',    // ⤴ jump
    0x91: 'OP_JZ',     // ⤵ jump if zero
    0x93: 'OP_CALL',   // ⚙ call
    0x94: 'OP_RET',    // ↩ return
    
    // Stack
    0x00: 'OP_NOP',    // empty
    0xFF: 'OP_HALT'    // halt
};

export class CompilerAgent {
    constructor(device, options = {}) {
        this.device = device;
        
        // Grid dimensions
        this.gridWidth = options.gridWidth || 64;
        this.gridHeight = options.gridHeight || 64;
        
        // Buffer sizes
        this.inputSize = this.gridWidth * this.gridHeight * 4 * 4;  // RGBA u32 per pixel
        this.outputSize = 64 * 1024;  // 64KB for generated SPIR-V
        this.symbolSize = 16 * 1024;  // 16KB for symbols
        this.errorSize = 4 * 1024;    // 4KB for errors
        
        // GPU buffers
        this.inputBuffer = null;
        this.outputBuffer = null;
        this.symbolBuffer = null;
        this.errorBuffer = null;
        this.controlBuffer = null;
        
        // Pipeline
        this.pipeline = null;
        this.bindGroup = null;
        
        // Compilation state
        this.status = COMPILER_STATUS.IDLE;
        this.lastOutput = null;
        this.errors = [];
    }
    
    /**
     * Initialize the compiler agent.
     */
    async init() {
        // Create input buffer (visual grid)
        this.inputBuffer = this.device.createBuffer({
            size: this.inputSize,
            usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
        });
        
        // Create output buffer (generated SPIR-V)
        this.outputBuffer = this.device.createBuffer({
            size: this.outputSize,
            usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC,
        });
        
        // Create symbol table buffer
        this.symbolBuffer = this.device.createBuffer({
            size: this.symbolSize,
            usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC,
        });
        
        // Create error log buffer
        this.errorBuffer = this.device.createBuffer({
            size: this.errorSize,
            usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC,
        });
        
        // Create control buffer
        this.controlBuffer = this.device.createBuffer({
            size: 32,  // ControlBlock size
            usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST | GPUBufferUsage.COPY_SRC,
        });
        
        // Load and compile the compiler shader
        await this._loadPipeline();
        
        // Initialize control block
        this._resetControl();
        
        console.log('[CompilerAgent] GPU-native compiler initialized');
    }
    
    /**
     * Load the compiler pipeline.
     */
    async _loadPipeline() {
        // Fetch the compiler shader
        const response = await fetch('compiler.wgsl');
        const shaderCode = await response.text();
        
        const shaderModule = this.device.createShaderModule({
            code: shaderCode
        });
        
        // Create bind group layout
        const bindGroupLayout = this.device.createBindGroupLayout({
            entries: [
                { binding: 0, visibility: GPUShaderStage.COMPUTE, buffer: { type: 'read-only-storage' } },
                { binding: 1, visibility: GPUShaderStage.COMPUTE, buffer: { type: 'storage' } },
                { binding: 2, visibility: GPUShaderStage.COMPUTE, buffer: { type: 'storage' } },
                { binding: 3, visibility: GPUShaderStage.COMPUTE, buffer: { type: 'storage' } },
                { binding: 4, visibility: GPUShaderStage.COMPUTE, buffer: { type: 'storage' } },
            ]
        });
        
        // Create pipeline
        this.pipeline = this.device.createComputePipeline({
            layout: this.device.createPipelineLayout({
                bindGroupLayouts: [bindGroupLayout]
            }),
            compute: {
                module: shaderModule,
                entryPoint: 'main'
            }
        });
        
        // Create bind group
        this.bindGroup = this.device.createBindGroup({
            layout: bindGroupLayout,
            entries: [
                { binding: 0, resource: { buffer: this.inputBuffer } },
                { binding: 1, resource: { buffer: this.outputBuffer } },
                { binding: 2, resource: { buffer: this.symbolBuffer } },
                { binding: 3, resource: { buffer: this.errorBuffer } },
                { binding: 4, resource: { buffer: this.controlBuffer } },
            ]
        });
    }
    
    /**
     * Reset control block.
     */
    _resetControl() {
        const control = new Uint32Array(8);
        control[0] = COMPILER_COMMAND.NONE;  // command
        control[1] = 0;                       // input_size
        control[2] = 0;                       // output_size
        control[3] = COMPILER_STATUS.IDLE;   // status
        control[4] = 0;                       // error_count
        control[5] = this.gridWidth;         // grid_width
        control[6] = this.gridHeight;        // grid_height
        control[7] = 0;                       // padding
        
        this.device.queue.writeBuffer(this.controlBuffer, 0, control);
    }
    
    /**
     * Load a visual grid into the input buffer.
     * @param {ImageData|Uint8ClampedArray|Uint32Array} grid - The visual grid
     */
    loadGrid(grid) {
        let inputData;
        
        if (grid instanceof ImageData) {
            // Convert ImageData to RGBA u32 format
            inputData = this._convertImageData(grid.data);
        } else if (grid instanceof Uint8ClampedArray) {
            inputData = this._convertImageData(grid);
        } else {
            inputData = grid;
        }
        
        this.device.queue.writeBuffer(this.inputBuffer, 0, inputData);
    }
    
    /**
     * Convert RGBA bytes to u32 array.
     */
    _convertImageData(rgba) {
        const pixels = this.gridWidth * this.gridHeight;
        const output = new Uint32Array(pixels * 4);
        
        for (let i = 0; i < pixels; i++) {
            const src = i * 4;
            const dst = i * 4;
            
            output[dst + 0] = rgba[src + 0];  // R
            output[dst + 1] = rgba[src + 1];  // G (opcode)
            output[dst + 2] = rgba[src + 2];  // B
            output[dst + 3] = rgba[src + 3];  // A
        }
        
        return output;
    }
    
    /**
     * Compile the loaded visual grid.
     * @returns {Promise<{success: boolean, spirv: ArrayBuffer, errors: Array}>}
     */
    async compile() {
        return new Promise((resolve, reject) => {
            // Set command to compile
            const control = new Uint32Array(8);
            control[0] = COMPILER_COMMAND.COMPILE;
            control[5] = this.gridWidth;
            control[6] = this.gridHeight;
            this.device.queue.writeBuffer(this.controlBuffer, 0, control);
            
            // Dispatch compiler
            const commandEncoder = this.device.createCommandEncoder();
            const passEncoder = commandEncoder.beginComputePass();
            passEncoder.setPipeline(this.pipeline);
            passEncoder.setBindGroup(0, this.bindGroup);
            passEncoder.dispatchWorkgroups(1);
            passEncoder.end();
            
            this.device.queue.submit([commandEncoder.finish()]);
            
            // Poll for completion
            this._pollCompletion(resolve);
        });
    }
    
    /**
     * Poll for compilation completion.
     */
    async _pollCompletion(resolve) {
        // Read control buffer
        const control = await this._readBuffer(this.controlBuffer, 32);
        const view = new DataView(control);
        
        const status = view.getUint32(12, true);
        
        if (status === COMPILER_STATUS.COMPILING) {
            // Still compiling, poll again
            requestAnimationFrame(() => this._pollCompletion(resolve));
            return;
        }
        
        // Compilation complete
        this.status = status;
        
        if (status === COMPILER_STATUS.SUCCESS) {
            // Read generated SPIR-V
            const outputSize = view.getUint32(8, true);
            const spirv = await this._readBuffer(this.outputBuffer, outputSize);
            
            this.lastOutput = spirv;
            
            resolve({
                success: true,
                spirv: spirv,
                size: outputSize,
                errors: []
            });
        } else {
            // Read errors
            const errorCount = view.getUint32(16, true);
            const errors = await this._readErrors(errorCount);
            
            this.errors = errors;
            
            resolve({
                success: false,
                spirv: null,
                size: 0,
                errors: errors
            });
        }
    }
    
    /**
     * Read errors from error buffer.
     */
    async _readErrors(count) {
        if (count === 0) return [];
        
        const errorData = await this._readBuffer(this.errorBuffer, count * 16);
        const view = new DataView(errorData);
        const errors = [];
        
        for (let i = 0; i < count; i++) {
            const offset = i * 16;
            errors.push({
                code: view.getUint32(offset + 0, true),
                line: view.getUint32(offset + 4, true),
                column: view.getUint32(offset + 8, true),
                opcode: view.getUint32(offset + 12, true),
                message: this._errorMessage(view.getUint32(offset + 0, true))
            });
        }
        
        return errors;
    }
    
    /**
     * Get error message for code.
     */
    _errorMessage(code) {
        const messages = {
            [COMPILER_ERROR.NONE]: 'No error',
            [COMPILER_ERROR.UNKNOWN_OPCODE]: 'Unknown opcode',
            [COMPILER_ERROR.INVALID_OPERAND]: 'Invalid operand',
            [COMPILER_ERROR.STACK_OVERFLOW]: 'Stack overflow',
            [COMPILER_ERROR.SYNTAX_ERROR]: 'Syntax error'
        };
        return messages[code] || 'Unknown error';
    }
    
    /**
     * Read a GPU buffer.
     */
    async _readBuffer(buffer, size) {
        const staging = this.device.createBuffer({
            size: size,
            usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST
        });
        
        const commandEncoder = this.device.createCommandEncoder();
        commandEncoder.copyBufferToBuffer(buffer, 0, staging, 0, size);
        this.device.queue.submit([commandEncoder.finish()]);
        
        await staging.mapAsync(GPUMapMode.READ);
        const data = staging.getMappedRange().slice(0);
        staging.unmap();
        staging.destroy();
        
        return data;
    }
    
    /**
     * Get the last compiled SPIR-V as a downloadable blob.
     */
    getDownloadableSPIRV() {
        if (!this.lastOutput) return null;
        
        return new Blob([this.lastOutput], { type: 'application/octet-stream' });
    }
    
    /**
     * Convert visual grid to a simple format for display.
     */
    visualizeGrid(grid) {
        const pixels = this.gridWidth * this.gridHeight;
        const result = [];
        
        for (let i = 0; i < pixels; i++) {
            const offset = i * 4;
            const r = grid[offset + 0];
            const g = grid[offset + 1];
            const b = grid[offset + 2];
            
            const opcode = GLYPH_TO_OPCODE[g] || `CONST(${g})`;
            
            if (g !== 0) {
                result.push({
                    index: i,
                    x: i % this.gridWidth,
                    y: Math.floor(i / this.gridWidth),
                    rgb: `rgb(${r},${g},${b})`,
                    opcode: opcode
                });
            }
        }
        
        return result;
    }
    
    /**
     * Create a simple test grid with a basic program.
     * This creates a visual program that computes 2 + 3.
     */
    createTestGrid() {
        const grid = new Uint32Array(this.gridWidth * this.gridHeight * 4);
        
        // Pixel 0: Push constant 2.0
        // R=0x0000, G=0x0000 (constant), B=float bits for 2.0
        const two = new Float32Array([2.0]);
        const twoBits = new Uint32Array(two.buffer);
        grid[0] = 0;              // R (unused for constant)
        grid[1] = 0;              // G = 0 means constant
        grid[2] = twoBits[0];     // B = 2.0 as bits
        grid[3] = 0;
        
        // Pixel 1: Push constant 3.0
        const three = new Float32Array([3.0]);
        const threeBits = new Uint32Array(three.buffer);
        grid[4] = 0;
        grid[5] = 0;              // G = 0 means constant
        grid[6] = threeBits[0];   // B = 3.0 as bits
        grid[7] = 0;
        
        // Pixel 2: Add (opcode 0x6A)
        grid[8] = 0;
        grid[9] = 0x6A;           // G = OP_FADD
        grid[10] = 0;
        grid[11] = 0;
        
        // Pixel 3: Halt
        grid[12] = 0;
        grid[13] = 0xFF;          // G = OP_HALT
        grid[14] = 0;
        grid[15] = 0;
        
        return grid;
    }
    
    /**
     * Run a self-test compilation.
     */
    async selfTest() {
        console.log('[CompilerAgent] Running self-test...');
        
        // Create test grid
        const testGrid = this.createTestGrid();
        this.loadGrid(testGrid);
        
        // Compile
        const result = await this.compile();
        
        if (result.success) {
            console.log('[CompilerAgent] Self-test PASSED');
            console.log(`  Generated ${result.size} bytes of SPIR-V`);
        } else {
            console.log('[CompilerAgent] Self-test FAILED');
            console.log(`  Errors: ${result.errors.length}`);
            result.errors.forEach(e => {
                console.log(`    Line ${e.line}, Col ${e.column}: ${e.message}`);
            });
        }
        
        return result;
    }
}

export { COMPILER_STATUS, COMPILER_COMMAND, COMPILER_ERROR, GLYPH_TO_OPCODE };
