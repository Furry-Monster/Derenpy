//! Translation cache using SQLite

use anyhow::{Context, Result};
use rusqlite::{Connection, params};
use std::path::PathBuf;

pub struct TranslationCache {
    conn: Connection,
}

#[derive(Debug, Default)]
#[allow(dead_code)]
pub struct CacheStats {
    pub total_entries: usize,
    pub providers: Vec<(String, usize)>,
}

impl TranslationCache {
    pub fn open() -> Result<Self> {
        let path = Self::cache_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(&path).context("Failed to open translation cache")?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS translations (
                id INTEGER PRIMARY KEY,
                source_text TEXT NOT NULL,
                target_lang TEXT NOT NULL,
                provider TEXT NOT NULL,
                translated_text TEXT NOT NULL,
                created_at INTEGER DEFAULT (strftime('%s', 'now')),
                UNIQUE(source_text, target_lang, provider)
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_lookup 
             ON translations(source_text, target_lang, provider)",
            [],
        )?;

        Ok(Self { conn })
    }

    pub fn get(&self, text: &str, lang: &str, provider: &str) -> Option<String> {
        self.conn
            .query_row(
                "SELECT translated_text FROM translations 
                 WHERE source_text = ?1 AND target_lang = ?2 AND provider = ?3",
                params![text, lang, provider],
                |row| row.get(0),
            )
            .ok()
    }

    pub fn set(&self, text: &str, lang: &str, provider: &str, translated: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO translations (source_text, target_lang, provider, translated_text)
             VALUES (?1, ?2, ?3, ?4)",
            params![text, lang, provider, translated],
        )?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn stats(&self) -> Result<CacheStats> {
        let total: usize = self
            .conn
            .query_row("SELECT COUNT(*) FROM translations", [], |row| row.get(0))?;

        let mut stmt = self
            .conn
            .prepare("SELECT provider, COUNT(*) FROM translations GROUP BY provider")?;
        let providers: Vec<(String, usize)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(CacheStats {
            total_entries: total,
            providers,
        })
    }

    #[allow(dead_code)]
    pub fn clear(&self) -> Result<()> {
        self.conn.execute("DELETE FROM translations", [])?;
        Ok(())
    }

    fn cache_path() -> Result<PathBuf> {
        let cache_dir = dirs::cache_dir()
            .context("Failed to find cache directory")?
            .join("derenpy");
        Ok(cache_dir.join("translations.db"))
    }
}
