use std::borrow::Borrow;

use fava_query::{Kind, PublicKey, Query, QuerySnapshot, SingleLetterTag};

use crate::bounds::{MAX_DISCOVERY_INPUT_ITEMS, MAX_GROUP_CONTENT_RESULTS, collect_at_most};
use crate::{Group, GroupError};

const RECORD_KINDS: [u16; 6] = [39_000, 39_001, 39_002, 39_003, 39_004, 39_005];

/// Pure namespace for NIP-29 discovery and saved-list operations.
pub struct SimpleGroups;

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

impl SimpleGroups {
    /// Query kind-10009 saved-group rows by exact saving authors.
    pub fn saved_groups<I>(authors: I) -> Result<Query, GroupError>
    where
        I: IntoIterator,
        I::Item: Borrow<PublicKey>,
    {
        Ok(Query::events()
            .kind(Kind::from_u16(10_009))
            .authors(bounded_keys(authors)?))
    }

    /// Query kind-10009 saved-relay rows by exact saving authors.
    pub fn saved_relays<I>(authors: I) -> Result<Query, GroupError>
    where
        I: IntoIterator,
        I::Item: Borrow<PublicKey>,
    {
        Ok(Query::events()
            .kind(Kind::from_u16(10_009))
            .authors(bounded_keys(authors)?))
    }

    /// Query kind-39001 records containing exact lowercase-p subjects.
    pub fn groups_where_admin<I>(subjects: I) -> Result<Query, GroupError>
    where
        I: IntoIterator,
        I::Item: Borrow<PublicKey>,
    {
        Ok(discovery_by_subject(39_001, bounded_keys(subjects)?))
    }

    /// Query kind-39002 records containing exact lowercase-p subjects.
    pub fn groups_where_member<I>(subjects: I) -> Result<Query, GroupError>
    where
        I: IntoIterator,
        I::Item: Borrow<PublicKey>,
    {
        Ok(discovery_by_subject(39_002, bounded_keys(subjects)?))
    }

    /// Project exact saving authors for one group's selected id-host pairs.
    pub fn groups_saved_by(
        snapshot: &QuerySnapshot,
        group: &Group,
    ) -> Result<Vec<PublicKey>, GroupError> {
        let records = collect_at_most(snapshot.events.iter(), MAX_DISCOVERY_INPUT_ITEMS).map_err(
            |actual| GroupError::TooManyDiscoveryItems {
                actual,
                maximum: MAX_DISCOVERY_INPUT_ITEMS,
            },
        )?;
        let hosts = group.hosts().collect::<std::collections::BTreeSet<_>>();
        let mut authors = std::collections::BTreeSet::new();
        for record in records {
            let Ok(rows) = crate::SavedGroup::from_event(&record.event) else {
                continue;
            };
            if let Some(author) = rows.into_iter().flatten().find_map(|saved| {
                (saved.id() == group.id() && hosts.contains(saved.relay())).then(|| saved.author())
            }) {
                authors.insert(author);
            }
        }
        Ok(authors.into_iter().collect())
    }
}

fn bounded_keys<I>(input: I) -> Result<Vec<PublicKey>, GroupError>
where
    I: IntoIterator,
    I::Item: Borrow<PublicKey>,
{
    collect_at_most(input, MAX_DISCOVERY_INPUT_ITEMS)
        .map(|values| values.into_iter().map(|value| *value.borrow()).collect())
        .map_err(|actual| GroupError::TooManyDiscoveryItems {
            actual,
            maximum: MAX_DISCOVERY_INPUT_ITEMS,
        })
}

fn discovery_by_subject(kind: u16, subjects: Vec<PublicKey>) -> Query {
    Query::events().kind(Kind::from_u16(kind)).tag_values(
        SingleLetterTag::LOWERCASE_P,
        subjects.into_iter().map(|subject| subject.to_hex()),
    )
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
