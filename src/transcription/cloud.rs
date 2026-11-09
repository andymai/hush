//! Transcription over HTTP through the OpenAI `/audio/transcriptions` API:
//! Groq, OpenAI, or a server at an address the user sets. Hush uses it only
//! when `transcription.backend` names one, and a failure is reported, never
//! retried on another backend.

use crate::config::keys;
use crate::config::server::{self, Sends};
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
use std::path::Path;
use std::time::{Duration, Instant};
use tracing::info;

/// Both hosted services refuse uploads over 25 MB, which is about 13
/// minutes of 16 kHz, 16-bit mono audio.
const MAX_UPLOAD_BYTES: usize = 25 * 1024 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Long enough to upload a ten-minute recording on a slow connection.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

pub struct CloudTranscriber {
    backend: TranscriptionBackend,
    endpoint: String,
    /// The name in log lines and errors: the service, or the server's host
    service: String,
    model: String,
    language: String,
    api_key: Option<String>,
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
    /// environment or `env_file` (`~/.config/hush/.env`).
    pub fn from_config(config: &TranscriptionConfig, env_file: &Path) -> Result<Self> {
        if config.backend != TranscriptionBackend::OpenAiCompatible {
            let key = config
                .backend
                .key_var()
                .and_then(|var| keys::api_key_in(env_file, var));
            return Self::new(
                config.backend,
                config.cloud_model().unwrap_or_default(),
                &config.language,
                key,
            );
        }
        let key = match config.key_var() {
            Some(var) => Some(keys::api_key_in(env_file, var).with_context(|| {
                format!(
                    "transcription.server_key_env is {} but {} is not set. Put it in \
                     ~/.config/hush/.env, or set server_key_env = \"\"",
                    var, var
                )
            })?),
            None => None,
        };
        Self::for_server(
            &config.server_url,
            &config.server_model,
            &config.language,
            key,
        )
    }

