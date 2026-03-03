/**
 * Tests for Geometry OS SpirvRunner - Sound Integration
 *
 * Tests that the runner integrates with SoundSystem when tone opcodes are executed.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

// Mock fetch for loading executor.wgsl
globalThis.fetch = vi.fn((url) => {
  if (url === 'executor.wgsl') {
    return Promise.resolve({
      text: () => Promise.resolve('// mock executor shader'),
    });
  }
  return Promise.reject(new Error('Unknown URL'));
});

// Mock GPUBufferUsage global
globalThis.GPUBufferUsage = {
  MAP_READ: 1,
  MAP_WRITE: 2,
  COPY_SRC: 4,
  COPY_DST: 8,
  INDEX: 16,
  VERTEX: 32,
  UNIFORM: 64,
  STORAGE: 128,
  INDIRECT: 256,
  QUERY_RESOLVE: 512,
};

// Mock GPUMapMode global
globalThis.GPUMapMode = {
  READ: 1,
  WRITE: 2,
};

// Mock Web Audio API
class MockAudioContext {
  constructor() {
    this.state = 'running';
    this.sampleRate = 44100;
    this.currentTime = 0;
    this._oscillators = [];
  }

  createOscillator() {
    const osc = {
      type: 'sine',
      frequency: { value: 440 },
      connect: vi.fn(),
      start: vi.fn(),
      stop: vi.fn(),
      disconnect: vi.fn()
    };
    this._oscillators.push(osc);
    return osc;
  }

  createGain() {
    return {
      gain: { value: 1.0, setValueAtTime: vi.fn() },
      connect: vi.fn(),
      disconnect: vi.fn()
    };
  }

  destination = { connect: vi.fn() };

  close() {
    this.state = 'closed';
  }
}

globalThis.AudioContext = MockAudioContext;

// Mock WebGPU device
const mockDevice = {
  createBuffer: vi.fn(() => ({
    mapAsync: vi.fn().mockResolvedValue(undefined),
    getMappedRange: vi.fn(() => new ArrayBuffer(1024)),
    unmap: vi.fn(),
  })),
  createComputePipeline: vi.fn(() => ({
    getBindGroupLayout: vi.fn(() => ({})),
  })),
  createBindGroup: vi.fn(),
  createShaderModule: vi.fn(),
  createCommandEncoder: vi.fn(() => ({
    beginComputePass: vi.fn(() => ({
      setPipeline: vi.fn(),
      setBindGroup: vi.fn(),
      dispatchWorkgroups: vi.fn(),
      end: vi.fn(),
    })),
    copyBufferToBuffer: vi.fn(),
    finish: vi.fn(() => ({})),
  })),
  queue: {
    writeBuffer: vi.fn(),
    submit: vi.fn(),
  },
};

// Mock navigator.gpu
Object.defineProperty(globalThis, 'navigator', {
  value: {
    gpu: {
      requestAdapter: vi.fn().mockResolvedValue({
        requestDevice: vi.fn().mockResolvedValue(mockDevice),
      }),
    },
  },
  writable: true,
  configurable: true,
});

// Import after mocking
import { SpirvRunner } from './SpirvRunner.js';
import { SoundSystem } from './SoundSystem.js';

describe('SpirvRunner Sound Integration', () => {
  let runner;

  beforeEach(() => {
    runner = new SpirvRunner();
    vi.clearAllMocks();
  });

  afterEach(() => {
    if (runner && runner.dispose) {
      runner.dispose();
    }
  });

  describe('sound system initialization', () => {
    it('creates a SoundSystem instance', () => {
      expect(runner.soundSystem).toBeInstanceOf(SoundSystem);
    });
  });

  describe('tone opcode handling', () => {
    it('calls playTone when RAM sound registers are set', async () => {
      await runner.init();

      const playToneSpy = vi.spyOn(runner.soundSystem, 'playTone');

      // Create mock RAM with sound command
      const mockRamResult = new Float32Array(256);
      mockRamResult[125] = 440;  // frequency
      mockRamResult[124] = 0.5;  // duration
      mockRamResult[123] = 0.8;  // volume
      mockRamResult[126] = 1;    // sound trigger flag

      // Override createBuffer for MAP_READ buffers
      mockDevice.createBuffer.mockImplementation((options) => {
        const GPUBufferUsage = { MAP_READ: 1, STORAGE: 4, COPY_DST: 8, COPY_SRC: 2 };
        if (options.usage === (GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST)) {
          return {
            mapAsync: vi.fn().mockResolvedValue(undefined),
            getMappedRange: vi.fn(() => mockRamResult.buffer.slice(0)),
            unmap: vi.fn(),
          };
        }
        return {
          mapAsync: vi.fn().mockResolvedValue(undefined),
          getMappedRange: vi.fn(() => new ArrayBuffer(1024)),
          unmap: vi.fn(),
        };
      });

      const binary = new Uint32Array([
        0x07230203, 0x00010000, 0, 10, 0,
      ]);

      await runner.execute('test-process', binary.buffer, {});

      expect(playToneSpy).toHaveBeenCalledWith(440, 0.5, expect.closeTo(0.8, 5));
    });

    it('does not play sound when trigger flag is not set', async () => {
      await runner.init();

      const playToneSpy = vi.spyOn(runner.soundSystem, 'playTone');

      // Create mock RAM without sound trigger
      const mockRamResult = new Float32Array(256);
      mockRamResult[125] = 440;
      mockRamResult[124] = 0.5;
      mockRamResult[123] = 0.8;
      // RAM[126] = 0 (no trigger)

      mockDevice.createBuffer.mockImplementation((options) => {
        const GPUBufferUsage = { MAP_READ: 1, STORAGE: 4, COPY_DST: 8, COPY_SRC: 2 };
        if (options.usage === (GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST)) {
          return {
            mapAsync: vi.fn().mockResolvedValue(undefined),
            getMappedRange: vi.fn(() => mockRamResult.buffer.slice(0)),
            unmap: vi.fn(),
          };
        }
        return {
          mapAsync: vi.fn().mockResolvedValue(undefined),
          getMappedRange: vi.fn(() => new ArrayBuffer(1024)),
          unmap: vi.fn(),
        };
      });

      const binary = new Uint32Array([0x07230203, 0x00010000, 0, 10, 0]);
      await runner.execute('test-process-2', binary.buffer, {});

      expect(playToneSpy).not.toHaveBeenCalled();
    });
  });

  describe('dispose', () => {
    it('disposes sound system on cleanup', async () => {
      await runner.init();
      const disposeSpy = vi.spyOn(runner.soundSystem, 'dispose');
      runner.dispose();
      expect(disposeSpy).toHaveBeenCalled();
    });
  });
});
