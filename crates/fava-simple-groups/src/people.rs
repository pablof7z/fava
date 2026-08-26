use fava_write::{EventValue, PublicKey};

use crate::records::{SimpleGroupDecodeError, required_value, state_event};

/// Semantic kind-39001 administrator entries from one event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SimpleGroupAdmins {
    id: String,
    author: PublicKey,
    admins: Vec<Result<(String, Vec<String>), SimpleGroupDecodeError>>,
}

/// Semantic kind-39002 member entries from one event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SimpleGroupMembers {
    id: String,
    author: PublicKey,
    members: Vec<Result<String, SimpleGroupDecodeError>>,
}

/// Semantic kind-39003 role entries from one event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SimpleGroupRoles {
    id: String,
    author: PublicKey,
    roles: Vec<Result<(String, Option<String>), SimpleGroupDecodeError>>,
}

/// Semantic kind-39004 `LiveKit` participant entries from one event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SimpleGroupLivekitParticipants {
    id: String,
    author: PublicKey,
    participants: Vec<Result<PublicKey, SimpleGroupDecodeError>>,
}

macro_rules! common_accessors {
    ($type:ty) => {
        impl $type {
            /// Borrow the first `d` tag's first value.
            #[must_use]
            pub fn id(&self) -> &str {
                &self.id
            }

            /// Return the event author.
            #[must_use]
            pub const fn author(&self) -> PublicKey {
                self.author
            }
        }
    };
}

common_accessors!(SimpleGroupAdmins);
common_accessors!(SimpleGroupMembers);
common_accessors!(SimpleGroupRoles);
common_accessors!(SimpleGroupLivekitParticipants);

impl SimpleGroupAdmins {
    /// Decode one kind-39001 event, retaining `p`-tag-local failures.
    ///
    /// # Errors
    ///
    /// Returns [`SimpleGroupDecodeError`] for the wrong kind or a missing first `d` value.
    pub fn from_event(event: &EventValue) -> Result<Self, SimpleGroupDecodeError> {
        let (id, author, tags) = state_event(event, 39_001)?;
        let admins = tags
            .iter()
            .enumerate()
            .filter(|(_, tag)| tag.as_slice().first().map(String::as_str) == Some("p"))
            .map(|(tag_index, tag)| parse_admin(tag.as_slice(), tag_index))
            .collect();
        Ok(Self {
            id: id.to_owned(),
            author,
            admins,
        })
    }

    /// Return every `p` tag as key plus roles or a local failure.
    pub fn admins(&self) -> &[Result<(String, Vec<String>), SimpleGroupDecodeError>] {
        &self.admins
    }
}

impl SimpleGroupMembers {
    /// Decode one kind-39002 event, retaining `p`-tag-local failures.
    ///
    /// # Errors
    ///
    /// Returns [`SimpleGroupDecodeError`] for the wrong kind or a missing first `d` value.
    pub fn from_event(event: &EventValue) -> Result<Self, SimpleGroupDecodeError> {
        let (id, author, tags) = state_event(event, 39_002)?;
        let members = tags
            .iter()
            .enumerate()
            .filter(|(_, tag)| tag.as_slice().first().map(String::as_str) == Some("p"))
            .map(|(tag_index, tag)| required_value(tag.as_slice(), tag_index, 1).map(str::to_owned))
            .collect();
        Ok(Self {
            id: id.to_owned(),
            author,
            members,
        })
    }

    /// Return every `p` tag as its exact first value or a local failure.
    pub fn members(&self) -> &[Result<String, SimpleGroupDecodeError>] {
        &self.members
    }
}

impl SimpleGroupRoles {
    /// Decode one kind-39003 event, retaining `role`-tag-local failures.
    ///
    /// # Errors
    ///
    /// Returns [`SimpleGroupDecodeError`] for the wrong kind or a missing first `d` value.
    pub fn from_event(event: &EventValue) -> Result<Self, SimpleGroupDecodeError> {
        let (id, author, tags) = state_event(event, 39_003)?;
        let roles = tags
            .iter()
            .enumerate()
            .filter(|(_, tag)| tag.as_slice().first().map(String::as_str) == Some("role"))
            .map(|(tag_index, tag)| {
                let values = tag.as_slice();
                required_value(values, tag_index, 1)
                    .map(|name| (name.to_owned(), values.get(2).cloned()))
            })
            .collect();
        Ok(Self {
            id: id.to_owned(),
            author,
            roles,
        })
    }

    /// Return every `role` tag as name plus optional description or a local failure.
    pub fn roles(&self) -> &[Result<(String, Option<String>), SimpleGroupDecodeError>] {
        &self.roles
    }
}

impl SimpleGroupLivekitParticipants {
    /// Decode one kind-39004 event, retaining `participant`-tag-local failures.
    ///
    /// # Errors
    ///
    /// Returns [`SimpleGroupDecodeError`] for the wrong kind or a missing first `d` value.
    pub fn from_event(event: &EventValue) -> Result<Self, SimpleGroupDecodeError> {
        let (id, author, tags) = state_event(event, 39_004)?;
        let participants = tags
            .iter()
            .enumerate()
            .filter(|(_, tag)| tag.as_slice().first().map(String::as_str) == Some("participant"))
            .map(|(tag_index, tag)| parse_livekit_key(tag.as_slice(), tag_index))
            .collect();
        Ok(Self {
            id: id.to_owned(),
            author,
            participants,
        })
    }

    /// Return every `participant` tag as its parsed first value or a local failure.
    pub fn participants(&self) -> &[Result<PublicKey, SimpleGroupDecodeError>] {
        &self.participants
    }
}

fn parse_admin(
    values: &[String],
    tag_index: usize,
) -> Result<(String, Vec<String>), SimpleGroupDecodeError> {
    let key = required_value(values, tag_index, 1)?.to_owned();
    required_value(values, tag_index, 2)?;
    Ok((key, values[2..].to_vec()))
}

fn parse_livekit_key(
    values: &[String],
    tag_index: usize,
) -> Result<PublicKey, SimpleGroupDecodeError> {
    let raw = required_value(values, tag_index, 1)?;
    if raw.len() != 64
        || !raw
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(SimpleGroupDecodeError::InvalidLivekitParticipantPublicKey {
            tag_index,
            value_index: 1,
        });
    }
    PublicKey::from_hex(raw).map_err(|_| {
        SimpleGroupDecodeError::InvalidLivekitParticipantPublicKey {
            tag_index,
            value_index: 1,
        }
    })
}
