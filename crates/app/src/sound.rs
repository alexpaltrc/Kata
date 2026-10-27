//! The capture sound: a short, soft glass tap, synthesized once at startup.

use std::{
    f32::consts::{PI, TAU},
    sync::OnceLock,
};

use windows::{
    Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT},
    core::PCWSTR,
};

const SAMPLE_RATE: u32 = 48_000;
/// Seconds.
const LENGTH: f32 = 0.12;
const ATTACK: f32 = 0.0015;
const RELEASE: f32 = 0.01;
/// Loudest sample, as a fraction of full scale: deliberately quiet.
const PEAK: f32 = 0.2;
/// Frequency (Hz), loudness and decay time (s) of each overtone: an open fifth on A.
const PARTIALS: [(f32, f32, f32); 4] = [
    (880.0, 0.25, 0.030),
    (1760.0, 0.55, 0.022),
    (2637.0, 0.30, 0.014),
    (3520.0, 0.12, 0.008),
];

/// Builds the sound ahead of time, so the first capture plays it instantly.
pub fn prepare() {
    wav();
}

/// Plays the sound without waiting for it to finish.
pub fn play() {
    let wav = wav();
    unsafe {
        let _ = PlaySoundW(
            PCWSTR(wav.as_ptr().cast()),
            None,
            SND_MEMORY | SND_ASYNC | SND_NODEFAULT,
        );
    }
}

fn wav() -> &'static [u8] {
    static WAV: OnceLock<Vec<u8>> = OnceLock::new();
    WAV.get_or_init(|| encode(&synthesize()))
}

fn synthesize() -> Vec<f32> {
    let count = (SAMPLE_RATE as f32 * LENGTH) as usize;
    let mut samples: Vec<f32> = (0..count)
        .map(|i| {
            let t = i as f32 / SAMPLE_RATE as f32;
            let envelope = ramp(t / ATTACK) * ramp((LENGTH - t) / RELEASE);
            let tone: f32 = PARTIALS
                .iter()
                .map(|&(frequency, gain, decay)| {
                    gain * (-t / decay).exp() * (TAU * frequency * t).sin()
                })
                .sum();
            tone * envelope
        })
        .collect();
    let loudest = samples
        .iter()
        .fold(0.0f32, |max, sample| max.max(sample.abs()));
    for sample in &mut samples {
        *sample *= PEAK / loudest;
    }
    samples
}

/// A smooth (raised-cosine) rise from 0 to 1 as `x` goes from 0 to 1.
fn ramp(x: f32) -> f32 {
    0.5 - 0.5 * (PI * x.clamp(0.0, 1.0)).cos()
}

/// 16-bit mono PCM in a WAV container.
fn encode(samples: &[f32]) -> Vec<u8> {
    const BYTES_PER_SAMPLE: u16 = 2;
    let data_len = samples.len() as u32 * u32::from(BYTES_PER_SAMPLE);
    let mut wav = Vec::with_capacity(44 + data_len as usize);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes()); // Format chunk length.
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM.
    wav.extend_from_slice(&1u16.to_le_bytes()); // Mono.
    wav.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    wav.extend_from_slice(&(SAMPLE_RATE * u32::from(BYTES_PER_SAMPLE)).to_le_bytes());
    wav.extend_from_slice(&BYTES_PER_SAMPLE.to_le_bytes());
    wav.extend_from_slice(&(BYTES_PER_SAMPLE * 8).to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    for sample in samples {
        wav.extend_from_slice(&((sample * f32::from(i16::MAX)) as i16).to_le_bytes());
    }
    wav
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sound_is_short_quiet_and_silent_at_both_ends() {
        let samples = synthesize();
        let peak = samples.iter().fold(0.0f32, |max, s| max.max(s.abs()));
        assert!((peak - PEAK).abs() < 1e-4);
        assert!(samples[0].abs() < 1e-6);
        assert!(samples.last().unwrap().abs() < 1e-3);
    }

    #[test]
    fn wav_header_describes_the_samples() {
        let wav = encode(&[0.0; 10]);
        assert_eq!(wav.len(), 44 + 20);
        assert_eq!(&wav[..4], b"RIFF");
        assert_eq!(u32::from_le_bytes(wav[40..44].try_into().unwrap()), 20);
    }
}
