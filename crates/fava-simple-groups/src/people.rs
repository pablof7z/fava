//! Kind-39001 through kind-39004 group-membership state: admins, members,
//! roles, and `LiveKit` participants, each with its own decoder.

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

/// Generate the `id()`/`author()` pair shared by every kind-3900x state struct in this module.
///
/// All four types here decode to the same `(id, author, ..)` shape from
/// [`state_event`](crate::records::state_event); this spares each of them a
/// hand-written, identical pair of accessors.
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
    /// # Examples
    ///
    /// ```
    /// # use fava_simple_groups::SimpleGroupAdmins;
    /// # use fava_write::{EventValue, Kind, Tag, Timestamp};
    /// # use nostr::event::{EventBuilder as NostrEventBuilder, FinalizeEvent};
    /// # use nostr::key::Keys;
    /// let keys = Keys::generate();
    /// let admin_key = Keys::generate().public_key().to_hex();
    /// let event = NostrEventBuilder::new(Kind::from_u16(39_001), "")
    ///     .tags([
    ///         Tag::parse(["d", "photos"])?,
    ///         Tag::parse(["p", &admin_key, "add-user", "remove-user"])?,
    ///     ])
    ///     .custom_created_at(Timestamp::from(1))
    ///     .finalize(&keys)?;
    ///
    /// let admins = SimpleGroupAdmins::from_event(&EventValue::Signed(event))?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
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
    /// # Examples
    ///
    /// ```
    /// # use fava_simple_groups::SimpleGroupMembers;
    /// # use fava_write::{EventValue, Kind, Tag, Timestamp};
    /// # use nostr::event::{EventBuilder as NostrEventBuilder, FinalizeEvent};
    /// # use nostr::key::Keys;
    /// let keys = Keys::generate();
    /// let member_key = Keys::generate().public_key().to_hex();
    /// let event = NostrEventBuilder::new(Kind::from_u16(39_002), "")
    ///     .tags([
    ///         Tag::parse(["d", "photos"])?,
    ///         Tag::parse(["p", &member_key])?,
    ///     ])
    ///     .custom_created_at(Timestamp::from(1))
    ///     .finalize(&keys)?;
    ///
    /// let members = SimpleGroupMembers::from_event(&EventValue::Signed(event))?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
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
    /// # Examples
    ///
    /// ```
    /// # use fava_simple_groups::SimpleGroupRoles;
    /// # use fava_write::{EventValue, Kind, Tag, Timestamp};
    /// # use nostr::event::{EventBuilder as NostrEventBuilder, FinalizeEvent};
    /// # use nostr::key::Keys;
    /// let keys = Keys::generate();
    /// let event = NostrEventBuilder::new(Kind::from_u16(39_003), "")
    ///     .tags([
    ///         Tag::parse(["d", "photos"])?,
    ///         Tag::parse(["role", "admin", "Group administrator"])?,
    ///     ])
    ///     .custom_created_at(Timestamp::from(1))
    ///     .finalize(&keys)?;
    ///
    /// let roles = SimpleGroupRoles::from_event(&EventValue::Signed(event))?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
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
    /// # Examples
    ///
    /// ```
    /// # use fava_simple_groups::SimpleGroupLivekitParticipants;
    /// # use fava_write::{EventValue, Kind, Tag, Timestamp};
    /// # use nostr::event::{EventBuilder as NostrEventBuilder, FinalizeEvent};
    /// # use nostr::key::Keys;
    /// let keys = Keys::generate();
    /// let participant = Keys::generate().public_key();
    /// let event = NostrEventBuilder::new(Kind::from_u16(39_004), "")
    ///     .tags([
    ///         Tag::parse(["d", "photos"])?,
    ///         Tag::parse(["participant", &participant.to_hex()])?,
    ///     ])
    ///     .custom_created_at(Timestamp::from(1))
    ///     .finalize(&keys)?;
    ///
    /// let participants = SimpleGroupLivekitParticipants::from_event(&EventValue::Signed(event))?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
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

/// Parses a `p` tag into (pubkey, roles), requiring at least one role.
fn parse_admin(
    values: &[String],
    tag_index: usize,
) -> Result<(String, Vec<String>), SimpleGroupDecodeError> {
    let key = required_value(values, tag_index, 1)?.to_owned();
    required_value(values, tag_index, 2)?;
    Ok((key, values[2..].to_vec()))
}

/// Parses a `participant` tag's key, rejecting anything but exact lowercase hex.
fn parse_livekit_key(
    values: &[String],
    tag_index: usize,
) -> Result<PublicKey, SimpleGroupDecodeError> {
    let raw = required_value(values, tag_index, 1)?;
    if raw != &raw.to_lowercase() {
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
