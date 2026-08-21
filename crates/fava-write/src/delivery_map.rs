use std::collections::BTreeMap;

use fava_state::RelaySessionKey;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::RelayDeliveryOutcome;

pub(super) fn serialize<S>(
    value: &BTreeMap<RelaySessionKey, RelayDeliveryOutcome>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    value.iter().collect::<Vec<_>>().serialize(serializer)
}

pub(super) fn deserialize<'de, D>(
    deserializer: D,
) -> Result<BTreeMap<RelaySessionKey, RelayDeliveryOutcome>, D::Error>
where
    D: Deserializer<'de>,
{
    let pairs = Vec::<(RelaySessionKey, RelayDeliveryOutcome)>::deserialize(deserializer)?;
    let count = pairs.len();
    let map: BTreeMap<_, _> = pairs.into_iter().collect();
    if map.len() == count {
        Ok(map)
    } else {
        Err(D::Error::custom("duplicate relay delivery destination"))
    }
}
