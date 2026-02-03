//! Game translation patch generator

use anyhow::Result;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use walkdir::WalkDir;

use crate::cli::PatchArgs;
use crate::config::Config;
use crate::translate::cache::TranslationCache;
use crate::translate::extractor::TextExtractor;
use crate::translate::glossary::Glossary;
use crate::translate::llm::{LlmClient, LlmConfig, LlmProvider};
use crate::translate::machine_translate::{MachineTranslateClient, MachineTranslateConfig};
use crate::translate::renpy_tl::{DialogueEntry, RenpyTranslationGenerator, StringEntry};
use crate::unpack::rpa::RpaArchive;

struct TranslationStats {
    cache_hits: usize,
    api_calls: usize,
}

enum Translator {
    Llm(LlmClient),
    Machine(MachineTranslateClient),
}

impl Translator {
    fn translate_batch_with_stats<F>(
        &self,
        texts: &[String],
        cache: Option<&TranslationCache>,
        progress_callback: Option<F>,
    ) -> (Vec<Result<String>>, TranslationStats)
    where
        F: Fn(usize) + Send + Sync,
    {
        match self {
            Self::Machine(c) => {
                if let Some(cache) = cache {
                    let result = c.translate_batch_cached(texts, cache, progress_callback);
                    let stats = TranslationStats {
                        cache_hits: result.cache_hits,
                        api_calls: result.api_calls,
                    };
                    (result.translations, stats)
                } else {
                    let results = c.translate_batch(texts, progress_callback);
                    let stats = TranslationStats {
                        cache_hits: 0,
                        api_calls: texts.len(),
                    };
                    (results, stats)
                }
            }
            Self::Llm(c) => {
                let results: Vec<Result<String>> = texts
                    .iter()
                    .enumerate()
                    .map(|(i, t)| {
                        let result = c.translate(t, None);
                        if let Some(ref cb) = progress_callback {
                            cb(i + 1);
                        }
                        result
                    })
                    .collect();
                let stats = TranslationStats {
                    cache_hits: 0,
                    api_calls: texts.len(),
                };
                (results, stats)
            }
        }
    }
}

