pub mod cuda;
pub mod device;
pub mod models;
/// Speech-to-text transcription using OpenAI's Whisper models
///
/// This module provides GPU-accelerated speech transcription using locally-run
/// Whisper models. Supports CUDA for NVIDIA GPUs and falls back to CPU when needed.
///
/// # Components
///
/// - [`WhisperTranscriber`] - Main transcriber with PyTorch and Candle backend support
/// - [`SimpleWhisperTranscriber`] - Simplified transcriber for testing and fallback
/// - [`ModelManager`] - Downloads and manages Whisper model files
/// - [`TranscriptionResult`] - Transcription output with text and metadata
///
/// # Model Sizes
///
/// - **Tiny** (75MB) - Fast, basic accuracy, ~0.3s on GPU
/// - **Base** (145MB) - Recommended balance, ~0.5s on GPU
/// - **Small** (466MB) - Better accuracy, ~0.8s on GPU
/// - **Medium** (1.5GB) - High accuracy, ~1.5s on GPU
/// - **Large** (2.9GB) - Highest accuracy, ~2.5s on GPU
///
/// # Examples
///
/// ```no_run
/// use hush::transcription::{WhisperTranscriber, ModelSize};
///
/// // Create transcriber with base model
/// let transcriber = WhisperTranscriber::new(
///     Some(ModelSize::Base),
///     None  // Auto-detect CUDA
/// ).expect("Failed to initialize transcriber");
///
/// // Transcribe audio samples
/// let samples = vec![0.0f32; 48000]; // 3 seconds at 16kHz
/// let result = transcriber.transcribe(&samples)
///     .expect("Failed to transcribe");
///
/// println!("Transcription: {}", result.text);
/// ```
///
/// # GPU Acceleration
///
/// Automatically detects and uses NVIDIA CUDA GPUs when available. Falls back
/// to CPU processing if CUDA is unavailable. GPU acceleration provides 5-10x
/// speedup for most model sizes.
pub mod whisper;
pub mod whisper_simple;

pub use device::{GpuAvailability, GpuType};
pub use models::{ModelInfo, ModelManager, ModelSize};
pub use whisper::{TranscriptionResult, WhisperTranscriber};
pub use whisper_simple::{SimpleTranscriptionResult, SimpleWhisperTranscriber};
