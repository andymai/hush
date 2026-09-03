//! Speech-to-text with whisper.cpp.
//!
//! [`WhisperTranscriber`] runs ggml Whisper models through whisper-rs, on a
//! GPU when the binary was built with the `cuda` or `vulkan` feature and one
//! is present. [`ModelManager`] downloads and verifies models from the
//! ggerganov/whisper.cpp catalogue. [`GpuAvailability`] reports what ggml's
//! backend registry found.

pub mod device;
pub mod models;
pub mod whisper;

pub use device::{BackendDevice, GpuAvailability, GpuType};
pub use models::{ModelInfo, ModelManager, ModelSize};
pub use whisper::{TranscriptionResult, WhisperTranscriber, WHISPER_SAMPLE_RATE};
