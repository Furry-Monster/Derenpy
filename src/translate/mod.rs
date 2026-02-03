pub mod cache;
pub mod extractor;
pub mod glossary;
pub mod llm;
pub mod machine_translate;
pub mod renpy_tl;

use anyhow::{Context, Result};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

use crate::cli::TranslateArgs;
use crate::config::Config;
use extractor::{TextExtractor, TranslatableEntry};
use llm::{LlmClient, LlmConfig, LlmProvider};
use machine_translate::{MachineTranslateClient, MachineTranslateConfig};

pub enum TranslateClient {
    Llm(LlmClient),
    Machine(MachineTranslateClient),
}

impl TranslateClient {
    pub fn translate_batch<F>(
        &self,
        texts: &[String],
        progress_callback: Option<F>,
    ) -> Vec<Result<String>>
    where
        F: Fn(usize) + Send + Sync,
    {
        match self {
            Self::Machine(client) => client.translate_batch(texts, progress_callback),
            Self::Llm(client) => texts
                .iter()
                .enumerate()
                .map(|(i, t)| {
                    let result = client.translate(t, None);
                    if let Some(ref cb) = progress_callback {
                        cb(i + 1);
                    }
                    result
                })
                .collect(),
        }
    }
}

pub fn run(args: TranslateArgs) -> Result<()> {
    // Load config
    let cfg = Config::load().unwrap_or_default();

    // Determine provider (CLI arg > config > default)
    let provider_str = if args.api != "openai" {
        args.api.clone()
    } else {
        cfg.api.provider.clone()
    };
    let provider = LlmProvider::from_str(&provider_str);

    // Determine language (CLI arg > config)
    let lang = if args.lang != "zh-CN" {
        args.lang.clone()
    } else {
        cfg.translation.default_language.clone()
    };

    // Create appropriate client based on provider
    let client = if provider.is_machine_translate() {
        create_machine_client(provider, &lang, &cfg, &args)?
    } else {
        create_llm_client(provider, &provider_str, &lang, &cfg, &args)?
    };

    let extractor = TextExtractor::new();
    let input = &args.input;

    if input.is_file() {
        translate_single(&extractor, &client, input, args.output.as_deref())?;
    } else if input.is_dir() {
        translate_directory(
            &extractor,
            &client,
            input,
            args.output.as_deref(),
            args.recursive,
        )?;
    } else {
        anyhow::bail!("Input path does not exist: {}", input.display());
    }

    Ok(())
}

fn create_machine_client(
    provider: LlmProvider,
    lang: &str,
    cfg: &Config,
    args: &TranslateArgs,
) -> Result<TranslateClient> {
    let config = match provider {
        LlmProvider::Google => {
            println!("{}", "[Translate] Using Google Translate".cyan());
            MachineTranslateConfig::google(lang)
        }
        LlmProvider::DeepL => {
            let api_key = args
                .api_key
                .clone()
                .or_else(|| cfg.get_api_key("deepl"))
                .context("DeepL API key required. Get free key at https://www.deepl.com/pro-api")?;
            println!("{}", "[Translate] Using DeepL".cyan());
            MachineTranslateConfig::deepl(lang, api_key)
        }
        _ => unreachable!(),
    };

    let client = MachineTranslateClient::new(config)?;
    Ok(TranslateClient::Machine(client))
}

fn create_llm_client(
    provider: LlmProvider,
    provider_str: &str,
    lang: &str,
    cfg: &Config,
    args: &TranslateArgs,
) -> Result<TranslateClient> {
    let api_key = args
        .api_key
        .clone()
        .or_else(|| cfg.get_api_key(provider_str));

    if api_key.is_none() && provider != LlmProvider::Ollama {
        anyhow::bail!(
            "API key required for {}. Set via --api-key, config, or environment variable.\n\
             Run 'derenpy config init' to create a config file.\n\
             Or use --api google for free translation.",
            provider_str
        );
    }

    let api_base = args
        .api_base
        .clone()
        .or_else(|| cfg.get_api_base(provider_str));
    let model = args.model.clone().or_else(|| cfg.get_model(provider_str));

    let config = LlmConfig::new(provider, lang)
        .with_api_key(api_key)
        .with_base_url(api_base)
        .with_model(model);

    let client = LlmClient::new(config)?;
    Ok(TranslateClient::Llm(client))
}

