//! External compile and behavior tracer for the simple-groups capability.

use std::collections::BTreeSet;
use std::error::Error;

use fava::{Fava, Observation, Write};
use fava_query::{
    Kind, Query, QueryAcquisition, QuerySnapshot, RelayUrl, ResultAuthority, SingleLetterTag,
};
use fava_write::{EventBuilder, PublicKey, Timestamp};

use fava_simple_groups::{
    SimpleGroup, SimpleGroupAdmins, SimpleGroupError, SimpleGroupMembers, SimpleGroupMetadata, SimpleGroupParticipants, SimpleGroupPins,
    SimpleGroupRecords, SimpleGroupRoles, SimpleGroupSnapshot, PinnedItem, SavedSimpleGroup, SavedRelay, SimpleGroups,
};

type ReadmePublishResult = Result<Write, Box<dyn Error>>;
type PublishUnsigned = fn(&Fava, &SimpleGroup, fava_write::UnsignedEvent) -> ReadmePublishResult;
type PublishSigned = fn(&Fava, &SimpleGroup, fava_write::Event) -> ReadmePublishResult;
type PublishSavedEdit = fn(&Fava, &SimpleGroup, PublicKey) -> ReadmePublishResult;

fn metadata_signature_is_public(event: &fava_write::EventValue) -> Result<String, SimpleGroupError> {
    SimpleGroupMetadata::from_event(event).map(|metadata| metadata.id().to_owned())
}

fn saved_signatures_are_public(event: &fava_write::EventValue) {
    drop(SavedSimpleGroup::from_event(event));
    drop(SavedRelay::from_event(event));
}

fn relay(url: &str) -> RelayUrl {
    RelayUrl::parse(url).expect("test relay URL")
}

fn simple_group(host: RelayUrl, id: &str) -> Result<SimpleGroup, SimpleGroupError> {
    SimpleGroup::on([host], id)
}

#[test]
fn one_host_group_traces_pure_preparation_and_queries() {
    let host = relay("wss://groups.example");
    let simple_group_id = " photos ";
    let simple_group = simple_group(host.clone(), simple_group_id).expect("one host is a valid group");
    let author =
        PublicKey::from_hex("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798")
            .expect("generator public key");
    let draft = EventBuilder::new(author, Kind::from_u16(9_007))
        .created_at(Timestamp::from(7))
        .content("opaque content")
        .build()
        .expect("bounded draft");
    let prepared = simple_group.prepare(draft).expect("pure preparation");
    let repeated = simple_group
        .prepare(prepared.clone())
        .expect("repeated preparation is inert");
    let content = simple_group
        .events(Query::events().kind(Kind::from_u16(9)))
        .expect("ordinary content query");
    let h = SingleLetterTag::from_char('h').expect("tag key");
    let contexts: Vec<_> = prepared
        .tags
        .iter()
        .filter(|tag| tag.as_slice().first().map(String::as_str) == Some("h"))
        .map(|tag| tag.as_slice().to_vec())
        .collect();
    let hosts: Vec<_> = simple_group.hosts().collect();

    assert_eq!(simple_group.id(), simple_group_id, "the opaque id must not be trimmed");
    assert_eq!(hosts.len(), 1);
    assert_eq!(hosts[0], host);
    assert_eq!(
        (
            contexts,
            content.selection().tag_values.get(&h),
            content.source().acquisition(),
            content.source().authority(),
        ),
        (
            vec![vec!["h".to_owned(), simple_group_id.to_owned()]],
            Some(&BTreeSet::from([simple_group_id.to_owned()])),
            &QueryAcquisition::Explicit(BTreeSet::from([host])),
            &ResultAuthority::AnyLocal,
        )
    );
    assert_eq!(content.result_limit(), None);
    assert_eq!(repeated, prepared);
}

#[test]
fn group_records_uses_exact_fixed_kind_set() {
    let host = relay("wss://groups.example");
    let simple_group = simple_group(host.clone(), "photos").expect("one host is a valid group");
    let records = simple_group
        .records(SimpleGroupRecords::all())
        .expect("ordinary record query");
    let d = SingleLetterTag::from_char('d').expect("tag key");
    let kinds = BTreeSet::from([
        Kind::from_u16(39_000),
        Kind::from_u16(39_001),
        Kind::from_u16(39_002),
        Kind::from_u16(39_003),
        Kind::from_u16(39_004),
        Kind::from_u16(39_005),
    ]);

    assert_eq!(
        (
            records.selection().kinds.as_ref(),
            records.selection().tag_values.get(&d),
            records.source().acquisition(),
            records.source().authority(),
        ),
        (
            Some(&kinds),
            Some(&BTreeSet::from(["photos".to_owned()])),
            &QueryAcquisition::Explicit(BTreeSet::from([host.clone()])),
            &ResultAuthority::OnlyRelays(BTreeSet::from([host])),
        )
    );
}

#[test]
fn metadata_parser_accessors_compile_externally() {
    let parser: fn(&fava_write::EventValue) -> Result<String, SimpleGroupError> =
        metadata_signature_is_public;
    let _ = parser;
}

