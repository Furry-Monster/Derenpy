//! Machine translation API clients (Google Translate, DeepL)

use anyhow::{Context, Result};
use rayon::prelude::*;
use regex::Regex;
use serde::Deserialize;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;

use super::cache::TranslationCache;

const DEFAULT_CONCURRENCY: usize = 16;
const DEEPL_BATCH_SIZE: usize = 50;
const GOOGLE_BATCH_SIZE: usize = 20;
const GOOGLE_SEPARATOR: &str = "\n\u{2029}\n";
const MAX_RETRIES: u32 = 3;
const BASE_RETRY_DELAY_MS: u64 = 500;

fn wrap_callback<F>(
    callback: &Option<F>,
    offset: usize,
) -> Option<impl Fn(usize) + Send + Sync + '_>
where
    F: Fn(usize) + Send + Sync,
{
    callback
        .as_ref()
        .map(move |cb| move |count: usize| cb(count + offset))
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MachineTranslateProvider {
    Google,
    DeepL,
}

#[derive(Debug, Clone)]
pub struct MachineTranslateConfig {
    pub provider: MachineTranslateProvider,
    pub target_lang: String,
    pub source_lang: String,
    pub api_key: Option<String>,
    pub concurrency: usize,
}

impl MachineTranslateConfig {
    pub fn google(target_lang: &str) -> Self {
        Self {
            provider: MachineTranslateProvider::Google,
            target_lang: Self::normalize_lang_google(target_lang),
            source_lang: "en".to_string(),
            api_key: None,
            concurrency: DEFAULT_CONCURRENCY,
        }
    }

    pub fn deepl(target_lang: &str, api_key: String) -> Self {
        Self {
            provider: MachineTranslateProvider::DeepL,
            target_lang: Self::normalize_lang_deepl(target_lang),
            source_lang: "EN".to_string(),
            api_key: Some(api_key),
            concurrency: DEFAULT_CONCURRENCY,
        }
    }

    fn normalize_lang_google(lang: &str) -> String {
        match lang.to_lowercase().as_str() {
            "chinese" | "zh-cn" | "zh_cn" | "chs" => "zh-CN".to_string(),
            "zh-tw" | "zh_tw" | "cht" => "zh-TW".to_string(),
            "japanese" | "ja" | "jp" => "ja".to_string(),
            "korean" | "ko" | "kr" => "ko".to_string(),
            "english" | "en" => "en".to_string(),
            "french" | "fr" => "fr".to_string(),
            "german" | "de" => "de".to_string(),
            "spanish" | "es" => "es".to_string(),
            "russian" | "ru" => "ru".to_string(),
            _ => lang.to_string(),
        }
    }

    fn normalize_lang_deepl(lang: &str) -> String {
        match lang.to_lowercase().as_str() {
            "chinese" | "zh-cn" | "zh_cn" | "chs" => "ZH".to_string(),
            "japanese" | "ja" | "jp" => "JA".to_string(),
            "korean" | "ko" | "kr" => "KO".to_string(),
            "english" | "en" => "EN".to_string(),
            "french" | "fr" => "FR".to_string(),
            "german" | "de" => "DE".to_string(),
            "spanish" | "es" => "ES".to_string(),
            "russian" | "ru" => "RU".to_string(),
            _ => lang.to_uppercase(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct DeepLResponse {
    translations: Vec<DeepLTranslation>,
}

#[derive(Debug, Deserialize)]
struct DeepLTranslation {
    text: String,
}

pub struct MachineTranslateClient {
    config: MachineTranslateConfig,
    client: reqwest::blocking::Client,
}

pub struct BatchResult {
    pub translations: Vec<Result<String>>,
    pub cache_hits: usize,
    pub api_calls: usize,
}

impl MachineTranslateClient {
    pub fn new(config: MachineTranslateConfig) -> Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .pool_max_idle_per_host(config.concurrency)
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self { config, client })
    }

    pub fn provider_name(&self) -> &'static str {
        match self.config.provider {
            MachineTranslateProvider::Google => "google",
            MachineTranslateProvider::DeepL => "deepl",
        }
    }

    pub fn translate_batch<F>(
        &self,
        texts: &[String],
        progress_callback: Option<F>,
    ) -> Vec<Result<String>>
    where
        F: Fn(usize) + Send + Sync,
    {
        match self.config.provider {
            MachineTranslateProvider::DeepL => {
                self.translate_batch_deepl(texts, &progress_callback, 0)
            }
            MachineTranslateProvider::Google => {
                self.translate_batch_google(texts, &progress_callback, 0)
            }
        }
    }

    pub fn translate_batch_cached<F>(
        &self,
        texts: &[String],
        cache: &TranslationCache,
        progress_callback: Option<F>,
    ) -> BatchResult
    where
        F: Fn(usize) + Send + Sync,
    {
        let provider = self.provider_name();
        let lang = &self.config.target_lang;

        let mut results: Vec<Option<Result<String>>> = Vec::with_capacity(texts.len());
        for _ in 0..texts.len() {
            results.push(None);
        }

        let mut to_translate: Vec<(usize, String)> = Vec::new();
        let mut cache_hits = 0;

        for (i, text) in texts.iter().enumerate() {
            if text.trim().is_empty() {
                results[i] = Some(Ok(text.clone()));
                cache_hits += 1;
            } else if let Some(cached) = cache.get(text, lang, provider) {
                results[i] = Some(Ok(cached));
                cache_hits += 1;
            } else {
                to_translate.push((i, text.clone()));
            }
        }

        let api_calls = to_translate.len();

        if to_translate.is_empty() {
            if let Some(cb) = progress_callback {
                cb(texts.len());
            }
            return BatchResult {
                translations: results.into_iter().map(|r| r.unwrap()).collect(),
                cache_hits,
                api_calls: 0,
            };
        }

        let texts_to_translate: Vec<String> = to_translate.iter().map(|(_, t)| t.clone()).collect();
        let translated = self.translate_batch(
            &texts_to_translate,
            wrap_callback(&progress_callback, cache_hits),
        );

        for ((orig_idx, orig_text), result) in to_translate.into_iter().zip(translated.into_iter())
        {
            if let Ok(ref translated_text) = result {
                let _ = cache.set(&orig_text, lang, provider, translated_text);
            }
            results[orig_idx] = Some(result);
        }

        BatchResult {
            translations: results.into_iter().map(|r| r.unwrap()).collect(),
            cache_hits,
            api_calls,
        }
    }

    fn translate_batch_google<F>(
        &self,
        texts: &[String],
        progress_callback: &Option<F>,
        progress_offset: usize,
    ) -> Vec<Result<String>>
    where
        F: Fn(usize) + Send + Sync,
    {
        let counter = Arc::new(AtomicUsize::new(0));
        let callback = progress_callback;

        let batches: Vec<Vec<String>> = texts
            .chunks(GOOGLE_BATCH_SIZE)
            .map(|c| c.to_vec())
            .collect();

        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(self.config.concurrency)
            .build()
            .unwrap_or_else(|_| rayon::ThreadPoolBuilder::new().build().unwrap());

        let batch_results: Vec<Vec<Result<String>>> = pool.install(|| {
            batches
                .par_iter()
                .map(|batch| {
                    let result = self.translate_google_merged(batch);
                    let batch_len = batch.len();

                    let count = counter.fetch_add(batch_len, Ordering::SeqCst) + batch_len;
                    if let Some(cb) = callback {
                        cb(count + progress_offset);
                    }

                    result
                })
                .collect()
        });

        batch_results.into_iter().flatten().collect()
    }

    fn translate_google_merged(&self, texts: &[String]) -> Vec<Result<String>> {
        if texts.is_empty() {
            return vec![];
        }
        if texts.len() == 1 {
            return vec![self.translate_google(&texts[0])];
        }

        let merged = texts.join(GOOGLE_SEPARATOR);
        match self.translate_google(&merged) {
            Ok(translated) => {
                let parts: Vec<&str> = translated.split(GOOGLE_SEPARATOR).collect();
                if parts.len() == texts.len() {
                    parts
                        .into_iter()
                        .map(|s| Ok(s.trim().to_string()))
                        .collect()
                } else {
                    texts.iter().map(|t| self.translate_google(t)).collect()
                }
            }
            Err(e) => texts
                .iter()
                .map(|_| Err(anyhow::anyhow!("Batch failed: {}", e)))
                .collect(),
        }
    }

    fn translate_batch_deepl<F>(
        &self,
        texts: &[String],
        progress_callback: &Option<F>,
        progress_offset: usize,
    ) -> Vec<Result<String>>
    where
        F: Fn(usize) + Send + Sync,
    {
        let api_key = match self.config.api_key.as_ref() {
            Some(k) => k,
            None => {
                return texts
                    .iter()
                    .map(|_| Err(anyhow::anyhow!("DeepL API key is required")))
                    .collect();
            }
        };

        let base_url = if api_key.ends_with(":fx") {
            "https://api-free.deepl.com/v2"
        } else {
            "https://api.deepl.com/v2"
        };

        let url = format!("{}/translate", base_url);
        let mut all_results = Vec::with_capacity(texts.len());
        let mut processed = 0;

        for chunk in texts.chunks(DEEPL_BATCH_SIZE) {
            let result = self.translate_deepl_batch_request(&url, api_key, chunk);

            match result {
                Ok(translations) => {
                    for t in translations {
                        all_results.push(Ok(t));
                        processed += 1;
                        if let Some(cb) = progress_callback {
                            cb(processed + progress_offset);
                        }
                    }
                }
                Err(e) => {
                    for _ in chunk {
                        all_results.push(Err(anyhow::anyhow!("Batch translation failed: {}", e)));
                        processed += 1;
                        if let Some(cb) = progress_callback {
                            cb(processed + progress_offset);
                        }
                    }
                }
            }
        }

        all_results
    }

    fn translate_deepl_batch_request(
        &self,
        url: &str,
        api_key: &str,
        texts: &[String],
    ) -> Result<Vec<String>> {
        let mut last_error = None;

        for attempt in 0..MAX_RETRIES {
            if attempt > 0 {
                let delay = BASE_RETRY_DELAY_MS * 2u64.pow(attempt - 1);
                thread::sleep(Duration::from_millis(delay));
            }

            match self.do_deepl_batch_request(url, api_key, texts) {
                Ok(result) => return Ok(result),
                Err(e) => {
                    last_error = Some(e);
                    continue;
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("DeepL batch request failed")))
    }

    fn do_deepl_batch_request(
        &self,
        url: &str,
        api_key: &str,
        texts: &[String],
    ) -> Result<Vec<String>> {
        let mut form_params: Vec<(&str, &str)> = Vec::new();

        for text in texts {
            form_params.push(("text", text.as_str()));
        }
        form_params.push(("target_lang", &self.config.target_lang));
        form_params.push(("source_lang", &self.config.source_lang));

        let response = self
            .client
            .post(url)
            .header("Authorization", format!("DeepL-Auth-Key {}", api_key))
            .form(&form_params)
            .send()
            .context("Failed to send batch request to DeepL")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            anyhow::bail!("DeepL batch request failed ({}): {}", status, body);
        }

        let result: DeepLResponse = response.json().context("Failed to parse DeepL response")?;

        Ok(result.translations.into_iter().map(|t| t.text).collect())
    }

    fn translate_google(&self, text: &str) -> Result<String> {
        let (protected, placeholders) = Self::protect_formatting(text);

        let url = format!(
            "https://translate.googleapis.com/translate_a/single?client=gtx&sl={}&tl={}&dt=t&q={}",
            self.config.source_lang,
            self.config.target_lang,
            urlencoding::encode(&protected)
        );

        let mut last_error = None;

        for attempt in 0..MAX_RETRIES {
            if attempt > 0 {
                let delay = BASE_RETRY_DELAY_MS * 2u64.pow(attempt - 1);
                thread::sleep(Duration::from_millis(delay));
            }

            match self.do_google_request(&url) {
                Ok(result) => {
                    return Ok(Self::restore_formatting(&result, &placeholders));
                }
                Err(e) => {
                    last_error = Some(e);
                    continue;
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Translation failed")))
    }

    fn protect_formatting(text: &str) -> (String, Vec<(String, String)>) {
        let mut protected = text.to_string();
        let mut placeholders = Vec::new();

        // Patterns to protect: \n, \t, [variables], {tags}, %(format)
        let patterns = [
            (r"\\n", "⟦NL⟧"),
            (r"\\t", "⟦TB⟧"),
            (r"\[([^\]]+)\]", "⟦VAR$1⟧"),
            (r"\{([^}]+)\}", "⟦TAG$1⟧"),
            (r"%\(([^)]+)\)s", "⟦FMT$1⟧"),
        ];

        for (pattern, prefix) in patterns {
            let re = Regex::new(pattern).unwrap();
            for cap in re.captures_iter(text) {
                let original = cap.get(0).unwrap().as_str().to_string();
                let placeholder = if cap.len() > 1 {
                    format!(
                        "{}{}",
                        prefix.replace("$1", cap.get(1).unwrap().as_str()),
                        ""
                    )
                } else {
                    prefix.to_string()
                };
                if !placeholders.iter().any(|(o, _)| o == &original) {
                    placeholders.push((original.clone(), placeholder.clone()));
                }
            }
        }

        for (original, placeholder) in &placeholders {
            protected = protected.replace(original, placeholder);
        }

        (protected, placeholders)
    }

    fn restore_formatting(text: &str, placeholders: &[(String, String)]) -> String {
        let mut restored = text.to_string();
        for (original, placeholder) in placeholders {
            restored = restored.replace(placeholder, original);
        }
        // Also restore common mistranslations
        restored = restored.replace("⟦NL⟧", "\\n");
        restored = restored.replace("⟦TB⟧", "\\t");
        restored
    }

    fn do_google_request(&self, url: &str) -> Result<String> {
        let response = self
            .client
            .get(url)
            .header("User-Agent", "Mozilla/5.0")
            .send()
            .context("Failed to send request to Google Translate")?;

        if !response.status().is_success() {
            let status = response.status();
            anyhow::bail!("Google Translate request failed: {}", status);
        }

        let body = response.text().context("Failed to read response")?;

        let parsed: serde_json::Value =
            serde_json::from_str(&body).context("Failed to parse Google Translate response")?;

        let mut result = String::new();
        if let Some(outer) = parsed.get(0).and_then(|v| v.as_array()) {
            for item in outer {
                if let Some(translated) = item.get(0).and_then(|v| v.as_str()) {
                    result.push_str(translated);
                }
            }
        }

        if result.is_empty() {
            anyhow::bail!("No translation result from Google");
        }

        Ok(result)
    }
}
