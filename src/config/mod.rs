//! Configuration management

pub mod commands;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const CONFIG_FILE_NAME: &str = "config.toml";
const APP_NAME: &str = "derenpy";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub general: GeneralConfig,

    #[serde(default)]
    pub api: ApiConfig,

    #[serde(default)]
    pub translation: TranslationConfig,

    #[serde(default)]
    pub paths: PathsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GeneralConfig {
    #[serde(default)]
    pub output_dir: Option<String>,
    #[serde(default)]
    pub verbose: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    /// Default API provider (openai, claude, ollama)
    #[serde(default = "default_provider")]
    pub provider: String,

    /// OpenAI API key
    #[serde(default)]
    pub openai_api_key: Option<String>,

    /// OpenAI API base URL
    #[serde(default)]
    pub openai_api_base: Option<String>,

    /// OpenAI model
    #[serde(default)]
    pub openai_model: Option<String>,

    /// Anthropic API key
    #[serde(default)]
    pub anthropic_api_key: Option<String>,

    /// Anthropic API base URL
    #[serde(default)]
    pub anthropic_api_base: Option<String>,

    /// Anthropic model
    #[serde(default)]
    pub anthropic_model: Option<String>,

    /// Ollama API base URL
    #[serde(default = "default_ollama_base")]
    pub ollama_api_base: String,

    /// Ollama model
    #[serde(default = "default_ollama_model")]
    pub ollama_model: String,

    /// DeepL API key (free or pro)
    #[serde(default)]
    pub deepl_api_key: Option<String>,
}

fn default_provider() -> String {
    "openai".to_string()
}

fn default_ollama_base() -> String {
    "http://localhost:11434".to_string()
}

fn default_ollama_model() -> String {
    "llama3".to_string()
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            provider: default_provider(),
            openai_api_key: None,
            openai_api_base: None,
            openai_model: None,
            anthropic_api_key: None,
            anthropic_api_base: None,
            anthropic_model: None,
            ollama_api_base: default_ollama_base(),
            ollama_model: default_ollama_model(),
            deepl_api_key: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationConfig {
    /// Default target language
    #[serde(default = "default_language")]
    pub default_language: String,

    /// Use patch mode by default
    #[serde(default)]
    pub patch_mode: bool,

    /// Custom translation prompt
    #[serde(default)]
    pub custom_prompt: Option<String>,
}

fn default_language() -> String {
    "chinese".to_string()
}

impl Default for TranslationConfig {
    fn default() -> Self {
        Self {
            default_language: default_language(),
            patch_mode: true,
            custom_prompt: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PathsConfig {
    /// Custom Python path
    #[serde(default)]
    pub python: Option<String>,

    /// Custom unrpyc path
    #[serde(default)]
    pub unrpyc: Option<String>,
}

impl Config {
    /// Get the config directory path
    pub fn config_dir() -> Option<PathBuf> {
        dirs::config_dir().map(|p| p.join(APP_NAME))
    }

    /// Get the config file path
    pub fn config_path() -> Option<PathBuf> {
        Self::config_dir().map(|p| p.join(CONFIG_FILE_NAME))
    }

    /// Load config from default location
    pub fn load() -> Result<Self> {
        let path = Self::config_path().context("Could not determine config path")?;

        if !path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&path)
            .context(format!("Failed to read config file: {}", path.display()))?;

        let config: Config = toml::from_str(&content).context("Failed to parse config file")?;

        Ok(config)
    }

    /// Save config to default location
    pub fn save(&self) -> Result<PathBuf> {
        let dir = Self::config_dir().context("Could not determine config directory")?;
        fs::create_dir_all(&dir).context("Failed to create config directory")?;

        let path = dir.join(CONFIG_FILE_NAME);
        let content = toml::to_string_pretty(self).context("Failed to serialize config")?;

        fs::write(&path, content).context("Failed to write config file")?;

        Ok(path)
    }

    /// Get API key for the specified provider
    pub fn get_api_key(&self, provider: &str) -> Option<String> {
        match provider.to_lowercase().as_str() {
            "openai" => self
                .api
                .openai_api_key
                .clone()
                .or_else(|| std::env::var("OPENAI_API_KEY").ok()),
            "claude" | "anthropic" => self
                .api
                .anthropic_api_key
                .clone()
                .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok()),
            "deepl" => self
                .api
                .deepl_api_key
                .clone()
                .or_else(|| std::env::var("DEEPL_API_KEY").ok()),
            "ollama" | "google" => None,
            _ => None,
        }
    }

    /// Get API base URL for the specified provider
    pub fn get_api_base(&self, provider: &str) -> Option<String> {
        match provider.to_lowercase().as_str() {
            "openai" => self.api.openai_api_base.clone(),
            "claude" | "anthropic" => self.api.anthropic_api_base.clone(),
            "ollama" => Some(self.api.ollama_api_base.clone()),
            _ => None,
        }
    }

    /// Get model for the specified provider
    pub fn get_model(&self, provider: &str) -> Option<String> {
        match provider.to_lowercase().as_str() {
            "openai" => self.api.openai_model.clone(),
            "claude" | "anthropic" => self.api.anthropic_model.clone(),
            "ollama" => Some(self.api.ollama_model.clone()),
            _ => None,
        }
    }
}
