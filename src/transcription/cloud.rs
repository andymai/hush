//! Transcription by a hosted service, Groq or OpenAI, through the
//! `/audio/transcriptions` API they share. Hush uses it only when
//! `transcription.backend` names the service, and a failure is reported,
//! never retried on another backend.

use crate::config::keys;
use crate::config::settings::{TranscriptionBackend, TranscriptionConfig};
use crate::transcription::whisper::{
    resample, strip_non_speech, TranscriptionResult, WHISPER_SAMPLE_RATE,
};
use crate::Result;
use anyhow::Context;
use reqwest::multipart::{Form, Part};
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use std::borrow::Cow;
use std::io::Cursor;
use std::time::{Duration, Instant};
use tracing::info;

/// Both services refuse uploads over 25 MB, which is about 13 minutes of
/// 16 kHz, 16-bit mono audio.
const MAX_UPLOAD_BYTES: usize = 25 * 1024 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Long enough to upload a ten-minute recording on a slow connection.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

pub struct CloudTranscriber {
    backend: TranscriptionBackend,
    model: String,
    language: String,
    api_key: String,
    client: Client,
}

#[derive(Deserialize)]
struct Transcription {
    text: String,
}

#[derive(Deserialize)]
struct ErrorBody {
    error: ErrorDetail,
}

#[derive(Deserialize)]
struct ErrorDetail {
    message: String,
}

impl CloudTranscriber {
    /// The service `config.backend` names, with its key from the
    /// environment or `~/.config/hush/.env`.
    pub fn from_config(config: &TranscriptionConfig) -> Result<Self> {
        let key = config.backend.key_var().and_then(keys::api_key);
        Self::new(
            config.backend,
            config.cloud_model().unwrap_or_default(),
            &config.language,
            key,
        )
    }

    pub fn new(
        backend: TranscriptionBackend,
        model: &str,
        language: &str,
        api_key: Option<String>,
    ) -> Result<Self> {
        let Some(var) = backend.key_var() else {
            anyhow::bail!("{} is not a cloud service", backend.name());
        };
        let api_key = api_key
            .filter(|key| !key.trim().is_empty())
            .with_context(|| {
                format!(
                    "transcription.backend is {} but {} is not set. Put it in ~/.config/hush/.env, \
                     or set backend = \"local\"",
                    backend.name(),
                    var
                )
            })?;
        let client = Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .context("HTTP client")?;
        let language = language.trim();
        Ok(Self {
            backend,
            model: model.trim().to_string(),
            language: if language.is_empty() {
                "auto".to_string()
            } else {
                language.to_string()
            },
            api_key,
            client,
        })
    }

    /// The service and model, such as "Groq whisper-large-v3-turbo".
    pub fn name(&self) -> String {
        format!("{} {}", self.backend.name(), self.model)
    }

