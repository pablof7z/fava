use std::collections::BTreeSet;

use fava_state::RelayUrl;
use fava_write::{EventValue, PublicKey};

use crate::SimpleGroupError;
use crate::records::saved_boundary;

/// One public saved-simple-group row from a signed kind-10009 list.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SavedSimpleGroup {
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

impl SavedSimpleGroup {
    /// Parse bounded public `group` rows while conserving row-local failures.
    ///
    /// # Errors
    ///
    /// Returns [`SimpleGroupError`] when the signed kind-10009 boundary is invalid.
    pub fn from_event(
        event: &EventValue,
    ) -> Result<Vec<Result<Self, SimpleGroupError>>, SimpleGroupError> {
        let event = saved_boundary(event)?;
        let mut seen = BTreeSet::new();
        Ok(event
            .tags
            .iter()
            .enumerate()
            .filter(|(_, tag)| tag.as_slice().first().map(String::as_str) == Some("group"))
            .map(|(tag_index, tag)| {
                match parse_simple_group(tag_index, tag.as_slice(), event.pubkey) {
                    Ok(simple_group)
                        if seen.insert((simple_group.id.clone(), simple_group.relay.clone())) =>
                    {
                        Ok(simple_group)
                    }
                    Ok(_) => Err(SimpleGroupError::DuplicateRecordRow { tag_index }),
                    Err(error) => Err(error),
                }
            })
            .collect())
    }

    /// Exact opaque simple group id.
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
    /// Returns [`SimpleGroupError`] when the signed kind-10009 boundary is invalid.
    pub fn from_event(
        event: &EventValue,
    ) -> Result<Vec<Result<Self, SimpleGroupError>>, SimpleGroupError> {
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
                    Ok(_) => Err(SimpleGroupError::DuplicateRecordRow { tag_index }),
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

fn parse_simple_group(
    tag_index: usize,
    values: &[String],
    author: PublicKey,
) -> Result<SavedSimpleGroup, SimpleGroupError> {
    if values.len() < 3 {
        return Err(SimpleGroupError::MalformedRecordRow {
            tag_index,
            reason: "saved simple group row is missing id or relay",
        });
    }
    if values.len() > 4 {
        return Err(SimpleGroupError::MalformedRecordRow {
            tag_index,
            reason: "saved simple group row has extra columns",
        });
    }
    if values[1].is_empty() {
        return Err(SimpleGroupError::MalformedRecordRow {
            tag_index,
            reason: "saved simple group id is empty",
        });
    }
    if values[2].is_empty() {
        return Err(SimpleGroupError::MalformedRecordRow {
            tag_index,
            reason: "saved simple group relay is empty",
        });
    }
    let relay = RelayUrl::parse(&values[2]).map_err(|_| SimpleGroupError::MalformedRecordRow {
        tag_index,
        reason: "saved simple group relay is invalid",
    })?;
    Ok(SavedSimpleGroup {
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
) -> Result<SavedRelay, SimpleGroupError> {
    if values.len() < 2 {
        return Err(SimpleGroupError::MalformedRecordRow {
            tag_index,
            reason: "saved relay row is missing its URL",
        });
    }
    if values.len() > 2 {
        return Err(SimpleGroupError::MalformedRecordRow {
            tag_index,
            reason: "saved relay row has extra columns",
        });
    }
    if values[1].is_empty() {
        return Err(SimpleGroupError::MalformedRecordRow {
            tag_index,
            reason: "saved relay URL is empty",
        });
    }
    let relay = RelayUrl::parse(&values[1]).map_err(|_| SimpleGroupError::MalformedRecordRow {
        tag_index,
        reason: "saved relay URL is invalid",
    })?;
    Ok(SavedRelay { relay, author })
}
