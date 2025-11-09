/// LLM-based text polishing using Anthropic Claude API via HTTP
use anyhow::{Result, Context};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};
use super::config::{EditingMode, LlmProvider};

/// LLM processor for grammar correction and polishing
pub struct LlmProcessor {
    provider: LlmProvider,
    max_tokens: u32,
    temperature: f32,
    client: Client,
}

#[derive(Serialize)]
struct ClaudeRequest {
    model: String,
    max_tokens: u32,
    temperature: f32,
    system: String,
    messages: Vec<ClaudeMessage>,
}

#[derive(Serialize)]
struct ClaudeMessage {
    role: String,
    content: String,
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

impl LlmProcessor {
    /// Initialize the LLM processor with the given provider
    pub fn new(provider: LlmProvider, max_tokens: u32, temperature: f32) -> Result<Self> {
        match &provider {
            LlmProvider::Anthropic { api_key, model } => {
                info!("Initializing Claude API client");
                info!("  Model: {}", model);

                // Verify API key is available
                let key = if let Some(k) = api_key {
                    k.clone()
                } else {
                    std::env::var("ANTHROPIC_API_KEY")
                        .context("ANTHROPIC_API_KEY not found in environment")?
                };

                if key.is_empty() {
                    anyhow::bail!("ANTHROPIC_API_KEY is empty");
                }

                info!("✅ Claude API client initialized");
                Ok(Self {
                    provider,
                    max_tokens,
                    temperature,
                    client: Client::new(),
                })
            }
            LlmProvider::OpenAI { api_key, model } => {
                info!("Initializing OpenAI API client");
                info!("  Model: {}", model);

                // Verify API key is available
                let key = if let Some(k) = api_key {
                    k.clone()
                } else {
                    std::env::var("OPENAI_API_KEY")
                        .context("OPENAI_API_KEY not found in environment")?
                };

                if key.is_empty() {
                    anyhow::bail!("OPENAI_API_KEY is empty");
                }

                warn!("OpenAI integration not yet implemented, will use rule-based");
                Ok(Self {
                    provider,
                    max_tokens,
                    temperature,
                    client: Client::new(),
                })
            }
            LlmProvider::None => {
                anyhow::bail!("No LLM provider configured");
            }
        }
    }

    /// Polish text using the configured LLM
    pub async fn polish(&self, text: &str, mode: EditingMode) -> Result<String> {
        match &self.provider {
            LlmProvider::Anthropic { api_key, model } => {
                self.polish_with_claude(text, mode, api_key, model).await
            }
            LlmProvider::OpenAI { .. } => {
                warn!("OpenAI polishing not implemented, returning input");
                Ok(text.to_string())
            }
            LlmProvider::None => {
                debug!("No LLM provider, skipping polishing");
                Ok(text.to_string())
            }
        }
    }

    /// Polish text using Claude API via HTTP
    async fn polish_with_claude(
        &self,
        text: &str,
        mode: EditingMode,
        api_key: &Option<String>,
        model_name: &str,
    ) -> Result<String> {
        debug!("Polishing with Claude: '{}'", text);

        // Get API key
        let key = if let Some(k) = api_key {
            k.clone()
        } else {
            std::env::var("ANTHROPIC_API_KEY")
                .context("ANTHROPIC_API_KEY not found")?
        };

        // Create prompts
        let system_prompt = Self::create_system_prompt(mode);
        let user_prompt = format!(
            "Edit this voice transcription:\n\n{}\n\nReturn ONLY the edited text, nothing else.",
            text
        );

        debug!("System prompt: {}", system_prompt);
        debug!("User prompt: {}", user_prompt);

        // Create request
        let request = ClaudeRequest {
            model: model_name.to_string(),
            max_tokens: self.max_tokens,
            temperature: self.temperature,
            system: system_prompt,
            messages: vec![ClaudeMessage {
                role: "user".to_string(),
                content: user_prompt,
            }],
        };

        // Send request
        let start = std::time::Instant::now();
        let response = self.client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to send request to Claude API")?;

        let elapsed = start.elapsed();

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("Claude API error ({}): {}", status, error_text);
        }

        let claude_response: ClaudeResponse = response
            .json()
            .await
            .context("Failed to parse Claude API response")?;

        // Extract text from response
        let polished = claude_response
            .content
            .iter()
            .filter_map(|block| {
                if block.content_type == "text" {
                    block.text.as_deref()
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
            .trim()
            .to_string();

        // Log cost estimation
        if let Some(usage) = claude_response.usage {
            let input_tokens = usage.input_tokens;
            let output_tokens = usage.output_tokens;

            // Haiku pricing: $0.25/M input, $1.25/M output
            let cost = (input_tokens as f64 / 1_000_000.0) * 0.25
                     + (output_tokens as f64 / 1_000_000.0) * 1.25;

            info!("Claude API: {} input tokens, {} output tokens, ~${:.6}, {:.2}ms",
                input_tokens, output_tokens, cost, elapsed.as_millis());
        }

        debug!("Polished result: '{}'", polished);

        Ok(polished)
    }

    /// Create system prompt based on editing mode
    fn create_system_prompt(mode: EditingMode) -> String {
        let base = "You are a professional text editor. Your job is to polish voice transcriptions.";

        let instructions = match mode {
            EditingMode::Light => {
                "Make minimal edits:
- Fix only obvious grammar errors
- Remove only the most common filler words (um, uh, er, ah)
- Add basic punctuation if completely missing
- Preserve the speaker's original style and voice completely"
            }
            EditingMode::Medium => {
                "Make balanced edits:
- Fix grammar and punctuation errors
- Remove filler words (um, uh, like, you know, I mean, basically, actually)
- Improve sentence structure slightly while maintaining meaning
- Keep the speaker's tone but make it more professional"
            }
            EditingMode::Aggressive => {
                "Make comprehensive edits:
- Rewrite for maximum clarity and professionalism
- Remove all filler words and hesitations
- Fix grammar, spelling, and sentence structure completely
- Reorganize for better flow while preserving all key information
- Make it sound polished and articulate"
            }
        };

        format!("{}\n\n{}\n\nRETURN ONLY THE EDITED TEXT. Do not add explanations, quotes, or any other content.", base, instructions)
    }

    /// Check if LLM is available and ready
    pub fn is_ready(&self) -> bool {
        !matches!(self.provider, LlmProvider::None)
    }
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
}
