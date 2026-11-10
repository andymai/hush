//! Audio files: FLAC for the audio that Hush keeps, and WAV or FLAC for
//! `hush transcribe` and `hush recover`.

use crate::transcription::whisper::{resample, WHISPER_SAMPLE_RATE};
use anyhow::{anyhow, bail, Context, Result};
use flacenc::component::BitRepr;
use flacenc::error::Verify;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

const FLAC_MAGIC: &[u8; 4] = b"fLaC";
const WAV_MAGIC: &[u8; 4] = b"RIFF";
const BITS: usize = 16;

/// Mono samples from -1.0 to 1.0.
#[derive(Debug, Clone, PartialEq)]
pub struct Decoded {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

/// 16-bit mono FLAC of `samples`.
pub fn encode_flac(samples: &[f32], sample_rate: u32) -> Result<Vec<u8>> {
    if samples.is_empty() {
        bail!("No audio to encode");
    }
    let scale = f32::from(i16::MAX);
    let ints: Vec<i32> = samples
        .iter()
        .map(|sample| (sample.clamp(-1.0, 1.0) * scale).round() as i32)
        .collect();
    let config = flacenc::config::Encoder::default()
        .into_verified()
        .map_err(|(_, e)| anyhow!("Invalid FLAC encoder settings: {:?}", e))?;
    let source = flacenc::source::MemSource::from_samples(&ints, 1, BITS, sample_rate as usize);
    let stream = flacenc::encode_with_fixed_block_size(&config, source, config.block_size)
        .map_err(|e| anyhow!("FLAC encoding failed: {:?}", e))?;
    let mut sink = flacenc::bitsink::ByteSink::new();
    stream
        .write(&mut sink)
        .map_err(|e| anyhow!("FLAC encoding failed: {:?}", e))?;
    Ok(sink.as_slice().to_vec())
}

/// Write `samples` as FLAC that only the owner can read. The file appears
/// under its name only when it is complete.
pub fn write_flac(path: &Path, samples: &[f32], sample_rate: u32) -> Result<()> {
    let bytes = encode_flac(samples, sample_rate)?;
    let partial = partial_path(path)?;
    let written = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&partial)
        .and_then(|mut file| file.write_all(&bytes))
        .and_then(|()| std::fs::rename(&partial, path));
    if let Err(e) = written {
        let _ = std::fs::remove_file(&partial);
        return Err(e).with_context(|| format!("Failed to write {}", path.display()));
    }
    Ok(())
}

fn partial_path(path: &Path) -> Result<PathBuf> {
    let name = path
        .file_name()
        .with_context(|| format!("{} names no file", path.display()))?;
    let mut partial = std::ffi::OsString::from(".");
    partial.push(name);
    partial.push(".partial");
    Ok(path.with_file_name(partial))
}

/// A WAV or FLAC file, mixed down to mono.
pub fn read(path: &Path) -> Result<Decoded> {
    let mut magic = [0u8; 4];
    File::open(path)
        .and_then(|mut file| file.read_exact(&mut magic))
        .with_context(|| format!("Failed to read {}", path.display()))?;
    let decoded = match &magic {
        FLAC_MAGIC => read_flac(path),
        WAV_MAGIC => read_wav(path),
        _ => bail!("{} is not a WAV or FLAC file", path.display()),
    }
    .with_context(|| format!("Failed to decode {}", path.display()))?;
    if decoded.sample_rate == 0 {
        bail!("{} has no sample rate", path.display());
    }
    Ok(decoded)
}

/// How long a FLAC file plays, from its header.
pub fn flac_length(path: &Path) -> Result<std::time::Duration> {
    let info = claxon::FlacReader::open(path)
        .with_context(|| format!("Failed to read {}", path.display()))?
        .streaminfo();
    let samples = info.samples.unwrap_or(0);
    if info.sample_rate == 0 {
        bail!("{} has no sample rate", path.display());
    }
    Ok(std::time::Duration::from_secs_f64(
        samples as f64 / f64::from(info.sample_rate),
    ))
}

/// A WAV or FLAC file as Whisper takes it: mono at 16 kHz.
pub fn read_for_whisper(path: &Path) -> Result<Vec<f32>> {
    let decoded = read(path)?;
    resample(&decoded.samples, decoded.sample_rate, WHISPER_SAMPLE_RATE)
}

