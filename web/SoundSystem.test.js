/**
 * Tests for Geometry OS Sound System
 *
 * Tests the Web Audio API integration for the tone opcode.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

// Mock Web Audio API for Node.js test environment
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

// Mock global AudioContext
globalThis.AudioContext = MockAudioContext;

// Import after mocking
import { SoundSystem } from './SoundSystem.js';

describe('SoundSystem', () => {
  let soundSystem;

  beforeEach(() => {
    soundSystem = new SoundSystem();
  });

  afterEach(() => {
    if (soundSystem) {
      soundSystem.dispose();
    }
  });

  describe('initialization', () => {
    it('creates an AudioContext on first use', () => {
      expect(soundSystem.context).toBeNull();
      soundSystem.init();
      expect(soundSystem.context).toBeInstanceOf(MockAudioContext);
    });
  });

  describe('playTone', () => {
    it('plays a tone with frequency, duration, and volume', () => {
      soundSystem.init();

      soundSystem.playTone(440, 0.5, 0.8);

      const ctx = soundSystem.context;
      expect(ctx._oscillators.length).toBe(1);

      const osc = ctx._oscillators[0];
      expect(osc.frequency.value).toBe(440);
      expect(osc.type).toBe('sine');
      expect(osc.start).toHaveBeenCalled();
      expect(osc.stop).toHaveBeenCalled();
    });

    it('uses default values when parameters are omitted', () => {
      soundSystem.init();

      soundSystem.playTone();

      const osc = soundSystem.context._oscillators[0];
      expect(osc.frequency.value).toBe(440); // A4 default
    });

    it('supports different waveform types', () => {
      soundSystem.init();

      soundSystem.playTone(440, 0.5, 0.5, 'square');
      soundSystem.playTone(880, 0.5, 0.5, 'sawtooth');

      expect(soundSystem.context._oscillators[0].type).toBe('square');
      expect(soundSystem.context._oscillators[1].type).toBe('sawtooth');
    });
  });

  describe('memory-mapped I/O', () => {
    it('processes tone command from RAM address 125', () => {
      soundSystem.init();

      // RAM[125] = frequency, RAM[124] = duration, RAM[123] = volume
      soundSystem.processSoundCommand(440, 0.25, 0.5);

      const osc = soundSystem.context._oscillators[0];
      expect(osc.frequency.value).toBe(440);
    });
  });

  describe('polyphony', () => {
    it('supports multiple concurrent tones', () => {
      soundSystem.init();

      soundSystem.playTone(440, 1.0, 0.5);
      soundSystem.playTone(554, 1.0, 0.5);
      soundSystem.playTone(659, 1.0, 0.5);

      expect(soundSystem.context._oscillators.length).toBe(3);
    });
  });
});
