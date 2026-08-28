//! Typed NIP-29 management event constructors.
//!
//! This module owns the group lifecycle write surface. Every NIP-29 management
//! kind number lives here and nowhere else; callers see typed functions that
//! return [`UnsignedEvent`] values, not raw kind integers.
//!
//! # Publish path
//!
//! ```ignore
//! // create_group returns a ready-to-publish UnsignedEvent.
//! let event = create_group(author.public_key(), &group)?;
//! let write = fava.to(group.relays().collect::<Vec<_>>())?.publish(event)?;
//! ```
//!
//! # Encapsulation guarantee
//!
//! Kind numbers 9000–9009 and 9021–9022 are private constants in this module.
//! No caller, test, or example writes `Kind::from_u16(9NNN)`.

use fava_write::{EventBuildError, EventBuilder, EventId, PublicKey, Tag, UnsignedEvent};
use nostr::types::RelayUrl;

use crate::SimpleGroup;

// ── Private kind constants ────────────────────────────────────────────────────
// These are the ONLY occurrences of the magic numbers in the workspace.
// Re-export them as nothing; callers use the typed constructors.

const KIND_PUT_USER: u16 = 9000;
const KIND_REMOVE_USER: u16 = 9001;
/// NIP-29 kind requesting a change to a group's metadata.
const KIND_EDIT_METADATA: u16 = 9002;
const KIND_DELETE_EVENT: u16 = 9005;
const KIND_CREATE_GROUP: u16 = 9007;
const KIND_DELETE_GROUP: u16 = 9008;
const KIND_INVITE: u16 = 9009;
const KIND_JOIN_REQUEST: u16 = 9021;
const KIND_LEAVE_GROUP: u16 = 9022;

// ── MetadataEdit ─────────────────────────────────────────────────────────────

/// Partial metadata update for a NIP-29 group (kind 9002).
///
/// `None` fields are omitted from the event. The relay interprets absent
/// visibility or access tags as public and open respectively.
///
/// # Examples
///
/// ```
/// # use fava_simple_groups::{MetadataEdit, GroupVisibility, GroupAccess};
/// let edit = MetadataEdit {
///     name: Some("Cats".to_owned()),
///     about: Some("A group about cats".to_owned()),
///     picture: None,
///     visibility: Some(GroupVisibility::Private),
///     access: Some(GroupAccess::Closed),
/// };
/// ```
#[derive(Clone, Debug, Default)]
pub struct MetadataEdit {
    /// Human-readable group name.
    pub name: Option<String>,
    /// Group description.
    pub about: Option<String>,
    /// URL of the group picture.
    pub picture: Option<String>,
    /// Whether the group is private (relay withholds content from non-members).
    /// `None` means public (no `["private"]` tag emitted).
    pub visibility: Option<GroupVisibility>,
    /// Whether joining requires approval.
    /// `None` means open (no `["closed"]` tag emitted).
    pub access: Option<GroupAccess>,
}

/// Group visibility: whether the relay exposes content to non-members.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GroupVisibility {
    /// Content is visible to all; no `["private"]` tag emitted.
    Public,
    /// Content is withheld from non-members; emits `["private"]` tag.
    Private,
}

/// Group access: whether joining requires an admin invitation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GroupAccess {
    /// Anyone may join; no `["closed"]` tag emitted.
    Open,
    /// Joining requires admin approval; emits `["closed"]` tag.
    Closed,
}

// ── Typed constructors ────────────────────────────────────────────────────────

/// Build a kind-9007 create-group event for `group`.
///
/// The event carries a single `h` tag with the group id. The relay creates
/// the group when it accepts this event from an authorized pubkey.
///
/// # Examples
///
/// ```
/// # use fava_simple_groups::{SimpleGroup, create_group};
/// # use nostr::key::Keys;
/// # use nostr::types::RelayUrl;
/// let relay = RelayUrl::parse("wss://relay.example")?;
/// let group = SimpleGroup::new("cats", vec![relay])?;
/// let keys = Keys::generate();
///
/// let event = create_group(keys.public_key(), &group)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
///
/// Returns [`EventBuildError`] when the event exceeds declared bounds.
pub fn create_group(
    author: PublicKey,
    group: &SimpleGroup,
) -> Result<UnsignedEvent, EventBuildError> {
    build(author, KIND_CREATE_GROUP, group, [])
}