fn read_flac(path: &Path) -> Result<Decoded> {
    let mut reader = claxon::FlacReader::open(path)?;
    let info = reader.streaminfo();
    let scale = full_scale(info.bits_per_sample)?;
    let interleaved = reader
        .samples()
        .map(|sample| sample.map(|value| value as f32 / scale))
        .collect::<std::result::Result<Vec<f32>, _>>()?;
    Ok(Decoded {
        samples: mono(&interleaved, info.channels as usize),
        sample_rate: info.sample_rate,
    })
}

fn read_wav(path: &Path) -> Result<Decoded> {
    let mut reader = hound::WavReader::new(BufReader::new(File::open(path)?))?;
    let spec = reader.spec();
    let interleaved = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().collect::<Result<Vec<_>, _>>()?,
        hound::SampleFormat::Int => {
            let scale = full_scale(u32::from(spec.bits_per_sample))?;
            reader
                .samples::<i32>()
                .map(|sample| sample.map(|value| value as f32 / scale))
                .collect::<Result<Vec<_>, _>>()?
        },
    };
    Ok(Decoded {
        samples: mono(&interleaved, usize::from(spec.channels)),
        sample_rate: spec.sample_rate,
    })
}

fn full_scale(bits: u32) -> Result<f32> {
    if !(1..=32).contains(&bits) {
        bail!("Unsupported sample size: {} bits", bits);
    }
    Ok(2f32.powi(bits as i32 - 1))
}

fn mono(interleaved: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return interleaved.to_vec();
    }
    interleaved
        .chunks_exact(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn tone(len: usize) -> Vec<f32> {
        (0..len).map(|i| (i as f32 * 0.05).sin() * 0.5).collect()
    }

    #[test]
    fn flac_round_trips_within_one_step_of_16_bits() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("take.flac");
        let samples = tone(40_000);
        write_flac(&path, &samples, 16_000).unwrap();

        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        let names: Vec<_> = std::fs::read_dir(dir.path()).unwrap().collect();
        assert_eq!(names.len(), 1, "no partial file stays");

        let decoded = read(&path).unwrap();
        assert_eq!(decoded.sample_rate, 16_000);
        assert_eq!(decoded.samples.len(), samples.len());
        assert_eq!(flac_length(&path).unwrap().as_millis(), 2_500);
        let step = 1.0 / f32::from(i16::MAX);
        for (a, b) in samples.iter().zip(&decoded.samples) {
            assert!((a - b).abs() <= step, "{a} vs {b}");
        }
    }

    #[test]
    fn flac_clips_out_of_range_samples_and_refuses_nothing() {
        let bytes = encode_flac(&[2.0, -2.0, 0.0], 16_000).unwrap();
        assert!(bytes.starts_with(FLAC_MAGIC));
        assert!(encode_flac(&[], 16_000).is_err());
    }

    #[test]
    fn a_stereo_wav_becomes_mono_at_16_khz() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("talk.wav");
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 48_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&path, spec).unwrap();
        for _ in 0..48_000 {
            writer.write_sample(i16::MAX / 2).unwrap();
            writer.write_sample(0i16).unwrap();
        }
        writer.finalize().unwrap();

        let decoded = read(&path).unwrap();
        assert_eq!(decoded.sample_rate, 48_000);
        assert_eq!(decoded.samples.len(), 48_000);
        assert!((decoded.samples[100] - 0.25).abs() < 0.01);
        assert_eq!(read_for_whisper(&path).unwrap().len(), 16_000);
    }

    #[test]
    fn a_float_wav_is_read_as_it_is() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("float.wav");
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16_000,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer = hound::WavWriter::create(&path, spec).unwrap();
        for sample in [0.5f32, -0.25, 0.0] {
            writer.write_sample(sample).unwrap();
        }
        writer.finalize().unwrap();
        assert_eq!(read(&path).unwrap().samples, vec![0.5, -0.25, 0.0]);
    }

    #[test]
    fn other_files_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("notes.txt");
        std::fs::write(&path, b"hello there").unwrap();
        let message = format!("{:#}", read(&path).unwrap_err());
        assert!(message.contains("not a WAV or FLAC file"), "{message}");
        assert!(read(&dir.path().join("missing.wav")).is_err());
    }
}
