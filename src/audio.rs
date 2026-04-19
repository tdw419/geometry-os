// audio.rs -- WAV beep/note synthesis and async playback via aplay

use std::io::Write;
use std::sync::mpsc::{channel, Sender};
use std::sync::OnceLock;

static BEEP_SENDER: OnceLock<Sender<Vec<u8>>> = OnceLock::new();

fn get_beep_sender() -> &'static Sender<Vec<u8>> {
    BEEP_SENDER.get_or_init(|| {
        let (tx, rx) = channel::<Vec<u8>>();
        std::thread::spawn(move || {
            while let Ok(wav_data) = rx.recv() {
                match std::process::Command::new("aplay")
                    .args(["-q", "-t", "wav", "-"])
                    .stdin(std::process::Stdio::piped())
                    .spawn()
                {
                    Ok(mut child) => {
                        if let Some(mut stdin) = child.stdin.take() {
                            let _ = stdin.write_all(&wav_data);
                        }
                        let _ = child.wait();
                    }
                    Err(_) => {}
                }
            }
        });
        tx
    })
}

/// Waveform types for NOTE opcode.
/// 0 = sine, 1 = square, 2 = triangle, 3 = sawtooth, 4 = noise.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Waveform {
    Sine,
    Square,
    Triangle,
    Sawtooth,
    Noise,
}

impl Waveform {
    pub fn from_u32(v: u32) -> Self {
        match v {
            0 => Waveform::Sine,
            1 => Waveform::Square,
            2 => Waveform::Triangle,
            3 => Waveform::Sawtooth,
            _ => Waveform::Noise,
        }
    }
}

/// Generate a single sample for the given waveform at time t (seconds) and frequency (Hz).
fn waveform_sample(wave: Waveform, t: f64, freq: f64, noise_state: &mut u32) -> f64 {
    let phase = (freq * t) % 1.0;
    match wave {
        Waveform::Sine => (2.0 * std::f64::consts::PI * freq * t).sin(),
        Waveform::Square => {
            if phase < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
        Waveform::Triangle => 4.0 * (phase - 0.5).abs() - 1.0,
        Waveform::Sawtooth => 2.0 * phase - 1.0,
        Waveform::Noise => {
            // Simple LCG noise: generate a pseudo-random value in [-1, 1]
            *noise_state = noise_state.wrapping_mul(1103515245).wrapping_add(12345);
            let raw = (*noise_state >> 16) as i32;
            (raw as f64) / 32768.0
        }
    }
}

fn build_wav_header(num_samples: usize, sample_rate: u32) -> Vec<u8> {
    let data_size = num_samples * 2; // 16-bit mono
    let mut wav = Vec::with_capacity(44 + data_size);
    wav.extend_from_slice(b"RIFF");
    let file_size = (36 + data_size) as u32;
    wav.extend_from_slice(&file_size.to_le_bytes());
    wav.extend_from_slice(b"WAVE");
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes()); // chunk size
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&1u16.to_le_bytes()); // mono
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    let byte_rate = sample_rate * 2;
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes()); // block align
    wav.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&(data_size as u32).to_le_bytes());
    wav
}

pub fn play_beep(freq: u32, dur_ms: u32) {
    let _ = play_note_internal(Waveform::Sine, freq, dur_ms);
}

/// Play a note with the given waveform, frequency (Hz), and duration (ms).
pub fn play_note(wave: Waveform, freq: u32, dur_ms: u32) {
    let _ = play_note_internal(wave, freq, dur_ms);
}

fn play_note_internal(wave: Waveform, freq: u32, dur_ms: u32) -> Result<(), ()> {
    let sender = get_beep_sender().clone();
    let sample_rate = 22050u32;
    let num_samples = (sample_rate * dur_ms / 1000) as usize;

    let mut wav = build_wav_header(num_samples, sample_rate);

    let amplitude = 16000i16;
    let mut noise_state: u32 = 0xCAFEBABE;
    for i in 0..num_samples {
        let t = i as f64 / sample_rate as f64;
        let sample = waveform_sample(wave, t, freq as f64, &mut noise_state);
        let val = (amplitude as f64 * sample) as i16;
        wav.extend_from_slice(&val.to_le_bytes());
    }

    let _ = sender.send(wav);
    Ok(())
}
