// Test script to explore whisper-rs GPU capabilities
use whisper_rs::WhisperContextParameters;

fn main() {
    println!("🔍 Exploring whisper-rs GPU capabilities");
    
    // Create default parameters
    let mut params = WhisperContextParameters::default();
    
    // Try to set GPU mode
    // Let's see what methods are available
    println!("Default parameters created");
    
    // The whisper-rs library typically has these methods for GPU:
    // Let's try some common GPU-related methods
    
    // Method 1: Try use_gpu
    #[cfg(feature = "cuda")]
    {
        params.use_gpu(true);
        println!("✅ GPU enabled via use_gpu()");
    }
    
    #[cfg(not(feature = "cuda"))]
    {
        println!("❌ CUDA feature not enabled in whisper-rs");
    }
    
    println!("whisper-rs parameters configured");
}