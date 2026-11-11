use crate::config::settings::LlmConfig;
use crate::config::{keys, paths};
use crate::text_processing::llm;
use crate::text_processing::{
    DomainVocabulary, EditingMode, LlmProvider, ProcessingConfig, TextProcessor,
};
use crate::transcription::SpeechToText;
use crate::{AudioCapture, Config};

#[cfg(target_os = "linux")]
use crate::TextInserter;
/// Record command implementation
///
/// Handles single audio recording with optional transcription and text insertion.
use anyhow::Result;
use hound;
use std::path::{Path, PathBuf};
use tokio::io::{AsyncBufReadExt, BufReader};
use tracing::{info, warn};

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
/// # use hush::cli::commands::handle_record;
/// # use std::path::PathBuf;
/// # async fn example() -> anyhow::Result<()> {
/// // Record for 10 seconds and insert text
/// handle_record(10, false, None).await?;
///
/// // Record for 5 seconds, print only, save to file
/// handle_record(5, true, Some(PathBuf::from("output.wav"))).await?;
/// # Ok(())
/// # }
/// ```
///
/// # Workflow
///
/// 1. User presses Enter to start recording
/// 2. Records audio for up to `duration` seconds
/// 3. Stops recording automatically or on Enter press
/// 4. Transcribes audio (unless `print_only`)
/// 5. Cleans up and polishes the text as dictation does, with the `[llm]`
///    settings. With `llm.polish = false` or `llm.provider = "none"`, no LLM
///    polishes it
/// 6. Inserts text with 3-second delay (unless `print_only`)
/// 7. Optionally saves audio to file
pub async fn handle_record(
    duration: u64,
    print_only: bool,
    save_audio: Option<PathBuf>,
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

    // Wait for either timeout or user input (Enter key)
    let timeout = tokio::time::Duration::from_secs(duration);
    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin);
    let mut line = String::new();

    tokio::select! {
        _ = tokio::time::sleep(timeout) => {
            info!("Recording timeout reached");
        }
        result = reader.read_line(&mut line) => {
            match result {
                Ok(_) => info!("Recording stopped by user"),
                Err(e) => info!("Stdin error (stopping recording): {}", e),
            }
        }
    }

    // Stop recording and get samples
    let audio_data = audio_capture.stop_recording()?;
    println!("✅ Recording stopped ({} samples)", audio_data.len());

    // Transcribe if not print-only mode
    if !print_only {
        println!("🗣️ Transcribing...");
        keys::load_env_files();
        let transcriber = SpeechToText::from_config(
            &config.transcription,
            &paths::models_dir(),
            &keys::env_file(),
        )
        .await?;
        info!("Transcriber: {}", transcriber.describe());

        let text_processor = record_processor(
            &config.llm,
            &keys::env_file(),
            Some(DomainVocabulary::default_path()),
        )
        .await;

        match transcriber
            .transcribe_async(&audio_data, config.audio.sample_rate)
            .await
        {
            Ok(result) => {
                println!("✅ Raw transcription: '{}'", result.text);

                // Apply text processing
                let processed_text = if let Some(ref processor) = text_processor {
                    match processor.process(&result.text).await {
                        Ok(polished) => {
                            if polished != result.text {
                                println!("✨ Processed text: '{}'", polished);
                            }
                            polished
                        },
                        Err(e) => {
                            warn!("Text processing failed: {}", e);
                            result.text.clone()
                        },
                    }
                } else {
                    result.text.clone()
                };

                // Insert text
                #[cfg(target_os = "linux")]
                {
                    let mut text_inserter = TextInserter::new()?;
                    println!("⌨️ Inserting text (3 second delay)...");
                    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

                    match text_inserter.insert_text(&processed_text) {
                        Ok(_) => println!("✅ Text inserted successfully"),
                        Err(e) => println!("❌ Text insertion failed: {}", e),
                    }
                }

                #[cfg(not(target_os = "linux"))]
                {
                    println!("ℹ️  Text insertion not available on this platform");
                    println!(
                        "💡 Use the trait-based adapters for platform-specific text insertion"
                    );
                }
            },
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

/// The editing mode of a daemon that starts without `--editing-mode`.
const EDITING_MODE: EditingMode = EditingMode::Light;

/// The text processor of `hush record`, with the LLM and settings that
/// dictation polishes with. With `llm.polish` off, no LLM is probed or used.
async fn record_processor(
    llm_config: &LlmConfig,
    env_file: &Path,
    vocabulary: Option<PathBuf>,
) -> Option<TextProcessor> {
    let provider = llm::resolve_polish_provider(llm_config, env_file).await;
    text_processor(llm_config, provider, vocabulary)
}

fn text_processor(
    llm_config: &LlmConfig,
    provider: LlmProvider,
    vocabulary: Option<PathBuf>,
) -> Option<TextProcessor> {
    let config = ProcessingConfig::from_llm(EDITING_MODE, provider, llm_config);
    match TextProcessor::new(config, vocabulary) {
        Ok(processor) => {
            match processor.llm_name() {
                Some(name) => info!("Polish: {}", name),
                None => info!("Polish: none, rule-based cleanup only"),
            }
            Some(processor)
        },
        Err(e) => {
            warn!("Failed to initialize text processor: {}", e);
            None
        },
    }
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
    use crate::config::settings::LlmSetting;
    use crate::text_processing::llm::choose_provider;
    use tempfile::NamedTempFile;

    #[test]
    fn the_configured_provider_and_model_reach_the_client() {
        let llm = LlmConfig {
            provider: LlmSetting::Anthropic,
            anthropic_model: "claude-test-model".to_string(),
            ..LlmConfig::defaults()
        };
        let provider = choose_provider(&llm, false, Some("test-key".to_string()), None);
        let processor = text_processor(&llm, provider, None).expect("a processor");
        assert_eq!(
            processor.llm_name().as_deref(),
            Some("Anthropic claude-test-model")
        );

        let llm = LlmConfig {
            provider: LlmSetting::Ollama,
            ollama_model: "qwen-test".to_string(),
            ..LlmConfig::defaults()
        };
        let provider = choose_provider(&llm, true, None, None);
        let processor = text_processor(&llm, provider, None).expect("a processor");
        assert_eq!(processor.llm_name().as_deref(), Some("Ollama qwen-test"));
    }

    #[tokio::test]
    async fn record_resolves_the_configured_server() {
        let home = tempfile::tempdir().unwrap();
        let llm = LlmConfig {
            provider: LlmSetting::OpenAiCompatible,
            server_url: "http://127.0.0.1:9/v1".to_string(),
            server_model: "qwen3".to_string(),
            ..LlmConfig::defaults()
        };
        let processor = record_processor(&llm, &home.path().join(".env"), None)
            .await
            .expect("a processor");
        let name = processor.llm_name().unwrap_or_default();
        assert!(name.starts_with("OpenAI-compatible qwen3"), "{name}");
    }

    #[tokio::test]
    async fn no_polish_without_polish_or_a_provider() {
        let home = tempfile::tempdir().unwrap();
        let env_file = home.path().join(".env");
        let server = LlmConfig {
            provider: LlmSetting::OpenAiCompatible,
            server_url: "http://127.0.0.1:9/v1".to_string(),
            server_model: "qwen3".to_string(),
            ..LlmConfig::defaults()
        };
        let cases = [
            LlmConfig {
                provider: LlmSetting::None,
                ..server.clone()
            },
            LlmConfig {
                polish: false,
                ..server
            },
            LlmConfig {
                provider: LlmSetting::Ollama,
                ollama_url: "http://127.0.0.1:9".to_string(),
                polish: false,
                ..LlmConfig::defaults()
            },
        ];
        for llm in cases {
            let processor = record_processor(&llm, &env_file, None)
                .await
                .expect("a processor");
            assert_eq!(processor.llm_name(), None, "{llm:?}");
            assert_eq!(
                processor.process("um so the api works").await.unwrap(),
                "So the API works.",
                "{llm:?}"
            );
        }
    }

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
