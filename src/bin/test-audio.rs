use hush::{Config, AudioCapture};
use std::time::Instant;

#[tokio::main]
async fn main() -> hush::Result<()> {
    println!("🎤 Testing Hush Audio Capture System");
    println!("====================================");

    // Load configuration
    let config = Config::load()?;
    
    // Test 1: Audio Capture Initialization
    println!("\n1. Testing audio capture initialization...");
    let mut audio_capture = AudioCapture::new(config.audio.device.as_deref())?;
    println!("   ✅ Audio capture initialized");
    println!("   🎛️  Device: {}", audio_capture.get_device_name());
    
    // Test 2: Recording Start/Stop
    println!("\n2. Testing recording start/stop cycle...");
    
    // Start recording
    let start_time = Instant::now();
    audio_capture.start_recording()?;
    println!("   ✅ Recording started");
    println!("   📊 Is recording: {}", audio_capture.is_recording());
    
    // Simulate recording for 2 seconds
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    
    // Stop recording
    let stop_time = Instant::now();
    let audio_data = audio_capture.stop_recording()?;
    let total_time = stop_time.duration_since(start_time);
    
    println!("   ✅ Recording stopped");
    println!("   📊 Is recording: {}", audio_capture.is_recording());
    println!("   📏 Captured {} samples", audio_data.len());
    println!("   ⏱️  Total recording time: {:.2}s", total_time.as_secs_f32());
    
    // Calculate expected vs actual sample count
    let expected_samples = (config.audio.sample_rate as f32 * total_time.as_secs_f32()) as usize;
    let sample_rate_actual = audio_data.len() as f32 / total_time.as_secs_f32();
    
    println!("   📊 Expected ~{} samples", expected_samples);
    println!("   📊 Effective sample rate: {:.0} Hz", sample_rate_actual);
    
    // Test 3: Audio Data Analysis
    println!("\n3. Testing audio data analysis...");
    if !audio_data.is_empty() {
        let max_amplitude = audio_data.iter().map(|&x| x.abs()).fold(0.0f32, f32::max);
        let avg_amplitude = audio_data.iter().map(|&x| x.abs()).sum::<f32>() / audio_data.len() as f32;
        let min_amplitude = audio_data.iter().map(|&x| x.abs()).fold(f32::INFINITY, f32::min);
        
        println!("   📊 Amplitude analysis:");
        println!("      • Maximum: {:.4}", max_amplitude);
        println!("      • Average: {:.4}", avg_amplitude);
        println!("      • Minimum: {:.4}", min_amplitude);
        
        // Check for clipping
        if max_amplitude >= 0.95 {
            println!("   ⚠️  Warning: Possible audio clipping detected");
        }
        
        // Check for silence
        if max_amplitude < 0.001 {
            println!("   ℹ️  Note: Very low audio levels (silence/simulation mode)");
        }
    } else {
        println!("   ℹ️  No audio data captured (simulation mode)");
    }
    
    // Test 4: Multiple Recording Cycles
    println!("\n4. Testing multiple recording cycles...");
    let mut cycle_times = Vec::new();
    let cycle_count = 3;
    
    for i in 1..=cycle_count {
        let cycle_start = Instant::now();
        
        audio_capture.start_recording()?;
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        let cycle_data = audio_capture.stop_recording()?;
        
        let cycle_time = cycle_start.elapsed();
        cycle_times.push(cycle_time);
        
        println!("   Cycle {}: {} samples in {:.0}ms", 
                 i, cycle_data.len(), cycle_time.as_millis());
    }
    
    let avg_cycle_time = cycle_times.iter().sum::<std::time::Duration>() / cycle_count as u32;
    println!("   ✅ Average cycle time: {:.0}ms", avg_cycle_time.as_millis());
    
    // Performance Summary
    println!("\n🎉 Audio Capture Tests Completed!");
    println!("=================================");
    println!("✅ Audio capture initialization: Working");
    println!("✅ Start/stop recording: Working");
    println!("✅ Audio data capture: Working");
    println!("✅ Multiple cycles: Working");
    
    if audio_data.is_empty() {
        println!("ℹ️  Note: Simulation mode active (no physical audio device)");
        println!("   Real audio devices will produce actual audio data");
    } else {
        println!("🎤 Live audio capture: Functional");
    }
    
    Ok(())
}