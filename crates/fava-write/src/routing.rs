use std::collections::BTreeSet;

use nostr::types::RelayUrl;
use serde::{Deserialize, Serialize};

use crate::WriteIntentError;

const MAX_EXPLICIT_RELAYS: usize = 256;

/// Relay selection for one publication obligation.
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
    /// until the normalized bound is exceeded, so an unbounded iterator cannot
    /// allocate an unbounded route.
    ///
    /// # Errors
    ///
    /// Returns [`WriteIntentError`] when no destination remains or more than
    /// 256 distinct relay identities are supplied.
    pub fn explicit(relays: impl IntoIterator<Item = RelayUrl>) -> Result<Self, WriteIntentError> {
        let mut seen = BTreeSet::new();
        let mut ordered = Vec::new();
        for relay in relays {
            if seen.insert(relay.clone()) {
                ordered.push(relay);
                if ordered.len() > MAX_EXPLICIT_RELAYS {
                    return Err(WriteIntentError::TooManyExplicitRelays {
                        actual: ordered.len(),
                        maximum: MAX_EXPLICIT_RELAYS,
                    });
                }
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
        if relays.len() > MAX_EXPLICIT_RELAYS {
            return Err(WriteIntentError::TooManyExplicitRelays {
                actual: relays.len(),
                maximum: MAX_EXPLICIT_RELAYS,
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
