use std::borrow::Borrow;

use fava_query::RelayUrl;
use fava_query::{Kind, PublicKey, Query, QuerySnapshot, SingleLetterTag};

use crate::bounds::{MAX_DISCOVERY_INPUT_ITEMS, MAX_SIMPLE_GROUP_QUERY_RESULTS, collect_at_most};
use crate::{SimpleGroup, SimpleGroupError};

const RECORD_KINDS: [u16; 6] = [39_000, 39_001, 39_002, 39_003, 39_004, 39_005];

/// Pure namespace for NIP-29 discovery and saved-list operations.
pub struct SimpleGroups;

/// Relay-authored NIP-29 record selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimpleGroupRecords {
    /// All six relay-authored record kinds.
    All,
    /// Kind 39000 simple group metadata.
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

impl SimpleGroupRecords {
    /// Select all six simple group record kinds.
    #[must_use]
    pub const fn all() -> Self {
        Self::All
    }

    /// Select simple group metadata.
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
    /// Query kind-10009 saved-simple-group rows by exact saving authors.
    ///
    /// # Errors
    ///
    /// Returns [`SimpleGroupError`] after consuming at most the declared bound plus one.
    pub fn saved_simple_groups<I>(authors: I) -> Result<Query, SimpleGroupError>
    where
        I: IntoIterator,
        I::Item: Borrow<PublicKey>,
    {
        bounded_query(
            Query::events()
                .kind(Kind::from_u16(10_009))
                .authors(bounded_keys(authors)?),
        )
    }

    /// Query kind-10009 saved-relay rows by exact saving authors.
    ///
    /// # Errors
    ///
    /// Returns [`SimpleGroupError`] after consuming at most the declared bound plus one.
    pub fn saved_relays<I>(authors: I) -> Result<Query, SimpleGroupError>
    where
        I: IntoIterator,
        I::Item: Borrow<PublicKey>,
    {
        bounded_query(
            Query::events()
                .kind(Kind::from_u16(10_009))
                .authors(bounded_keys(authors)?),
        )
    }

    /// Query kind-39001 records containing exact lowercase-p subjects.
    ///
    /// # Errors
    ///
    /// Returns [`SimpleGroupError`] after consuming at most the declared bound plus one.
    pub fn simple_groups_where_admin<I>(subjects: I) -> Result<Query, SimpleGroupError>
    where
        I: IntoIterator,
        I::Item: Borrow<PublicKey>,
    {
        bounded_query(discovery_by_subject(39_001, bounded_keys(subjects)?))
    }

    /// Query kind-39002 records containing exact lowercase-p subjects.
    ///
    /// # Errors
    ///
    /// Returns [`SimpleGroupError`] after consuming at most the declared bound plus one.
    pub fn simple_groups_where_member<I>(subjects: I) -> Result<Query, SimpleGroupError>
    where
        I: IntoIterator,
        I::Item: Borrow<PublicKey>,
    {
        bounded_query(discovery_by_subject(39_002, bounded_keys(subjects)?))
    }

    /// Project exact saving authors for one simple group's selected id-host pairs.
    ///
    /// # Errors
    ///
    /// Returns [`SimpleGroupError`] when the supplied snapshot exceeds the projection bound.
    pub fn simple_groups_saved_by(
        snapshot: &QuerySnapshot,
        simple_group: &SimpleGroup,
    ) -> Result<Vec<PublicKey>, SimpleGroupError> {
        let records = collect_at_most(snapshot.events.iter(), MAX_DISCOVERY_INPUT_ITEMS).map_err(
            |actual| SimpleGroupError::TooManyDiscoveryItems {
                actual,
                maximum: MAX_DISCOVERY_INPUT_ITEMS,
            },
        )?;
        let hosts = simple_group
            .hosts()
            .collect::<std::collections::BTreeSet<_>>();
        let mut authors = std::collections::BTreeSet::new();
        for record in records {
            let Ok(rows) = crate::SavedSimpleGroup::from_event(record.event()) else {
                continue;
            };
            if let Some(author) = rows.into_iter().flatten().find_map(|saved| {
                (saved.id() == simple_group.id() && hosts.contains(saved.relay()))
                    .then(|| saved.author())
            }) {
                authors.insert(author);
            }
        }
        Ok(authors.into_iter().collect())
    }
}

fn bounded_keys<I>(input: I) -> Result<Vec<PublicKey>, SimpleGroupError>
where
    I: IntoIterator,
    I::Item: Borrow<PublicKey>,
{
    collect_at_most(input, MAX_DISCOVERY_INPUT_ITEMS)
        .map(|values| values.into_iter().map(|value| *value.borrow()).collect())
        .map_err(|actual| SimpleGroupError::TooManyDiscoveryItems {
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

fn bounded_query(query: Query) -> Result<Query, SimpleGroupError> {
    query
        .limit(MAX_SIMPLE_GROUP_QUERY_RESULTS)
        .map_err(Into::into)
}

pub(crate) fn content(
    simple_group: &SimpleGroup,
    selection: Query,
) -> Result<Query, SimpleGroupError> {
    let limit = selection.result_limit().ok_or_else(|| {
        SimpleGroupError::Query("simple group content requires an explicit result bound".to_owned())
    })?;
    if limit.get() > MAX_SIMPLE_GROUP_QUERY_RESULTS {
        return Err(SimpleGroupError::Query(format!(
            "simple group content result bound exceeds limit: {} > {MAX_SIMPLE_GROUP_QUERY_RESULTS}",
            limit.get()
        )));
    }
    let h = SingleLetterTag::from_char('h').expect("lowercase h is a valid tag key");
    let selection = match selection.selection().tag_values.get(&h) {
        None => selection.tag_values(h, [simple_group.id()]),
        Some(values) if values.is_empty() => {
            return Err(SimpleGroupError::EmptySimpleGroupContext);
        }
        Some(_) => return Err(SimpleGroupError::ConflictingSimpleGroupContext),
    };
    Ok(selection.from_relays(simple_group.hosts())?)
}

pub(crate) fn records(
    simple_group: &SimpleGroup,
    records: SimpleGroupRecords,
) -> Result<Vec<(RelayUrl, Query)>, SimpleGroupError> {
    let d = SingleLetterTag::from_char('d').expect("lowercase d is a valid tag key");
    let query = records.kinds().iter().fold(Query::events(), |query, kind| {
        query.kind(Kind::from_u16(*kind))
    });
    simple_group
        .hosts()
        .map(|host| {
            let exact = bounded_query(
                query
                    .clone()
                    .tag_values(d, [simple_group.id()])
                    .only_from_relays([host.clone()])?,
            )?;
            Ok((host, exact))
        })
        .collect()
}
