use std::collections::BTreeMap;

use fava_relay::RelaySessionKey;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::RelayDeliveryOutcome;
use crate::relay_session_serde::{decode, encode};

pub(super) fn serialize<S>(
    value: &BTreeMap<RelaySessionKey, RelayDeliveryOutcome>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    value
        .iter()
        .map(|(session, outcome)| (encode(session), outcome))
        .collect::<Vec<_>>()
        .serialize(serializer)
}

pub(super) fn deserialize<'de, D>(
    deserializer: D,
) -> Result<BTreeMap<RelaySessionKey, RelayDeliveryOutcome>, D::Error>
where
    D: Deserializer<'de>,
{
    let pairs = Vec::<(
        (nostr::types::RelayUrl, Option<nostr::key::PublicKey>),
        RelayDeliveryOutcome,
    )>::deserialize(deserializer)?;
    let count = pairs.len();
    let map: BTreeMap<_, _> = pairs
        .into_iter()
        .map(|(session, outcome)| (decode(session), outcome))
        .collect();
    if map.len() == count {
        Ok(map)
    } else {
        Err(D::Error::custom("duplicate relay delivery destination"))
    }
}
