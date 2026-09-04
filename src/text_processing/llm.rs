//! LLM polishing and Command Mode rewrites over HTTP: Anthropic's Messages
//! API or a local Ollama server. Only text is sent, never audio.

use super::config::{EditingMode, LlmProvider};
use crate::config::settings::{LlmConfig, LlmSetting};
use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, info, warn};

const OLLAMA_PROBE_TIMEOUT: Duration = Duration::from_millis(600);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

pub struct LlmProcessor {
    provider: LlmProvider,
    max_tokens: u32,
    temperature: f32,
    client: Client,
}

#[derive(Serialize)]
struct ClaudeRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    temperature: f32,
    system: &'a str,
    messages: Vec<ClaudeMessage<'a>>,
}

#[derive(Serialize)]
struct ClaudeMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct ClaudeResponse {
    content: Vec<ClaudeContent>,
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct ClaudeContent {
    #[serde(rename = "type")]
    content_type: String,
    text: Option<String>,
}

#[derive(Deserialize)]
struct Usage {
    input_tokens: u32,
    output_tokens: u32,
}

#[derive(Serialize)]
struct OllamaRequest<'a> {
    model: &'a str,
    messages: Vec<OllamaMessage<'a>>,
    stream: bool,
    options: OllamaOptions,
}

#[derive(Serialize)]
struct OllamaMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Serialize)]
struct OllamaOptions {
    temperature: f32,
    num_predict: u32,
}

#[derive(Deserialize)]
struct OllamaResponse {
    message: OllamaReply,
}

#[derive(Deserialize)]
struct OllamaReply {
    content: String,
}

const REWRITE_SYSTEM: &str = "You edit text on the user's behalf. Apply the spoken instruction to the text and return the resulting text. \
Keep everything the instruction does not ask to change, including the language, formatting, line breaks, and code. \
RETURN ONLY THE RESULTING TEXT. Do not add explanations, quotes, or any other content.";

impl LlmProcessor {
    pub fn new(provider: LlmProvider, max_tokens: u32, temperature: f32) -> Result<Self> {
        match &provider {
            LlmProvider::Anthropic { api_key, model } => {
                let key = match api_key {
                    Some(key) => key.clone(),
                    None => std::env::var("ANTHROPIC_API_KEY")
                        .context("ANTHROPIC_API_KEY not found in environment")?,
                };
                if key.is_empty() {
                    anyhow::bail!("ANTHROPIC_API_KEY is empty");
                }
                info!("LLM: Anthropic {}", model);
            },
            LlmProvider::Ollama { url, model } => {
                info!("LLM: Ollama {} at {}", model, url);
            },
            LlmProvider::None => anyhow::bail!("No LLM provider configured"),
        }
        let client = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .context("HTTP client")?;
        Ok(Self {
            provider,
            max_tokens,
            temperature,
            client,
        })
    }

    pub fn provider_name(&self) -> String {
        match &self.provider {
            LlmProvider::Anthropic { model, .. } => format!("Anthropic {}", model),
            LlmProvider::Ollama { model, .. } => format!("Ollama {}", model),
            LlmProvider::None => "none".to_string(),
        }
    }

    /// Polish text using the configured LLM
    pub async fn polish(&self, text: &str, mode: EditingMode) -> Result<String> {
        self.polish_with_context(text, mode, None, None).await
    }

    /// Polish with the application's tone and, when known, where the text goes.
    pub async fn polish_with_context(
        &self,
        text: &str,
        mode: EditingMode,
        tone: Option<&str>,
        place: Option<&str>,
    ) -> Result<String> {
        let system = Self::create_system_prompt_with(mode, tone, place);
        let user = format!(
            "Edit this voice transcription:\n\n{}\n\nReturn ONLY the edited text, nothing else.",
            text
        );
        let polished = self.complete(&system, &user).await?;
        debug!("Polished result: '{}'", polished);
        Ok(polished)
    }

    /// Command Mode: apply a spoken instruction to some text.
    pub async fn rewrite(&self, instruction: &str, target: &str) -> Result<String> {
        let user = rewrite_prompt(instruction, target);
        let result = self.complete(REWRITE_SYSTEM, &user).await?;
        Ok(strip_wrapping(&result))
    }

    async fn complete(&self, system: &str, user: &str) -> Result<String> {
        match &self.provider {
            LlmProvider::Anthropic { api_key, model } => {
                self.claude_complete(system, user, api_key.as_deref(), model)
                    .await
            },
            LlmProvider::Ollama { url, model } => {
                self.ollama_complete(system, user, url, model).await
            },
            LlmProvider::None => Ok(user.to_string()),
        }
    }

