use std::collections::BTreeSet;

use nostr::types::RelayUrl;
use serde::{Deserialize, Serialize};

use crate::WriteIntentError;

const MAX_EXPLICIT_RELAYS: usize = 256;
pub(crate) const MAX_RAW_EXPLICIT_RELAYS: usize = 1_024;

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
    /// Duplicate relay identities collapse to one entry. The finite owned raw
    /// input is refused before normalization when it exceeds 1,024 occurrences;
    /// normalized routes separately refuse more than 256 distinct destinations.
    ///
    /// # Arguments
    ///
    /// * `relays` - the relay sequence to publish to, in caller order
    ///
    /// # Errors
    ///
    /// Returns [`WriteIntentError`] when no destination remains, raw input
    /// exceeds 1,024 occurrences, or more than 256 distinct identities remain.
    ///
    /// # Examples
    ///
    /// ```
    /// # use fava_write::WriteRouting;
    /// # use nostr::types::RelayUrl;
    /// let a = RelayUrl::parse("wss://relay.a").expect("valid relay URL");
    /// let b = RelayUrl::parse("wss://relay.b").expect("valid relay URL");
    ///
    /// // Duplicates collapse, and the first-seen order is kept.
    /// let routing = WriteRouting::explicit([a.clone(), b.clone(), a.clone()])
    ///     .expect("non-empty route within bound");
    /// ```
    ///
    /// Arbitrary iterators are intentionally not accepted at this boundary:
    ///
    /// ```compile_fail
    /// use fava_write::WriteRouting;
    /// use nostr::types::RelayUrl;
    /// let relay = RelayUrl::parse("wss://relay.example").unwrap();
    /// let _ = WriteRouting::explicit(std::iter::repeat(relay));
    /// ```
    pub fn explicit(relays: impl Into<Vec<RelayUrl>>) -> Result<Self, WriteIntentError> {
        let relays = relays.into();
        refuse_raw_input(relays.len())?;
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

    pub(crate) fn append(self, relays: Vec<RelayUrl>) -> Result<Self, WriteIntentError> {
        let mut ordered = match self {
            Self::Automatic => Vec::new(),
            Self::Explicit(relays) => relays,
        };
        let mut seen = ordered.iter().cloned().collect::<BTreeSet<_>>();
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

pub(crate) fn refuse_raw_input(actual: usize) -> Result<(), WriteIntentError> {
    if actual > MAX_RAW_EXPLICIT_RELAYS {
        return Err(WriteIntentError::TooManyRawExplicitRelays {
            actual,
            maximum: MAX_RAW_EXPLICIT_RELAYS,
        });
    }
    Ok(())
}
