/// Test and demonstrate the simple whisper-rs based transcription
use hush::{Result, audio::AudioCapture, model_downloader::{ModelDownloader, WhisperModel}, transcription::SimpleWhisperTranscriber};
use std::path::PathBuf;
use tokio::time::{sleep, Duration};
use tracing::{info, warn, error};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("debug")
        .init();

    info!("🎤 Hush Simple Whisper Test");
    info!("=============================");

    // Set up models directory
    let models_dir = dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("hush")
        .join("models");
    
    let downloader = ModelDownloader::new(&models_dir);
    
    // List available models
    info!("📋 Checking available models...");
    let models = downloader.list_models().await;
    for (model, exists) in &models {
        let status = if *exists { "✅ Downloaded" } else { "❌ Not available" };
        info!("  {} - {} - {}", model.filename(), model.description(), status);
    }
    
    // Find or download a model
    let target_model = if let Some(model) = downloader.find_best_available_model().await {
        info!("🎯 Using existing model: {}", model.filename());
        model
    } else {
        info!("📥 No models found. Downloading base model...");
        let model = WhisperModel::Base;
        downloader.ensure_model(&model).await?;
        model
    };
    
    // Set up transcriber
    let model_path = downloader.model_path(&target_model);
    info!("🔧 Initializing transcriber with model: {:?}", model_path);
    
    let transcriber = SimpleWhisperTranscriber::new(&model_path).await?;
    
    if !transcriber.is_ready() {
        error!("❌ Transcriber not ready. Cannot proceed with test.");
        return Ok(());
    }
    
    info!("✅ Transcriber ready! Device: {}", transcriber.get_device_info());
    info!("");
    info!("🎙️ Starting audio capture...");
    info!("Press Ctrl+C to stop");
    
    // Set up audio capture
    let mut audio_capture = AudioCapture::new(None)?; // Use default device
    
    let buffer_duration = Duration::from_secs(3); // 3 second chunks
    
    loop {
        info!("🎙️ Recording for {} seconds...", buffer_duration.as_secs());
        
        // Start recording
        audio_capture.start_recording()?;
        sleep(buffer_duration).await;
        
        // Stop recording and get samples
        let audio_samples = audio_capture.stop_recording()?;
        
        if audio_samples.is_empty() {
            warn!("⚠️ No audio samples captured");
            continue;
        }
        
        info!("📊 Captured {} samples ({:.2}s)", 
              audio_samples.len(), 
              audio_samples.len() as f32 / 16000.0);
        
        // Only transcribe if we have enough audio
        if audio_samples.len() > 16000 { // At least 1 second of audio
            info!("🔄 Transcribing {} samples...", audio_samples.len());
            
            match transcriber.transcribe(&audio_samples).await {
                Ok(result) => {
                    if !result.text.trim().is_empty() {
                        info!("💬 Transcription: \"{}\"", result.text);
                        info!("   📊 Confidence: {:.2}, Time: {:?}", 
                              result.confidence, result.processing_time);
                    } else {
                        info!("🔇 (silence or no speech detected)");
                    }
                },
                Err(e) => {
                    error!("❌ Transcription failed: {}", e);
                }
            }
        } else {
            info!("🔇 Not enough audio data for transcription");
        }
        
        // Small delay before next recording cycle
        sleep(Duration::from_millis(500)).await;
    }
}