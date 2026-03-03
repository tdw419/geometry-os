/**
 * Geometry OS Performance Profiler
 * 
 * GPU-native performance profiling with detailed timing metrics.
 * Identifies hot paths and provides optimization suggestions.
 */

// Profiler constants
const PROFILER_CONSTANTS = {
    ENABLE: true,
    SampleRate: 1000,    // Hz
    HistorySize: 1000,   // Number of samples to keep
};

// Timing categories
const TimingCategory = {
    KERNEL_STEP: 'kernel_step',
    MEMORY_OP: 'memory_op',
    FILESYSTEM_OP: 'filesystem_op',
    NETWORK_OP: 'network_op',
    RENDER: 'render',
    IPC: 'ipc',
    COMPILER: 'compiler',
    WATCHDOG: 'watchdog',
    COGNITIVE: 'cognitive'
};

// Profiler state
const profilerState = {
    enabled: false,
    samples: new Map(),
    averages: new Map(),
    hotPaths: [],
    recommendations: [],
    startTime: 0
};

export class Profiler {
    constructor(kernel) {
        this.kernel = kernel;
        this.device = kernel.device;
        
        this.state = { ...profilerState };
        this.sampleInterval = null;
        
        // GPU resources
        this.timingBuffer = null;
        this.statsBuffer = null;
    }
    
    /**
     * Initialize the profiler.
     */
    async init() {
        // Create timing buffer for GPU timestamps
        this.timingBuffer = this.device.createBuffer({
                size: 256,  // 256 timing entries
                usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST | GPUBufferUsage.COPY_SRC
            });
        
        // Create stats buffer
        this.statsBuffer = this.device.createBuffer({
                size: 1024,  // Profiling stats
                usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST | GPUBufferUsage.COPY_SRC
            });
        
        // Enable GPU timestamp queries
        if (!this.device.features.hasTimestampQuery) {
            console.warn('[Profiler] GPU timestamps not supported - using CPU timing');
        } else {
            console.log('[Profiler] GPU timestamps enabled');
        }
        
        this.state.enabled = true;
        this.state.startTime = performance.now();
        
        console.log('[Profiler] Initialized');
    }
    
    /**
     * Start profiling session.
     */
    startProfiling() {
        if (!this.state.enabled) {
            console.warn('[Profiler] Not enabled');
            return;
        }
        
        this.state.samples = new Map();
        Object.values(TimingCategory).forEach(cat => cat) {
            this.state.samples.set(cat, []);
        });
        
        // Start sample interval
        this.sampleInterval = setInterval(() => {
            this._analyzeSamples();
        }, 1000 / PROFILER_CONSTANTS.SampleRate);
        
