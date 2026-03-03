/**
 * Geometry OS Sound System
 *
 * Web Audio API integration for the tone opcode.
 * Memory-mapped I/O: RAM[123]=volume, RAM[124]=duration, RAM[125]=frequency
 */

export class SoundSystem {
    constructor() {
        this.context = null;
    }

    init() {
        if (!this.context) {
            this.context = new AudioContext();
        }
        return this.context;
    }

    playTone(frequency = 440, duration = 0.5, volume = 1.0, waveform = 'sine') {
        if (!this.context) {
            this.init();
        }

        const osc = this.context.createOscillator();
        const gain = this.context.createGain();

        osc.type = waveform;
        osc.frequency.value = frequency;

        gain.gain.value = volume;

        osc.connect(gain);
        gain.connect(this.context.destination);

        const now = this.context.currentTime;
        osc.start(now);
        osc.stop(now + duration);

        return osc;
    }

    processSoundCommand(frequency, duration, volume) {
        this.playTone(frequency, duration, volume);
    }

    dispose() {
        if (this.context) {
            this.context.close();
            this.context = null;
        }
    }
}
