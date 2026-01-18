use terminal_size::{terminal_size, Width};

/// Get the optimal text width for wrapping
/// - Detects terminal width
/// - Caps at 120 characters maximum
/// - Defaults to 80 if detection fails
pub fn get_wrap_width() -> usize {
    const MAX_WIDTH: usize = 120;
    const DEFAULT_WIDTH: usize = 80;
    const MARGIN: usize = 2; // Leave margin for padding/indentation

    if let Some((Width(w), _)) = terminal_size() {
        let width = w as usize;
        // Use terminal width minus margin, but cap at MAX_WIDTH
        std::cmp::min(width.saturating_sub(MARGIN), MAX_WIDTH)
    } else {
        DEFAULT_WIDTH
    }
}

/// Get wrap width for indented text (e.g., threaded replies)
/// - Accounts for indentation level
pub fn get_wrap_width_with_indent(indent_size: usize) -> usize {
    get_wrap_width().saturating_sub(indent_size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_wrap_width_returns_reasonable_value() {
        let width = get_wrap_width();
        // Should be between 1 and 120
        assert!(width > 0);
        assert!(width <= 120);
    }

    #[test]
    fn test_get_wrap_width_with_indent() {
        let base_width = get_wrap_width();
        let indented_width = get_wrap_width_with_indent(4);

        // Indented width should be 4 less than base width
        assert_eq!(indented_width, base_width.saturating_sub(4));
    }

    #[test]
    fn test_get_wrap_width_with_large_indent() {
        // Should not underflow
        let width = get_wrap_width_with_indent(200);
        assert_eq!(width, 0);
    }
}
