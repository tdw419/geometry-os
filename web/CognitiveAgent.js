/**
 * Cognitive Debug Agent
 * 
 * Integrates Geometry OS with the claude_gemini_auto_prompting toolkit.
 * When a process triggers an error (SIGSEGV, etc.), the kernel writes the
 * fault context to a "Cognitive Mailbox". This agent monitors the mailbox
 * and triggers an Autonomous Debug Loop.
 * 
 * This enables Geometry OS to "ask for help" when it encounters errors,
 * creating a self-improving system.
 */

// Error types
const ERROR_TYPE = {
    SIGSEGV: 11,      // Segmentation fault
    SIGFPE: 8,        // Floating point error
    SIGILL: 4,        // Illegal instruction
    SIGBUS: 7,        // Bus error
    SIGABRT: 6,       // Abort
    UNKNOWN: 0
};

// Debug states
const DEBUG_STATE = {
    IDLE: 0,
    ANALYZING: 1,
    PROMPTING: 2,
    FIXING: 3,
    VERIFYING: 4,
    COMPLETE: 5,
    FAILED: 6
};

// Cognitive mailbox structure (layout in GPU buffer)
// Layout: errorType(4), pid(4), pc(4), sp(4), memBase(4), memSize(4),
//         faultAddr(4), opcode(4), timestamp(4), processed(4), padding(12)
const MAILBOX_SIZE = 64;

// Debug context for LLM prompting
class DebugContext {
    constructor() {
        this.errorType = 0;
        this.pid = 0;
        this.pc = 0;
        this.faultAddr = 0;
        this.opcode = 0;
        this.memoryDump = null;
        this.sourceCode = null;
        this.suggestions = [];
    }
    
    toPromptContext() {
        return {
            R: this._buildContext(),
            G: "DIAGNOSE and FIX the error",
            B: "Process " + this.pid
        };
    }
    
    _buildContext() {
        const errorNames = {
            [ERROR_TYPE.SIGSEGV]: 'SIGSEGV (Segmentation Fault)',
            [ERROR_TYPE.SIGFPE]: 'SIGFPE (Floating Point Error)',
            [ERROR_TYPE.SIGILL]: 'SIGILL (Illegal Instruction)',
            [ERROR_TYPE.SIGBUS]: 'SIGBUS (Bus Error)',
            [ERROR_TYPE.SIGABRT]: 'SIGABRT (Abort)'
        };
        
        return `
Error: ${errorNames[this.errorType] || 'Unknown Error'}
Process ID: ${this.pid}
Program Counter: 0x${this.pc.toString(16)}
Fault Address: 0x${this.faultAddr.toString(16)}
Opcode: 0x${this.opcode.toString(16)}
Memory Base: 0x${this.memBase?.toString(16) || '0'}
${this.sourceCode ? `\nSource Code:\n${this.sourceCode}` : ''}
${this.memoryDump ? `\nMemory Dump (first 64 bytes):\n${this._formatHexDump(this.memoryDump)}` : ''}
`.trim();
    }
    
    _formatHexDump(data) {
        const bytes = new Uint8Array(data);
        let result = '';
        for (let i = 0; i < Math.min(bytes.length, 64); i += 16) {
            const hex = Array.from(bytes.slice(i, i + 16))
                .map(b => b.toString(16).padStart(2, '0'))
                .join(' ');
            result += `${i.toString(16).padStart(4, '0')}: ${hex}\n`;
        }
        return result;
    }
}

