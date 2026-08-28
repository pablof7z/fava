//! Typed NIP-29 management event constructors.
//!
//! This module owns the group lifecycle write surface. Every NIP-29 management
//! kind number lives here and nowhere else; callers see typed functions that
//! return [`EventBuilder`] values, not raw kind integers.
//!
//! # Publish path
//!
//! Every constructor returns an [`EventBuilder`] already routed to the group's
//! relays. Publish through `Fava::publish`:
//!
//! ```ignore
//! let builder = create_group(author.public_key(), &group)?;
//! let write = fava.publish(builder)?;
//! ```
//!
//! # Encapsulation guarantee
//!
//! Kind numbers 9000–9009 and 9021–9022 are private constants in this module.
//! No caller, test, or example writes `Kind::from_u16(9NNN)`.

use fava_write::{EventBuilder, EventId, PublicKey, Tag, WriteIntentError};

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
/// Returns an [`EventBuilder`] routed to the group's relays; publish via
/// `fava.publish(builder)`. The event carries a single `h` tag with the group
/// id. The relay creates the group when it accepts this event from an authorized
/// pubkey.
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
/// let builder = create_group(keys.public_key(), &group)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
///
/// Returns [`WriteIntentError`] when the group's relay route exceeds declared
/// bounds.
pub fn create_group(
    author: PublicKey,
    group: &SimpleGroup,
) -> Result<EventBuilder, WriteIntentError> {
    build(author, KIND_CREATE_GROUP, group, [])
}

