//! Speech-to-text with whisper.cpp, or with a cloud service the user chose.
//!
//! [`WhisperTranscriber`] runs ggml Whisper models through whisper-rs, on a
//! GPU when the binary was built with the `cuda` or `vulkan` feature and one
//! is present. [`CloudTranscriber`] sends the audio to Groq or OpenAI, only
//! when `transcription.backend` names that service. [`SpeechToText`] builds
//! whichever the config asks for, and [`guard`] drops a transcript that
//! Whisper invents for silence or that echoes the prompt. [`ModelManager`]
//! downloads and verifies models from the ggerganov/whisper.cpp catalogue,
//! and the Silero voice activity detection model that trims silence before
//! local transcription. [`GpuAvailability`] reports what ggml's backend
//! registry found. [`cpu`] refuses to call ggml on a CPU below the floor of a
//! prebuilt binary. [`ModelSlot`] frees the local model after idle minutes,
//! and the next take loads it again.

pub mod cloud;
pub mod cpu;
pub mod device;
pub mod engine;
pub mod guard;
pub mod models;
pub mod slot;
pub mod whisper;

pub use cloud::CloudTranscriber;
pub use device::{BackendDevice, GpuAvailability, GpuType};
pub use engine::SpeechToText;
pub use models::{ModelInfo, ModelManager, ModelSize};
pub use slot::{IdleTimer, ModelSlot, SlotState};
pub use whisper::{TranscriptionResult, WhisperTranscriber, WHISPER_SAMPLE_RATE};