export class CognitiveAgent {
    constructor(kernel, options = {}) {
        this.kernel = kernel;
        this.device = kernel.device;
        
        // Configuration
        this.autoFix = options.autoFix ?? false;  // Don't auto-fix by default
        this.maxRetries = options.maxRetries || 3;
        this.debugTimeout = options.debugTimeout || 30000;  // 30 seconds
        
        // GPU resources
        this.mailboxBuffer = null;
        this.responseBuffer = null;
        this.pipeline = null;
        this.bindGroup = null;
        
        // State
        this.state = DEBUG_STATE.IDLE;
        this.currentContext = null;
        this.debugHistory = [];
        this.fixCount = 0;
        this.failCount = 0;
        
        // Callbacks
        this.onDebugStart = options.onDebugStart || null;
        this.onDebugComplete = options.onDebugComplete || null;
        this.onFix = options.onFix || null;
        this.onPrompt = options.onPrompt || null;
        
        // External LLM integration
        this.llmEndpoint = options.llmEndpoint || null;
        this.apiKey = options.apiKey || null;
    }
    
    /**
     * Initialize the cognitive agent.
     */
    async init() {
        // Create mailbox buffer (shared with kernel)
        this.mailboxBuffer = this.device.createBuffer({
            size: 64,  // CognitiveMailbox size
            usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST | GPUBufferUsage.COPY_SRC,
        });
        
        // Create response buffer
        this.responseBuffer = this.device.createBuffer({
            size: 4096,  // Space for debug response
            usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST | GPUBufferUsage.COPY_SRC,
        });
        
        // Initialize mailbox to idle state
        const initMailbox = new Uint32Array(16);
        initMailbox[8] = 1;  // processed = done (nothing pending)
        this.device.queue.writeBuffer(this.mailboxBuffer, 0, initMailbox);
        
        console.log('[CognitiveAgent] Initialized with auto-fix:', this.autoFix);
    }
    
    /**
     * Write an error to the cognitive mailbox.
     * Called by the kernel when a process faults.
     */
    async reportError(errorType, pid, pc, faultAddr, opcode) {
        const mailbox = new Uint32Array(16);
        mailbox[0] = errorType;
        mailbox[1] = pid;
        mailbox[2] = pc;
        mailbox[5] = faultAddr;
        mailbox[6] = opcode;
        mailbox[7] = Date.now();
        mailbox[8] = 0;  // processed = pending
        
        this.device.queue.writeBuffer(this.mailboxBuffer, 0, mailbox);
        
        console.log(`[CognitiveAgent] Error reported: ${errorType} in PID ${pid}`);
        
        // Trigger debug loop
        if (this.autoFix) {
            await this.runDebugLoop();
        }
    }
    
    /**
     * Check if there's a pending error in the mailbox.
     */
    async checkMailbox() {
        const data = await this._readBuffer(this.mailboxBuffer, 64);
        const view = new DataView(data);
        
        const processed = view.getUint32(32, true);
        if (processed !== 0) {
            return null;  // No pending error
        }
        
        return {
            errorType: view.getUint32(0, true),
            pid: view.getUint32(4, true),
            pc: view.getUint32(8, true),
            sp: view.getUint32(12, true),
            memBase: view.getUint32(16, true),
            memSize: view.getUint32(20, true),
            faultAddr: view.getUint32(24, true),
            opcode: view.getUint32(28, true),
            timestamp: view.getUint32(36, true)
        };
    }
    
