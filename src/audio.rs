// audio.rs -- WAV beep synthesis and async playback via aplay

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

pub fn play_beep(freq: u32, dur_ms: u32) {
    let sender = get_beep_sender().clone();
    let sample_rate = 22050u32;
    let num_samples = (sample_rate * dur_ms / 1000) as usize;

    // Build WAV in memory
    let data_size = num_samples * 2; // 16-bit mono
    let mut wav = Vec::with_capacity(44 + data_size);
    // RIFF header
    wav.extend_from_slice(b"RIFF");
    let file_size = (36 + data_size) as u32;
    wav.extend_from_slice(&file_size.to_le_bytes());
    wav.extend_from_slice(b"WAVE");
    // fmt chunk
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes()); // chunk size
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&1u16.to_le_bytes()); // mono
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    let byte_rate = sample_rate * 2;
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes()); // block align
    wav.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    // data chunk
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&(data_size as u32).to_le_bytes());

    let amplitude = 16000i16;
    for i in 0..num_samples {
        let t = i as f64 / sample_rate as f64;
        let val = (amplitude as f64 * (2.0 * std::f64::consts::PI * freq as f64 * t).sin()) as i16;
        wav.extend_from_slice(&val.to_le_bytes());
    }

    let _ = sender.send(wav);
}
