//! External public-surface and behavior canary.

use std::collections::BTreeSet;
use std::sync::Arc;

use fava_query::{Kind, PublicKey, Query, RelayUrl};
use fava_simple_groups::{
    SavedGroupList, SavedGroupListDecodeError, SavedSimpleGroup, SimpleGroup, SimpleGroupAdmins,
    SimpleGroupConstructionError, SimpleGroupDecodeError, SimpleGroupLivekitParticipants,
    SimpleGroupMembers, SimpleGroupMetadata, SimpleGroupPins, SimpleGroupRoles,
    SimpleGroupStateEventKind, remove_saved_relay, remove_saved_simple_group,
    rename_saved_simple_group, save_relay, save_simple_group, saved_group_list_materializer,
    saved_group_lists,
};
use fava_state::EventCoordinate;
use fava_write::{
    EventBuildError, EventBuilder, EventValue, ReplaceableEventEdit, ReplaceableEventMaterializer,
    Tag, UnsignedEvent, WriteIntentError,
};

fn key() -> PublicKey {
    PublicKey::from_hex("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798")
        .expect("public key")
}

fn relay() -> RelayUrl {
    RelayUrl::parse("wss://relay.example").expect("relay")
}

fn inspect_decoders(
    values: (
        &SimpleGroupMetadata,
        &SimpleGroupAdmins,
        &SimpleGroupMembers,
        &SimpleGroupRoles,
        &SimpleGroupLivekitParticipants,
        &SimpleGroupPins,
        &SavedGroupList,
        &SavedSimpleGroup,
    ),
) {
    let (metadata, admins, members, roles, participants, pins, list, saved) = values;
    let _: Option<&[Result<Kind, SimpleGroupDecodeError>]> = metadata.supported_kinds();
    let _: &[Result<String, SimpleGroupDecodeError>] = metadata.children();
    let _: &[Result<(PublicKey, Vec<String>), SimpleGroupDecodeError>] = admins.admins();
    let _: &[Result<PublicKey, SimpleGroupDecodeError>] = members.members();
    let _: &[Result<(String, Option<String>), SimpleGroupDecodeError>] = roles.roles();
    let _: &[Result<PublicKey, SimpleGroupDecodeError>] = participants.participants();
    let _: &[Result<EventCoordinate, SimpleGroupDecodeError>] = pins.pins();
    let _: &[Result<SavedSimpleGroup, SavedGroupListDecodeError>] = list.simple_groups();
    let _: &[Result<RelayUrl, SavedGroupListDecodeError>] = list.relays();
    let _: Option<&str> = saved.display_name();
}

#[test]
fn constructors_queries_and_preparation_compile_at_the_current_surface() {
    let constructor: Result<SimpleGroup, SimpleGroupConstructionError> =
        SimpleGroup::from_relays("photos", vec![relay()]);
    let group = constructor.expect("valid public construction");
    let _: Vec<RelayUrl> = group.relays().collect();
    assert_eq!(group.id(), "photos");

    let content_result: Result<Query, fava_query::QueryError> = group.events(
        Query::events()
            .kinds([Kind::from_u16(9)])
            .expect("one kind is bounded"),
    );
    let content = content_result.unwrap();
    let state_result: Result<Query, fava_query::QueryError> = group.meta_events([
        SimpleGroupStateEventKind::Metadata,
        SimpleGroupStateEventKind::Pins,
    ]);
    let state = state_result.unwrap();
    assert_eq!(
        state.selection().kinds,
        Some(BTreeSet::from([
            Kind::from_u16(39_000),
            Kind::from_u16(39_005),
        ]))
    );
    assert!(
        content
            .selection()
            .kinds
            .as_ref()
            .unwrap()
            .contains(&Kind::from_u16(9))
    );

    let draft = EventBuilder::new(key(), Kind::from_u16(42))
        .build()
        .unwrap();
    let _: Result<UnsignedEvent, EventBuildError> = group.prepare(draft);
}

#[test]
fn all_decoder_signatures_and_return_types_are_public() {
    let _: fn(&EventValue) -> Result<SimpleGroupMetadata, SimpleGroupDecodeError> =
        SimpleGroupMetadata::from_event;
    let _: fn(&EventValue) -> Result<SimpleGroupAdmins, SimpleGroupDecodeError> =
        SimpleGroupAdmins::from_event;
    let _: fn(&EventValue) -> Result<SimpleGroupMembers, SimpleGroupDecodeError> =
        SimpleGroupMembers::from_event;
    let _: fn(&EventValue) -> Result<SimpleGroupRoles, SimpleGroupDecodeError> =
        SimpleGroupRoles::from_event;
    let _: fn(&EventValue) -> Result<SimpleGroupLivekitParticipants, SimpleGroupDecodeError> =
        SimpleGroupLivekitParticipants::from_event;
    let _: fn(&EventValue) -> Result<SimpleGroupPins, SimpleGroupDecodeError> =
        SimpleGroupPins::from_event;
    let _: fn(&EventValue) -> Result<SavedGroupList, SavedGroupListDecodeError> =
        SavedGroupList::from_event;

    let _ = inspect_decoders;
}

#[test]
fn simple_group_list_query_and_edit_functions_compile_at_crate_root() {
    let _: Result<Query, fava_query::QueryError> = saved_group_lists([key()]);
    let group = SimpleGroup::from_relays("photos", vec![relay()]).expect("valid group");
    let _: Result<ReplaceableEventEdit, WriteIntentError> = save_simple_group(&group, None);
    let _: Result<ReplaceableEventEdit, WriteIntentError> = remove_saved_simple_group(&group);
    let _: Result<ReplaceableEventEdit, WriteIntentError> =
        rename_saved_simple_group(&group, "Photos");
    let _: Result<ReplaceableEventEdit, WriteIntentError> = save_relay(relay());
    let _: Result<ReplaceableEventEdit, WriteIntentError> = remove_saved_relay(relay());
    let _: Arc<dyn ReplaceableEventMaterializer> = saved_group_list_materializer();
}

#[test]
fn errors_expose_the_current_typed_fields() {
    assert_eq!(
        SimpleGroup::from_relays("", vec![relay()]),
        Err(SimpleGroupConstructionError::EmptyId)
    );
    assert_eq!(
        SimpleGroup::from_relays("photos", Vec::new()),
        Err(SimpleGroupConstructionError::EmptyRelays)
    );
    let decode = SimpleGroupDecodeError::MissingTagValue {
        tag_index: 1,
        value_index: 2,
    };
    match decode {
        SimpleGroupDecodeError::MissingTagValue {
            tag_index,
            value_index,
        } => assert_eq!((tag_index, value_index), (1, 2)),
        _ => unreachable!(),
    }
    let _: Tag = Tag::parse(["x"]).unwrap();
}
