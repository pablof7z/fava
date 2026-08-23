//! Fava-owned byte bound on relay- and OS-supplied text.

/// Relay- or OS-supplied text retained under a Fava-owned byte bound.
///
/// Authority: GOALS:1428 (OPS-004, "frame and message sizes"), GOALS:1105
/// (RELAY-008, verbatim evidence). Truncation is recorded, never silent.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BoundedReason {
    text: String,
    truncated_bytes: usize,
}

impl BoundedReason {
    /// Maximum retained bytes.
    pub const MAX_BYTES: usize = 512;

    /// Retain at most `MAX_BYTES`, recording how many were dropped.
    #[must_use]
    pub fn new(text: impl AsRef<str>) -> Self {
        let text = text.as_ref();
        let mut end = text.len().min(Self::MAX_BYTES);
        while end > 0 && !text.is_char_boundary(end) {
            end -= 1;
        }
        Self {
            text: text[..end].to_owned(),
            truncated_bytes: text.len() - end,
        }
    }

    /// Retained text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// Bytes dropped by the bound. Non-zero means the fact is a shortfall.
    #[must_use]
    pub const fn truncated_bytes(&self) -> usize {
        self.truncated_bytes
    }
}
