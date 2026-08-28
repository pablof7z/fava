//! Real-relay, end-to-end demo of Fava's simple-group public APIs.
//!
//! Use an already-running relay:
//!
//! ```text
//! cargo run --manifest-path examples/simple-groups/Cargo.toml -- \\
//!   --relay ws://127.0.0.1:8080
//! ```
//!
//! A generic Nostr relay stores the real management events but usually does not
//! derive NIP-29 state events, and does not enforce NIP-29 authority either. The
//! demo detects that and says which checks it skipped. For the full
//! create/mutate/read/delete lifecycle, including the refusals, let the demo
//! start a local Croissant NIP-29 relay whose owner is the generated Alice key:
//!
//! ```text
//! cargo run --manifest-path examples/simple-groups/Cargo.toml -- \
//!   --spawn-croissant /path/to/croissant \
//!   --saved-relay ws://127.0.0.1:8080
//! ```

mod support;

use std::error::Error;
use std::time::{SystemTime, UNIX_EPOCH};

use fava::{EventBuilder, Kind, Query, Tag};
use fava_simple_groups::{
    GroupAccess, GroupVisibility, MetadataEdit, SimpleGroup, SimpleGroupEventBuilder,
    SimpleGroupStateEventKind, create_group, delete_event, delete_group, edit_metadata, invite,
    join_request, leave_group, put_user, remove_saved_relay, remove_saved_simple_group,
    remove_user, rename_saved_simple_group, save_relay, save_simple_group, saved_group_lists,
};
use nostr::event::EventId;
use nostr::key::Keys;

use support::{DemoResult, ExpectedGroupState, Options};

