//! NIP-11 relay information values, validation, and the fetch contract.
//!
//! This crate owns what a relay says about itself and how that becomes exact
//! planning and publication limits. It owns no acquisition mechanism and makes
//! no freshness claim: `fava-nip11-http` performs one bounded fetch, and
//! caching, staleness, negative caching, and last-good-document semantics are
//! not promised by this milestone.

use std::collections::BTreeSet;
use std::future::Future;
use std::pin::Pin;

use fava_publisher::RelayWriteLimits;
use fava_state::RelayUrl;
use fava_subscriptions::RelayLimits;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Largest NIP-11 document Fava will parse.
pub const MAX_DOCUMENT_BYTES: usize = 65_536;

/// Largest text field retained from a relay information document.
pub const MAX_TEXT_BYTES: usize = 4_096;

/// What one relay declares it will accept.
///
/// Every field is optional and absent means exactly "the relay did not say".
/// A missing, malformed, or unsupported claim stays unknown. It never becomes
/// an invented default.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct RelayLimitation {
    /// Largest accepted WebSocket message in bytes.
    #[serde(default)]
    pub max_message_length: Option<usize>,
    /// Largest number of concurrent subscriptions per connection.
    #[serde(default)]
    pub max_subscriptions: Option<usize>,
    /// Largest number of filters in one REQ.
    #[serde(default)]
    pub max_filters: Option<usize>,
    /// Largest accepted per-filter `limit` value.
    #[serde(default)]
    pub max_limit: Option<usize>,
    /// Largest accepted subscription id in bytes.
    #[serde(default)]
    pub max_subid_length: Option<usize>,
    /// Largest accepted tag count on one event.
    #[serde(default)]
    pub max_event_tags: Option<usize>,
    /// Largest accepted event content in bytes.
    #[serde(default)]
    pub max_content_length: Option<usize>,
    /// Smallest accepted proof-of-work difficulty.
    #[serde(default)]
    pub min_pow_difficulty: Option<u8>,
    /// Whether the relay requires NIP-42 before serving or accepting work.
    #[serde(default)]
    pub auth_required: Option<bool>,
    /// Whether the relay requires payment.
    #[serde(default)]
    pub payment_required: Option<bool>,
    /// Whether the relay restricts who may write.
    #[serde(default)]
    pub restricted_writes: Option<bool>,
}

impl RelayLimitation {
    /// Project the claims subscription planning can interpret deterministically.
    #[must_use]
    pub const fn planning(&self) -> RelayLimits {
        RelayLimits {
            max_subscriptions: self.max_subscriptions,
            max_filters: self.max_filters,
            max_message_length: self.max_message_length,
            max_subscription_id_length: self.max_subid_length,
            max_filter_limit: self.max_limit,
        }
    }

    /// Project the claims publication can evaluate locally before handoff.
    #[must_use]
    pub const fn writes(&self) -> RelayWriteLimits {
        RelayWriteLimits {
            max_message_length: self.max_message_length,
            max_content_length: self.max_content_length,
            max_event_tags: self.max_event_tags,
            min_pow_difficulty: self.min_pow_difficulty,
            auth_required: self.auth_required,
            restricted_writes: self.restricted_writes,
        }
    }
}

/// One relay's validated NIP-11 information document.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct RelayInformation {
    /// Operator-supplied relay name.
    #[serde(default)]
    pub name: Option<String>,
    /// Operator-supplied description.
    #[serde(default)]
    pub description: Option<String>,
    /// Software identifier the relay reports.
    #[serde(default)]
    pub software: Option<String>,
    /// Software version the relay reports.
    #[serde(default)]
    pub version: Option<String>,
    /// NIPs the relay claims to support.
    #[serde(default)]
    pub supported_nips: BTreeSet<u16>,
    /// Declared limits, empty when the relay declares none.
    #[serde(default)]
    pub limitation: RelayLimitation,
}

impl RelayInformation {
    /// Whether the relay claims NIP-42 support.
    #[must_use]
    pub fn supports_authentication(&self) -> bool {
        self.supported_nips.contains(&42)
    }
}

