//! Relay-declared read limits, or their honest absence.

use std::num::NonZeroUsize;

/// One relay-declared read limit, or the honest absence of one.
///
/// Authority: GOALS:1068 (RELAY-004) "Missing, stale, malformed, or unsupported
/// claims remain unknown rather than becoming invented defaults."
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DeclaredLimit {
    /// The relay declared nothing Fava can interpret deterministically.
    #[default]
    Unknown,
    /// The relay declared this exact limit.
    Declared(NonZeroUsize),
}

impl DeclaredLimit {
    /// The declared value, if any. `None` means unknown — never a default.
    #[must_use]
    pub const fn get(self) -> Option<NonZeroUsize> {
        match self {
            Self::Unknown => None,
            Self::Declared(value) => Some(value),
        }
    }
}

/// Read limits one relay declares, per relay session.
///
/// Authority: ARCH:1488 (`constraints: &RelayReadConstraints`);
/// GOALS:1055-1064 (RELAY-004) enumerates exactly these five.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RelayReadConstraints {
    /// Concurrent wire subscriptions this relay accepts.
    pub max_subscriptions: DeclaredLimit,
    /// Maximum encoded bytes of one client message.
    pub max_message_bytes: DeclaredLimit,
    /// Maximum characters in a subscription id.
    pub max_subscription_id_chars: DeclaredLimit,
    /// Maximum `limit` a filter may request.
    pub max_filter_limit: DeclaredLimit,
    /// `limit` the relay applies when a filter declares none. Its presence
    /// forbids merging filters that declare no limit (GOALS:1049).
    pub default_filter_limit: DeclaredLimit,
}

impl RelayReadConstraints {
    /// Constraints for a relay whose NIP-11 document is absent, stale, or
    /// uninterpretable. Every field is `Unknown`, never invented.
    #[must_use]
    pub const fn unknown() -> Self {
        Self {
            max_subscriptions: DeclaredLimit::Unknown,
            max_message_bytes: DeclaredLimit::Unknown,
            max_subscription_id_chars: DeclaredLimit::Unknown,
            max_filter_limit: DeclaredLimit::Unknown,
            default_filter_limit: DeclaredLimit::Unknown,
        }
    }
}
