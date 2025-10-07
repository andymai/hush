/// Test real voice-to-text with actual text field insertion
/// This combines the working transcription with text insertion functionality
use hush::{
    Result, 
    audio::AudioCapture, 
    model_downloader::{ModelDownloader, WhisperModel}, 
    transcription::SimpleWhisperTranscriber,
    text::TextInserter,
};
use std::path::PathBuf;
use tokio::time::{sleep, Duration};
use tracing::{info, warn, error};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    info!("🎤 Hush Voice-to-Text Integration Test");
    info!("=====================================");
    info!("");
    info!("This will:");
    info!("1. Set up real Whisper transcription");
    info!("2. Initialize text insertion system");
    info!("3. Record audio and transcribe it");
    info!("4. Insert transcribed text into focused text field");
    info!("");

    // Check system dependencies first
    info!("🔍 Checking system dependencies...");
    match hush::text::check_dependencies() {
        Ok(_) => info!("✅ All dependencies are available"),
        Err(e) => {
            error!("❌ System dependency check failed: {}", e);
            info!("💡 Make sure you have:");
            info!("  - X11 running (check DISPLAY environment variable)");
            info!("  - xclip installed (sudo apt install xclip)");
            return Err(e);
        }
    }

    // Set up models directory and downloader
    let models_dir = dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("hush")
        .join("models");
    
    let downloader = ModelDownloader::new(&models_dir);
    
    // Find or ensure we have a model
    let target_model = if let Some(model) = downloader.find_best_available_model().await {
        info!("🎯 Using existing model: {}", model.filename());
        model
    } else {
        info!("📥 No models found. Downloading tiny model for testing...");
        let model = WhisperModel::Tiny;
        downloader.ensure_model(&model).await?;
        model
    };
    
    // Set up transcriber
    let model_path = downloader.model_path(&target_model);
    info!("🔧 Initializing transcriber with model: {:?}", model_path);
    
    let transcriber = SimpleWhisperTranscriber::new(&model_path).await?;
    
    if !transcriber.is_ready() {
        error!("❌ Transcriber not ready. Cannot proceed.");
        return Ok(());
    }
    
    info!("✅ Transcriber ready! Device: {}", transcriber.get_device_info());
    
    // Set up text inserter
    info!("🔧 Initializing text insertion system...");
    let mut text_inserter = TextInserter::new()?;
    info!("✅ Text insertion system ready!");
    
    // Get current focused window info
    match text_inserter.get_focused_window() {
        Ok(window) => {
            info!("🎯 Current focused window:");
            info!("   Title: {}", window.title);
            info!("   Class: {}", window.class);
        }
        Err(e) => {
            warn!("⚠️  Could not get focused window info: {}", e);
        }
    }
    
    // Set up audio capture
    info!("🔧 Setting up audio capture...");
    let mut audio_capture = AudioCapture::new(None)?; // Use default device
    info!("✅ Audio capture ready!");
    
    info!("");
    info!("🚀 Ready to start voice-to-text!");
    info!("");
    info!("Instructions:");
    info!("1. Open any text editor, text field, or document");
    info!("2. Click in the text field to focus it");
    info!("3. Press Enter to start recording");
    info!("4. Speak clearly for a few seconds");
    info!("5. The transcribed text will be inserted automatically");
    info!("6. Press Ctrl+C to exit");
    info!("");

    // Wait for user to be ready
    info!("Press Enter when you're ready to start the first recording...");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    let recording_duration = Duration::from_secs(5); // 5 second recordings
    let mut session_count = 0;

    loop {
        session_count += 1;
        info!("");
        info!("🎙️  SESSION {} - Recording for {} seconds...", session_count, recording_duration.as_secs());
        info!("🗣️  Speak now!");
        
        // Start recording
        audio_capture.start_recording()?;
        
        // Show countdown
        for remaining in (1..=recording_duration.as_secs()).rev() {
            if remaining <= 3 {
                info!("⏰ {}...", remaining);
            }
            sleep(Duration::from_secs(1)).await;
        }
        
        // Stop recording and get samples
        let audio_samples = audio_capture.stop_recording()?;
        
        info!("📊 Captured {} samples ({:.2}s)", 
              audio_samples.len(), 
              audio_samples.len() as f32 / 16000.0);
        
        if audio_samples.len() < 8000 { // Less than 0.5 seconds
            warn!("⚠️  Very short audio captured, skipping transcription");
            continue;
        }
        
        info!("🔄 Transcribing audio...");
        
        match transcriber.transcribe(&audio_samples).await {
            Ok(result) => {
                let transcribed_text = result.text.trim();
                
                if transcribed_text.is_empty() {
                    info!("🔇 No speech detected or transcription was empty");
                } else {
                    info!("💬 Transcribed: \"{}\"", transcribed_text);
                    info!("📊 Confidence: {:.2}, Processing time: {:?}", 
                          result.confidence, result.processing_time);
                    
                    // Insert the transcribed text
                    info!("⌨️  Inserting text into focused application...");
                    
                    match text_inserter.insert_text(transcribed_text) {
                        Ok(_) => {
                            info!("✅ Text inserted successfully!");
                        }
                        Err(e) => {
                            error!("❌ Failed to insert text: {}", e);
                            error!("💡 Make sure a text field is focused and try again");
                        }
                    }
                }
            },
            Err(e) => {
                error!("❌ Transcription failed: {}", e);
            }
        }
        
        // Wait a bit before next recording
        info!("");
        info!("Press Enter for another recording, or Ctrl+C to exit...");
        let mut input = String::new();
        match std::io::stdin().read_line(&mut input) {
            Ok(_) => continue,
            Err(_) => break,
        }
    }
    
    info!("👋 Voice-to-text session ended. Goodbye!");
    Ok(())
}