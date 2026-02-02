//! LLM API client for AI translation

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LlmProvider {
    OpenAI,
    Claude,
    Ollama,
    Google,
    DeepL,
}

impl LlmProvider {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "openai" => Self::OpenAI,
            "claude" | "anthropic" => Self::Claude,
            "ollama" => Self::Ollama,
            "google" => Self::Google,
            "deepl" => Self::DeepL,
            _ => Self::OpenAI,
        }
    }

    pub fn is_machine_translate(&self) -> bool {
        matches!(self, Self::Google | Self::DeepL)
    }

    pub fn default_base_url(&self) -> &str {
        match self {
            Self::OpenAI => "https://api.openai.com/v1",
            Self::Claude => "https://api.anthropic.com/v1",
            Self::Ollama => "http://localhost:11434",
            Self::Google | Self::DeepL => "", // Handled by machine_translate module
        }
    }

    pub fn default_model(&self) -> &str {
        match self {
            Self::OpenAI => "gpt-4o-mini",
            Self::Claude => "claude-sonnet-4-20250514",
            Self::Ollama => "llama3",
            Self::Google | Self::DeepL => "", // Not applicable
        }
    }
}

#[derive(Debug, Clone)]
pub struct LlmConfig {
    pub provider: LlmProvider,
    pub api_key: Option<String>,
    pub base_url: String,
    pub model: String,
    pub target_lang: String,
}

impl LlmConfig {
    pub fn new(provider: LlmProvider, target_lang: &str) -> Self {
        Self {
            base_url: provider.default_base_url().to_string(),
            model: provider.default_model().to_string(),
            provider,
            api_key: None,
            target_lang: target_lang.to_string(),
        }
    }

    pub fn with_api_key(mut self, key: Option<String>) -> Self {
        self.api_key = key;
        self
    }

    pub fn with_base_url(mut self, url: Option<String>) -> Self {
        if let Some(u) = url {
            self.base_url = u;
        }
        self
    }

    pub fn with_model(mut self, model: Option<String>) -> Self {
        if let Some(m) = model {
            self.model = m;
        }
        self
    }
}

#[derive(Debug, Serialize)]
struct OpenAIRequest {
    model: String,
    messages: Vec<Message>,
    temperature: f32,
}

#[derive(Debug, Serialize, Deserialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct OpenAIResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: Message,
}

#[derive(Debug, Serialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct OllamaResponse {
    response: String,
}

pub struct LlmClient {
    config: LlmConfig,
    client: reqwest::blocking::Client,
}

impl LlmClient {
    pub fn new(config: LlmConfig) -> Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self { config, client })
    }

    pub fn translate(&self, text: &str, context: Option<&str>) -> Result<String> {
        match self.config.provider {
            LlmProvider::OpenAI | LlmProvider::Claude => {
                self.translate_openai_compatible(text, context)
            }
            LlmProvider::Ollama => self.translate_ollama(text, context),
            LlmProvider::Google | LlmProvider::DeepL => {
                anyhow::bail!("Use MachineTranslateClient for Google/DeepL")
            }
        }
    }

    fn translate_openai_compatible(&self, text: &str, context: Option<&str>) -> Result<String> {
        let system_prompt = self.build_system_prompt();
        let user_prompt = self.build_user_prompt(text, context);

        let request = OpenAIRequest {
            model: self.config.model.clone(),
            messages: vec![
                Message {
                    role: "system".to_string(),
                    content: system_prompt,
                },
                Message {
                    role: "user".to_string(),
                    content: user_prompt,
                },
            ],
            temperature: 0.3,
        };

        let url = format!("{}/chat/completions", self.config.base_url);

        let mut req = self.client.post(&url).json(&request);

        if let Some(ref key) = self.config.api_key {
            req = req.header("Authorization", format!("Bearer {}", key));
        }

        let response = req.send().context("Failed to send request to LLM API")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            anyhow::bail!("API request failed ({}): {}", status, body);
        }

        let result: OpenAIResponse = response.json().context("Failed to parse API response")?;

        result
            .choices
            .first()
            .map(|c| c.message.content.trim().to_string())
            .context("No response from API")
    }

    fn translate_ollama(&self, text: &str, context: Option<&str>) -> Result<String> {
        let prompt = format!(
            "{}\n\n{}",
            self.build_system_prompt(),
            self.build_user_prompt(text, context)
        );

        let request = OllamaRequest {
            model: self.config.model.clone(),
            prompt,
            stream: false,
        };

        let url = format!("{}/api/generate", self.config.base_url);

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .context("Failed to send request to Ollama")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            anyhow::bail!("Ollama request failed ({}): {}", status, body);
        }

        let result: OllamaResponse = response.json().context("Failed to parse Ollama response")?;

        Ok(result.response.trim().to_string())
    }

    fn build_system_prompt(&self) -> String {
        format!(
            "You are a professional game translator. Translate the given text to {}. \
             Follow these rules:\n\
             1. Preserve any formatting tags like {{color}}, [variables], etc.\n\
             2. Keep the original tone and style.\n\
             3. Only output the translated text, nothing else.\n\
             4. Do not add quotes around the translation.",
            self.config.target_lang
        )
    }

    fn build_user_prompt(&self, text: &str, context: Option<&str>) -> String {
        match context {
            Some(ctx) => format!("Context: {}\n\nTranslate: {}", ctx, text),
            None => format!("Translate: {}", text),
        }
    }
}