    /// Groq or OpenAI, which always need a key.
    pub fn new(
        backend: TranscriptionBackend,
        model: &str,
        language: &str,
        api_key: Option<String>,
    ) -> Result<Self> {
        let (Some(var), Some(endpoint)) = (backend.key_var(), hosted_endpoint(backend)) else {
            anyhow::bail!("{} is not a hosted service", backend.name());
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
        Self::build(
            backend,
            endpoint.to_string(),
            backend.name().to_string(),
            model,
            language,
            Some(api_key),
        )
    }

    /// A server that speaks the OpenAI transcription API at `base_url`. The
    /// key goes as a bearer token when there is one.
    pub fn for_server(
        base_url: &str,
        model: &str,
        language: &str,
        api_key: Option<String>,
    ) -> Result<Self> {
        let endpoint = server::endpoint(base_url, server::TRANSCRIPTIONS)
            .map_err(|e| anyhow::anyhow!("Invalid transcription.server_url: {}", e))?;
        let service = server::host_label(&endpoint);
        Self::build(
            TranscriptionBackend::OpenAiCompatible,
            endpoint,
            service,
            model,
            language,
            api_key.filter(|key| !key.trim().is_empty()),
        )
    }

    fn build(
        backend: TranscriptionBackend,
        endpoint: String,
        service: String,
        model: &str,
        language: &str,
        api_key: Option<String>,
    ) -> Result<Self> {
        let client = Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .context("HTTP client")?;
        let language = language.trim();
        Ok(Self {
            backend,
            endpoint,
            service,
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
        format!("{} {}", self.service, self.model)
    }

    /// Where the audio goes, for logs and status lines.
    pub fn describe(&self) -> String {
        match self.backend {
            TranscriptionBackend::OpenAiCompatible => {
                server::describe(&self.model, &self.endpoint, Sends::Audio)
            },
            _ => format!("{}; audio leaves this machine", self.name()),
        }
    }

    fn endpoint(&self) -> &str {
        &self.endpoint
    }

    fn key_hint(&self) -> String {
        match self.backend.key_var() {
            Some(var) => format!(" (check {} in ~/.config/hush/.env)", var),
            None => " (check server_key_env under [transcription])".to_string(),
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
        let hosted = self.backend != TranscriptionBackend::OpenAiCompatible;
        if hosted && wav.len() > MAX_UPLOAD_BYTES {
            anyhow::bail!(
                "The recording is {} MB and {} accepts at most 25 MB, about 13 minutes",
                wav.len() / (1024 * 1024),
                self.service
            );
        }

        let seconds = samples.len() as f32 / WHISPER_SAMPLE_RATE as f32;
        info!(
            service = %self.service,
            model = %self.model,
            seconds,
            "Sending the recording to {}",
            self.service
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

        let mut request = self.client.post(self.endpoint()).multipart(form);
        if let Some(key) = &self.api_key {
            request = request.bearer_auth(key);
        }
        let response = request
            .send()
            .await
            .with_context(|| format!("{} did not answer", self.service))?;
        let status = response.status();
        let body = response
            .text()
            .await
            .with_context(|| format!("{} sent an unreadable answer", self.service))?;
        if !status.is_success() {
            anyhow::bail!(
                "{}",
                error_message(&self.service, &self.key_hint(), status, &body)
            );
        }
        let text = parse_text(&body)
            .with_context(|| format!("{} sent an unexpected answer", self.service))?;

        let processing_time = start.elapsed();
        info!(
            service = %self.service,
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

fn hosted_endpoint(backend: TranscriptionBackend) -> Option<&'static str> {
    match backend {
        TranscriptionBackend::Groq => Some("https://api.groq.com/openai/v1/audio/transcriptions"),
        TranscriptionBackend::OpenAi => Some("https://api.openai.com/v1/audio/transcriptions"),
        TranscriptionBackend::Local | TranscriptionBackend::OpenAiCompatible => None,
    }
}

fn error_message(service: &str, key_hint: &str, status: StatusCode, body: &str) -> String {
    let detail = serde_json::from_str::<ErrorBody>(body)
        .map(|body| body.error.message)
        .unwrap_or_else(|_| body.chars().take(200).collect());
    let hint = match status {
        StatusCode::UNAUTHORIZED => key_hint,
        _ => "",
    };
    format!(
        "{} answered HTTP {}: {}{}",
        service,
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
        let groq = groq("en");
        let message = error_message(
            &groq.service,
            &groq.key_hint(),
            StatusCode::UNAUTHORIZED,
            body,
        );
        assert_eq!(
            message,
            "Groq answered HTTP 401: Invalid API Key (check GROQ_API_KEY in ~/.config/hush/.env)"
        );
        let message = error_message("OpenAI", "", StatusCode::BAD_GATEWAY, "upstream down");
        assert_eq!(message, "OpenAI answered HTTP 502: upstream down");

        let lan = on_server("http://192.168.1.5:8080", None);
        let message = error_message(
            &lan.service,
            &lan.key_hint(),
            StatusCode::UNAUTHORIZED,
            "no key",
        );
        assert_eq!(
            message,
            "192.168.1.5:8080 answered HTTP 401: no key (check server_key_env under [transcription])"
        );
    }

    fn on_server(base: &str, key: Option<&str>) -> CloudTranscriber {
        CloudTranscriber::for_server(base, "whisper-large-v3", "en", key.map(String::from)).unwrap()
    }

    #[test]
    fn a_server_gets_the_api_path_and_says_where_it_runs() {
        let local = on_server("http://localhost:8080/", None);
        assert_eq!(
            local.endpoint(),
            "http://localhost:8080/v1/audio/transcriptions"
        );
        assert_eq!(
            local.describe(),
            "whisper-large-v3 on the local server localhost:8080"
        );
        let remote = on_server("https://stt.example.com/v1", Some("k"));
        assert_eq!(
            remote.endpoint(),
            "https://stt.example.com/v1/audio/transcriptions"
        );
        assert_eq!(
            remote.describe(),
            "whisper-large-v3 on stt.example.com, a service that receives the audio"
        );
        assert!(on_server("http://h", Some("  ")).api_key.is_none());
        for bad in ["", "localhost:8080"] {
            let err = CloudTranscriber::for_server(bad, "m", "en", None)
                .err()
                .unwrap()
                .to_string();
            assert!(err.contains("transcription.server_url"), "{err}");
        }
    }

    #[test]
    fn a_server_key_that_is_not_set_names_the_variable() {
        let dir = tempfile::tempdir().unwrap();
        let env_file = dir.path().join(".env");
        let config = TranscriptionConfig {
            backend: TranscriptionBackend::OpenAiCompatible,
            server_url: "http://localhost:8080".into(),
            server_model: "whisper-1".into(),
            server_key_env: "HUSH_TEST_UNSET_SERVER_KEY".into(),
            ..crate::config::Config::defaults().unwrap().transcription
        };
        let err = CloudTranscriber::from_config(&config, &env_file)
            .err()
            .unwrap()
            .to_string();
        assert!(err.contains("HUSH_TEST_UNSET_SERVER_KEY"), "{err}");
        assert!(err.contains("server_key_env = \"\""), "{err}");

        std::fs::write(&env_file, "HUSH_TEST_UNSET_SERVER_KEY=sk-from-file\n").unwrap();
        assert_eq!(
            CloudTranscriber::from_config(&config, &env_file)
                .unwrap()
                .api_key
                .as_deref(),
            Some("sk-from-file")
        );

        let no_key = TranscriptionConfig {
            server_key_env: String::new(),
            ..config
        };
        assert!(CloudTranscriber::from_config(&no_key, &env_file)
            .unwrap()
            .api_key
            .is_none());
    }

    #[tokio::test]
    async fn a_take_goes_to_the_server_as_multipart_with_the_key() {
        let (url, request) = server::fake_server(r#"{"text":" Hello there. "}"#.into()).await;
        let transcriber = CloudTranscriber::for_server(
            &url,
            "my-whisper",
            "de",
            Some("local-secret".to_string()),
        )
        .unwrap();
        let result = transcriber
            .transcribe_with_prompt(&[0.1; 1600], 16_000, Some("Hush"))
            .await
            .unwrap();
        assert_eq!(result.text, "Hello there.");

        let request = String::from_utf8_lossy(&request.await.unwrap()).into_owned();
        let lower = request.to_lowercase();
        assert!(
            request.starts_with("POST /v1/audio/transcriptions HTTP/1.1\r\n"),
            "{request}"
        );
        assert!(
            lower.contains("\r\nauthorization: bearer local-secret\r\n"),
            "{request}"
        );
        assert!(
            lower.contains("content-type: multipart/form-data; boundary="),
            "{request}"
        );
        for (name, value) in [
            ("model", "my-whisper"),
            ("language", "de"),
            ("prompt", "Hush"),
        ] {
            let field = format!("name=\"{}\"\r\n\r\n{}\r\n", name, value);
            assert!(request.contains(&field), "{name}: {request}");
        }
        assert!(request.contains("name=\"file\"; filename=\"dictation.wav\""));
    }

    #[tokio::test]
    async fn a_server_without_a_key_gets_no_authorization_header() {
        let (url, request) = server::fake_server(r#"{"text":"ok"}"#.into()).await;
        let transcriber = CloudTranscriber::for_server(&url, "m", "en", None).unwrap();
        transcriber
            .transcribe_with_prompt(&[0.1; 160], 16_000, None)
            .await
            .unwrap();
        let request = String::from_utf8_lossy(&request.await.unwrap()).to_lowercase();
        assert!(!request.contains("authorization:"), "{request}");
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
