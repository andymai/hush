//! Speech-to-text with Whisper on this machine, or with a cloud service the
//! user chose.
//!
//! [`WhisperTranscriber`] runs Whisper models through transcribe.cpp, on a
//! GPU when the binary was built with the `cuda` or `vulkan` feature and one
//! is present. It loads the ggml files of the whisper.cpp catalogue as they
//! are. [`CloudTranscriber`] sends the audio to Groq or OpenAI, only when
//! `transcription.backend` names that service. [`SpeechToText`] builds
//! whichever the config asks for, and [`guard`] drops a transcript that
//! Whisper invents for silence or that echoes the prompt. [`ModelManager`]
//! downloads and verifies models from the ggerganov/whisper.cpp catalogue,
//! the Silero voice activity detection model that trims silence before local
//! transcription, and the VoxLingua107 language-ID model that
//! `transcription.detect_languages` uses through [`langid`].
//! [`device::init_backends`] loads the ggml backend modules that sit next to
//! libtranscribe: the CPU module that fits this CPU, and the GPU module.
//! [`GpuAvailability`] reports what the device registry of transcribe.cpp
//! found. [`gpu_fallback`] waits for the GPU at login, and keeps a GPU that
//! failed a take on the CPU until its driver changes. [`ModelSlot`] frees the
//! local model after idle minutes,
//! and the next take loads it again. [`language`] lists Whisper's languages
//! and picks the language of each take.

pub mod cloud;
pub mod device;
pub mod engine;
pub mod gpu_fallback;
pub mod guard;
pub mod langid;
pub mod language;
pub mod models;
pub mod parts;
pub mod slot;
pub mod whisper;

pub use cloud::CloudTranscriber;
pub use device::{BackendDevice, GpuAvailability, GpuType};
pub use engine::SpeechToText;
pub use language::Language;
pub use models::{ModelInfo, ModelManager, ModelSize};
pub use slot::{IdleTimer, ModelSlot, SlotState};
pub use whisper::{Family, TranscriptionResult, WhisperTranscriber, WHISPER_SAMPLE_RATE};