/// Build a kind-9002 edit-metadata event for `group`.
///
/// Tags for `None` fields in `edit` are omitted; the relay resets absent
/// fields to their default values.
///
/// # Examples
///
/// ```
/// # use fava_simple_groups::{GroupVisibility, MetadataEdit, SimpleGroup, edit_metadata};
/// # use nostr::key::Keys;
/// # use nostr::types::RelayUrl;
/// let relay = RelayUrl::parse("wss://relay.example")?;
/// let group = SimpleGroup::new("cats", vec![relay])?;
/// let keys = Keys::generate();
///
/// let event = edit_metadata(
///     keys.public_key(),
///     &group,
///     &MetadataEdit {
///         name: Some("Cats".to_owned()),
///         visibility: Some(GroupVisibility::Private),
///         ..Default::default()
///     },
/// )?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
///
/// Returns [`EventBuildError`] when the event exceeds declared bounds.
pub fn edit_metadata(
    author: PublicKey,
    group: &SimpleGroup,
    edit: &MetadataEdit,
) -> Result<UnsignedEvent, EventBuildError> {
    let mut extra: Vec<Tag> = Vec::new();
    if let Some(name) = &edit.name {
        extra.push(parse_tag(["name", name])?);
    }
    if let Some(about) = &edit.about {
        extra.push(parse_tag(["about", about])?);
    }
    if let Some(picture) = &edit.picture {
        extra.push(parse_tag(["picture", picture])?);
    }
    if edit.visibility == Some(GroupVisibility::Private) {
        extra.push(parse_tag(["private"])?);
    }
    if edit.access == Some(GroupAccess::Closed) {
        extra.push(parse_tag(["closed"])?);
    }
    build(author, KIND_EDIT_METADATA, group, extra)
}

/// Build a kind-9009 invite event for `group`.
///
/// Emits `h`, one optional `p` tag for every invitee, and a `relay` tag.
///
/// NIP-29's kind 9009 requires a `code` tag while invitee `p` tags are
/// optional, so an empty `invitees` slice is valid. This constructor does not
/// yet accept the required code: callers must reopen this unsigned body through
/// [`EventBuilder::from`] and append the exact `code` tag before publication.
/// That temporary composition gap is a caller-visible DX limitation.
///
/// # Examples
///
/// ```
/// # use fava_simple_groups::{SimpleGroup, invite};
/// # use nostr::key::Keys;
/// # use nostr::types::RelayUrl;
/// let relay = RelayUrl::parse("wss://relay.example")?;
/// let group = SimpleGroup::new("cats", vec![relay.clone()])?;
/// let admin = Keys::generate();
/// let invitees = [Keys::generate().public_key(), Keys::generate().public_key()];
///
/// let event = invite(admin.public_key(), &group, &invitees, &relay)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
///
/// Returns [`EventBuildError`] when the completed event exceeds a universal
/// `fava-write` construction bound.
pub fn invite(
    author: PublicKey,
    group: &SimpleGroup,
    invitees: &[PublicKey],
    relay: &RelayUrl,
) -> Result<UnsignedEvent, EventBuildError> {
    let mut extra = invitees
        .iter()
        .map(|invitee| parse_tag(["p", &invitee.to_hex()]))
        .collect::<Result<Vec<_>, _>>()?;
    extra.push(parse_tag(["relay", relay.as_str()])?);
    build(author, KIND_INVITE, group, extra)
}

