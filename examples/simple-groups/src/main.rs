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
//! derive NIP-29 state events. For the full create/mutate/read/delete lifecycle,
//! let the demo start a local Croissant NIP-29 relay whose owner is the generated
//! Alice key:
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
use nostr::key::Keys;

use support::{DemoResult, Options};

#[tokio::main]
#[allow(
    clippy::too_many_lines,
    reason = "one linear CLI narrative keeps each real relay action and result visible"
)]
async fn main() -> DemoResult<()> {
    let options = Options::parse()?;
    println!("Fava simple-groups real-relay demo\n");

    println!("1. Keys::generate() — create disposable Alice, Bob, and Carol keypairs");
    let alice = Keys::generate();
    let bob = Keys::generate();
    let carol = Keys::generate();
    support::print_identity("Alice", &alice);
    support::print_identity("Bob", &bob);
    support::print_identity("Carol", &carol);

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

    let fava = support::assemble(&alice, &bob, &carol)?;
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

    support::publish_event(
        &fava,
        &relay,
        "create_group(Alice, group)",
        create_group(alice.public_key(), &group)?,
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
    let management_kinds = typed_management_kinds(&group, &alice, &bob, &relay)?;
    let metadata = with_supported_kinds(metadata, &management_kinds)?;
    support::publish_event(
        &fava,
        &relay,
        "edit_metadata(Alice, group, name/about/picture) + supported_kinds tag",
        metadata,
    )
    .await?;

    let invite_code = format!("invite-{group_id}");
    let invitation = append_tags(
        invite(alice.public_key(), &group, &bob.public_key(), &relay)?,
        [Tag::parse(["code", &invite_code])?],
    )?;
    support::publish_event(
        &fava,
        &relay,
        "invite(Alice, Bob) + relay-required code tag",
        invitation,
    )
    .await?;
    let bob_join = append_tags(
        join_request(bob.public_key(), &group)?,
        [Tag::parse(["code", &invite_code])?],
    )?;
    support::publish_event(&fava, &relay, "join_request(Bob) + invite code", bob_join).await?;
    support::publish_event(
        &fava,
        &relay,
        "put_user(Alice, Bob, member)",
        put_user(alice.public_key(), &group, &bob.public_key(), &["member"])?,
    )
    .await?;
    support::publish_event(
        &fava,
        &relay,
        "put_user(Alice, Carol, member)",
        put_user(alice.public_key(), &group, &carol.public_key(), &["member"])?,
    )
    .await?;
    support::publish_event(
        &fava,
        &relay,
        "leave_group(Carol)",
        leave_group(carol.public_key(), &group)?,
    )
    .await?;
    support::publish_event(
        &fava,
        &relay,
        "put_user(Alice, Carol) — add her again before removal",
        put_user(alice.public_key(), &group, &carol.public_key(), &[])?,
    )
    .await?;
    support::publish_event(
        &fava,
        &relay,
        "remove_user(Alice, Carol)",
        remove_user(alice.public_key(), &group, &carol.public_key())?,
    )
    .await?;

    println!("\n6. EventBuilder::simple_group() — publish ordinary content through the group");
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

    println!("\n7. SimpleGroup::meta_events(ALL) — decode relay-authoritative group state");
    match support::wait_for_optional(&mut group_state, support::has_metadata_and_members).await? {
        Some(snapshot) => support::print_group_state(&snapshot),
        None => println!(
            "   relay returned no derived metadata/member state; management events are real and stored, but this relay does not implement NIP-29 state derivation"
        ),
    }

    println!(
        "\n8. Saved group list — create, observe, rename, add/remove relay, then remove group"
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

    support::publish_event(
        &fava,
        &relay,
        "delete_event(Alice, group content)",
        delete_event(alice.public_key(), &group, &content_id)?,
    )
    .await?;
    support::publish_event(
        &fava,
        &relay,
        "delete_group(Alice, group)",
        delete_group(alice.public_key(), &group)?,
    )
    .await?;

    group_events.close();
    group_state.close();
    saved_lists.close();
    println!(
        "\nPASS — real keypairs, real websocket relay, typed management, live queries, saved-list materialization, and deletion completed"
    );
    drop(endpoint);
    Ok(())
}

fn with_supported_kinds(
    event: fava::UnsignedEvent,
    management_kinds: &[Kind],
) -> DemoResult<fava::UnsignedEvent> {
    let mut values = vec!["supported_kinds".to_owned()];
    values.extend(
        management_kinds
            .iter()
            .chain([Kind::TextNote, Kind::Reaction].iter())
            .map(|kind| kind.as_u16().to_string()),
    );
    append_tags(event, [Tag::parse(values)?])
}

fn typed_management_kinds(
    group: &SimpleGroup,
    alice: &Keys,
    bob: &Keys,
    relay: &fava::RelayUrl,
) -> DemoResult<Vec<Kind>> {
    let create = create_group(alice.public_key(), group)?;
    let target = create.compute_id();
    Ok(vec![
        create.kind,
        edit_metadata(alice.public_key(), group, &MetadataEdit::default())?.kind,
        invite(alice.public_key(), group, &bob.public_key(), relay)?.kind,
        join_request(bob.public_key(), group)?.kind,
        put_user(alice.public_key(), group, &bob.public_key(), &[])?.kind,
        remove_user(alice.public_key(), group, &bob.public_key())?.kind,
        delete_event(alice.public_key(), group, &target)?.kind,
        delete_group(alice.public_key(), group)?.kind,
        leave_group(bob.public_key(), group)?.kind,
    ])
}

fn append_tags(
    event: fava::UnsignedEvent,
    extra: impl IntoIterator<Item = Tag>,
) -> DemoResult<fava::UnsignedEvent> {
    let mut tags = event.tags.to_vec();
    tags.extend(extra);
    Ok(EventBuilder::from_parts(
        event.pubkey,
        event.kind,
        event.created_at,
        tags,
        event.content,
    )
    .build()?)
}

fn unique_group_id() -> Result<String, Box<dyn Error + Send + Sync>> {
    let millis = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
    Ok(format!("fava-demo-{}-{millis}", std::process::id()))
}
