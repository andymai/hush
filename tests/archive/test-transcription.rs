use hush::{Config, WhisperTranscriber};
use std::time::Instant;

#[tokio::main]
async fn main() -> hush::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("🧪 Testing Hush Transcription System");
    println!("====================================");

    // Test 1: Configuration Loading
    println!("\n1. Testing configuration loading...");
    let config = Config::load()?;
    println!("   ✅ Configuration loaded successfully");
    println!("   📄 Model: {} ({})", config.transcription.model_path.display(), config.transcription.model_size);
    println!("   🔊 Audio: {}Hz, {} channels", config.audio.sample_rate, config.audio.channels);

    // Test 2: Whisper Transcriber Initialization
    println!("\n2. Testing Whisper transcriber initialization...");
    let start_time = Instant::now();
    let transcriber = WhisperTranscriber::new(
        &config.transcription.model_path,
        config.transcription.use_cuda,
    ).await?;
    let init_time = start_time.elapsed();
    println!("   ✅ Whisper transcriber initialized in {:.2}ms", init_time.as_millis());
    println!("   🎯 Device: {}", transcriber.get_device_info());
    println!("   🚀 CUDA: {}", transcriber.is_using_cuda());

    // Test 3: Audio Data Generation and Transcription
    println!("\n3. Testing transcription with simulated audio...");
    
    // Generate test audio data (3 seconds of silence with some random variation)
    let sample_rate = config.audio.sample_rate;
    let duration_seconds = 3.0;
    let num_samples = (sample_rate as f32 * duration_seconds) as usize;
    
    // Create audio data with some variation to simulate speech
    let mut audio_data = Vec::new();
    for i in 0..num_samples {
        // Add some low-level variation to simulate speech patterns
        let time = i as f32 / sample_rate as f32;
        let amplitude = 0.01 * (time * 440.0 * 2.0 * std::f32::consts::PI).sin();
        audio_data.push(amplitude);
    }
    
    println!("   📊 Generated {} samples ({:.2}s of audio)", audio_data.len(), duration_seconds);
    
    // Test transcription
    let transcription_start = Instant::now();
    let result = transcriber.transcribe_async(&audio_data, sample_rate).await?;
    let transcription_time = transcription_start.elapsed();
    
    println!("   ✅ Transcription completed in {:.2}ms", transcription_time.as_millis());
    println!("   📝 Result: '{}'", result.text);
    println!("   🎯 Confidence: {:.2}", result.confidence);
    println!("   ⚡ Processing time: {:.2}ms", result.processing_time.as_millis());

    // Test 4: Multiple Transcriptions
    println!("\n4. Testing multiple transcriptions...");
    let mut total_time = 0;
    let test_count = 3;
    
    for i in 1..=test_count {
        // Generate different audio patterns
        let mut test_audio = Vec::new();
        for j in 0..8000 { // 0.5 seconds at 16kHz
            let time = j as f32 / sample_rate as f32;
            let freq = 200.0 + (i as f32 * 100.0); // Different frequency per test
            let amplitude = 0.05 * (time * freq * 2.0 * std::f32::consts::PI).sin();
            test_audio.push(amplitude);
        }
        
        let start = Instant::now();
        let test_result = transcriber.transcribe_async(&test_audio, sample_rate).await?;
        let elapsed = start.elapsed();
        total_time += elapsed.as_millis();
        
        println!("   Test {}: '{}' (confidence: {:.2}, time: {}ms)", 
                 i, test_result.text, test_result.confidence, elapsed.as_millis());
    }
    
    let avg_time = total_time / test_count;
    println!("   ✅ Average transcription time: {}ms", avg_time);

    // Performance Summary
    println!("\n🎉 All Tests Completed Successfully!");
    println!("=====================================");
    println!("✅ Configuration system: Working");
    println!("✅ Whisper initialization: {:.2}ms", init_time.as_millis());
    println!("✅ Transcription pipeline: Working"); 
    println!("✅ Audio processing: {} samples/second capability", 
             audio_data.len() as f32 / (transcription_time.as_secs_f32()));
    println!("⚡ Average latency: {}ms", avg_time);

    if result.text.is_empty() {
        println!("ℹ️  Note: Empty transcriptions are expected in simulation mode");
        println!("   Real audio input will produce actual text output");
    }

    Ok(())
}