/// Exact refusal of one relay-information acquisition or document.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum RelayInformationError {
    /// The relay could not be reached for its information document.
    #[error("relay information is unreachable: {0}")]
    Unreachable(String),
    /// The relay answered with something that is not a NIP-11 document.
    #[error("relay information document is malformed: {0}")]
    Malformed(String),
    /// The relay answered with a document beyond the declared bound.
    #[error("relay information document uses {bytes} bytes but Fava allows {maximum}")]
    TooLarge {
        /// Exact document size.
        bytes: usize,
        /// Declared maximum document size.
        maximum: usize,
    },
    /// The relay refused the information request.
    #[error("relay refused the information request: {0}")]
    Refused(String),
}

/// Replaceable acquisition of one relay's NIP-11 information document.
pub trait RelayInformationFetcher: Send + Sync {
    /// Acquire the current document for one exact relay.
    ///
    /// Implementations own acquisition and its bounds. They do not decide
    /// planning or publication policy.
    #[allow(clippy::type_complexity)] // Inline future keeps this contract object-safe.
    fn get(
        &self,
        relay: RelayUrl,
    ) -> Pin<Box<dyn Future<Output = Result<RelayInformation, RelayInformationError>> + Send + '_>>;
}

/// Parse and validate one bounded NIP-11 document body.
///
/// # Errors
///
/// Returns [`RelayInformationError::TooLarge`] beyond [`MAX_DOCUMENT_BYTES`]
/// and [`RelayInformationError::Malformed`] for anything that is not a NIP-11
/// object. Unrecognized fields are ignored rather than becoming claims.
pub fn parse_relay_information(body: &[u8]) -> Result<RelayInformation, RelayInformationError> {
    if body.len() > MAX_DOCUMENT_BYTES {
        return Err(RelayInformationError::TooLarge {
            bytes: body.len(),
            maximum: MAX_DOCUMENT_BYTES,
        });
    }
    let mut document: RelayInformation = serde_json::from_slice(body)
        .map_err(|error| RelayInformationError::Malformed(error.to_string()))?;
    for text in [
        &mut document.name,
        &mut document.description,
        &mut document.software,
        &mut document.version,
    ] {
        if let Some(value) = text
            && value.len() > MAX_TEXT_BYTES
        {
            return Err(RelayInformationError::Malformed(format!(
                "relay information text uses {} bytes but Fava allows {MAX_TEXT_BYTES}",
                value.len()
            )));
        }
    }
    Ok(document)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_absent_limitation_block_declares_nothing() {
        let document = parse_relay_information(br#"{"name":"quiet relay"}"#).expect("parses");
        assert_eq!(document.limitation, RelayLimitation::default());
        assert_eq!(document.limitation.planning(), RelayLimits::unknown());
        assert!(!document.supports_authentication());
    }

    #[test]
    fn declared_limits_project_into_planning_and_write_bounds() {
        let document = parse_relay_information(
            br#"{"supported_nips":[1,11,42],"limitation":{"max_subscriptions":1,
                 "max_message_length":512,"max_limit":50,"max_subid_length":24,
                 "max_content_length":140,"max_event_tags":8,"auth_required":true}}"#,
        )
        .expect("parses");
        assert!(document.supports_authentication());
        let planning = document.limitation.planning();
        assert_eq!(planning.max_subscriptions, Some(1));
        assert_eq!(planning.max_message_length, Some(512));
        assert_eq!(planning.max_subscription_id_length, Some(24));
        assert_eq!(planning.max_filter_limit, Some(50));
        let writes = document.limitation.writes();
        assert_eq!(writes.max_content_length, Some(140));
        assert_eq!(writes.max_event_tags, Some(8));
        assert_eq!(writes.auth_required, Some(true));
        assert_eq!(writes.min_pow_difficulty, None);
    }

    #[test]
    fn an_oversized_document_is_refused_with_exact_counts() {
        let body = vec![b' '; MAX_DOCUMENT_BYTES + 1];
        assert_eq!(
            parse_relay_information(&body),
            Err(RelayInformationError::TooLarge {
                bytes: MAX_DOCUMENT_BYTES + 1,
                maximum: MAX_DOCUMENT_BYTES,
            })
        );
    }

    #[test]
    fn a_non_document_body_is_malformed_rather_than_an_invented_default() {
        assert!(matches!(
            parse_relay_information(b"<html>not a relay document</html>"),
            Err(RelayInformationError::Malformed(_))
        ));
    }
}
