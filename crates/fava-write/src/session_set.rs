use std::collections::BTreeSet;

use fava_state::RelaySessionKey;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub(super) fn serialize<S>(
    value: &BTreeSet<RelaySessionKey>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    value.iter().collect::<Vec<_>>().serialize(serializer)
}

pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<BTreeSet<RelaySessionKey>, D::Error>
where
    D: Deserializer<'de>,
{
    let values = Vec::<RelaySessionKey>::deserialize(deserializer)?;
    let count = values.len();
    let set: BTreeSet<_> = values.into_iter().collect();
    if set.len() == count {
        Ok(set)
    } else {
        Err(D::Error::custom("duplicate desired relay destination"))
    }
}
