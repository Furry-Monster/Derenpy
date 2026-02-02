//! Config command handlers

use anyhow::{Context, Result};
use colored::Colorize;

use super::Config;
use crate::cli::{ConfigAction, ConfigArgs};

pub fn run(args: ConfigArgs) -> Result<()> {
    match args.action {
        ConfigAction::Show => show_config(),
        ConfigAction::Init { force } => init_config(force),
        ConfigAction::Set { key, value } => set_config(&key, &value),
        ConfigAction::Get { key } => get_config(&key),
        ConfigAction::Path => show_path(),
        ConfigAction::Edit => edit_config(),
    }
}

fn show_config() -> Result<()> {
    let config = Config::load()?;
    let content = toml::to_string_pretty(&config)?;

    println!("{}", "[Config]".green());
    println!("{}", content);

    Ok(())
}

fn init_config(force: bool) -> Result<()> {
    let path = Config::config_path().context("Could not determine config path")?;

    if path.exists() && !force {
        println!(
            "{}",
            format!("Config file already exists: {}", path.display()).yellow()
        );
        println!("Use --force to overwrite");
        return Ok(());
    }

    let config = Config::default();
    let saved_path = config.save()?;

    println!("{}", "[Config] Initialized".green());
    println!("  Created: {}", saved_path.display());
    println!();
    println!("Edit the config file to set your API keys:");
    println!("  derenpy config edit");

    Ok(())
}

fn set_config(key: &str, value: &str) -> Result<()> {
    let mut config = Config::load()?;

    // Parse key path (e.g., "api.openai_api_key")
    let parts: Vec<&str> = key.split('.').collect();

    match parts.as_slice() {
        ["general", "output_dir"] => {
            config.general.output_dir = if value.is_empty() {
                None
            } else {
                Some(value.to_string())
            };
        }
        ["general", "verbose"] => {
            config.general.verbose = value.parse().unwrap_or(false);
        }
        ["api", "provider"] => {
            config.api.provider = value.to_string();
        }
        ["api", "openai_api_key"] => {
            config.api.openai_api_key = if value.is_empty() {
                None
            } else {
                Some(value.to_string())
            };
        }
        ["api", "openai_api_base"] => {
            config.api.openai_api_base = if value.is_empty() {
                None
            } else {
                Some(value.to_string())
            };
        }
        ["api", "openai_model"] => {
            config.api.openai_model = if value.is_empty() {
                None
            } else {
                Some(value.to_string())
            };
        }
        ["api", "anthropic_api_key"] => {
            config.api.anthropic_api_key = if value.is_empty() {
                None
            } else {
                Some(value.to_string())
            };
        }
        ["api", "anthropic_api_base"] => {
            config.api.anthropic_api_base = if value.is_empty() {
                None
            } else {
                Some(value.to_string())
            };
        }
        ["api", "anthropic_model"] => {
            config.api.anthropic_model = if value.is_empty() {
                None
            } else {
                Some(value.to_string())
            };
        }
        ["api", "ollama_api_base"] => {
            config.api.ollama_api_base = value.to_string();
        }
        ["api", "ollama_model"] => {
            config.api.ollama_model = value.to_string();
        }
        ["translation", "default_language"] => {
            config.translation.default_language = value.to_string();
        }
        ["translation", "patch_mode"] => {
            config.translation.patch_mode = value.parse().unwrap_or(true);
        }
        ["translation", "custom_prompt"] => {
            config.translation.custom_prompt = if value.is_empty() {
                None
            } else {
                Some(value.to_string())
            };
        }
        ["paths", "python"] => {
            config.paths.python = if value.is_empty() {
                None
            } else {
                Some(value.to_string())
            };
        }
        ["paths", "unrpyc"] => {
            config.paths.unrpyc = if value.is_empty() {
                None
            } else {
                Some(value.to_string())
            };
        }
        _ => {
            anyhow::bail!("Unknown config key: {}", key);
        }
    }

    config.save()?;
    println!("{}", format!("[Config] Set {} = {}", key, value).green());

    Ok(())
}

fn get_config(key: &str) -> Result<()> {
    let config = Config::load()?;
    let parts: Vec<&str> = key.split('.').collect();

    let value: Option<String> = match parts.as_slice() {
        ["general", "output_dir"] => config.general.output_dir,
        ["general", "verbose"] => Some(config.general.verbose.to_string()),
        ["api", "provider"] => Some(config.api.provider),
        ["api", "openai_api_key"] => config.api.openai_api_key.map(|k| mask_key(&k)),
        ["api", "openai_api_base"] => config.api.openai_api_base,
        ["api", "openai_model"] => config.api.openai_model,
        ["api", "anthropic_api_key"] => config.api.anthropic_api_key.map(|k| mask_key(&k)),
        ["api", "anthropic_api_base"] => config.api.anthropic_api_base,
        ["api", "anthropic_model"] => config.api.anthropic_model,
        ["api", "ollama_api_base"] => Some(config.api.ollama_api_base),
        ["api", "ollama_model"] => Some(config.api.ollama_model),
        ["translation", "default_language"] => Some(config.translation.default_language),
        ["translation", "patch_mode"] => Some(config.translation.patch_mode.to_string()),
        ["translation", "custom_prompt"] => config.translation.custom_prompt,
        ["paths", "python"] => config.paths.python,
        ["paths", "unrpyc"] => config.paths.unrpyc,
        _ => {
            anyhow::bail!("Unknown config key: {}", key);
        }
    };

    match value {
        Some(v) => println!("{} = {}", key, v),
        None => println!("{} = (not set)", key),
    }

    Ok(())
}

fn show_path() -> Result<()> {
    match Config::config_path() {
        Some(path) => {
            println!("{}", path.display());
            if path.exists() {
                println!("{}", "(exists)".green());
            } else {
                println!("{}", "(not created)".yellow());
            }
        }
        None => {
            println!("{}", "Could not determine config path".red());
        }
    }
    Ok(())
}

fn edit_config() -> Result<()> {
    let path = Config::config_path().context("Could not determine config path")?;

    // Create default config if it doesn't exist
    if !path.exists() {
        let config = Config::default();
        config.save()?;
        println!("{}", "[Config] Created default config".green());
    }

    // Get editor from environment
    let editor = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| {
            if cfg!(windows) {
                "notepad".to_string()
            } else {
                "nano".to_string()
            }
        });

    println!("Opening config with: {}", editor);
    println!("Path: {}", path.display());

    std::process::Command::new(&editor)
        .arg(&path)
        .status()
        .context(format!("Failed to open editor: {}", editor))?;

    Ok(())
}

fn mask_key(key: &str) -> String {
    if key.len() <= 8 {
        "*".repeat(key.len())
    } else {
        format!("{}...{}", &key[..4], &key[key.len() - 4..])
    }
}