    /**
     * Run the autonomous debug loop.
     */
    async runDebugLoop() {
        if (this.state !== DEBUG_STATE.IDLE) {
            console.log('[CognitiveAgent] Debug loop already running');
            return;
        }
        
        const error = await this.checkMailbox();
        if (!error) {
            return;
        }
        
        this.state = DEBUG_STATE.ANALYZING;
        console.log('[CognitiveAgent] Starting debug loop for PID', error.pid);
        
        if (this.onDebugStart) {
            this.onDebugStart(error);
        }
        
        try {
            // Create debug context
            this.currentContext = new DebugContext();
            this.currentContext.errorType = error.errorType;
            this.currentContext.pid = error.pid;
            this.currentContext.pc = error.pc;
            this.currentContext.faultAddr = error.faultAddr;
            this.currentContext.opcode = error.opcode;
            this.currentContext.memBase = error.memBase;
            this.currentContext.memSize = error.memSize;
            
            // Gather more context
            await this._gatherContext(error);
            
            // Analyze the error
            this.state = DEBUG_STATE.ANALYZING;
            const analysis = await this._analyzeError();
            
            // Generate fix suggestions
            this.state = DEBUG_STATE.PROMPTING;
            const suggestions = await this._generateSuggestions(analysis);
            
            if (this.onPrompt) {
                this.onPrompt(this.currentContext.toPromptContext(), suggestions);
            }
            
            // Apply fix if auto-fix is enabled
            if (this.autoFix && suggestions.length > 0) {
                this.state = DEBUG_STATE.FIXING;
                const fixApplied = await this._applyFix(suggestions[0]);
                
                if (fixApplied) {
                    // Verify the fix
                    this.state = DEBUG_STATE.VERIFYING;
                    const verified = await this._verifyFix();
                    
                    if (verified) {
                        this.state = DEBUG_STATE.COMPLETE;
                        this.fixCount++;
                        console.log('[CognitiveAgent] Fix verified successfully');
                        
                        if (this.onFix) {
                            this.onFix(error.pid, suggestions[0], true);
                        }
                    } else {
                        this.state = DEBUG_STATE.FAILED;
                        this.failCount++;
                        console.log('[CognitiveAgent] Fix verification failed');
                    }
                } else {
                    this.state = DEBUG_STATE.FAILED;
                    this.failCount++;
                }
            } else {
                this.state = DEBUG_STATE.COMPLETE;
            }
            
            // Record in history
            this.debugHistory.push({
                timestamp: Date.now(),
                error: error,
                context: this.currentContext,
                suggestions: suggestions,
                result: this.state === DEBUG_STATE.COMPLETE ? 'success' : 'failed'
            });
            
        } catch (e) {
            this.state = DEBUG_STATE.FAILED;
            this.failCount++;
            console.error('[CognitiveAgent] Debug loop failed:', e);
        } finally {
            // Mark mailbox as processed
            const complete = new Uint32Array([1]);
            this.device.queue.writeBuffer(this.mailboxBuffer, 32, complete);
            
            if (this.onDebugComplete) {
                this.onDebugComplete(this.state);
            }
            
            this.state = DEBUG_STATE.IDLE;
            this.currentContext = null;
        }
    }
    
    /**
     * Gather additional context for debugging.
     */
    async _gatherContext(error) {
        // Try to read process memory
        if (this.kernel.memoryManager && error.memBase) {
            try {
                this.currentContext.memoryDump = await this._readProcessMemory(
                    error.memBase, 
                    Math.min(error.memSize, 256)
                );
            } catch (e) {
                console.log('[CognitiveAgent] Could not read process memory');
            }
        }
        
        // Try to get source code if available
        // (Would need to be implemented based on how source is stored)
    }
    
    /**
     * Analyze the error to determine root cause.
     */
    async _analyzeError() {
        const ctx = this.currentContext;
        
        const analysis = {
            category: 'unknown',
            severity: 'high',
            rootCause: null,
            suggestions: []
        };
        
        switch (ctx.errorType) {
            case ERROR_TYPE.SIGSEGV:
                analysis.category = 'memory';
                if (ctx.faultAddr < ctx.memBase || 
                    ctx.faultAddr >= ctx.memBase + ctx.memSize) {
                    analysis.rootCause = 'out_of_bounds';
                    analysis.suggestions.push('Check array bounds');
                    analysis.suggestions.push('Verify pointer arithmetic');
                } else {
                    analysis.rootCause = 'null_pointer';
                    analysis.suggestions.push('Check for null pointer dereference');
                    analysis.suggestions.push('Verify memory allocation');
                }
                break;
                
            case ERROR_TYPE.SIGFPE:
                analysis.category = 'arithmetic';
                analysis.rootCause = 'division_by_zero';
                analysis.suggestions.push('Add zero check before division');
                analysis.suggestions.push('Validate divisor operand');
                break;
                
            case ERROR_TYPE.SIGILL:
                analysis.category = 'instruction';
                analysis.rootCause = 'invalid_opcode';
                analysis.suggestions.push(`Opcode 0x${ctx.opcode.toString(16)} is not implemented`);
                analysis.suggestions.push('Check instruction encoding');
                break;
                
            case ERROR_TYPE.SIGBUS:
                analysis.category = 'memory';
                analysis.rootCause = 'alignment';
                analysis.suggestions.push('Check memory alignment');
                analysis.suggestions.push('Use aligned memory access');
                break;
        }
        
        return analysis;
    }
    
