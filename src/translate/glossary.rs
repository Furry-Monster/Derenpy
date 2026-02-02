//! Glossary support for consistent term translation

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Default)]
pub struct Glossary {
    terms: HashMap<String, String>,
    case_insensitive: HashMap<String, String>,
}

impl Glossary {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path.as_ref()).context("Failed to read glossary file")?;
        let mut glossary = Self::new();

        for (line_num, line) in content.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
                continue;
            }
            if let Some((source, target)) = Self::parse_line(line) {
                glossary.add(source, target);
            } else {
                tracing::warn!("Invalid glossary entry at line {}: {}", line_num + 1, line);
            }
        }
        Ok(glossary)
    }

    fn parse_line(line: &str) -> Option<(String, String)> {
        // Supports: "source = target" and "source\ttarget" formats
        for sep in ['=', '\t'] {
            let parts: Vec<&str> = line.splitn(2, sep).collect();
            if parts.len() == 2 {
                let source = parts[0].trim();
                let target = parts[1].trim();
                if !source.is_empty() && !target.is_empty() {
                    return Some((source.to_string(), target.to_string()));
                }
            }
        }
        None
    }

    pub fn add(&mut self, source: String, target: String) {
        self.case_insensitive
            .insert(source.to_lowercase(), target.clone());
        self.terms.insert(source, target);
    }

    pub fn len(&self) -> usize {
        self.terms.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }

    pub fn apply(&self, text: &str) -> String {
        let mut result = text.to_string();
        // Longer terms first to avoid partial replacements
        let mut sorted_terms: Vec<_> = self.terms.iter().collect();
        sorted_terms.sort_by(|a, b| b.0.len().cmp(&a.0.len()));
        for (source, target) in sorted_terms {
            result = result.replace(source, target);
        }
        result
    }

    #[allow(dead_code)]
    pub fn build_prompt_context(&self) -> String {
        if self.terms.is_empty() {
            return String::new();
        }
        let mut context = String::from("Use the following translations for specific terms:\n");
        for (source, target) in &self.terms {
            context.push_str(&format!("- \"{}\" → \"{}\"\n", source, target));
        }
        context
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_equals() {
        let glossary = Glossary::load_from_str("Sylvie = 西尔维\nProfessor Eileen = 艾琳教授");
        assert_eq!(glossary.len(), 2);
        assert_eq!(glossary.terms.get("Sylvie"), Some(&"西尔维".to_string()));
    }

    #[test]
    fn test_apply() {
        let mut glossary = Glossary::new();
        glossary.add("Sylvie".to_string(), "西尔维".to_string());

        let result = glossary.apply("Hello, Sylvie!");
        assert_eq!(result, "Hello, 西尔维!");
    }

    impl Glossary {
        fn load_from_str(content: &str) -> Self {
            let mut glossary = Self::new();
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if let Some((source, target)) = Self::parse_line(line) {
                    glossary.add(source, target);
                }
            }
            glossary
        }
    }
}
