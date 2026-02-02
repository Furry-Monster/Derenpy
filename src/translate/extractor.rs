//! Text extraction from Ren'Py scripts

use anyhow::{Context, Result};
use regex::Regex;
use std::fs;
use std::path::Path;

use crate::utils::{is_code_like, is_renpy_keyword, unquote};

#[derive(Debug, Clone)]
pub struct TranslatableEntry {
    pub id: usize,
    pub text: String,
    pub line_number: usize,
    pub entry_type: EntryType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EntryType {
    Dialogue,
    Narration,
    MenuChoice,
}

pub struct TextExtractor {
    dialogue_re: Regex,
    narration_re: Regex,
    menu_re: Regex,
}

impl Default for TextExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl TextExtractor {
    pub fn new() -> Self {
        Self {
            dialogue_re: Regex::new(
                r#"^\s*(\w+)\s+("[^"\\]*(?:\\.[^"\\]*)*"|'[^'\\]*(?:\\.[^'\\]*)*')"#,
            )
            .unwrap(),
            narration_re: Regex::new(
                r#"^\s*("[^"\\]*(?:\\.[^"\\]*)*"|'[^'\\]*(?:\\.[^'\\]*)*')\s*$"#,
            )
            .unwrap(),
            menu_re: Regex::new(r#"^\s*("[^"\\]*(?:\\.[^"\\]*)*"|'[^'\\]*(?:\\.[^'\\]*)*')\s*:"#)
                .unwrap(),
        }
    }

    pub fn extract_from_file<P: AsRef<Path>>(&self, path: P) -> Result<Vec<TranslatableEntry>> {
        let content = fs::read_to_string(path.as_ref()).context("Failed to read script file")?;
        self.extract_from_string(&content)
    }

    pub fn extract_from_string(&self, content: &str) -> Result<Vec<TranslatableEntry>> {
        let mut entries = Vec::new();
        let mut id = 0;

        for (line_num, line) in content.lines().enumerate() {
            let line_number = line_num + 1;
            let trimmed = line.trim();

            if trimmed.is_empty() || trimmed.starts_with('#') || is_renpy_keyword(trimmed) {
                continue;
            }

            if let Some(caps) = self.dialogue_re.captures(line) {
                let text = caps.get(2).map(|m| m.as_str()).unwrap_or("");
                let text = unquote(text);

                if !text.is_empty() && !is_code_like(&text) {
                    entries.push(TranslatableEntry {
                        id,
                        text,
                        line_number,
                        entry_type: EntryType::Dialogue,
                    });
                    id += 1;
                }
                continue;
            }

            if let Some(caps) = self.menu_re.captures(line) {
                let text = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                let text = unquote(text);

                if !text.is_empty() {
                    entries.push(TranslatableEntry {
                        id,
                        text,
                        line_number,
                        entry_type: EntryType::MenuChoice,
                    });
                    id += 1;
                }
                continue;
            }

            if let Some(caps) = self.narration_re.captures(line) {
                let text = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                let text = unquote(text);

                if !text.is_empty() && !is_code_like(&text) {
                    entries.push(TranslatableEntry {
                        id,
                        text,
                        line_number,
                        entry_type: EntryType::Narration,
                    });
                    id += 1;
                }
            }
        }

        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_dialogue() {
        let extractor = TextExtractor::new();
        let content = r#"
label start:
    e "Hello, world!"
    "This is narration."
    menu:
        "Choice 1":
            pass
        "Choice 2":
            pass
"#;
        let entries = extractor.extract_from_string(content).unwrap();
        assert_eq!(entries.len(), 4);
    }
}
