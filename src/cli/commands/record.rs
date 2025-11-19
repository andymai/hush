/// Record command implementation
///
/// Handles single audio recording with optional transcription and text insertion.

use anyhow::Result;
use crate::{AudioCapture, WhisperTranscriber, TextInserter, Config};
use std::path::PathBuf;
use tracing::info;
use hound;

/// Handle the record command
///
/// Records audio for a specified duration, then optionally transcribes and inserts the text.
///
/// # Arguments
///
/// * `duration` - Maximum recording duration in seconds
/// * `print_only` - If true, only print transcription without inserting text
/// * `save_audio` - Optional path to save the recorded audio WAV file
///
/// # Examples
///
/// ```no_run
/// // Record for 10 seconds and insert text
/// handle_record(10, false, None).await?;
///
/// // Record for 5 seconds, print only, save to file
/// handle_record(5, true, Some(PathBuf::from("output.wav"))).await?;
/// ```
///
/// # Workflow
///
/// 1. User presses Enter to start recording
/// 2. Records audio for up to `duration` seconds
/// 3. Stops recording automatically or on Enter press
/// 4. Transcribes audio (unless `print_only`)
/// 5. Inserts text with 3-second delay (unless `print_only`)
/// 6. Optionally saves audio to file
pub async fn handle_record(
    duration: u64,
    print_only: bool,
    save_audio: Option<PathBuf>
) -> Result<()> {
    info!("🎙️ Starting single recording (max {}s)", duration);

    // Load configuration
    let config = Config::load()?;
    let mut audio_capture = AudioCapture::new(config.audio.device.as_deref())?;

    // Wait for user to start
    println!("Press Enter to start recording...");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    // Start recording
    println!(
        "🎤 Recording... (will auto-stop in {}s or press Enter to stop earlier)",
        duration
    );
    audio_capture.start_recording()?;

    // Wait for either timeout or user input
    let start_time = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(duration);

    while start_time.elapsed() < timeout {
        // Check for user input to stop early
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        // TODO: Add non-blocking stdin check for early stop
    }

    // Stop recording and get samples
    let audio_data = audio_capture.stop_recording()?;
    println!("✅ Recording stopped ({} samples)", audio_data.len());

    // Transcribe if not print-only mode
    if !print_only {
        println!("🗣️ Transcribing...");
        let transcriber = WhisperTranscriber::new(
            &config.transcription.model_path,
            config.transcription.use_cuda
        ).await?;

        match transcriber.transcribe_async(&audio_data, config.audio.sample_rate).await {
            Ok(result) => {
                println!("✅ Transcription: '{}'", result.text);

                // Insert text
                let mut text_inserter = TextInserter::new()?;
                println!("⌨️ Inserting text (3 second delay)...");
                tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

                match text_inserter.insert_text(&result.text) {
                    Ok(()) => println!("✅ Text inserted successfully"),
                    Err(e) => println!("❌ Text insertion failed: {}", e),
                }
            }
            Err(e) => println!("❌ Transcription failed: {}", e),
        }
    } else {
        println!("Print-only mode: audio recorded but not transcribed");
    }

    // Save audio to file if requested
    if let Some(path) = save_audio {
        save_audio_to_file(&audio_data, &path, config.audio.sample_rate)?;
    }

    Ok(())
}

/// Save audio samples to a WAV file
///
/// # Arguments
///
/// * `samples` - Audio samples as f32 values
/// * `path` - Output file path
/// * `sample_rate` - Sample rate in Hz
fn save_audio_to_file(samples: &[f32], path: &PathBuf, sample_rate: u32) -> Result<()> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = hound::WavWriter::create(path, spec)?;

    for sample in samples {
        let amplitude = (sample * i16::MAX as f32) as i16;
        writer.write_sample(amplitude)?;
    }

    writer.finalize()?;
    println!("💾 Audio saved to: {}", path.display());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_save_audio_to_file() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();

        // Create some test samples
        let samples: Vec<f32> = (0..1000).map(|i| (i as f32 / 1000.0) * 0.5).collect();

        // Save to file
        let result = save_audio_to_file(&samples, &path, 16000);
        assert!(result.is_ok());

        // Verify file exists and has content
        assert!(path.exists());
        let metadata = std::fs::metadata(&path).unwrap();
        assert!(metadata.len() > 0);
    }
}