#[tokio::main]
#[allow(
    clippy::too_many_lines,
    reason = "one linear CLI narrative keeps each real relay action and result visible"
)]
async fn main() -> DemoResult<()> {
    let options = Options::parse()?;
    println!("Fava simple-groups real-relay demo\n");

    println!("1. Keys::generate() — create disposable Alice, Bob, Carol, and Dave keypairs");
    let alice = Keys::generate();
    let bob = Keys::generate();
    let carol = Keys::generate();
    let dave = Keys::generate();
    support::print_identity("Alice (group admin)", &alice);
    support::print_identity("Bob (invited member)", &bob);
    support::print_identity("Carol (joins, leaves, is re-added, is removed)", &carol);
    support::print_identity("Dave (outsider who never joins)", &dave);

    let endpoint = support::RelayEndpoint::open(&options, alice.public_key()).await?;
    let relay = endpoint.url().clone();
    let saved_relay = options.saved_relay(&relay)?;
    println!("\n2. WebSocketTransport — connect to real relay {relay}");
    if saved_relay != relay {
        println!("   saved lists use user relay {saved_relay}");
    }

    let group_id = unique_group_id()?;
    let group = SimpleGroup::new(&group_id, vec![relay.clone(), relay.clone()])?;
    println!(
        "3. SimpleGroup::new({group_id:?}, [relay, duplicate relay]) -> id={}, normalized_relays={}",
        group.id(),
        group.relays().count()
    );

    let fava = support::assemble(&alice, &bob, &carol, &dave)?;
    println!(
        "4. Fava::builder() — real websocket transport, publisher, signers, cache, and observer ready"
    );

    let mut group_events = fava
        .observe(group.events(Query::events().limit(128)?)?)
        .await?;
    let mut group_state = fava
        .observe(group.meta_events(SimpleGroupStateEventKind::ALL)?)
        .await?;
    let saved_query =
        saved_group_lists([alice.public_key()])?.only_from_relays([saved_relay.clone()])?;
    let mut saved_lists = fava.observe(saved_query).await?;
    println!(
        "5. Fava::observe() — group events, all six state kinds, and Alice's saved lists are live"
    );

    println!("\n6. Typed management — every call is acknowledged and then read back off the relay");
    let created = support::publish_builder(
        &fava,
        "create_group(Alice, group)",
        create_group(alice.public_key(), &group)?,
    )
    .await?;
    support::confirm_relay_copy(
        &mut group_events,
        "create_group(Alice, group)",
        created.current.id(),
        Kind::from_u16(created.current.event.kind().as_u16()),
        &[("h", group_id.clone())],
    )
    .await?;

    let metadata = edit_metadata(
        alice.public_key(),
        &group,
        &MetadataEdit {
            name: Some("Fava Relay Demo".to_owned()),
            about: Some("Created through typed Fava NIP-29 APIs".to_owned()),
            picture: Some("https://example.com/fava-demo.png".to_owned()),
            visibility: Some(GroupVisibility::Public),
            access: Some(GroupAccess::Closed),
        },
    )?;
    let management_kinds = typed_management_kinds();
    let metadata = with_supported_kinds(metadata, &management_kinds)?;
    let edited = support::publish_builder(
        &fava,
        "edit_metadata(Alice, group, name/about/picture) + supported_kinds tag",
        metadata,
    )
    .await?;
    support::confirm_relay_copy(
        &mut group_events,
        "edit_metadata(Alice, group)",
        edited.current.id(),
        edited.current.event.kind(),
        &[
            ("h", group_id.clone()),
            ("name", "Fava Relay Demo".to_owned()),
            ("about", "Created through typed Fava NIP-29 APIs".to_owned()),
        ],
    )
    .await?;

    let invite_code = format!("invite-{group_id}");
    let invited = support::publish_builder(
        &fava,
        "invite(Alice, group, code)",
        invite(alice.public_key(), &group, &invite_code)?,
    )
    .await?;
    support::confirm_relay_copy(
        &mut group_events,
        "invite(Alice, group)",
        invited.current.id(),
        invited.current.event.kind(),
        &[("h", group_id.clone()), ("code", invite_code.clone())],
    )
    .await?;

    let joined = support::publish_builder(
        &fava,
        "join_request(Bob, code)",
        join_request(bob.public_key(), &group, Some(&invite_code))?,
    )
    .await?;
    support::confirm_relay_copy(
        &mut group_events,
        "join_request(Bob)",
        joined.current.id(),
        joined.current.event.kind(),
        &[("h", group_id.clone()), ("code", invite_code.clone())],
    )
    .await?;

    let bob_member = support::publish_builder(
        &fava,
        "put_user(Alice, Bob, member)",
        put_user(alice.public_key(), &group, &[bob.public_key()], &["member"])?,
    )
    .await?;
    support::confirm_relay_copy(
        &mut group_events,
        "put_user(Alice, Bob, member)",
        bob_member.current.id(),
        bob_member.current.event.kind(),
        &[("h", group_id.clone()), ("p", bob.public_key().to_hex())],
    )
    .await?;

    support::publish_builder(
        &fava,
        "put_user(Alice, Carol, member)",
        put_user(alice.public_key(), &group, &[carol.public_key()], &["member"])?,
    )
    .await?;
    let left = support::publish_builder(
        &fava,
        "leave_group(Carol)",
        leave_group(carol.public_key(), &group)?,
    )
    .await?;
    let leave_kind = left.current.event.kind();
    support::confirm_relay_copy(
        &mut group_events,
        "leave_group(Carol)",
        left.current.id(),
        leave_kind,
        &[("h", group_id.clone())],
    )
    .await?;
    support::publish_builder(
        &fava,
        "put_user(Alice, Carol) — add her again before removal",
        put_user(alice.public_key(), &group, &[carol.public_key()], &[])?,
    )
    .await?;
    let removed = support::publish_builder(
        &fava,
        "remove_user(Alice, Carol)",
        remove_user(alice.public_key(), &group, &[carol.public_key()])?,
    )
    .await?;
    let remove_kind = removed.current.event.kind();
    support::confirm_relay_copy(
        &mut group_events,
        "remove_user(Alice, Carol)",
        removed.current.id(),
        remove_kind,
        &[("h", group_id.clone()), ("p", carol.public_key().to_hex())],
    )
    .await?;

    println!("\n7. EventBuilder::simple_group() — publish ordinary content through the group");
    let content = EventBuilder::new(alice.public_key(), Kind::TextNote)
        .content("Hello from the runnable Fava simple-groups demo")
        .simple_group(&group)?;
    let receipt = support::publish_builder(
        &fava,
        "Fava::publish(text_note.simple_group(group))",
        content,
    )
    .await?;
    let content_id = receipt.current.id();
    let content_event = receipt.current.event.clone();
    let observed = support::wait_for(&mut group_events, |snapshot| {
        snapshot
            .events
            .iter()
            .any(|record| record.id() == content_id)
    })
    .await?;
    println!(
        "   SimpleGroup::events(Query::events().limit(128)) observed {} real relay events; content {} has relay provenance",
        observed.events.len(),
        content_id
    );

    println!(
        "\n8. SimpleGroup::meta_events(ALL) — assert the state the management sequence should have produced"
    );
    let expected = ExpectedGroupState {
        id: group_id.clone(),
        name: "Fava Relay Demo".to_owned(),
        closed: true,
        admin: alice.public_key(),
        members: vec![alice.public_key(), bob.public_key()],
        former_members: vec![carol.public_key()],
    };
    println!(
        "   expected: name={:?}, closed={}, admin={}, members include Alice and Bob, Carol removed",
        expected.name,
        expected.closed,
        expected.admin.to_hex()
    );
    let settled = support::wait_for_optional(
        &mut group_state,
        support::DERIVED_STATE_TIMEOUT,
        |snapshot| support::unmet_state_expectations(snapshot, &expected).is_empty(),
    )
    .await?;
    let derives_state = if let Some(snapshot) = settled {
        support::print_group_state(&snapshot);
        println!("   every derived-state expectation above holds on the real relay");
        true
    } else {
        let current = group_state.current();
        if support::has_metadata_and_members(&current) {
            support::print_group_state(&current);
            return Err(format!(
                "step 8 — {relay} derives NIP-29 state but it does not match the published management sequence: {}",
                support::unmet_state_expectations(&current, &expected).join("; ")
            )
            .into());
        }
        println!(
            "   SKIPPED — {relay} returned no derived metadata/member state within {:?}.",
            support::DERIVED_STATE_TIMEOUT
        );
        println!(
            "   The management events above are real, acknowledged, and were read back, but this"
        );
        println!(
            "   relay does not implement NIP-29 state derivation, so these checks did not run:"
        );
        println!("      - metadata name and closed flag reflect edit_metadata");
        println!("      - Alice holds the primary role in the derived admin list");
        println!("      - the derived member list contains Alice and Bob");
        println!("      - the derived member list no longer contains Carol");
        false
    };

    println!(
        "\n9. Management refusals — the same typed calls made without the authority to make them"
    );
    if derives_state {
        support::publish_builder_rejected(
            &fava,
            "create_group(Alice, group) again",
            "the group already exists",
            create_group(alice.public_key(), &group)?,
        )
        .await?;
        support::publish_builder_rejected(
            &fava,
            "edit_metadata(Dave, group, name=Hijacked)",
            "Dave is not a member of this closed group",
            edit_metadata(
                dave.public_key(),
                &group,
                &MetadataEdit {
                    name: Some("Hijacked".to_owned()),
                    ..MetadataEdit::default()
                },
            )?,
        )
        .await?;
        support::publish_builder_rejected(
            &fava,
            "invite(Dave, group, code)",
            "Dave cannot invite anyone into a group he is not in",
            invite(dave.public_key(), &group, "demo-code")?,
        )
        .await?;
        support::publish_builder_rejected(
            &fava,
            "join_request(Dave) with no invite code",
            "the group is closed and Dave holds no code",
            join_request(dave.public_key(), &group, None)?,
        )
        .await?;
        support::publish_builder_rejected(
            &fava,
            "put_user(Alice, Bob, member) again",
            "Bob already holds exactly that role",
            put_user(alice.public_key(), &group, &[bob.public_key()], &["member"])?,
        )
        .await?;
        support::publish_builder_rejected(
            &fava,
            "remove_user(Alice, Carol) again",
            "Carol has already been removed",
            remove_user(alice.public_key(), &group, &[carol.public_key()])?,
        )
        .await?;
        support::publish_builder_rejected(
            &fava,
            "delete_event(Alice, an id the relay never stored)",
            "the target event does not exist on this relay",
            delete_event(
                alice.public_key(),
                &group,
                &EventId::from_byte_array([7u8; 32]),
            )?,
        )
        .await?;
        support::publish_builder_rejected(
            &fava,
            "delete_group(Bob)",
            "Bob is a plain member, not an admin",
            delete_group(bob.public_key(), &group)?,
        )
        .await?;
        support::publish_builder_rejected(
            &fava,
            "leave_group(Dave)",
            "Dave cannot leave a group he never joined",
            leave_group(dave.public_key(), &group)?,
        )
        .await?;
    } else {
        println!(
            "   SKIPPED — {relay} stores management events without deriving NIP-29 state, so it"
        );
        println!(
            "   does not enforce NIP-29 authority either and would acknowledge every call below."
        );
        println!(
            "   Asserting a rejection here would assert the relay's permissiveness, not the API:"
        );
        println!("      - create_group on a group that already exists");
        println!("      - edit_metadata by a non-member");
        println!("      - invite by a non-member");
        println!("      - join_request to a closed group with no invite code");
        println!("      - put_user that changes nothing");
        println!("      - remove_user targeting someone already removed");
        println!("      - delete_event targeting an event the relay never stored");
        println!("      - delete_group by a plain member");
        println!("      - leave_group by someone who never joined");
        println!("   Re-run with --spawn-croissant to exercise them.");
    }

    println!(
        "\n10. Saved group list — create, observe, rename, add/remove relay, then remove group"
    );
    support::publish_edit(
        &fava,
        &saved_relay,
        alice.public_key(),
        "save_simple_group(group, Fava Relay Demo)",
        save_simple_group(&group, Some("Fava Relay Demo"))?,
    )
    .await?;
    let saved = support::wait_for(&mut saved_lists, support::has_saved_group).await?;
    support::print_saved_lists(&saved);

    support::wait_next_second().await;
    support::publish_edit(
        &fava,
        &saved_relay,
        alice.public_key(),
        "rename_saved_simple_group(group, Renamed Demo)",
        rename_saved_simple_group(&group, "Renamed Demo")?,
    )
    .await?;
    support::wait_for(&mut saved_lists, support::has_renamed_group).await?;

    support::wait_next_second().await;
    support::publish_edit(
        &fava,
        &saved_relay,
        alice.public_key(),
        "save_relay(relay)",
        save_relay(relay.clone())?,
    )
    .await?;
    support::wait_for(&mut saved_lists, support::has_saved_relay).await?;

    support::wait_next_second().await;
    support::publish_edit(
        &fava,
        &saved_relay,
        alice.public_key(),
        "remove_saved_relay(relay)",
        remove_saved_relay(relay.clone())?,
    )
    .await?;
    support::wait_for(&mut saved_lists, |snapshot| {
        !support::has_saved_relay(snapshot)
    })
    .await?;

    support::wait_next_second().await;
    support::publish_edit(
        &fava,
        &saved_relay,
        alice.public_key(),
        "remove_saved_simple_group(group)",
        remove_saved_simple_group(&group)?,
    )
    .await?;
    support::wait_for(&mut saved_lists, |snapshot| {
        !support::has_saved_group(snapshot)
    })
    .await?;

    println!("\n11. Deletion — and the relay-visible effect of each deletion");
    let deleted_event = support::publish_builder(
        &fava,
        "delete_event(Alice, group content)",
        delete_event(alice.public_key(), &group, &content_id)?,
    )
    .await?;
    let delete_event_kind = deleted_event.current.event.kind();
    support::confirm_relay_copy(
        &mut group_events,
        "delete_event(Alice, group content)",
        deleted_event.current.id(),
        delete_event_kind,
        &[("h", group_id.clone()), ("e", content_id.to_hex())],
    )
    .await?;
    if derives_state {
        support::publish_rejected(
            &fava,
            &relay,
            "re-publish the deleted content event verbatim",
            "the relay remembers that this exact event was deleted",
            republish(&content_event)?,
        )
        .await?;
    } else {
        println!(
            "   SKIPPED — re-publishing the deleted event: {relay} does not act on kind-9005, so"
        );
        println!("   it would simply store the content again.");
    }

    let deleted_group = support::publish_builder(
        &fava,
        "delete_group(Alice, group)",
        delete_group(alice.public_key(), &group)?,
    )
    .await?;
    let delete_group_kind = deleted_group.current.event.kind();
    if derives_state {
        // A NIP-29 relay that honours kind-9008 removes the group, so it stops
        // serving the group's own events — including this one. The effect is
        // visible in what it does next, not in a read-back: every further
        // management call for this id must be refused as unknown.
        support::wait_next_second().await;
        support::publish_builder_rejected(
            &fava,
            "edit_metadata(Alice, group) after delete_group",
            "the group no longer exists on this relay",
            edit_metadata(
                alice.public_key(),
                &group,
                &MetadataEdit {
                    name: Some("Should Not Exist".to_owned()),
                    ..MetadataEdit::default()
                },
            )?,
        )
        .await?;
    } else {
        support::confirm_relay_copy(
            &mut group_events,
            "delete_group(Alice, group)",
            deleted_group.current.id(),
            delete_group_kind,
            &[("h", group_id.clone())],
        )
        .await?;
        println!(
            "   SKIPPED — confirming the group is gone: {relay} stored kind {delete_group_kind} but does not act on it."
        );
    }

    group_events.close();
    group_state.close();
    saved_lists.close();
    if derives_state {
        println!(
            "\nPASS — real keypairs, real websocket relay, typed management read back off the relay, derived NIP-29 state asserted, every management refusal exercised, live queries, saved-list materialization, and deletion completed"
        );
    } else {
        println!(
            "\nPASS (partial) — real keypairs, real websocket relay, typed management read back off the relay, live queries, saved-list materialization, and deletion completed; the derived-state and refusal checks listed above were SKIPPED because this relay does not implement NIP-29"
        );
    }
    drop(endpoint);
    Ok(())
}

