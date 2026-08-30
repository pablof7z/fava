//! Presentation mechanics shared exactly by runnable examples.

/// Elide one terminal value to an exact character-cell budget.
#[must_use]
pub fn elide(value: &str, width: usize) -> String {
    if width == 0 {
        String::new()
    } else if value.chars().count() <= width {
        value.to_owned()
    } else {
        format!(
            "{}…",
            value
                .chars()
                .take(width.saturating_sub(1))
                .collect::<String>()
        )
    }
}
