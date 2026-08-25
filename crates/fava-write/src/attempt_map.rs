use std::collections::BTreeMap;

use fava_relay::RelaySessionKey;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::relay_session_serde::{decode, encode};

pub(super) fn serialize<S>(
    value: &BTreeMap<RelaySessionKey, u32>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    value
        .iter()
        .map(|(session, attempts)| (encode(session), attempts))
        .collect::<Vec<_>>()
        .serialize(serializer)
}

pub(super) fn deserialize<'de, D>(
    deserializer: D,
) -> Result<BTreeMap<RelaySessionKey, u32>, D::Error>
where
    D: Deserializer<'de>,
{
    let pairs = Vec::<((nostr::types::RelayUrl, Option<nostr::key::PublicKey>), u32)>::deserialize(
        deserializer,
    )?;
    let count = pairs.len();
    let map: BTreeMap<_, _> = pairs
        .into_iter()
        .map(|(session, attempts)| (decode(session), attempts))
        .collect();
    if map.len() == count {
        Ok(map)
    } else {
        Err(D::Error::custom("duplicate relay delivery attempt"))
    }
}