    fn endpoint(&self) -> &'static str {
        match self.backend {
            TranscriptionBackend::Groq => "https://api.groq.com/openai/v1/audio/transcriptions",
            TranscriptionBackend::OpenAi | TranscriptionBackend::Local => {
                "https://api.openai.com/v1/audio/transcriptions"
            },
        }
    }

    /// The text fields of the request; the audio goes in a `file` part.
    fn fields(&self, prompt: Option<&str>) -> Vec<(&'static str, String)> {
        let mut fields = vec![
            ("model", self.model.clone()),
            ("response_format", "json".to_string()),
        ];
        if self.language != "auto" {
            fields.push(("language", self.language.clone()));
        }
        if let Some(prompt) = prompt.map(str::trim).filter(|p| !p.is_empty()) {
            fields.push(("prompt", prompt.to_string()));
        }
        fields
    }

    pub async fn transcribe_with_prompt(
        &self,
        audio: &[f32],
        sample_rate: u32,
        prompt: Option<&str>,
    ) -> Result<TranscriptionResult> {
        let start = Instant::now();
        if audio.is_empty() {
            return Ok(TranscriptionResult {
                text: String::new(),
                confidence: 0.0,
                processing_time: start.elapsed(),
            });
        }

        let samples = if sample_rate == WHISPER_SAMPLE_RATE {
            Cow::Borrowed(audio)
        } else {
            Cow::Owned(resample(audio, sample_rate, WHISPER_SAMPLE_RATE)?)
        };
        let wav = encode_wav(&samples)?;
        if wav.len() > MAX_UPLOAD_BYTES {
            anyhow::bail!(
                "The recording is {} MB and {} accepts at most 25 MB, about 13 minutes",
                wav.len() / (1024 * 1024),
                self.backend.name()
            );
        }

        let seconds = samples.len() as f32 / WHISPER_SAMPLE_RATE as f32;
        info!(
            service = self.backend.name(),
            model = %self.model,
            seconds,
            "Sending the recording to {}",
            self.backend.name()
        );

        let mut form = Form::new();
        for (name, value) in self.fields(prompt) {
            form = form.text(name, value);
        }
        let file = Part::bytes(wav)
            .file_name("dictation.wav")
            .mime_str("audio/wav")
            .context("audio part")?;
        form = form.part("file", file);

        let response = self
            .client
            .post(self.endpoint())
            .bearer_auth(&self.api_key)
            .multipart(form)
            .send()
            .await
            .with_context(|| format!("{} did not answer", self.backend.name()))?;
        let status = response.status();
        let body = response
            .text()
            .await
            .with_context(|| format!("{} sent an unreadable answer", self.backend.name()))?;
        if !status.is_success() {
            anyhow::bail!("{}", error_message(self.backend, status, &body));
        }
        let text = parse_text(&body)
            .with_context(|| format!("{} sent an unexpected answer", self.backend.name()))?;

        let processing_time = start.elapsed();
        info!(
            service = self.backend.name(),
            text_length = text.len(),
            processing_time_ms = processing_time.as_millis(),
            "Cloud transcription completed"
        );
        Ok(TranscriptionResult {
            confidence: if text.is_empty() { 0.0 } else { 0.9 },
            text,
            processing_time,
        })
    }
}

/// 16 kHz, 16-bit mono WAV in memory.
pub fn encode_wav(samples: &[f32]) -> Result<Vec<u8>> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: WHISPER_SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut cursor = Cursor::new(Vec::with_capacity(44 + samples.len() * 2));
    {
        let mut writer = hound::WavWriter::new(&mut cursor, spec).context("WAV header")?;
        for &sample in samples {
            writer
                .write_sample((sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16)
                .context("WAV sample")?;
        }
        writer.finalize().context("WAV trailer")?;
    }
    Ok(cursor.into_inner())
}

fn parse_text(body: &str) -> Result<String> {
    let transcription: Transcription = serde_json::from_str(body)?;
    Ok(strip_non_speech(&transcription.text))
}

