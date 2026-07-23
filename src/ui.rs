use inquire::Select;

/// Replacement for `inquire::Confirm` — presents a Select with "Yes" / "No" choices.
/// Cancellation (Esc/Ctrl-C) is always treated as "No".
pub fn confirm(prompt: &str, default_val: bool) -> bool {
    let choices = if default_val {
        vec!["Yes", "No"]
    } else {
        vec!["No", "Yes"]
    };
    match Select::new(prompt, choices).prompt() {
        Ok("Yes") => true,
        Ok("No") => false,
        _ => false,
    }
}

/// Strip tree-drawing Unicode characters from a string for cleaner display.
pub fn strip_tree_chars(s: &str) -> String {
    s.chars()
        .filter(|c| {
            !matches!(
                c,
                '\u{2500}'
                    ..='\u{257F}' | // Box Drawing block
                '\u{25B6}' | '\u{25BC}' // Arrows ▶ ▼
            )
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_tree_chars_branch() {
        let input = "  ├── Provider              groq";
        let result = strip_tree_chars(input);
        assert_eq!(result, "Provider groq");
    }

    #[test]
    fn test_strip_tree_chars_last_branch() {
        let input = "  └── API Key               ****";
        let result = strip_tree_chars(input);
        assert_eq!(result, "API Key ****");
    }

    #[test]
    fn test_strip_tree_chars_nested() {
        let input = "  │   ├── Locale              en";
        let result = strip_tree_chars(input);
        assert_eq!(result, "Locale en");
    }

    #[test]
    fn test_strip_tree_chars_arrow() {
        let input = "▼ Basic";
        let result = strip_tree_chars(input);
        assert_eq!(result, "Basic");
    }

    #[test]
    fn test_strip_tree_chars_no_change() {
        let input = "Save & Exit";
        let result = strip_tree_chars(input);
        assert_eq!(result, "Save & Exit");
    }
}
