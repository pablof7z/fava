use std::collections::BTreeSet;

use fava_state::RelayUrl;
use fava_write::{EventValue, PublicKey};

use crate::GroupError;
use crate::records::saved_boundary;

/// One public saved-group row from a signed kind-10009 list.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SavedGroup {
    id: String,
    relay: RelayUrl,
    name: Option<String>,
    author: PublicKey,
}

/// One public relay-in-use row from a signed kind-10009 list.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SavedRelay {
    relay: RelayUrl,
    author: PublicKey,
}

impl SavedGroup {
    /// Parse bounded public `group` rows while conserving row-local failures.
    ///
    /// # Errors
    ///
    /// Returns [`GroupError`] when the signed kind-10009 boundary is invalid.
    pub fn from_event(event: &EventValue) -> Result<Vec<Result<Self, GroupError>>, GroupError> {
        let event = saved_boundary(event)?;
        let mut seen = BTreeSet::new();
        Ok(event
            .tags
            .iter()
            .enumerate()
            .filter(|(_, tag)| tag.as_slice().first().map(String::as_str) == Some("group"))
            .map(
                |(tag_index, tag)| match parse_group(tag_index, tag.as_slice(), event.pubkey) {
                    Ok(group) if seen.insert((group.id.clone(), group.relay.clone())) => Ok(group),
                    Ok(_) => Err(GroupError::DuplicateRecordRow { tag_index }),
                    Err(error) => Err(error),
                },
            )
            .collect())
    }

    /// Exact opaque group id.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Parsed inert host relay.
    #[must_use]
    pub const fn relay(&self) -> &RelayUrl {
        &self.relay
    }

    /// Optional exact display name; present-empty remains `Some("")`.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Author who saved this row.
    #[must_use]
    pub const fn author(&self) -> PublicKey {
        self.author
    }
}

impl SavedRelay {
    /// Parse bounded public `r` rows while conserving row-local failures.
    ///
    /// # Errors
    ///
    /// Returns [`GroupError`] when the signed kind-10009 boundary is invalid.
    pub fn from_event(event: &EventValue) -> Result<Vec<Result<Self, GroupError>>, GroupError> {
        let event = saved_boundary(event)?;
        let mut seen = BTreeSet::new();
        Ok(event
            .tags
            .iter()
            .enumerate()
            .filter(|(_, tag)| tag.as_slice().first().map(String::as_str) == Some("r"))
            .map(
                |(tag_index, tag)| match parse_relay(tag_index, tag.as_slice(), event.pubkey) {
                    Ok(relay) if seen.insert(relay.relay.clone()) => Ok(relay),
                    Ok(_) => Err(GroupError::DuplicateRecordRow { tag_index }),
                    Err(error) => Err(error),
                },
            )
            .collect())
    }

    /// Parsed inert relay URL.
    #[must_use]
    pub const fn relay(&self) -> &RelayUrl {
        &self.relay
    }

    /// Author who saved this row.
    #[must_use]
    pub const fn author(&self) -> PublicKey {
        self.author
    }
}

fn parse_group(
    tag_index: usize,
    values: &[String],
    author: PublicKey,
) -> Result<SavedGroup, GroupError> {
    if values.len() < 3 {
        return Err(GroupError::MalformedRecordRow {
            tag_index,
            reason: "saved group row is missing id or relay",
        });
    }
    if values.len() > 4 {
        return Err(GroupError::MalformedRecordRow {
            tag_index,
            reason: "saved group row has extra columns",
        });
    }
    if values[1].is_empty() {
        return Err(GroupError::MalformedRecordRow {
            tag_index,
            reason: "saved group id is empty",
        });
    }
    if values[2].is_empty() {
        return Err(GroupError::MalformedRecordRow {
            tag_index,
            reason: "saved group relay is empty",
        });
    }
    let relay = RelayUrl::parse(&values[2]).map_err(|_| GroupError::MalformedRecordRow {
        tag_index,
        reason: "saved group relay is invalid",
    })?;
    Ok(SavedGroup {
        id: values[1].clone(),
        relay,
        name: values.get(3).cloned(),
        author,
    })
}

fn parse_relay(
    tag_index: usize,
    values: &[String],
    author: PublicKey,
) -> Result<SavedRelay, GroupError> {
    if values.len() < 2 {
        return Err(GroupError::MalformedRecordRow {
            tag_index,
            reason: "saved relay row is missing its URL",
        });
    }
    if values.len() > 2 {
        return Err(GroupError::MalformedRecordRow {
            tag_index,
            reason: "saved relay row has extra columns",
        });
    }
    if values[1].is_empty() {
        return Err(GroupError::MalformedRecordRow {
            tag_index,
            reason: "saved relay URL is empty",
        });
    }
    let relay = RelayUrl::parse(&values[1]).map_err(|_| GroupError::MalformedRecordRow {
        tag_index,
        reason: "saved relay URL is invalid",
    })?;
    Ok(SavedRelay { relay, author })
}