fn translate_single(
    extractor: &TextExtractor,
    client: &TranslateClient,
    input: &Path,
    output: Option<&Path>,
) -> Result<()> {
    println!("{}", format!("[Translate] {}", input.display()).green());

    let entries = extractor.extract_from_file(input)?;

    if entries.is_empty() {
        println!("{}", "[WARN] No translatable text found".yellow());
        return Ok(());
    }

    println!("  Found {} translatable entries", entries.len());

    let pb = ProgressBar::new(entries.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len}")?
            .progress_chars("=>-"),
    );
    pb.enable_steady_tick(std::time::Duration::from_millis(100));

    let mut translations: HashMap<usize, String> = HashMap::new();

    // Use batch translation for better performance
    let texts: Vec<String> = entries.iter().map(|e| e.text.clone()).collect();
    let results = client.translate_batch(
        &texts,
        Some(|count| {
            pb.set_position(count as u64);
        }),
    );

    for (entry, result) in entries.iter().zip(results.into_iter()) {
        match result {
            Ok(translated) => {
                translations.insert(entry.id, translated);
            }
            Err(e) => {
                pb.suspend(|| {
                    eprintln!(
                        "{}",
                        format!(
                            "[ERROR] Failed to translate line {}: {}",
                            entry.line_number, e
                        )
                        .red()
                    );
                });
            }
        }
    }

    pb.finish_and_clear();

    let output_path = match output {
        Some(p) => {
            if p.is_dir() {
                p.join(input.file_name().unwrap_or_default())
            } else {
                p.to_path_buf()
            }
        }
        None => {
            let stem = input.file_stem().unwrap_or_default().to_string_lossy();
            let ext = input.extension().unwrap_or_default().to_string_lossy();
            input.with_file_name(format!("{}_translated.{}", stem, ext))
        }
    };

    write_translated_file(input, &output_path, &entries, &translations)?;

    println!(
        "{}",
        format!(
            "[OK] Translated {} entries -> {}",
            translations.len(),
            output_path.display()
        )
        .green()
    );

    Ok(())
}

fn translate_directory(
    extractor: &TextExtractor,
    client: &TranslateClient,
    dir: &Path,
    output: Option<&Path>,
    recursive: bool,
) -> Result<()> {
    let walker = if recursive {
        WalkDir::new(dir)
    } else {
        WalkDir::new(dir).max_depth(1)
    };

    let rpy_files: Vec<_> = walker
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "rpy" || ext == "rpym")
                .unwrap_or(false)
        })
        .collect();

    if rpy_files.is_empty() {
        println!("{}", "[WARN] No RPY files found".yellow());
        return Ok(());
    }

    println!(
        "{}",
        format!("[Translate] Found {} RPY file(s)", rpy_files.len()).green()
    );

    for entry in rpy_files {
        let rpy_path = entry.path();

        let out_path = match output {
            Some(base) => {
                let rel = rpy_path.strip_prefix(dir).unwrap_or(rpy_path);
                base.join(rel)
            }
            None => {
                let stem = rpy_path.file_stem().unwrap_or_default().to_string_lossy();
                let ext = rpy_path.extension().unwrap_or_default().to_string_lossy();
                rpy_path.with_file_name(format!("{}_translated.{}", stem, ext))
            }
        };

        if let Err(e) = translate_single(extractor, client, rpy_path, Some(&out_path)) {
            eprintln!(
                "{}",
                format!("[ERROR] Failed to translate {}: {}", rpy_path.display(), e).red()
            );
        }
    }

    Ok(())
}

fn write_translated_file(
    input: &Path,
    output: &Path,
    entries: &[TranslatableEntry],
    translations: &HashMap<usize, String>,
) -> Result<()> {
    let content = fs::read_to_string(input).context("Failed to read input file")?;
    let lines: Vec<&str> = content.lines().collect();

    let mut result_lines: Vec<String> = lines.iter().map(|s| s.to_string()).collect();

    // Build a map of line_number -> entries for that line
    let mut line_map: HashMap<usize, Vec<&TranslatableEntry>> = HashMap::new();
    for entry in entries {
        line_map.entry(entry.line_number).or_default().push(entry);
    }

    // Replace text in each line
    for (line_num, line_entries) in line_map {
        if line_num == 0 || line_num > result_lines.len() {
            continue;
        }

        let mut line = result_lines[line_num - 1].clone();

        for entry in line_entries {
            if let Some(translated) = translations.get(&entry.id) {
                // Simple replacement - find the original text and replace it
                line = line.replace(
                    &format!("\"{}\"", entry.text),
                    &format!("\"{}\"", translated),
                );
                line = line.replace(&format!("'{}'", entry.text), &format!("'{}'", translated));
            }
        }

        result_lines[line_num - 1] = line;
    }

    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).context("Failed to create output directory")?;
    }

    fs::write(output, result_lines.join("\n")).context("Failed to write output file")?;

    Ok(())
}