pub fn run(args: PatchArgs) -> Result<()> {
    let cfg = Config::load().unwrap_or_default();
    let input = &args.input;
    let mut temp_dir_to_cleanup: Option<PathBuf> = None;

    println!("{}", "[Patch] Translation Patch Generator".green());

    let work_dir = if input.extension().map(|e| e == "rpa").unwrap_or(false) {
        println!("  Unpacking RPA archive...");
        let temp_dir = std::env::temp_dir().join(format!("derenpy_{}", std::process::id()));
        let archive = RpaArchive::open(input)?;
        fs::create_dir_all(&temp_dir)?;
        archive.extract_all(&temp_dir, None)?;
        temp_dir_to_cleanup = Some(temp_dir.clone());
        temp_dir
    } else if input.is_dir() {
        input.clone()
    } else {
        anyhow::bail!("Input must be a game directory or RPA file");
    };

    // Find all RPY files
    let rpy_files: Vec<_> = WalkDir::new(&work_dir)
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
        anyhow::bail!("No RPY files found. You may need to decompile RPYC files first.");
    }

    println!("  Found {} script file(s)", rpy_files.len());

    // Setup translation generator
    let generator = RenpyTranslationGenerator::new(&args.lang);
    let extractor = TextExtractor::new();

    // Extract all dialogues
    let mut all_dialogues: HashMap<PathBuf, Vec<DialogueEntry>> = HashMap::new();
    let mut all_strings: Vec<StringEntry> = Vec::new();

    println!("  Extracting dialogues...");

    for entry in &rpy_files {
        let path = entry.path();
        let dialogues = generator.extract_dialogues(path)?;

        // Also extract menu choices as strings
        let entries = extractor.extract_from_file(path).unwrap_or_default();
        for e in entries {
            if e.entry_type == crate::translate::extractor::EntryType::MenuChoice {
                all_strings.push(StringEntry {
                    original: e.text,
                    translated: None,
                });
            }
        }

        if !dialogues.is_empty() {
            let rel_path = path.strip_prefix(&work_dir).unwrap_or(path);
            all_dialogues.insert(rel_path.to_path_buf(), dialogues);
        }
    }

    let total_dialogues: usize = all_dialogues.values().map(|v| v.len()).sum();
    println!(
        "  Total: {} dialogues, {} strings",
        total_dialogues,
        all_strings.len()
    );

    // Load glossary if provided
    let glossary = if let Some(ref glossary_path) = args.glossary {
        match Glossary::load(glossary_path) {
            Ok(g) => {
                println!("  Loaded {} glossary terms", g.len());
                Some(g)
            }
            Err(e) => {
                eprintln!(
                    "{}",
                    format!("[WARN] Failed to load glossary: {}", e).yellow()
                );
                None
            }
        }
    } else {
        None
    };

    // Translate if not template only
    if !args.template_only && total_dialogues > 0 {
        let provider_str = if args.api != "openai" {
            args.api.clone()
        } else {
            cfg.api.provider.clone()
        };
        let provider = LlmProvider::from_str(&provider_str);

        // Determine language
        let lang = if args.lang != "chinese" {
            args.lang.clone()
        } else {
            cfg.translation.default_language.clone()
        };

        // Create translator based on provider type
        let translator = if provider.is_machine_translate() {
            create_machine_translator(provider, &lang, &cfg, &args)?
        } else {
            create_llm_translator(provider, &provider_str, &lang, &cfg, &args)?
        };

        if let Some(translator) = translator {
            // Initialize cache
            let cache = TranslationCache::open().ok();
            if cache.is_some() {
                println!("  Translation cache enabled");
            }

            println!("  Translating dialogues...");

            let pb = ProgressBar::new(total_dialogues as u64);
            pb.set_style(
                ProgressStyle::default_bar()
                    .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len}")?
                    .progress_chars("=>-"),
            );
            pb.enable_steady_tick(std::time::Duration::from_millis(100));

            let mut all_texts: Vec<String> = Vec::new();
            let mut text_indices: Vec<(PathBuf, usize)> = Vec::new();

            for (path, dialogues) in all_dialogues.iter() {
                for (i, entry) in dialogues.iter().enumerate() {
                    all_texts.push(entry.original_text.clone());
                    text_indices.push((path.clone(), i));
                }
            }

            let (results, dialogue_stats) = translator.translate_batch_with_stats(
                &all_texts,
                cache.as_ref(),
                Some(|count| {
                    pb.set_position(count as u64);
                }),
            );

            for ((path, idx), result) in text_indices.into_iter().zip(results.into_iter()) {
                if let Some(dialogues) = all_dialogues.get_mut(&path)
                    && let Some(entry) = dialogues.get_mut(idx)
                {
                    match result {
                        Ok(translated) => {
                            // Apply glossary if available
                            let final_text = match &glossary {
                                Some(g) => g.apply(&translated),
                                None => translated,
                            };
                            entry.translated_text = Some(final_text);
                        }
                        Err(e) => {
                            pb.suspend(|| {
                                eprintln!("{}", format!("[ERROR] Translation failed: {}", e).red());
                            });
                        }
                    }
                }
            }

            pb.finish_and_clear();

            // Translate strings
            let mut string_stats = TranslationStats {
                cache_hits: 0,
                api_calls: 0,
            };
            if !all_strings.is_empty() {
                println!("  Translating strings...");
                let string_texts: Vec<String> =
                    all_strings.iter().map(|s| s.original.clone()).collect();
                let (string_results, stats) = translator.translate_batch_with_stats(
                    &string_texts,
                    cache.as_ref(),
                    None::<fn(usize)>,
                );
                string_stats = stats;

                for (string, result) in all_strings.iter_mut().zip(string_results.into_iter()) {
                    if let Ok(translated) = result {
                        let final_text = match &glossary {
                            Some(g) => g.apply(&translated),
                            None => translated,
                        };
                        string.translated = Some(final_text);
                    }
                }
            }

            // Print statistics
            let total_cache_hits = dialogue_stats.cache_hits + string_stats.cache_hits;
            let total_api_calls = dialogue_stats.api_calls + string_stats.api_calls;
            if total_cache_hits > 0 {
                println!(
                    "  Stats: {} cached, {} API calls",
                    format!("{}", total_cache_hits).green(),
                    total_api_calls
                );
            }
        }
    }

    // Determine output directory
    let output_dir = args.output.unwrap_or_else(|| {
        if input.is_dir() {
            input.join("game")
        } else {
            PathBuf::from("game")
        }
    });

    // Generate translation files
    println!("  Generating translation files...");
    let created = generator.write_translation_files(&output_dir, &all_dialogues, &all_strings)?;

    println!(
        "{}",
        format!("[OK] Created {} translation file(s)", created.len()).green()
    );

    for file in &created {
        println!("    {}", file.display());
    }

    println!();
    println!("To use this translation:");
    println!("  1. Copy the 'tl' folder to your game's 'game' directory");
    println!("  2. The game will auto-detect the translation");
    println!("  3. Add language selector to preferences if needed");

    if let Some(temp_dir) = temp_dir_to_cleanup {
        let _ = fs::remove_dir_all(temp_dir);
    }

    Ok(())
}

fn create_machine_translator(
    provider: LlmProvider,
    lang: &str,
    cfg: &Config,
    args: &PatchArgs,
) -> Result<Option<Translator>> {
    let config = match provider {
        LlmProvider::Google => {
            println!("{}", "  Using Google Translate".cyan());
            MachineTranslateConfig::google(lang)
        }
        LlmProvider::DeepL => {
            let api_key = args.api_key.clone().or_else(|| cfg.get_api_key("deepl"));

            if api_key.is_none() {
                println!(
                    "{}",
                    "[WARN] DeepL API key required. Get free key at https://www.deepl.com/pro-api"
                        .yellow()
                );
                println!("       Use --api google for no-key translation.");
                return Ok(None);
            }

            println!("{}", "  Using DeepL".cyan());
            MachineTranslateConfig::deepl(lang, api_key.unwrap())
        }
        _ => unreachable!(),
    };

    let client = MachineTranslateClient::new(config)?;
    Ok(Some(Translator::Machine(client)))
}

fn create_llm_translator(
    provider: LlmProvider,
    provider_str: &str,
    lang: &str,
    cfg: &Config,
    args: &PatchArgs,
) -> Result<Option<Translator>> {
    let api_key = args
        .api_key
        .clone()
        .or_else(|| cfg.get_api_key(provider_str));

    if api_key.is_none() && provider != LlmProvider::Ollama {
        println!(
            "{}",
            "[WARN] No API key provided, generating template only".yellow()
        );
        println!("       Run 'derenpy config init' to set up API keys.");
        println!("       Or use --api google for free translation.");
        return Ok(None);
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
    Ok(Some(Translator::Llm(client)))
}