    async fn claude_complete(
        &self,
        system: &str,
        user: &str,
        api_key: Option<&str>,
        model: &str,
    ) -> Result<String> {
        let key = match api_key {
            Some(key) => key.to_string(),
            None => std::env::var("ANTHROPIC_API_KEY").context("ANTHROPIC_API_KEY not found")?,
        };
        let request = ClaudeRequest {
            model,
            max_tokens: self.max_tokens,
            temperature: self.temperature,
            system,
            messages: vec![ClaudeMessage {
                role: "user",
                content: user,
            }],
        };
        let start = std::time::Instant::now();
        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to send request to the Anthropic API")?;
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("Anthropic API error ({}): {}", status, error_text);
        }
        let parsed: ClaudeResponse = response
            .json()
            .await
            .context("Failed to parse the Anthropic API response")?;
        let text = parsed
            .content
            .iter()
            .filter(|block| block.content_type == "text")
            .filter_map(|block| block.text.as_deref())
            .collect::<Vec<_>>()
            .join(" ")
            .trim()
            .to_string();
        if let Some(usage) = parsed.usage {
            info!(
                "Anthropic {}: {} in, {} out, {} ms",
                model,
                usage.input_tokens,
                usage.output_tokens,
                start.elapsed().as_millis()
            );
        }
        Ok(text)
    }

    async fn ollama_complete(
        &self,
        system: &str,
        user: &str,
        url: &str,
        model: &str,
    ) -> Result<String> {
        let request = OllamaRequest {
            model,
            messages: vec![
                OllamaMessage {
                    role: "system",
                    content: system,
                },
                OllamaMessage {
                    role: "user",
                    content: user,
                },
            ],
            stream: false,
            options: OllamaOptions {
                temperature: self.temperature,
                num_predict: self.max_tokens,
            },
        };
        let start = std::time::Instant::now();
        let response = self
            .client
            .post(format!("{}/api/chat", url.trim_end_matches('/')))
            .json(&request)
            .send()
            .await
            .with_context(|| format!("Failed to reach Ollama at {}", url))?;
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("Ollama error ({}): {}", status, error_text);
        }
        let parsed: OllamaResponse = response
            .json()
            .await
            .context("Failed to parse the Ollama response")?;
        info!("Ollama {}: {} ms", model, start.elapsed().as_millis());
        Ok(parsed.message.content.trim().to_string())
    }

    /// System prompt for the editing mode plus the application's tone.
    pub fn create_system_prompt_with(
        mode: EditingMode,
        tone: Option<&str>,
        place: Option<&str>,
    ) -> String {
        let mut prompt = Self::create_system_prompt(mode);
        let context: Vec<&str> = tone.into_iter().chain(place).collect();
        if !context.is_empty() {
            prompt.push_str("\n\nContext:\n");
            for line in context {
                prompt.push_str("- ");
                prompt.push_str(line);
                prompt.push('\n');
            }
        }
        prompt
    }

    /// Create system prompt based on editing mode
    fn create_system_prompt(mode: EditingMode) -> String {
        let base =
            "You are a professional text editor. Your job is to polish voice transcriptions.";

        let instructions = match mode {
            EditingMode::Light => {
                "Make minimal edits:
- Fix only obvious grammar errors
- Remove only the most common filler words (um, uh, er, ah)
- Add basic punctuation if completely missing
- Preserve the speaker's original style and voice completely"
            },
            EditingMode::Medium => {
                "Make balanced edits:
- Fix grammar and punctuation errors
- Remove filler words (um, uh, like, you know, I mean, basically, actually)
- Improve sentence structure slightly while maintaining meaning
- Keep the speaker's tone but make it more professional"
            },
            EditingMode::Aggressive => {
                "Make comprehensive edits:
- Rewrite for maximum clarity and professionalism
- Remove all filler words and hesitations
- Fix grammar, spelling, and sentence structure completely
- Reorganize for better flow while preserving all key information
- Make it sound polished and articulate"
            },
        };

        format!("{}\n\n{}\n\nRETURN ONLY THE EDITED TEXT. Do not add explanations, quotes, or any other content.", base, instructions)
    }

    /// Check if LLM is available and ready
    pub fn is_ready(&self) -> bool {
        !matches!(self.provider, LlmProvider::None)
    }
}

fn rewrite_prompt(instruction: &str, target: &str) -> String {
    format!(
        "Instruction: {}\n\nText:\n{}\n\nReturn ONLY the resulting text.",
        instruction.trim(),
        target
    )
}

/// Models sometimes wrap a short answer in quotes or a code fence.
fn strip_wrapping(text: &str) -> String {
    let trimmed = text.trim();
    let unfenced = trimmed
        .strip_prefix("```")
        .and_then(|rest| rest.strip_suffix("```"))
        .map(|inner| {
            inner
                .trim_start_matches(|c: char| c.is_alphanumeric())
                .trim()
        })
        .unwrap_or(trimmed);
    let unquoted = if unfenced.len() >= 2
        && unfenced.starts_with('"')
        && unfenced.ends_with('"')
        && !unfenced[1..unfenced.len() - 1].contains('"')
    {
        &unfenced[1..unfenced.len() - 1]
    } else {
        unfenced
    };
    unquoted.to_string()
}