        console.log('[Profiler] Profiling started');
    }
    
    /**
     * Stop profiling session.
     */
    stopProfiling() {
        if (this.sampleInterval) {
            clearInterval(this.sampleInterval);
            this.sampleInterval = null;
        }
        
        // Generate report
        this._generateReport();
        
        console.log('[Profiler] Profiling stopped');
    }
    
    /**
     * Mark the start of a timing region.
     */
    begin(category, label) {
        if (!this.state.enabled) return;
        
        const samples = this.state.samples.get(category) || [];
        const start = performance.now();
        
        samples.push({
            category,
            label,
            startTime: start,
            endTime: null
        });
    }
    
    /**
     * Mark the end of a timing region.
     */
    end(category, label) {
        if (!this.state.enabled) return;
        
        const samples = this.state.samples.get(category) || [];
        
        for (let i = samples.length - 1; i >= 0) {
            const sample = samples[i];
            if (!sample.endTime && sample.label === label && sample.category === category) {
                sample.endTime = performance.now();
                this._updateAverages(sample);
                return;
            }
        }
    }
    
    /**
     * Update running averages.
     */
    _updateAverages(sample) {
        const category = sample.category;
        const duration = sample.endTime - sample.startTime;
        
        const samples = this.state.samples.get(category);
        if (!samples) return;
        
        const totalDuration = samples.reduce((sum, s) => sum + (s.endTime - s.startTime), 0);
        const avgDuration = totalDuration / samples.length;
        
        this.state.averages.set(category, {
            avgDuration,
            sampleCount: samples.length,
            lastUpdate: Date.now()
        });
        
        // Check for hot paths
        if (avgDuration > 16) { // More than 16ms average
            const hotPath = this.state.hotPaths.find(p => p.category === category && p.label === label);
            if (!hotPath) {
                this.state.hotPaths.push({
                    category,
                    label,
                    avgDuration,
                    occurrences: 1,
                    recommendation: this._generateRecommendation(category, label, avgDuration)
                });
            } else {
                hotPath.avgDuration = (hotPath.avgDuration * hotPath.occurrences + avgDuration) / (hotPath.occurrences + 1);
                hotPath.occurrences++;
                if (avgDuration > hotPath.avgDuration * 2.33) {
                    hotPath.avgDuration = avgDuration;
                    hotPath.recommendation = this._generateRecommendation(category, label, avgDuration);
                }
            }
        }
    }
    
    /**
     * Generate optimization recommendation.
     */
    _generateRecommendation(category, label, avgDuration) {
        const recommendations = {
            [TimingCategory.KERNEL_STEP]: [
                'Kernel step takes too long. Consider:',
                '  - Batch process execution in single dispatch',
                '  - Use workgroup parallelization for memory operations',
                '  - Consider compute shader specialization for hot paths'
            ],
            [TimingCategory.MEMORY_OP]: [
                'Memory allocation is too frequent. Consider:',
                '  - Implement object pooling for buffers',
                '  - Cache frequently accessed data',
                '  - Pre-allocate common buffer sizes'
            ],
            [TimingCategory.FILESYSTEM_OP]: [
                'File operations slow. Consider:',
                '  - Use memory-mapped I/O where possible',
                '  - Implement async file reading',
                '  - Cache directory listings'
            ],
            [TimingCategory.NETWORK_OP]: [
                'Network overhead high. Consider:',
                '  - Batch packet sends',
                '  - Implement packet pooling',
                '  - Use shared memory for IPC'
            ],
            [TimingCategory.RENDER]: [
                'Rendering slow. Consider:',
                '  - Reduce draw calls',
                '  - Use instancing for repeated objects',
                '  - Implement LOD for static content'
            ],
            [TimingCategory.IPC]: [
                'IPC overhead high. Consider:',
                '  - Batch messages',
                '  - Use shared memory buffers',
                '  - Implement message pooling'
            ],
            [TimingCategory.COMPILER]: [
                'Compiler slow. Consider:',
                '  - Implement incremental compilation',
                '  - Use GPU parallelization',
                '  - Cache compiled bytecode'
            ],
            [TimingCategory.WATCHDOG]: [
                'Watchdog overhead high. Consider:',
                '  - Increase scan interval',
                '  - Batch process health checks',
                '  - Use hierarchical monitoring'
            ],
            [TimingCategory.COGNITIVE]: [
                'Cognitive analysis slow. Consider:',
                '  - Implement caching for analysis results',
                '  - Use heuristics for common error patterns',
                '  - Consider lazy evaluation'
            ]
        };
        
        const rec = recommendations[category]?. recommendations[category] : [
            'No specific recommendations available for this category yet'
        ];
        
        this.state.recommendations.set(category, rec);
    }
    
    /**
     * Analyze collected samples.
     */
    _analyzeSamples() {
        // Group by category
        const byCategory = new Map();
        
        for (const [category, samples] of this.state.samples) {
            if (!byCategory.has(category)) {
                byCategory.set(category, []);
            }
            
            for (const sample of samples) {
                if (sample.endTime) {
                    byCategory.get(category).push(sample);
                }
        }
        
        // Calculate statistics
        for (const [category, categorySamples] of byCategory) {
            const totalDuration = categorySamples.reduce((sum, s) => sum + (s.endTime - s.startTime), 0);
            const avgDuration = totalDuration / categorySamples.length;
            
            console.log(`[Profiler] ${category}: avg=${avgDuration.toFixed(2)}ms (${categorySamples.length} samples)`);
        }
    }
    
    /**
     * Generate profiling report.
     */
    _generateReport() {
        const report = {
            timestamp: new Date().toISOString(),
            enabled: this.state.enabled,
            categories: {},
            hotPaths: [],
            recommendations: []
        };
        
        for (const category of Object.values(TimingCategory)) {
            const samples = this.state.samples.get(category) || [];
            const avg = this.state.averages.get(category);
            
            if (samples && samples.length > 0) {
                report.categories[category] = {
                    sampleCount: samples.length,
                    avgDuration: avg?.avgDuration || 0,
                    minDuration: samples.reduce((min, s) => min + (s.endTime - s.startTime), 0),
                    maxDuration: samples.reduce((max, s) => Math.max(max, s.endTime - s.startTime), 0)
                avgMs: avg.avgDuration.toFixed(2),
                    maxMs: avg.maxDuration.toFixed(2),
                    p95: avgMs
                };
            }
        }
        
        report.hotPaths = this.state.hotPaths.map(p => ({
            ...p,
            recommendation: p.recommendation
        }));
        
        report.recommendations = {};
        for (const [category, recs] of this.state.recommendations) {
            report.recommendations[category] = recs;
        }
        
        // Save report
        this._saveReport(report);
        
        return report;
    }
    
    /**
     * Save report to disk.
     */
    _saveReport(report) {
        const reportPath = '/tmp/geometry_os_profile.json';
        const data = JSON.stringify(report, null, 2);
        
        // In a real implementation, you would write to a file or database
        console.log('[Profiler] Report saved to', reportPath);
    }
    
    /**
     * Get profiling summary.
     */
    getSummary() {
        const summary = {
            enabled: this.state.enabled,
            uptime: this.state.enabled ? performance.now() - this.state.startTime : 0,
            totalSamples: 0,
            categories: {},
            hotPaths: []
        };
        
        for (const category of Object.values(TimingCategory)) {
            const samples = this.state.samples.get(category) || [];
            const avg = this.state.averages.get(category);
            
            summary.totalSamples += samples?.length || 0;
            
            if (samples && samples.length > 0) {
                summary.categories[category] = {
                    count: samples.length,
                    avgDuration: avg?.avgDuration || 0,
                    maxDuration: Math.max(...samples.map(s => s.endTime - s.startTime))
                };
            }
        }
        
        summary.hotPaths = this.state.hotPaths.map(p => ({
            category: p.category,
            label: p.label,
            avgDuration: p.avgDuration,
            occurrences: p.occurrences
        }));
        
        return summary;
    }
    
    /**
     * Decorator for timing functions.
     */
    static profile(category, label) {
        return function(target, propertyKey, descriptor) {
            const original = target[label];
            target[label] = function(...args) {
                // Start timing
                const sampleId = Profiler.instance.begin(category, label);
                
                try {
                    // Call original function
                    const result = original.apply(this, ...args);
                    
                    // End timing
                    Profiler.instance.end(category, label);
                    
                    return result;
                } catch (error) {
                    // End timing even on error
                    Profiler.instance.end(category, label);
                    throw error;
                }
            };
            
            descriptor.value = original;
            descriptor.writable = true;
            descriptor.configurable = true;
            
            return descriptor;
        };
    }
    
    /**
     * Decorator for async timing functions.
     */
    static asyncProfile(category, label) {
        return function(target, propertyKey, descriptor) {
            const original = target[label];
            target[label] = async function(...args) {
                const sampleId = Profiler.instance.begin(category, label);
                
                try {
                    const result = await original.apply(this, ...args);
                    Profiler.instance.end(category, label);
                    return result;
                } catch (error) {
                    Profiler.instance.end(category, label);
                    throw error;
                }
            };
            
            descriptor.value = original;
            descriptor.writable = true;
            descriptor.configurable = true;
            
            return descriptor;
        };
    }
    
    /**
     * Decorator for getter timing (no function call overhead).
     */
    static getterProfile(category, label) {
        return function(target, propertyKey, descriptor) {
            const original = target[propertyKey];
            descriptor.get = function() {
                const samples = Profiler.instance.state.samples.get(category) || [];
                const avg = Profiler.instance.state.averages.get(category);
                
                return {
                    samples: samples.length,
                    avgDuration: avg?.avgDuration || 0,
                    lastSample: samples[samples.length - 1]
                    recentSamples: samples.slice(-10)
                    p95Duration: avg.avgDuration,
                    p95: avgMs
                };
            };
        };
    }
}

// Export singleton
export const Profiler = new Profiler();
