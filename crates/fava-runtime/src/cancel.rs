//! Owner-held cancellation tokens.

/// Owner-held cancellation token that propagates to every derived child.
#[derive(Clone, Debug)]
pub struct Cancellation {
    _placeholder: (),
}

impl Default for Cancellation {
    fn default() -> Self {
        Self::new()
    }
}

impl Cancellation {
    /// Create an uncancelled root token.
    #[must_use]
    pub fn new() -> Self {
        todo!()
    }

    /// Derive a child token cancelled by this token or on its own.
    #[must_use]
    pub fn child(&self) -> Self {
        todo!()
    }

    /// Cancel this token and every token derived from it.
    pub fn cancel(&self) {
        todo!()
    }

    /// Whether this token is cancelled.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        todo!()
    }

    /// Resolve once this token is cancelled.
    pub async fn cancelled(&self) {
        todo!()
    }
}
