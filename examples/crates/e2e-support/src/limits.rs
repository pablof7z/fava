//! Explicit resource limits for private E2E command sessions.

use crate::ShellError;
use crate::ingress::reject_prompted_value;

/// Every application-owned retained-input and execution boundary for one shell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    accounts: usize,
    relays: usize,
    captures: usize,
    history: usize,
    line_bytes: usize,
    capture_bytes: usize,
    result_fields: usize,
    arguments: usize,
    alias_bytes: usize,
}

impl Limits {
    /// Construct one finite command-session policy.
    ///
    /// # Errors
    ///
    /// Refuses zero limits because each named capability needs a usable minimum.
    #[allow(
        clippy::too_many_arguments,
        reason = "each independently retained input boundary needs an explicit caller-selected maximum"
    )]
    pub fn new(
        accounts: usize,
        relays: usize,
        captures: usize,
        history: usize,
        line_bytes: usize,
        capture_bytes: usize,
        result_fields: usize,
        arguments: usize,
        alias_bytes: usize,
    ) -> Result<Self, ShellError> {
        for (what, value) in [
            ("accounts", accounts),
            ("relay aliases", relays),
            ("captures", captures),
            ("history", history),
            ("command line bytes", line_bytes),
            ("capture bytes", capture_bytes),
            ("result fields", result_fields),
            ("command arguments", arguments),
            ("alias bytes", alias_bytes),
        ] {
            if value == 0 {
                return Err(ShellError::Limit { what, maximum: 0 });
            }
        }
        Ok(Self {
            accounts,
            relays,
            captures,
            history,
            line_bytes,
            capture_bytes,
            result_fields,
            arguments,
            alias_bytes,
        })
    }

    /// Return the explicit standard policy for one private E2E application session.
    #[must_use]
    pub const fn standard() -> Self {
        Self {
            accounts: 4,
            relays: 8,
            captures: 8,
            history: 16,
            line_bytes: 512,
            capture_bytes: 4_096,
            result_fields: 16,
            arguments: 16,
            alias_bytes: 32,
        }
    }

    pub(crate) const fn accounts(self) -> usize {
        self.accounts
    }
    pub(crate) const fn relays(self) -> usize {
        self.relays
    }
    pub(crate) const fn captures(self) -> usize {
        self.captures
    }
    pub(crate) const fn history(self) -> usize {
        self.history
    }
    pub(crate) const fn line_bytes(self) -> usize {
        self.line_bytes
    }
    pub(crate) const fn capture_bytes(self) -> usize {
        self.capture_bytes
    }
    pub(crate) const fn result_fields(self) -> usize {
        self.result_fields
    }
    pub(crate) const fn arguments(self) -> usize {
        self.arguments
    }
    pub(crate) const fn alias_bytes(self) -> usize {
        self.alias_bytes
    }

    /// Validate one ordinary interactive value before it reaches a domain parser.
    ///
    /// # Errors
    ///
    /// Refuses protected-looking input and values over this policy's command
    /// line bound, matching the shared buffered prompt path.
    pub fn validate_prompt_value(self, label: &str, value: &str) -> Result<(), ShellError> {
        reject_prompted_value(label, value)?;
        if value.len() > self.line_bytes {
            return Err(ShellError::Limit {
                what: "prompt value bytes",
                maximum: self.line_bytes,
            });
        }
        Ok(())
    }
}
