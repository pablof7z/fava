use std::collections::BTreeSet;

use fava_state::RelayUrl;
use serde::{Deserialize, Serialize};

use crate::WriteIntentError;

// Provisional implementation resource-safety cap. This is not a Nostr limit or
// publication-domain semantic; fava-write owns and may revise the shortcut.
const PROVISIONAL_MAX_EXPLICIT_RELAYS: usize = 256;

/// Relay selection for one publication obligation.
///
/// Explicit routes preserve normalized first-occurrence order.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum WriteRouting {
    /// Use the configured ordered router chain.
    Automatic,
    /// Use exactly this normalized relay sequence and open no automatic router.
    Explicit(Vec<RelayUrl>),
}

impl WriteRouting {
    /// Normalize an exact explicit route in caller first-occurrence order.
    ///
    /// Duplicate relay identities collapse to one entry. Input is consumed only
    /// until the provisional write resource cap is exceeded, so an unbounded
    /// iterator cannot allocate an unbounded route. The cap is not a protocol
    /// fact or publication-domain semantic.
    ///
    /// # Errors
    ///
    /// Returns [`WriteIntentError`] when no destination remains or more than
    /// 256 relay inputs are supplied; that resource-safety value is provisional.
    pub fn explicit(relays: impl IntoIterator<Item = RelayUrl>) -> Result<Self, WriteIntentError> {
        let mut seen = BTreeSet::new();
        let mut ordered = Vec::new();
        for (index, relay) in relays.into_iter().enumerate() {
            let actual = index.saturating_add(1);
            if actual > PROVISIONAL_MAX_EXPLICIT_RELAYS {
                return Err(WriteIntentError::TooManyExplicitRelays {
                    actual,
                    maximum: PROVISIONAL_MAX_EXPLICIT_RELAYS,
                });
            }
            if seen.insert(relay.clone()) {
                ordered.push(relay);
            }
        }
        if ordered.is_empty() {
            return Err(WriteIntentError::EmptyExplicitRelays);
        }
        Ok(Self::Explicit(ordered))
    }

    pub(crate) fn validate(&self) -> Result<(), WriteIntentError> {
        let Self::Explicit(relays) = self else {
            return Ok(());
        };
        if relays.is_empty() {
            return Err(WriteIntentError::EmptyExplicitRelays);
        }
        if relays.len() > PROVISIONAL_MAX_EXPLICIT_RELAYS {
            return Err(WriteIntentError::TooManyExplicitRelays {
                actual: relays.len(),
                maximum: PROVISIONAL_MAX_EXPLICIT_RELAYS,
            });
        }
        let mut seen = BTreeSet::new();
        for relay in relays {
            if !seen.insert(relay.clone()) {
                return Err(WriteIntentError::DuplicateExplicitRelay {
                    relay: relay.clone(),
                });
            }
        }
        Ok(())
    }
}