/// Build a kind-9002 edit-metadata event for `group`.
///
/// Returns an [`EventBuilder`] routed to the group's relays; publish via
/// `fava.publish(builder)`. Tags for `None` fields in `edit` are omitted; the
/// relay resets absent fields to their default values.
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
/// let builder = edit_metadata(
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
/// Returns [`WriteIntentError`] when the group's relay route exceeds declared
/// bounds.
pub fn edit_metadata(
    author: PublicKey,
    group: &SimpleGroup,
    edit: &MetadataEdit,
) -> Result<EventBuilder, WriteIntentError> {
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

/// Build a kind-9009 invite event for `group` with the exact invite code.
///
/// Returns an [`EventBuilder`] routed to the group's relays; publish via
/// `fava.publish(builder)`. Emits `h` and the exact `code` tag only. An
/// empty code string is valid and emits `["code", ""]`.
///
/// # Examples
///
/// ```
/// # use fava_simple_groups::{SimpleGroup, invite};
/// # use nostr::key::Keys;
/// # use nostr::types::RelayUrl;
/// let relay = RelayUrl::parse("wss://relay.example")?;
/// let group = SimpleGroup::new("cats", vec![relay])?;
/// let admin = Keys::generate();
///
/// let builder = invite(admin.public_key(), &group, "my-invite-code")?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
///
/// Returns [`WriteIntentError`] when the group's relay route exceeds declared
/// bounds.
pub fn invite(
    author: PublicKey,
    group: &SimpleGroup,
    code: &str,
) -> Result<EventBuilder, WriteIntentError> {
    let code_tag = parse_tag(["code", code])?;
    build(author, KIND_INVITE, group, [code_tag])
}

/// Build a kind-9021 join-request event for `group`.
///
/// Returns an [`EventBuilder`] routed to the group's relays; publish via
/// `fava.publish(builder)`. Emits `h` and, when `code` is `Some`, the exact
/// invite code tag. The optional reason field stays ordinary builder
/// `.content(...)` on the returned builder.
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
/// let builder = join_request(keys.public_key(), &group, None)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
///
/// Returns [`WriteIntentError`] when the group's relay route exceeds declared
/// bounds.
pub fn join_request(
    author: PublicKey,
    group: &SimpleGroup,
    code: Option<&str>,
) -> Result<EventBuilder, WriteIntentError> {
    let extra: Vec<Tag> = if let Some(c) = code {
        vec![parse_tag(["code", c])?]
    } else {
        vec![]
    };
    build(author, KIND_JOIN_REQUEST, group, extra)
}

/// Build a kind-9000 put-user event for `group`.
///
/// Returns an [`EventBuilder`] routed to the group's relays; publish via
/// `fava.publish(builder)`. Emits `h` and one `p` tag for every supplied user.
/// `roles` are appended as additional values on each `p` tag:
/// `["p", "<pubkey>", "role1", ...]`. Empty slices emit no `p` tags.
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
/// let builder = put_user(admin.public_key(), &group, &members, &["member"])?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
///
/// Returns [`WriteIntentError`] when the group's relay route exceeds declared
/// bounds.
pub fn put_user(
    author: PublicKey,
    group: &SimpleGroup,
    users: &[PublicKey],
    roles: &[&str],
) -> Result<EventBuilder, WriteIntentError> {
    let p_tags = users
        .iter()
        .map(|user| {
            let mut values = vec!["p".to_owned(), user.to_hex()];
            values.extend(roles.iter().map(|role| (*role).to_owned()));
            Tag::parse(values).map_err(|error| WriteIntentError::Encoding(error.to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    build(author, KIND_PUT_USER, group, p_tags)
}

/// Build a kind-9001 remove-user event for `group`.
///
/// Returns an [`EventBuilder`] routed to the group's relays; publish via
/// `fava.publish(builder)`. Emits `h` and one `p` tag for every user pubkey
/// to remove. Empty slices emit no `p` tags.
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
/// let builder = remove_user(admin.public_key(), &group, &members)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
///
/// Returns [`WriteIntentError`] when the group's relay route exceeds declared
/// bounds.
pub fn remove_user(
    author: PublicKey,
    group: &SimpleGroup,
    users: &[PublicKey],
) -> Result<EventBuilder, WriteIntentError> {
    let p_tags = users
        .iter()
        .map(|user| parse_tag(["p", &user.to_hex()]))
        .collect::<Result<Vec<_>, _>>()?;
    build(author, KIND_REMOVE_USER, group, p_tags)
}

/// Build a kind-9005 delete-event event for `group`.
///
/// Returns an [`EventBuilder`] routed to the group's relays; publish via
/// `fava.publish(builder)`. Emits `h` and `e` (id of the event to delete).
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
/// let builder = delete_event(admin.public_key(), &group, &target)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
///
/// Returns [`WriteIntentError`] when the group's relay route exceeds declared
/// bounds.
pub fn delete_event(
    author: PublicKey,
    group: &SimpleGroup,
    event_id: &EventId,
) -> Result<EventBuilder, WriteIntentError> {
    let extra = [parse_tag(["e", &event_id.to_hex()])?];
    build(author, KIND_DELETE_EVENT, group, extra)
}

/// Build a kind-9008 delete-group event for `group`.
///
/// Returns an [`EventBuilder`] routed to the group's relays; publish via
/// `fava.publish(builder)`. The relay removes the group when it accepts this
/// event from an authorized pubkey. Only the `h` tag is emitted.
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
/// let builder = delete_group(admin.public_key(), &group)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
///
/// Returns [`WriteIntentError`] when the group's relay route exceeds declared
/// bounds.
pub fn delete_group(
    author: PublicKey,
    group: &SimpleGroup,
) -> Result<EventBuilder, WriteIntentError> {
    build(author, KIND_DELETE_GROUP, group, [])
}

/// Build a kind-9022 leave-group event for `group`.
///
/// Returns an [`EventBuilder`] routed to the group's relays; publish via
/// `fava.publish(builder)`. The relay removes the author from the group when
/// it accepts this event. Only the `h` tag is emitted.
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
/// let builder = leave_group(keys.public_key(), &group)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
///
/// Returns [`WriteIntentError`] when the group's relay route exceeds declared
/// bounds.
pub fn leave_group(
    author: PublicKey,
    group: &SimpleGroup,
) -> Result<EventBuilder, WriteIntentError> {
    build(author, KIND_LEAVE_GROUP, group, [])
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Build one management event body and attach the group's relay routing.
fn build(
    author: PublicKey,
    kind_u16: u16,
    group: &SimpleGroup,
    extra: impl IntoIterator<Item = Tag>,
) -> Result<EventBuilder, WriteIntentError> {
    let h_tag = parse_tag(["h", group.id()])?;
    let mut tags = vec![h_tag];
    tags.extend(extra);
    EventBuilder::new(author, nostr::event::Kind::from_u16(kind_u16))
        .tags(tags)
        .to_relays(group.relays().collect::<Vec<_>>())
}

/// Parses a tag, converting the error to [`WriteIntentError`].
fn parse_tag<'a>(values: impl IntoIterator<Item = &'a str>) -> Result<Tag, WriteIntentError> {
    Tag::parse(values).map_err(|e| WriteIntentError::Encoding(e.to_string()))
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use nostr::key::Keys;
    use nostr::types::RelayUrl;

    use fava_write::{EventBuildError, EventId, UnsignedEvent, WriteRouting};

    use super::*;

    fn group() -> SimpleGroup {
        let relay = RelayUrl::parse("wss://relay.example").unwrap();
        SimpleGroup::new("cats", vec![relay]).unwrap()
    }

    fn author() -> PublicKey {
        Keys::generate().public_key()
    }

    /// Build an event from a constructor result, panicking on any failure.
    fn build_event(result: Result<EventBuilder, WriteIntentError>) -> UnsignedEvent {
        result.unwrap().into_event_and_routing().unwrap().0
    }

    /// Build the routing from a constructor result, panicking on any failure.
    fn build_routing(result: Result<EventBuilder, WriteIntentError>) -> WriteRouting {
        result.unwrap().into_event_and_routing().unwrap().1
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

    fn group_relay() -> RelayUrl {
        RelayUrl::parse("wss://relay.example").unwrap()
    }

    #[test]
    fn create_group_kind_and_h() {
        let group = group();
        let event = build_event(create_group(author(), &group));
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
        let event = build_event(edit_metadata(author(), &group, &edit));
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
        let event = build_event(edit_metadata(author(), &group, &MetadataEdit::default()));
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
        let event = build_event(edit_metadata(author(), &group, &edit));
        assert!(!has_tag_name(&event, "private"));
        assert!(!has_tag_name(&event, "closed"));
    }

    #[test]
    fn invite_emits_exact_code_tag_and_no_p_or_relay_tags() {
        let group = group();
        let code = "test-invite-code";
        let event = build_event(invite(author(), &group, code));
        assert_eq!(event.kind.as_u16(), KIND_INVITE);
        assert!(h_tag(&event, "cats"));
        assert_eq!(
            tag_value(&event, "code", 1).as_deref(),
            Some(code),
            "code tag carries the exact supplied code"
        );
        assert!(p_tags(&event).is_empty(), "invite must not emit p tags");
        assert!(
            !has_tag_name(&event, "relay"),
            "invite must not emit relay tag"
        );
        assert_eq!(event.tags.len(), 2, "exactly h and code");

        // Empty code string is valid.
        let empty_code = build_event(invite(author(), &group, ""));
        assert_eq!(
            tag_value(&empty_code, "code", 1).as_deref(),
            Some(""),
            "empty code emits [\"code\", \"\"]"
        );
    }

    #[test]
    fn join_request_kind_h_and_optional_code() {
        let group = group();
        let event = build_event(join_request(author(), &group, None));
        assert_eq!(event.kind.as_u16(), KIND_JOIN_REQUEST);
        assert!(h_tag(&event, "cats"));
        assert_eq!(event.tags.len(), 1, "only h tag when no code");

        let with_code = build_event(join_request(author(), &group, Some("my-code")));
        assert_eq!(with_code.kind.as_u16(), KIND_JOIN_REQUEST);
        assert!(h_tag(&with_code, "cats"));
        assert_eq!(
            tag_value(&with_code, "code", 1).as_deref(),
            Some("my-code"),
            "code tag carries the exact supplied code"
        );
        assert_eq!(with_code.tags.len(), 2, "h and code");
    }

    #[test]
    fn put_user_preserves_target_cardinality_and_common_roles() {
        let group = group();
        let first = Keys::generate().public_key();
        let second = Keys::generate().public_key();
        let users = [first, second, first];
        let event = build_event(put_user(author(), &group, &users, &["admin", "moderator"]));
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
        let empty = build_event(put_user(author(), &group, &[], &[]));
        assert!(h_tag(&empty, "cats"));
        assert!(p_tags(&empty).is_empty());
    }

    #[test]
    fn put_user_build_refusals_surface_at_build_time() {
        let group = group();
        let users = vec![Keys::generate().public_key(); 2_001];
        assert!(matches!(
            put_user(author(), &group, &users, &[])
                .unwrap()
                .into_event_and_routing(),
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
        let event = build_event(put_user(author(), &group, &[user], &[]));
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
        let event = build_event(remove_user(author(), &group, &users));
        assert_eq!(event.kind.as_u16(), KIND_REMOVE_USER);
        assert!(h_tag(&event, "cats"));
        assert_eq!(
            p_tags(&event),
            users
                .iter()
                .map(|user| vec!["p".to_owned(), user.to_hex()])
                .collect::<Vec<_>>()
        );
        let empty = build_event(remove_user(author(), &group, &[]));
        assert!(h_tag(&empty, "cats"));
        assert!(p_tags(&empty).is_empty());
    }

    #[test]
    fn delete_event_kind_and_e() {
        let group = group();
        let target = EventId::from_byte_array([42u8; 32]);
        let event = build_event(delete_event(author(), &group, &target));
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
        let event = build_event(delete_group(author(), &group));
        assert_eq!(event.kind.as_u16(), KIND_DELETE_GROUP);
        assert!(h_tag(&event, "cats"));
        assert_eq!(event.tags.len(), 1);
    }

    #[test]
    fn leave_group_kind_and_h() {
        let group = group();
        let event = build_event(leave_group(author(), &group));
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
        let target = EventId::from_byte_array([1u8; 32]);

        let builders = [
            create_group(a, &group).unwrap(),
            edit_metadata(a, &group, &MetadataEdit::default()).unwrap(),
            invite(a, &group, "code").unwrap(),
            join_request(a, &group, None).unwrap(),
            put_user(a, &group, &[user], &[]).unwrap(),
            remove_user(a, &group, &[user]).unwrap(),
            delete_event(a, &group, &target).unwrap(),
            delete_group(a, &group).unwrap(),
            leave_group(a, &group).unwrap(),
        ];

        for builder in builders {
            let (event, _) = builder.into_event_and_routing().unwrap();
            assert!(
                h_tag(&event, "cats"),
                "missing h tag in kind {}",
                event.kind.as_u16()
            );
        }
    }

    /// Confirms every constructor embeds the group's relay route.
    #[test]
    fn all_constructors_have_group_relay_routing() {
        let group = group();
        let a = author();
        let user = Keys::generate().public_key();
        let target = EventId::from_byte_array([1u8; 32]);
        let expected = WriteRouting::Explicit(vec![group_relay()]);

        let builders = [
            create_group(a, &group).unwrap(),
            edit_metadata(a, &group, &MetadataEdit::default()).unwrap(),
            invite(a, &group, "code").unwrap(),
            join_request(a, &group, None).unwrap(),
            put_user(a, &group, &[user], &[]).unwrap(),
            remove_user(a, &group, &[user]).unwrap(),
            delete_event(a, &group, &target).unwrap(),
            delete_group(a, &group).unwrap(),
            leave_group(a, &group).unwrap(),
        ];

        for builder in builders {
            let routing = build_routing(Ok(builder));
            assert_eq!(routing, expected);
        }
    }
}
