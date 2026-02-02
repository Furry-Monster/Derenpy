//! Common utility functions

#[allow(dead_code)]
pub fn truncate_display(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        format!("{}...", s.chars().take(max_len).collect::<String>())
    }
}

pub fn unquote(s: &str) -> String {
    let s = s.trim();
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

pub fn is_code_like(s: &str) -> bool {
    s.starts_with('[')
        || s.starts_with('{')
        || s.contains("%(")
        || s.starts_with("!!")
        || s.chars()
            .all(|c| c.is_ascii_punctuation() || c.is_whitespace())
}

pub const RENPY_KEYWORDS: &[&str] = &[
    // Control flow
    "label ",
    "jump ",
    "call ",
    "return",
    "pass",
    "menu:",
    "if ",
    "elif ",
    "else:",
    "for ",
    "while ",
    // Python
    "$",
    "python:",
    "init ",
    // Definitions
    "define ",
    "default ",
    "image ",
    "transform ",
    "screen ",
    "style ",
    // Display
    "show ",
    "hide ",
    "scene ",
    "with ",
    // Audio
    "play ",
    "stop ",
    "queue ",
    "voice ",
    // UI
    "nvl ",
    "window ",
    "pause",
    // Screen language (ATL & displayables)
    "add ",
    "use ",
    "vbox",
    "hbox",
    "frame",
    "grid",
    "fixed",
    "side",
    "text ",
    "imagebutton",
    "textbutton",
    "button",
    "bar",
    "vbar",
    "input",
    "key",
    "timer",
    "viewport",
    "vpgrid",
    "drag",
    "draggroup",
    "mousearea",
    "imagemap",
    "hotspot",
    "hotbar",
    "on ",
    "action ",
    "has ",
    "at ",
    "as ",
    "behind ",
    "onlayer ",
    "zorder ",
    // Translate
    "translate ",
];

pub fn is_renpy_keyword(line: &str) -> bool {
    RENPY_KEYWORDS.iter().any(|k| line.starts_with(k))
}