/// Build a kind-9021 join-request event for `group`.
///
/// Emits only the required `h` tag. Callers that need the optional `p` or
/// `relay` fields construct that exact variant through the generic
/// [`EventBuilder`](fava_write::EventBuilder) before publication.
///
/// # Examples
///
/// ```
/// # use fava_simple_groups::{SimpleGroup, join_request};
/// # use nostr::key::Keys;
/// # use nostr::types::RelayUrl;
/// let relay = RelayUrl::parse("wss://relay.example")?;
/// let group = SimpleGroup::new("cats", vec![relay])?;
/// let keys = Keys::generate();
///
/// let event = join_request(keys.public_key(), &group)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
///
/// Returns [`EventBuildError`] when the event exceeds declared bounds.
pub fn join_request(
    author: PublicKey,
    group: &SimpleGroup,
) -> Result<UnsignedEvent, EventBuildError> {
    build(author, KIND_JOIN_REQUEST, group, [])
}

/// Build a kind-9000 put-user event for `group`.
///
/// Emits `h` and one `p` tag for every supplied user. `roles` are appended as
/// additional values on each `p` tag: `["p", "<pubkey>", "role1", ...]`.
///
/// # Examples
///
/// ```
/// # use fava_simple_groups::{SimpleGroup, put_user};
/// # use nostr::key::Keys;
/// # use nostr::types::RelayUrl;
/// let relay = RelayUrl::parse("wss://relay.example")?;
/// let group = SimpleGroup::new("cats", vec![relay])?;
/// let admin = Keys::generate();
/// let members = [Keys::generate().public_key(), Keys::generate().public_key()];
///
/// // Grant both users the member role in one event.
/// let event = put_user(admin.public_key(), &group, &members, &["member"])?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
///
/// Returns [`EventBuildError`] when the completed event exceeds a universal
/// `fava-write` construction bound.
pub fn put_user(
    author: PublicKey,
    group: &SimpleGroup,
    users: &[PublicKey],
    roles: &[&str],
) -> Result<UnsignedEvent, EventBuildError> {
    let p_tags = users
        .iter()
        .map(|user| {
            let mut values = vec!["p".to_owned(), user.to_hex()];
            values.extend(roles.iter().map(|role| (*role).to_owned()));
            Tag::parse(values).map_err(|error| EventBuildError::Encoding(error.to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    build(author, KIND_PUT_USER, group, p_tags)
}

/// Build a kind-9001 remove-user event for `group`.
///
/// Emits `h` and one `p` tag for every user pubkey to remove.
///
/// # Examples
///
/// ```
/// # use fava_simple_groups::{SimpleGroup, remove_user};
/// # use nostr::key::Keys;
/// # use nostr::types::RelayUrl;
/// let relay = RelayUrl::parse("wss://relay.example")?;
/// let group = SimpleGroup::new("cats", vec![relay])?;
/// let admin = Keys::generate();
/// let members = [Keys::generate().public_key(), Keys::generate().public_key()];
///
/// let event = remove_user(admin.public_key(), &group, &members)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
///
/// Returns [`EventBuildError`] when the completed event exceeds a universal
/// `fava-write` construction bound.
pub fn remove_user(
    author: PublicKey,
    group: &SimpleGroup,
    users: &[PublicKey],
) -> Result<UnsignedEvent, EventBuildError> {
    let p_tags = users
        .iter()
        .map(|user| parse_tag(["p", &user.to_hex()]))
        .collect::<Result<Vec<_>, _>>()?;
    build(author, KIND_REMOVE_USER, group, p_tags)
}

/// Build a kind-9005 delete-event event for `group`.
///
/// Emits `h` and `e` (id of the event to delete).
///
/// # Examples
///
/// ```
/// # use fava_simple_groups::{SimpleGroup, delete_event};
/// # use fava_write::EventId;
/// # use nostr::key::Keys;
/// # use nostr::types::RelayUrl;
/// let relay = RelayUrl::parse("wss://relay.example")?;
/// let group = SimpleGroup::new("cats", vec![relay])?;
/// let admin = Keys::generate();
/// let target = EventId::from_byte_array([1u8; 32]);
///
/// let event = delete_event(admin.public_key(), &group, &target)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
///
/// Returns [`EventBuildError`] when the event exceeds declared bounds.
pub fn delete_event(
    author: PublicKey,
    group: &SimpleGroup,
    event_id: &EventId,
) -> Result<UnsignedEvent, EventBuildError> {
    let extra = [parse_tag(["e", &event_id.to_hex()])?];
    build(author, KIND_DELETE_EVENT, group, extra)
}

/// Build a kind-9008 delete-group event for `group`.
///
/// The relay removes the group when it accepts this event from an authorized
/// pubkey. Only the `h` tag is emitted.
///
/// # Examples
///
/// ```
/// # use fava_simple_groups::{SimpleGroup, delete_group};
/// # use nostr::key::Keys;
/// # use nostr::types::RelayUrl;
/// let relay = RelayUrl::parse("wss://relay.example")?;
/// let group = SimpleGroup::new("cats", vec![relay])?;
/// let admin = Keys::generate();
///
/// let event = delete_group(admin.public_key(), &group)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
///
/// Returns [`EventBuildError`] when the event exceeds declared bounds.
pub fn delete_group(
    author: PublicKey,
    group: &SimpleGroup,
) -> Result<UnsignedEvent, EventBuildError> {
    build(author, KIND_DELETE_GROUP, group, [])
}

/// Build a kind-9022 leave-group event for `group`.
///
/// The relay removes the author from the group when it accepts this event.
/// Only the `h` tag is emitted.
///
/// # Examples
///
/// ```
/// # use fava_simple_groups::{SimpleGroup, leave_group};
/// # use nostr::key::Keys;
/// # use nostr::types::RelayUrl;
/// let relay = RelayUrl::parse("wss://relay.example")?;
/// let group = SimpleGroup::new("cats", vec![relay])?;
/// let keys = Keys::generate();
///
/// let event = leave_group(keys.public_key(), &group)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
///
/// Returns [`EventBuildError`] when the event exceeds declared bounds.
pub fn leave_group(
    author: PublicKey,
    group: &SimpleGroup,
) -> Result<UnsignedEvent, EventBuildError> {
    build(author, KIND_LEAVE_GROUP, group, [])
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Build one management event: `h` tag first, then `extra` tags.
fn build(
    author: PublicKey,
    kind_u16: u16,
    group: &SimpleGroup,
    extra: impl IntoIterator<Item = Tag>,
) -> Result<UnsignedEvent, EventBuildError> {
    let h_tag = parse_tag(["h", group.id()])?;
    let mut tags = vec![h_tag];
    tags.extend(extra);
    EventBuilder::new(author, nostr::event::Kind::from_u16(kind_u16))
        .tags(tags)
        .build()
}

/// Parses a tag, converting the error to [`EventBuildError`].
fn parse_tag<'a>(values: impl IntoIterator<Item = &'a str>) -> Result<Tag, EventBuildError> {
    Tag::parse(values).map_err(|e| EventBuildError::Encoding(e.to_string()))
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use nostr::key::Keys;
    use nostr::types::RelayUrl;

    use fava_write::EventId;

    use super::*;

    fn group() -> SimpleGroup {
        let relay = RelayUrl::parse("wss://relay.example").unwrap();
        SimpleGroup::new("cats", vec![relay]).unwrap()
    }

    fn author() -> PublicKey {
        Keys::generate().public_key()
    }

    fn h_tag(event: &UnsignedEvent, id: &str) -> bool {
        event.tags.iter().any(|t| {
            let s = t.as_slice();
            s.first().map(String::as_str) == Some("h") && s.get(1).map(String::as_str) == Some(id)
        })
    }

    fn has_tag_name(event: &UnsignedEvent, name: &str) -> bool {
        event
            .tags
            .iter()
            .any(|t| t.as_slice().first().map(String::as_str) == Some(name))
    }

    fn tag_value(event: &UnsignedEvent, name: &str, index: usize) -> Option<String> {
        event.tags.iter().find_map(|t| {
            let s = t.as_slice();
            if s.first().map(String::as_str) == Some(name) {
                s.get(index).cloned()
            } else {
                None
            }
        })
    }

    fn p_tags(event: &UnsignedEvent) -> Vec<Vec<String>> {
        event
            .tags
            .iter()
            .filter(|tag| tag.as_slice().first().map(String::as_str) == Some("p"))
            .map(|tag| tag.as_slice().to_vec())
            .collect()
    }

    #[test]
    fn create_group_kind_and_h() {
        let group = group();
        let event = create_group(author(), &group).unwrap();
        assert_eq!(event.kind.as_u16(), KIND_CREATE_GROUP);
        assert!(h_tag(&event, "cats"));
        assert_eq!(event.tags.len(), 1, "only h tag");
    }

    #[test]
    fn edit_metadata_kind_and_tags() {
        let group = group();
        let edit = MetadataEdit {
            name: Some("Cats".to_owned()),
            about: Some("About cats".to_owned()),
            picture: Some("https://example.com/cats.png".to_owned()),
            visibility: Some(GroupVisibility::Private),
            access: Some(GroupAccess::Closed),
        };
        let event = edit_metadata(author(), &group, &edit).unwrap();
        assert_eq!(event.kind.as_u16(), KIND_EDIT_METADATA);
        assert!(h_tag(&event, "cats"));
        assert_eq!(tag_value(&event, "name", 1).as_deref(), Some("Cats"));
        assert_eq!(tag_value(&event, "about", 1).as_deref(), Some("About cats"));
        assert!(has_tag_name(&event, "picture"));
        assert!(has_tag_name(&event, "private"));
        assert!(has_tag_name(&event, "closed"));
    }

    #[test]
    fn edit_metadata_omits_optional_tags() {
        let group = group();
        let event = edit_metadata(author(), &group, &MetadataEdit::default()).unwrap();
        assert_eq!(event.kind.as_u16(), KIND_EDIT_METADATA);
        assert!(!has_tag_name(&event, "name"));
        assert!(!has_tag_name(&event, "private"));
        assert!(!has_tag_name(&event, "closed"));
    }

    #[test]
    fn edit_metadata_public_open_omits_flags() {
        let group = group();
        let edit = MetadataEdit {
            visibility: Some(GroupVisibility::Public),
            access: Some(GroupAccess::Open),
            ..Default::default()
        };
        let event = edit_metadata(author(), &group, &edit).unwrap();
        assert!(!has_tag_name(&event, "private"));
        assert!(!has_tag_name(&event, "closed"));
    }

    #[test]
    fn invite_preserves_optional_target_cardinality() {
        let group = group();
        let invitees = [Keys::generate().public_key(), Keys::generate().public_key()];
        let relay = RelayUrl::parse("wss://invite.example").unwrap();
        let event = invite(author(), &group, &invitees, &relay).unwrap();
        assert_eq!(event.kind.as_u16(), KIND_INVITE);
        assert!(h_tag(&event, "cats"));
        assert_eq!(
            p_tags(&event),
            invitees
                .iter()
                .map(|invitee| vec!["p".to_owned(), invitee.to_hex()])
                .collect::<Vec<_>>()
        );
        assert_eq!(
            tag_value(&event, "relay", 1).as_deref(),
            Some(relay.as_str())
        );

        let empty = invite(author(), &group, &[], &relay).unwrap();
        assert!(p_tags(&empty).is_empty());
    }

    #[test]
    fn join_request_kind_and_h() {
        let group = group();
        let event = join_request(author(), &group).unwrap();
        assert_eq!(event.kind.as_u16(), KIND_JOIN_REQUEST);
        assert!(h_tag(&event, "cats"));
        assert_eq!(event.tags.len(), 1);
    }

    #[test]
    fn put_user_preserves_target_cardinality_and_common_roles() {
        let group = group();
        let first = Keys::generate().public_key();
        let second = Keys::generate().public_key();
        let users = [first, second, first];
        let event = put_user(author(), &group, &users, &["admin", "moderator"]).unwrap();
        assert_eq!(event.kind.as_u16(), KIND_PUT_USER);
        assert!(h_tag(&event, "cats"));
        assert_eq!(
            p_tags(&event),
            users
                .iter()
                .map(|user| {
                    vec![
                        "p".to_owned(),
                        user.to_hex(),
                        "admin".to_owned(),
                        "moderator".to_owned(),
                    ]
                })
                .collect::<Vec<_>>()
        );
        let empty = put_user(author(), &group, &[], &[]).unwrap();
        assert!(h_tag(&empty, "cats"));
        assert!(p_tags(&empty).is_empty());
    }

    #[test]
    fn put_user_preserves_generic_event_build_refusals() {
        let group = group();
        let users = vec![Keys::generate().public_key(); 2_001];
        assert!(matches!(
            put_user(author(), &group, &users, &[]),
            Err(EventBuildError::TooManyTags {
                actual: 2_002,
                maximum: 2_000,
            })
        ));
    }

    #[test]
    fn put_user_no_roles() {
        let group = group();
        let user = Keys::generate().public_key();
        let event = put_user(author(), &group, &[user], &[]).unwrap();
        assert_eq!(event.kind.as_u16(), KIND_PUT_USER);
        let p = event
            .tags
            .iter()
            .find(|t| t.as_slice().first().map(String::as_str) == Some("p"))
            .unwrap();
        assert_eq!(p.as_slice().len(), 2);
    }

    #[test]
    fn remove_user_preserves_target_cardinality_and_encodes_empty() {
        let group = group();
        let users = [Keys::generate().public_key(), Keys::generate().public_key()];
        let event = remove_user(author(), &group, &users).unwrap();
        assert_eq!(event.kind.as_u16(), KIND_REMOVE_USER);
        assert!(h_tag(&event, "cats"));
        assert_eq!(
            p_tags(&event),
            users
                .iter()
                .map(|user| vec!["p".to_owned(), user.to_hex()])
                .collect::<Vec<_>>()
        );
        let empty = remove_user(author(), &group, &[]).unwrap();
        assert!(h_tag(&empty, "cats"));
        assert!(p_tags(&empty).is_empty());
    }

    #[test]
    fn delete_event_kind_and_e() {
        let group = group();
        let target = EventId::from_byte_array([42u8; 32]);
        let event = delete_event(author(), &group, &target).unwrap();
        assert_eq!(event.kind.as_u16(), KIND_DELETE_EVENT);
        assert!(h_tag(&event, "cats"));
        assert_eq!(
            tag_value(&event, "e", 1).as_deref(),
            Some(target.to_hex().as_str())
        );
    }

    #[test]
    fn delete_group_kind_and_h() {
        let group = group();
        let event = delete_group(author(), &group).unwrap();
        assert_eq!(event.kind.as_u16(), KIND_DELETE_GROUP);
        assert!(h_tag(&event, "cats"));
        assert_eq!(event.tags.len(), 1);
    }

    #[test]
    fn leave_group_kind_and_h() {
        let group = group();
        let event = leave_group(author(), &group).unwrap();
        assert_eq!(event.kind.as_u16(), KIND_LEAVE_GROUP);
        assert!(h_tag(&event, "cats"));
        assert_eq!(event.tags.len(), 1);
    }

    /// Confirms every constructor emits the group's `h` tag correctly.
    #[test]
    fn all_constructors_have_h_tag() {
        let group = group();
        let a = author();
        let user = Keys::generate().public_key();
        let relay = RelayUrl::parse("wss://relay.example").unwrap();
        let target = EventId::from_byte_array([1u8; 32]);

        let events = [
            create_group(a, &group).unwrap(),
            edit_metadata(a, &group, &MetadataEdit::default()).unwrap(),
            invite(a, &group, &[user], &relay).unwrap(),
            join_request(a, &group).unwrap(),
            put_user(a, &group, &[user], &[]).unwrap(),
            remove_user(a, &group, &[user]).unwrap(),
            delete_event(a, &group, &target).unwrap(),
            delete_group(a, &group).unwrap(),
            leave_group(a, &group).unwrap(),
        ];

        for event in &events {
            assert!(
                h_tag(event, "cats"),
                "missing h tag in kind {}",
                event.kind.as_u16()
            );
        }
    }
}