fn error_message(backend: TranscriptionBackend, status: StatusCode, body: &str) -> String {
    let detail = serde_json::from_str::<ErrorBody>(body)
        .map(|body| body.error.message)
        .unwrap_or_else(|_| body.chars().take(200).collect());
    let hint = match status {
        StatusCode::UNAUTHORIZED => backend
            .key_var()
            .map(|var| format!(" (check {} in ~/.config/hush/.env)", var))
            .unwrap_or_default(),
        _ => String::new(),
    };
    format!(
        "{} answered HTTP {}: {}{}",
        backend.name(),
        status.as_u16(),
        detail.trim(),
        hint
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn groq(language: &str) -> CloudTranscriber {
        CloudTranscriber::new(
            TranscriptionBackend::Groq,
            "whisper-large-v3-turbo",
            language,
            Some("gsk_test".to_string()),
        )
        .unwrap()
    }

    #[test]
    fn a_missing_key_names_the_variable_and_the_way_back() {
        for key in [None, Some(String::new()), Some("   ".to_string())] {
            let err = CloudTranscriber::new(TranscriptionBackend::OpenAi, "whisper-1", "en", key)
                .err()
                .unwrap()
                .to_string();
            assert!(err.contains("OPENAI_API_KEY"), "{err}");
            assert!(err.contains("backend = \"local\""), "{err}");
        }
        assert!(
            CloudTranscriber::new(TranscriptionBackend::Local, "x", "en", Some("k".into()))
                .is_err(),
            "local is not a cloud service"
        );
    }

    #[test]
    fn each_service_has_its_endpoint() {
        assert!(groq("en").endpoint().starts_with("https://api.groq.com/"));
        let openai = CloudTranscriber::new(
            TranscriptionBackend::OpenAi,
            "gpt-4o-mini-transcribe",
            "en",
            Some("sk-test".into()),
        )
        .unwrap();
        assert!(openai.endpoint().starts_with("https://api.openai.com/"));
        assert_eq!(openai.name(), "OpenAI gpt-4o-mini-transcribe");
        for endpoint in [groq("en").endpoint(), openai.endpoint()] {
            assert!(endpoint.ends_with("/audio/transcriptions"));
        }
    }

    #[test]
    fn the_request_names_the_language_and_carries_the_prompt() {
        let fields = groq("de").fields(Some("  Hush, whisper.cpp. "));
        assert!(fields.contains(&("model", "whisper-large-v3-turbo".to_string())));
        assert!(fields.contains(&("response_format", "json".to_string())));
        assert!(fields.contains(&("language", "de".to_string())));
        assert!(fields.contains(&("prompt", "Hush, whisper.cpp.".to_string())));
    }

    #[test]
    fn detection_and_an_empty_prompt_leave_their_fields_out() {
        for language in ["auto", "", "  "] {
            let fields = groq(language).fields(Some("   "));
            let names: Vec<&str> = fields.iter().map(|(name, _)| *name).collect();
            assert_eq!(names, ["model", "response_format"], "language {language:?}");
        }
        assert_eq!(groq("en").fields(None).len(), 3);
    }

    #[test]
    fn audio_is_encoded_as_16_khz_mono_wav() {
        let samples = [0.0, 0.5, -0.5, 2.0, -2.0];
        let wav = encode_wav(&samples).unwrap();
        let mut reader = hound::WavReader::new(Cursor::new(wav)).unwrap();
        let spec = reader.spec();
        assert_eq!(spec.sample_rate, 16_000);
        assert_eq!(spec.channels, 1);
        assert_eq!(spec.bits_per_sample, 16);
        let decoded: Vec<i16> = reader.samples::<i16>().map(|s| s.unwrap()).collect();
        assert_eq!(decoded, [0, 16383, -16383, i16::MAX, -i16::MAX]);
    }

    #[test]
    fn ten_minutes_fits_the_upload_limit() {
        let ten_minutes = 600 * WHISPER_SAMPLE_RATE as usize;
        assert!(44 + ten_minutes * 2 < MAX_UPLOAD_BYTES);
    }

    #[test]
    fn the_answer_is_parsed_and_tags_are_dropped() {
        assert_eq!(
            parse_text(r#"{"text":" Hello there. [BLANK_AUDIO]"}"#).unwrap(),
            "Hello there."
        );
        assert_eq!(
            parse_text(r#"{"text":"ok","usage":{"seconds":3}}"#).unwrap(),
            "ok"
        );
        assert!(parse_text("<html>bad gateway</html>").is_err());
    }

    #[test]
    fn a_refusal_shows_the_service_message() {
        let body = r#"{"error":{"message":"Invalid API Key","type":"invalid_request_error"}}"#;
        let message = error_message(TranscriptionBackend::Groq, StatusCode::UNAUTHORIZED, body);
        assert_eq!(
            message,
            "Groq answered HTTP 401: Invalid API Key (check GROQ_API_KEY in ~/.config/hush/.env)"
        );
        let message = error_message(
            TranscriptionBackend::OpenAi,
            StatusCode::BAD_GATEWAY,
            "upstream down",
        );
        assert_eq!(message, "OpenAI answered HTTP 502: upstream down");
    }

    #[tokio::test]
    async fn silence_is_not_sent() {
        let result = groq("en")
            .transcribe_with_prompt(&[], 16_000, None)
            .await
            .unwrap();
        assert!(result.text.is_empty());
    }
}