#[test]
fn people_parser_signatures_compile_externally() {
    let _: fn(&fava_write::EventValue) -> Result<SimpleGroupAdmins, SimpleGroupError> = SimpleGroupAdmins::from_event;
    let _: fn(&fava_write::EventValue) -> Result<SimpleGroupMembers, SimpleGroupError> =
        SimpleGroupMembers::from_event;
    let _: fn(&fava_write::EventValue) -> Result<SimpleGroupRoles, SimpleGroupError> = SimpleGroupRoles::from_event;
    let _: fn(&fava_write::EventValue) -> Result<SimpleGroupParticipants, SimpleGroupError> =
        SimpleGroupParticipants::from_event;
}

#[test]
fn pin_and_saved_parser_signatures_compile_externally() {
    let _: fn(&fava_write::EventValue) -> Result<SimpleGroupPins, SimpleGroupError> = SimpleGroupPins::from_event;
    let _: fn(&fava_write::EventValue) = saved_signatures_are_public;
    let target: Option<PinnedItem> = None;
    assert!(target.is_none());
}

#[test]
fn group_snapshot_signatures_compile_externally() {
    let host = relay("wss://groups.example");
    let simple_group = simple_group(host.clone(), "photos").expect("one host");
    let input = QuerySnapshot::evaluated(Vec::new(), &[]);
    let snapshot: SimpleGroupSnapshot = simple_group.project(&input).expect("bounded projection");

    let mut hosts = snapshot.hosts();
    assert_eq!(hosts.next(), Some(&host));
    assert!(hosts.next().is_none());
    assert!(snapshot.at(&host).is_some());
    assert!(snapshot.events().is_empty());
    assert!(snapshot.metadata().next().is_none());
    assert!(snapshot.admins().next().is_none());
    assert!(snapshot.members().next().is_none());
}

#[test]
fn saved_edit_signatures_compile_externally() {
    let _: fn(&SimpleGroup, Option<&str>) -> Result<fava_write::ReplaceableEventEdit, SimpleGroupError> =
        SimpleGroups::save_simple_group;
    let _: fn(&SimpleGroup) -> Result<fava_write::ReplaceableEventEdit, SimpleGroupError> =
        SimpleGroups::remove_simple_group;
    let _: fn(&SimpleGroup, &str) -> Result<fava_write::ReplaceableEventEdit, SimpleGroupError> =
        SimpleGroups::rename_saved_simple_group;
    let _: fn(RelayUrl) -> Result<fava_write::ReplaceableEventEdit, SimpleGroupError> =
        SimpleGroups::save_relay;
    let _: fn(RelayUrl) -> Result<fava_write::ReplaceableEventEdit, SimpleGroupError> =
        SimpleGroups::remove_relay;
    let _: fn() -> std::sync::Arc<dyn fava_write::ReplaceableEventMaterializer> =
        SimpleGroups::materializer;
}

#[test]
fn management_event_signatures_compile_externally() {
    let _: fn(&SimpleGroup, fava_write::UnsignedEvent) -> Result<fava_write::UnsignedEvent, SimpleGroupError> =
        SimpleGroup::edit_metadata;
    let _: fn(&SimpleGroup, fava_write::UnsignedEvent) -> Result<fava_write::UnsignedEvent, SimpleGroupError> =
        SimpleGroup::set_pins;
}

fn readme_publishes_prepared_unsigned(
    fava: &Fava,
    simple_group: &SimpleGroup,
    draft: fava_write::UnsignedEvent,
) -> Result<Write, Box<dyn Error>> {
    let prepared = simple_group.prepare(draft)?;
    Ok(fava.to(simple_group.hosts())?.publish(prepared)?)
}

fn readme_publishes_prepared_signed(
    fava: &Fava,
    simple_group: &SimpleGroup,
    signed: fava_write::Event,
) -> Result<Write, Box<dyn Error>> {
    let prepared = simple_group.prepare(signed)?;
    Ok(fava.to(simple_group.hosts())?.publish(prepared)?)
}

fn readme_publishes_saved_edit(
    fava: &Fava,
    simple_group: &SimpleGroup,
    author: PublicKey,
) -> Result<Write, Box<dyn Error>> {
    let edit = SimpleGroups::save_simple_group(simple_group, Some("Photography"))?;
    Ok(fava.by(author).to(simple_group.hosts())?.publish(edit)?)
}

fn readme_cancels_and_closes(
    fava: &Fava,
    observation: &Observation,
    write: &Write,
) -> Result<(), fava::PublicationError> {
    let _cancelled = fava.cancel_publication(write.receipt_id())?;
    observation.close();
    Ok(())
}

#[test]
fn readme_facade_flow_compiles_externally() {
    let _: PublishUnsigned = readme_publishes_prepared_unsigned;
    let _: PublishSigned = readme_publishes_prepared_signed;
    let _: PublishSavedEdit = readme_publishes_saved_edit;
    let _: fn(&Fava, &Observation, &Write) -> Result<(), fava::PublicationError> =
        readme_cancels_and_closes;
}

/// `SimpleGroupError` already names the empty and oversized host-set refusals
/// exactly. A repeated host is the third refusal of the same kind and must
/// not arrive as a malformed event.
#[test]
fn a_repeated_group_host_is_reported_as_a_host_set_defect() {
    let relay = RelayUrl::parse("wss://relay.example").expect("relay url");

    let mapped = SimpleGroupError::from(fava_write::WriteIntentError::DuplicateExplicitRelay {
        relay: relay.clone(),
    });

    assert_eq!(mapped, SimpleGroupError::DuplicateHost { relay });
    assert!(
        !matches!(mapped, SimpleGroupError::Event(_)),
        "a bad host set is never a malformed event"
    );
}
