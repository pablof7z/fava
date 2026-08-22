use fava_query::{Kind, Query, SingleLetterTag};

use crate::bounds::MAX_GROUP_CONTENT_RESULTS;
use crate::{Group, GroupError};

const RECORD_KINDS: [u16; 6] = [39_000, 39_001, 39_002, 39_003, 39_004, 39_005];

/// Relay-authored NIP-29 record selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GroupRecords {
    /// All six relay-authored record kinds.
    All,
    /// Kind 39000 group metadata.
    Metadata,
    /// Kind 39001 administrator list.
    Admins,
    /// Kind 39002 member list.
    Members,
    /// Kind 39003 role definitions.
    Roles,
    /// Kind 39004 participant list.
    Participants,
    /// Kind 39005 pinned items.
    Pins,
}

impl GroupRecords {
    /// Select all six group record kinds.
    #[must_use]
    pub const fn all() -> Self {
        Self::All
    }

    /// Select group metadata.
    #[must_use]
    pub const fn metadata() -> Self {
        Self::Metadata
    }

    /// Select administrators.
    #[must_use]
    pub const fn admins() -> Self {
        Self::Admins
    }

    /// Select members.
    #[must_use]
    pub const fn members() -> Self {
        Self::Members
    }

    /// Select role definitions.
    #[must_use]
    pub const fn roles() -> Self {
        Self::Roles
    }

    /// Select participants.
    #[must_use]
    pub const fn participants() -> Self {
        Self::Participants
    }

    /// Select pinned items.
    #[must_use]
    pub const fn pins() -> Self {
        Self::Pins
    }

    fn kinds(self) -> &'static [u16] {
        match self {
            Self::All => &RECORD_KINDS,
            Self::Metadata => &RECORD_KINDS[0..1],
            Self::Admins => &RECORD_KINDS[1..2],
            Self::Members => &RECORD_KINDS[2..3],
            Self::Roles => &RECORD_KINDS[3..4],
            Self::Participants => &RECORD_KINDS[4..5],
            Self::Pins => &RECORD_KINDS[5..6],
        }
    }
}

pub(crate) fn content(group: &Group, selection: Query) -> Result<Query, GroupError> {
    let limit = selection.result_limit().ok_or_else(|| {
        GroupError::Query("group content requires an explicit result bound".to_owned())
    })?;
    if limit.get() > MAX_GROUP_CONTENT_RESULTS {
        return Err(GroupError::Query(format!(
            "group content result bound exceeds limit: {} > {MAX_GROUP_CONTENT_RESULTS}",
            limit.get()
        )));
    }
    let h = SingleLetterTag::from_char('h').expect("lowercase h is a valid tag key");
    Ok(selection
        .tag_values(h, [group.id()])
        .from_relays(group.hosts())?)
}

pub(crate) fn records(group: &Group, records: GroupRecords) -> Result<Query, GroupError> {
    let d = SingleLetterTag::from_char('d').expect("lowercase d is a valid tag key");
    let query = records.kinds().iter().fold(Query::events(), |query, kind| {
        query.kind(Kind::from_u16(*kind))
    });
    Ok(query
        .tag_values(d, [group.id()])
        .only_from_relays(group.hosts())?)
}