/// Rebuild an already-published event byte-for-byte so it keeps its event id.
fn republish(event: &fava::EventValue) -> DemoResult<fava::UnsignedEvent> {
    Ok(EventBuilder::from_parts(
        event.author(),
        event.kind(),
        event.created_at(),
        event.tags().to_vec(),
        content_of(event),
    )
    .build()?)
}

fn content_of(event: &fava::EventValue) -> String {
    match event {
        fava::EventValue::Unsigned(unsigned) => unsigned.content.clone(),
        fava::EventValue::Signed(signed) => signed.content.clone(),
    }
}

fn with_supported_kinds(
    builder: EventBuilder,
    management_kinds: &[Kind],
) -> DemoResult<EventBuilder> {
    let mut values = vec!["supported_kinds".to_owned()];
    values.extend(
        management_kinds
            .iter()
            .chain([Kind::TextNote, Kind::Reaction].iter())
            .map(|kind| kind.as_u16().to_string()),
    );
    Ok(builder.tags([Tag::parse(values)?]))
}

fn typed_management_kinds() -> Vec<Kind> {
    vec![
        Kind::from_u16(9000),
        Kind::from_u16(9001),
        Kind::from_u16(9002),
        Kind::from_u16(9005),
        Kind::from_u16(9007),
        Kind::from_u16(9008),
        Kind::from_u16(9009),
        Kind::from_u16(9021),
        Kind::from_u16(9022),
    ]
}

fn unique_group_id() -> Result<String, Box<dyn Error + Send + Sync>> {
    let millis = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
    Ok(format!("fava-demo-{}-{millis}", std::process::id()))
}