/// Whether an Ollama server answers at `url`.
pub async fn ollama_reachable(url: &str) -> bool {
    let client = match Client::builder().timeout(OLLAMA_PROBE_TIMEOUT).build() {
        Ok(client) => client,
        Err(_) => return false,
    };
    client
        .get(format!("{}/api/tags", url.trim_end_matches('/')))
        .send()
        .await
        .map(|response| response.status().is_success())
        .unwrap_or(false)
}

/// The provider for a setting, given what is available. `auto` prefers the
/// local Ollama server, then Anthropic when a key exists.
pub fn choose_provider(
    config: &LlmConfig,
    ollama_ok: bool,
    anthropic_key: Option<String>,
) -> LlmProvider {
    let ollama = || LlmProvider::Ollama {
        url: config.ollama_url.clone(),
        model: config.ollama_model.clone(),
    };
    let anthropic = |key: String| LlmProvider::Anthropic {
        api_key: Some(key),
        model: config.anthropic_model.clone(),
    };
    let key = anthropic_key.filter(|k| !k.trim().is_empty());
    match config.provider {
        LlmSetting::None => LlmProvider::None,
        LlmSetting::Ollama => {
            if ollama_ok {
                ollama()
            } else {
                warn!("Ollama is not reachable at {}", config.ollama_url);
                LlmProvider::None
            }
        },
        LlmSetting::Anthropic => match key {
            Some(key) => anthropic(key),
            None => {
                warn!("llm.provider is anthropic but ANTHROPIC_API_KEY is not set");
                LlmProvider::None
            },
        },
        LlmSetting::Auto => {
            if ollama_ok {
                ollama()
            } else if let Some(key) = key {
                anthropic(key)
            } else {
                LlmProvider::None
            }
        },
    }
}

/// Probe what the config needs and pick the provider.
pub async fn resolve_provider(config: &LlmConfig) -> LlmProvider {
    let ollama_ok = match config.provider {
        LlmSetting::Auto | LlmSetting::Ollama => ollama_reachable(&config.ollama_url).await,
        _ => false,
    };
    let key = std::env::var("ANTHROPIC_API_KEY").ok();
    choose_provider(config, ollama_ok, key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_prompt_creation() {
        let light = LlmProcessor::create_system_prompt(EditingMode::Light);
        assert!(light.contains("minimal"));
        assert!(light.contains("RETURN ONLY"));

        let aggressive = LlmProcessor::create_system_prompt(EditingMode::Aggressive);
        assert!(aggressive.contains("comprehensive"));
        assert!(aggressive.contains("professionalism"));
    }

    #[test]
    fn tone_and_place_are_appended_as_context() {
        let prompt = LlmProcessor::create_system_prompt_with(
            EditingMode::Light,
            Some("Chat message"),
            Some("The text goes into chat"),
        );
        assert!(prompt.contains("Context:\n- Chat message\n- The text goes into chat"));
        assert_eq!(
            LlmProcessor::create_system_prompt_with(EditingMode::Light, None, None),
            LlmProcessor::create_system_prompt(EditingMode::Light)
        );
    }

    #[test]
    fn rewrite_prompt_and_unwrapping() {
        let prompt = rewrite_prompt("  make it shorter ", "a long sentence");
        assert!(prompt.starts_with("Instruction: make it shorter\n\nText:\na long sentence"));
        assert_eq!(strip_wrapping("\"hello\""), "hello");
        assert_eq!(strip_wrapping("```text\nhello\n```"), "hello");
        assert_eq!(strip_wrapping("say \"hi\" now"), "say \"hi\" now");
        assert_eq!(strip_wrapping("  plain  "), "plain");
    }

    #[test]
    fn provider_order_is_ollama_then_anthropic() {
        let config = LlmConfig::defaults();
        assert!(matches!(
            choose_provider(&config, true, Some("k".into())),
            LlmProvider::Ollama { .. }
        ));
        assert!(matches!(
            choose_provider(&config, false, Some("k".into())),
            LlmProvider::Anthropic { .. }
        ));
        assert_eq!(
            choose_provider(&config, false, Some("  ".into())),
            LlmProvider::None
        );
        let anthropic_only = LlmConfig {
            provider: LlmSetting::Anthropic,
            ..LlmConfig::defaults()
        };
        assert_eq!(
            choose_provider(&anthropic_only, true, None),
            LlmProvider::None
        );
        let off = LlmConfig {
            provider: LlmSetting::None,
            ..LlmConfig::defaults()
        };
        assert_eq!(
            choose_provider(&off, true, Some("k".into())),
            LlmProvider::None
        );
    }

    #[test]
    fn ollama_request_shape() {
        let request = OllamaRequest {
            model: "llama3.2",
            messages: vec![OllamaMessage {
                role: "user",
                content: "hi",
            }],
            stream: false,
            options: OllamaOptions {
                temperature: 0.3,
                num_predict: 200,
            },
        };
        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(json["stream"], false);
        assert_eq!(json["options"]["num_predict"], 200);
        assert_eq!(json["messages"][0]["role"], "user");
    }
}