    /**
     * Generate fix suggestions using local analysis or external LLM.
     */
    async _generateSuggestions(analysis) {
        const suggestions = [];
        
        // Use local analysis first
        for (const suggestion of analysis.suggestions) {
            suggestions.push({
                type: 'code_change',
                description: suggestion,
                confidence: 0.7,
                autoApplicable: false
            });
        }
        
        // If external LLM is configured, get more detailed suggestions
        if (this.llmEndpoint && this.apiKey) {
            try {
                const llmSuggestions = await this._queryLLM(analysis);
                suggestions.push(...llmSuggestions);
            } catch (e) {
                console.log('[CognitiveAgent] LLM query failed:', e);
            }
        }
        
        return suggestions;
    }
    
    /**
     * Query external LLM for suggestions.
     */
    async _queryLLM(analysis) {
        const prompt = this.currentContext.toPromptContext();
        
        const response = await fetch(this.llmEndpoint, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'Authorization': `Bearer ${this.apiKey}`
            },
            body: JSON.stringify({
                prompt: `Analyze this GPU process error and suggest fixes:\n\n${prompt.R}\n\nTask: ${prompt.G}\nTarget: ${prompt.B}`,
                max_tokens: 500
            })
        });
        
        const data = await response.json();
        
        // Parse LLM response into suggestions
        // (Implementation depends on LLM response format)
        return [{
            type: 'llm_suggestion',
            description: data.choices?.[0]?.text || data.response || 'No suggestion',
            confidence: 0.8,
            autoApplicable: false
        }];
    }
    
    /**
     * Apply a fix suggestion.
     */
    async _applyFix(suggestion) {
        console.log('[CognitiveAgent] Applying fix:', suggestion.description);
        
        // Most fixes require human intervention, but some can be automated:
        if (suggestion.autoApplicable) {
            // Restart the process with modified memory/parameters
            // This would need to be implemented based on the specific fix type
            return false;
        }
        
        // Log the suggestion for human review
        return false;
    }
    
    /**
     * Verify that a fix was successful.
     */
    async _verifyFix() {
        // Check if the process can run without error
        // Would need to re-run the process and monitor for errors
        return false;
    }
    
    /**
     * Read process memory.
     */
    async _readProcessMemory(base, size) {
        // Would need to be implemented with kernel memory access
        return new ArrayBuffer(size);
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
     * Get debug statistics.
     */
    getStats() {
        return {
            totalDebugs: this.debugHistory.length,
            fixesApplied: this.fixCount,
            fixesFailed: this.failCount,
            successRate: this.debugHistory.length > 0 
                ? (this.fixCount / this.debugHistory.length * 100).toFixed(1) 
                : 0
        };
    }
    
    /**
     * Get current state.
     */
    getState() {
        return {
            state: this.state,
            stateName: Object.keys(DEBUG_STATE).find(k => DEBUG_STATE[k] === this.state),
            currentContext: this.currentContext
        };
    }
    
    /**
     * Enable or disable auto-fix mode.
     */
    setAutoFix(enabled) {
        this.autoFix = enabled;
        console.log(`[CognitiveAgent] Auto-fix ${enabled ? 'enabled' : 'disabled'}`);
    }
}

// Re-export constants
export { ERROR_TYPE, DEBUG_STATE, DebugContext };
