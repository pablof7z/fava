use std::collections::BTreeMap;

use fava_relay::{RelayAccess, RelaySessionKey};
use nostr::key::PublicKey;
use nostr::types::RelayUrl;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{CoverageState, PlannedRelay, RouteTarget};

fn encode_session(value: &RelaySessionKey) -> (RelayUrl, Option<PublicKey>) {
    (
        value.relay.clone(),
        match &value.access {
            RelayAccess::Public => None,
            RelayAccess::Authenticated(public_key) => Some(*public_key),
        },
    )
}

fn decode_session(value: (RelayUrl, Option<PublicKey>)) -> RelaySessionKey {
    RelaySessionKey {
        relay: value.0,
        access: value
            .1
            .map_or(RelayAccess::Public, RelayAccess::Authenticated),
    }
}

pub(super) mod session {
    use super::{
        Deserialize, Deserializer, PublicKey, RelaySessionKey, RelayUrl, Serialize, Serializer,
        decode_session, encode_session,
    };

    pub(crate) fn serialize<S>(value: &RelaySessionKey, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        encode_session(value).serialize(serializer)
    }

    pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<RelaySessionKey, D::Error>
    where
        D: Deserializer<'de>,
    {
        <(RelayUrl, Option<PublicKey>)>::deserialize(deserializer).map(decode_session)
    }
}

pub(super) mod sessions {
    use std::collections::BTreeSet;

    use serde::de::Error as _;

    use super::{
        Deserialize, Deserializer, PublicKey, RelaySessionKey, RelayUrl, Serialize, Serializer,
        decode_session, encode_session,
    };

    pub(crate) fn serialize<S>(
        value: &BTreeSet<RelaySessionKey>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value
            .iter()
            .map(encode_session)
            .collect::<Vec<_>>()
            .serialize(serializer)
    }

    pub(crate) fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<BTreeSet<RelaySessionKey>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let values = Vec::<(RelayUrl, Option<PublicKey>)>::deserialize(deserializer)?;
        let count = values.len();
        let sessions = values
            .into_iter()
            .map(decode_session)
            .collect::<BTreeSet<_>>();
        if sessions.len() == count {
            Ok(sessions)
        } else {
            Err(D::Error::custom("duplicate covered relay session"))
        }
    }
}

pub(super) mod destinations {
    use serde::de::Error as _;

    use super::{
        BTreeMap, Deserialize, Deserializer, PlannedRelay, PublicKey, RelaySessionKey, RelayUrl,
        Serialize, Serializer, decode_session, encode_session,
    };

    pub(crate) fn serialize<S>(
        value: &BTreeMap<RelaySessionKey, PlannedRelay>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value
            .iter()
            .map(|(session, relay)| (encode_session(session), relay))
            .collect::<Vec<_>>()
            .serialize(serializer)
    }

    pub(crate) fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<BTreeMap<RelaySessionKey, PlannedRelay>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let pairs =
            Vec::<((RelayUrl, Option<PublicKey>), PlannedRelay)>::deserialize(deserializer)?;
        let count = pairs.len();
        let map: BTreeMap<_, _> = pairs
            .into_iter()
            .map(|(session, relay)| (decode_session(session), relay))
            .collect();
        if map.len() == count {
            Ok(map)
        } else {
            Err(D::Error::custom("duplicate route-plan destination"))
        }
    }
}

pub(super) mod coverage {
    use serde::de::Error as _;

    use super::{
        BTreeMap, CoverageState, Deserialize, Deserializer, RouteTarget, Serialize, Serializer,
    };

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
