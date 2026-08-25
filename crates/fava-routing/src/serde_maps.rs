use std::collections::BTreeMap;

use fava_state::RelaySessionKey;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{CoverageState, PlannedRelay, RouteTarget};

pub(super) mod destinations {
    use super::*;

    pub(crate) fn serialize<S>(
        value: &BTreeMap<RelaySessionKey, PlannedRelay>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value.iter().collect::<Vec<_>>().serialize(serializer)
    }

    pub(crate) fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<BTreeMap<RelaySessionKey, PlannedRelay>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let pairs = Vec::<(RelaySessionKey, PlannedRelay)>::deserialize(deserializer)?;
        let count = pairs.len();
        let map: BTreeMap<_, _> = pairs.into_iter().collect();
        if map.len() == count {
            Ok(map)
        } else {
            Err(D::Error::custom("duplicate route-plan destination"))
        }
    }
}

pub(super) mod coverage {
    use super::*;

    pub(crate) fn serialize<S>(
        value: &BTreeMap<RouteTarget, CoverageState>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value.iter().collect::<Vec<_>>().serialize(serializer)
    }

    pub(crate) fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<BTreeMap<RouteTarget, CoverageState>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let pairs = Vec::<(RouteTarget, CoverageState)>::deserialize(deserializer)?;
        let count = pairs.len();
        let map: BTreeMap<_, _> = pairs.into_iter().collect();
        if map.len() == count {
            Ok(map)
        } else {
            Err(D::Error::custom("duplicate route-plan coverage target"))
        }
    }
}
