//! Interaksi terminal sederhana.

use std::io::{self, BufRead, IsTerminal, Write};

/// Tanya y/N. Tanpa terminal interaktif (stdin bukan TTY) jawabannya "tidak".
#[must_use]
pub fn confirm(question: &str) -> bool {
    let stdin = io::stdin();
    if !stdin.is_terminal() {
        return false;
    }
    print!("{question} [y/N] ");
    let _ = io::stdout().flush();
    let mut line = String::new();
    if stdin.lock().read_line(&mut line).is_err() {
        return false;
    }
    matches!(
        line.trim().to_ascii_lowercase().as_str(),
        "y" | "ya" | "yes"
    )
}

/// Potong teks ke `max` karakter (dengan `…`).
#[must_use]
pub fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        let mut s: String = text.chars().take(max.saturating_sub(1)).collect();
        s.push('…');
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clip_keeps_short_text() {
        assert_eq!(clip("abc", 5), "abc");
        assert_eq!(clip("abcdef", 4), "abc…");
    }
}
