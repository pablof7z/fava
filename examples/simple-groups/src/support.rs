use std::error::Error;
use std::fmt::Write as _;
use std::future::Future;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use fava::{
    EventBuilder, EventValue, Fava, Kind, Observation, PublicKey, QuerySnapshot, Receipt,
    RelayDeliveryOutcome, RelayUrl, ReplaceableEventEdit, Tag, UnsignedEvent, all_terminal,
};
use fava_delivery_standard::StandardDeliveryPolicy;
use fava_publisher_nip01::Nip01Publisher;
use fava_query_standard::StandardQueryEvaluator;
use fava_signer_local::LocalSigner;
use fava_simple_groups::{
    SavedGroupList, SimpleGroupAdmins, SimpleGroupLivekitParticipants, SimpleGroupMembers,
    SimpleGroupMetadata, SimpleGroupPins, SimpleGroupRoles, saved_group_list_materializer,
};
use fava_subscriptions_no_grouping::planner;
use fava_transport_websocket::WebSocketTransport;
use fava_write_store_memory::MemoryWriteStore;
use nostr::event::EventId;
use nostr::key::Keys;

pub(super) type DemoResult<T> = Result<T, Box<dyn Error + Send + Sync>>;
const OPERATION_TIMEOUT: Duration = Duration::from_secs(20);
/// How long the demo waits for a relay that derives NIP-29 state to publish it.
/// A generic relay never will, so the wait is bounded and its expiry is reported
/// as a skip rather than a failure.
pub(super) const DERIVED_STATE_TIMEOUT: Duration = Duration::from_secs(10);

pub(super) struct Options {
    relay: String,
    saved_relay: Option<String>,
    croissant: Option<PathBuf>,
}

impl Options {
    pub(super) fn parse() -> DemoResult<Self> {
        let mut relay = "ws://127.0.0.1:8080".to_owned();
        let mut relay_was_set = false;
        let mut saved_relay = None;
        let mut croissant = None;
        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--relay" => {
                    relay = args.next().ok_or("--relay requires a WebSocket URL")?;
                    relay_was_set = true;
                }
                "--spawn-croissant" => {
                    croissant = Some(PathBuf::from(
                        args.next()
                            .ok_or("--spawn-croissant requires a binary path")?,
                    ));
                }
                "--saved-relay" => {
                    saved_relay = Some(
                        args.next()
                            .ok_or("--saved-relay requires a WebSocket URL")?,
                    );
                }
                "-h" | "--help" => {
                    println!(
                        "Usage: simple-groups [--relay ws://127.0.0.1:8080] [--saved-relay ws://127.0.0.1:8080] [--spawn-croissant /path/to/croissant]\n\n--relay connects to an existing group relay.\n--saved-relay selects the user relay for kind-10009 saved lists (defaults to --relay).\n--spawn-croissant starts an isolated NIP-29 group relay owned by generated Alice."
                    );
                    std::process::exit(0);
                }
                _ => return Err(format!("unknown argument: {arg}").into()),
            }
        }
        if relay_was_set && croissant.is_some() {
            return Err("--relay and --spawn-croissant are mutually exclusive".into());
        }
        Ok(Self {
            relay,
            saved_relay,
            croissant,
        })
    }

    pub(super) fn saved_relay(&self, group_relay: &RelayUrl) -> DemoResult<RelayUrl> {
        match &self.saved_relay {
            Some(relay) => Ok(RelayUrl::parse(relay)?),
            None => Ok(group_relay.clone()),
        }
    }
}

pub(super) struct RelayEndpoint {
    url: RelayUrl,
    child: Option<ChildRelay>,
}

impl RelayEndpoint {
    pub(super) async fn open(options: &Options, owner: PublicKey) -> DemoResult<Self> {
        if let Some(binary) = &options.croissant {
            let child = ChildRelay::start(binary, owner).await?;
            let url = RelayUrl::parse(&child.url)?;
            return Ok(Self {
                url,
                child: Some(child),
            });
        }
        Ok(Self {
            url: RelayUrl::parse(&options.relay)?,
            child: None,
        })
    }

    pub(super) const fn url(&self) -> &RelayUrl {
        &self.url
    }
}

impl Drop for RelayEndpoint {
    fn drop(&mut self) {
        if self.child.is_some() {
            println!("   stopped isolated Croissant relay and removed its temporary data");
        }
    }
}

struct ChildRelay {
    process: Child,
    data: PathBuf,
    url: String,
}

impl ChildRelay {
    async fn start(binary: &Path, owner: PublicKey) -> DemoResult<Self> {
        if !binary.is_file() {
            return Err(format!("Croissant binary not found: {}", binary.display()).into());
        }
        let port = free_port()?;
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let data = std::env::temp_dir().join(format!("fava-demo-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&data)?;
        let working_directory = binary.parent().unwrap_or_else(|| Path::new("."));
        let process = Command::new(binary)
            .current_dir(working_directory)
            .env("HOST", "127.0.0.1")
            .env("PORT", port.to_string())
            .env("DATAPATH", &data)
            .env("OWNER_PUBLIC_KEY", owner.to_hex())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        let url = format!("ws://127.0.0.1:{port}");
        let mut child = Self { process, data, url };
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        loop {
            if tokio::net::TcpStream::connect(("127.0.0.1", port))
                .await
                .is_ok()
            {
                break;
            }
            if let Some(status) = child.process.try_wait()? {
                return Err(format!("Croissant exited during startup with {status}").into());
            }
            if tokio::time::Instant::now() >= deadline {
                return Err("Croissant readiness deadline elapsed".into());
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        println!(
            "   spawned isolated Croissant pid={} at {}",
            child.process.id(),
            child.url
        );
        Ok(child)
    }
}

impl Drop for ChildRelay {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
        let _ = std::fs::remove_dir_all(&self.data);
    }
}

fn free_port() -> DemoResult<u16> {
    Ok(TcpListener::bind("127.0.0.1:0")?.local_addr()?.port())
}

pub(super) fn print_identity(name: &str, keys: &Keys) {
    println!("   {name}: {}", keys.public_key().to_hex());
}

pub(super) fn assemble(alice: &Keys, bob: &Keys, carol: &Keys, dave: &Keys) -> DemoResult<Fava> {
    Ok(Fava::builder()
        .event_cache_ephemeral()
        .write_store(Arc::new(MemoryWriteStore::default()))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .subscription_planner(Arc::new(planner()))
        .transport(Arc::new(WebSocketTransport::new()))
        .publisher(Arc::new(Nip01Publisher))
        .delivery_policy(Arc::new(StandardDeliveryPolicy::default()))
        .signer(Arc::new(LocalSigner::new(alice.clone())))
        .signer(Arc::new(LocalSigner::new(bob.clone())))
        .signer(Arc::new(LocalSigner::new(carol.clone())))
        .signer(Arc::new(LocalSigner::new(dave.clone())))
        .materializers([saved_group_list_materializer()])
        .build()?)
}

/// Publish to `relay` and require exact relay *rejection* evidence.
///
/// `expected` states, in the demo's own words, why the relay should refuse. The
/// relay's verbatim message is printed next to it, so a management API that
/// silently starts accepting an unauthorized action fails here loudly.
pub(super) async fn publish_rejected(
    fava: &Fava,
    relay: &RelayUrl,
    label: &str,
    expected: &str,
    event: UnsignedEvent,
) -> DemoResult<String> {
    let event_id = event.compute_id();
    let kind = event.kind;
    println!("   {label}");
    println!("      expected: {expected}");
    let write = fava.to([relay.clone()])?.publish(event)?;
    let receipt = settle(label, write.settled(all_terminal())).await?;
    let Some(message) = rejection_message(&receipt) else {
        return Err(format!(
            "{label} — expected {relay} to reject kind {kind} ({event_id}) because {expected}; it returned {}",
            outcomes(&receipt)
        )
        .into());
    };
    if message.is_empty() {
        return Err(
            format!("{label} — {relay} rejected kind {kind} without stating a reason").into(),
        );
    }
    println!("      relay said: {message}");
    Ok(message.to_owned())
}

pub(super) async fn publish_builder(
    fava: &Fava,
    label: &str,
    builder: EventBuilder,
) -> DemoResult<Receipt> {
    println!("   {label}");
    let write = fava.publish(builder)?;
    let receipt = settle(label, write.settled(all_terminal())).await?;
    if receipt.acknowledged() == 0 {
        return Err(format!(
            "{label} — expected a relay acknowledgement; destinations returned {}",
            outcomes(&receipt)
        )
        .into());
    }
    println!(
        "      event={}, acknowledged={}, rejected={}",
        receipt.current.id(),
        receipt.acknowledged(),
        receipt.rejected()
    );
    Ok(receipt)
}

pub(super) async fn publish_builder_rejected(
    fava: &Fava,
    label: &str,
    expected: &str,
    builder: EventBuilder,
) -> DemoResult<String> {
    println!("   {label}");
    println!("      expected: {expected}");
    let write = fava.publish(builder)?;
    let receipt = settle(label, write.settled(all_terminal())).await?;
    let Some(message) = rejection_message(&receipt) else {
        return Err(format!(
            "{label} — expected rejection because {expected}; it returned {}",
            outcomes(&receipt)
        )
        .into());
    };
    if message.is_empty() {
        return Err(format!("{label} — relay rejected without stating a reason").into());
    }
    println!("      relay said: {message}");
    Ok(message.to_owned())
}

pub(super) async fn publish_edit(
    fava: &Fava,
    relay: &RelayUrl,
    author: PublicKey,
    label: &str,
    edit: ReplaceableEventEdit,
) -> DemoResult<Receipt> {
    println!("   {label}");
    let write = fava.by(author).to([relay.clone()])?.publish(edit)?;
    let write_id = write.write_id().as_u64();
    let receipt = settle(label, write.settled(all_terminal())).await?;
    if receipt.acknowledged() == 0 {
        return Err(format!(
            "{label} — expected {relay} to acknowledge write {write_id}; it returned {}",
            outcomes(&receipt)
        )
        .into());
    }
    println!(
        "      write={write_id}, acknowledged={}, rejected={}",
        receipt.acknowledged(),
        receipt.rejected()
    );
    Ok(receipt)
}

/// Bound one settlement wait and name the step that stalled.
async fn settle(
    label: &str,
    settled: impl Future<Output = Result<Receipt, fava::PublishError>>,
) -> DemoResult<Receipt> {
    match tokio::time::timeout(OPERATION_TIMEOUT, settled).await {
        Ok(result) => Ok(result?),
        Err(_) => Err(format!(
            "{label} — no destination reached a terminal fact within {OPERATION_TIMEOUT:?}"
        )
        .into()),
    }
}

/// Every current destination fact, verbatim, for a failure message.
fn outcomes(receipt: &Receipt) -> String {
    if receipt.destinations().is_empty() {
        return "no destination facts at all".to_owned();
    }
    receipt
        .destinations()
        .values()
        .map(|outcome| format!("{outcome:?}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The relay's verbatim refusal, when some destination rejected the event.
fn rejection_message(receipt: &Receipt) -> Option<&str> {
    receipt
        .destinations()
        .values()
        .find_map(|outcome| match outcome {
            RelayDeliveryOutcome::Rejected { message } => Some(message.as_str()),
            _ => None,
        })
}

/// Wait until the relay serves `id` back, then check the stored copy.
///
/// Presence in a snapshot is not enough: Fava also surfaces the application's
/// own local write, so this requires at least one relay occurrence before it
/// reads the kind and tags off the returned copy.
pub(super) async fn confirm_relay_copy(
    observation: &mut Observation,
    label: &str,
    id: EventId,
    kind: Kind,
    expected_tags: &[(&str, String)],
) -> DemoResult<()> {
    let snapshot = match tokio::time::timeout(OPERATION_TIMEOUT, async {
        loop {
            let current = observation.current();
            if current
                .events
                .iter()
                .any(|record| record.id() == id && !record.relay_occurrences().is_empty())
            {
                return Ok::<_, Box<dyn Error + Send + Sync>>(current);
            }
            observation.changed().await?;
        }
    })
    .await
    {
        Ok(result) => result?,
        Err(_) => {
            return Err(format!(
                "{label} — the relay never served kind {kind} ({id}) back within {OPERATION_TIMEOUT:?}; it was acknowledged but its effect is unconfirmed"
            )
            .into());
        }
    };

    let record = snapshot
        .events
        .iter()
        .find(|record| record.id() == id)
        .ok_or_else(|| format!("{label} — {id} left the snapshot before it could be read"))?;
    let event = record.event();
    if event.kind() != kind {
        return Err(format!(
            "{label} — the relay copy of {id} has kind {}, expected kind {kind}",
            event.kind()
        )
        .into());
    }
    for (name, value) in expected_tags {
        if !has_tag_value(event, name, value) {
            return Err(format!(
                "{label} — the relay copy of {id} does not carry [\"{name}\", \"{value}\"]; it carries {:?}",
                event.tags().iter().map(Tag::as_slice).collect::<Vec<_>>()
            )
            .into());
        }
    }
    let mut checked = String::new();
    for (name, value) in expected_tags {
        write!(checked, ", {name}={value}")?;
    }
    println!(
        "      relay served it back from {} session(s): kind={kind}{checked}",
        record.relay_occurrences().len()
    );
    Ok(())
}

fn has_tag_value(event: &EventValue, name: &str, value: &str) -> bool {
    event.tags().iter().any(|tag| {
        let values = tag.as_slice();
        values.first().map(String::as_str) == Some(name)
            && values.get(1).map(String::as_str) == Some(value)
    })
}

pub(super) async fn wait_for(
    observation: &mut Observation,
    predicate: impl Fn(&QuerySnapshot) -> bool,
) -> DemoResult<Arc<QuerySnapshot>> {
    tokio::time::timeout(OPERATION_TIMEOUT, async {
        loop {
            let current = observation.current();
            if predicate(&current) {
                return Ok::<_, Box<dyn Error + Send + Sync>>(current);
            }
            observation.changed().await?;
        }
    })
    .await?
}

pub(super) async fn wait_for_optional(
    observation: &mut Observation,
    within: Duration,
    predicate: impl Fn(&QuerySnapshot) -> bool,
) -> DemoResult<Option<Arc<QuerySnapshot>>> {
    match tokio::time::timeout(within, async {
        loop {
            let current = observation.current();
            if predicate(&current) {
                return Ok::<_, Box<dyn Error + Send + Sync>>(current);
            }
            observation.changed().await?;
        }
    })
    .await
    {
        Ok(result) => result.map(Some),
        Err(_) => Ok(None),
    }
}

pub(super) fn has_metadata_and_members(snapshot: &QuerySnapshot) -> bool {
    snapshot
        .events
        .iter()
        .any(|record| SimpleGroupMetadata::from_event(record.event()).is_ok())
        && snapshot
            .events
            .iter()
            .any(|record| SimpleGroupMembers::from_event(record.event()).is_ok())
}

/// The group state the published management sequence should have produced on a
/// relay that derives NIP-29 state.
pub(super) struct ExpectedGroupState {
    /// Group id the state events must be addressed to (`d` tag).
    pub(super) id: String,
    /// Name the last `edit_metadata` set.
    pub(super) name: String,
    /// Whether `edit_metadata` left the group closed.
    pub(super) closed: bool,
    /// Author who must hold the primary role in the kind-39001 admin list.
    pub(super) admin: PublicKey,
    /// Pubkeys the kind-39002 member list must contain.
    pub(super) members: Vec<PublicKey>,
    /// Pubkeys the kind-39002 member list must no longer contain.
    pub(super) former_members: Vec<PublicKey>,
}

/// Every expectation the current derived state does not yet meet.
///
/// An empty result is the assertion passing. A non-empty result is either a
/// state the relay has not converged on yet, or — once the wait expires — the
/// exact list of things that are wrong, ready to print.
pub(super) fn unmet_state_expectations(
    snapshot: &QuerySnapshot,
    expected: &ExpectedGroupState,
) -> Vec<String> {
    let mut unmet = Vec::new();

    match snapshot
        .events
        .iter()
        .filter_map(|record| SimpleGroupMetadata::from_event(record.event()).ok())
        .find(|metadata| metadata.id() == expected.id)
    {
        None => unmet.push(format!("no group metadata state for d={}", expected.id)),
        Some(metadata) => {
            if metadata.name() != Some(expected.name.as_str()) {
                unmet.push(format!(
                    "metadata name is {:?}, expected {:?}",
                    metadata.name(),
                    expected.name
                ));
            }
            if metadata.is_closed() != expected.closed {
                unmet.push(format!(
                    "metadata closed is {}, expected {}",
                    metadata.is_closed(),
                    expected.closed
                ));
            }
        }
    }

    match snapshot
        .events
        .iter()
        .filter_map(|record| SimpleGroupMembers::from_event(record.event()).ok())
        .find(|members| members.id() == expected.id)
    {
        None => unmet.push(format!("no member state for d={}", expected.id)),
        Some(state) => {
            let listed: Vec<&str> = state
                .members()
                .iter()
                .filter_map(|entry| entry.as_deref().ok())
                .collect();
            for member in &expected.members {
                let hex = member.to_hex();
                if !listed.contains(&hex.as_str()) {
                    unmet.push(format!("member list is missing {hex}"));
                }
            }
            for former in &expected.former_members {
                let hex = former.to_hex();
                if listed.contains(&hex.as_str()) {
                    unmet.push(format!("member list still contains {hex}"));
                }
            }
        }
    }

    match snapshot
        .events
        .iter()
        .filter_map(|record| SimpleGroupAdmins::from_event(record.event()).ok())
        .find(|admins| admins.id() == expected.id)
    {
        None => unmet.push(format!("no admin state for d={}", expected.id)),
        Some(state) => {
            let hex = expected.admin.to_hex();
            let held: Option<&Vec<String>> = state
                .admins()
                .iter()
                .filter_map(|entry| entry.as_ref().ok())
                .find(|(pubkey, _)| pubkey == &hex)
                .map(|(_, roles)| roles);
            match held {
                None => unmet.push(format!("admin list does not contain {hex}")),
                Some(roles) if roles.is_empty() => {
                    unmet.push(format!("admin list gives {hex} no role"));
                }
                Some(_) => {}
            }
        }
    }

    unmet
}

pub(super) fn print_group_state(snapshot: &QuerySnapshot) {
    for record in snapshot.events.iter() {
        if let Ok(value) = SimpleGroupMetadata::from_event(record.event()) {
            println!(
                "   SimpleGroupMetadata: id={} author={} name={:?} picture={:?} banner={:?} about={:?}",
                value.id(),
                value.author(),
                value.name(),
                value.picture(),
                value.banner(),
                value.about()
            );
            println!(
                "      private={} restricted={} hidden={} closed={} livekit={} supported_kinds={:?} parent={:?} children={:?}",
                value.is_private(),
                value.is_restricted(),
                value.is_hidden(),
                value.is_closed(),
                value.has_livekit(),
                value.supported_kinds(),
                value.parent(),
                value.children()
            );
        }
        if let Ok(value) = SimpleGroupAdmins::from_event(record.event()) {
            println!(
                "   SimpleGroupAdmins: id={} author={} admins={:?}",
                value.id(),
                value.author(),
                value.admins()
            );
        }
        if let Ok(value) = SimpleGroupMembers::from_event(record.event()) {
            println!(
                "   SimpleGroupMembers: id={} author={} members={:?}",
                value.id(),
                value.author(),
                value.members()
            );
        }
        if let Ok(value) = SimpleGroupRoles::from_event(record.event()) {
            println!(
                "   SimpleGroupRoles: id={} author={} roles={:?}",
                value.id(),
                value.author(),
                value.roles()
            );
        }
        if let Ok(value) = SimpleGroupLivekitParticipants::from_event(record.event()) {
            println!(
                "   SimpleGroupLivekitParticipants: id={} author={} participants={:?}",
                value.id(),
                value.author(),
                value.participants()
            );
        }
        if let Ok(value) = SimpleGroupPins::from_event(record.event()) {
            println!(
                "   SimpleGroupPins: id={} author={} pins={:?}",
                value.id(),
                value.author(),
                value.pins()
            );
        }
    }
}

pub(super) fn has_saved_group(snapshot: &QuerySnapshot) -> bool {
    snapshot.events.iter().any(|record| {
        SavedGroupList::from_event(record.event())
            .is_ok_and(|list| list.simple_groups().iter().any(Result::is_ok))
    })
}

pub(super) fn has_renamed_group(snapshot: &QuerySnapshot) -> bool {
    snapshot.events.iter().any(|record| {
        SavedGroupList::from_event(record.event()).is_ok_and(|list| {
            list.simple_groups().iter().any(|entry| {
                entry
                    .as_ref()
                    .is_ok_and(|saved| saved.display_name() == Some("Renamed Demo"))
            })
        })
    })
}

pub(super) fn has_saved_relay(snapshot: &QuerySnapshot) -> bool {
    snapshot.events.iter().any(|record| {
        SavedGroupList::from_event(record.event())
            .is_ok_and(|list| list.relays().iter().any(Result::is_ok))
    })
}

pub(super) fn print_saved_lists(snapshot: &QuerySnapshot) {
    for record in snapshot.events.iter() {
        let Ok(list) = SavedGroupList::from_event(record.event()) else {
            continue;
        };
        println!("   SavedGroupList::author() = {}", list.author());
        for entry in list.simple_groups() {
            match entry {
                Ok(group) => println!(
                    "      group id={} relay={} display_name={:?}",
                    group.id(),
                    group.relay(),
                    group.display_name()
                ),
                Err(error) => println!("      malformed group entry: {error}"),
            }
        }
        println!("      relays={:?}", list.relays());
    }
}

pub(super) async fn wait_next_second() {
    tokio::time::sleep(Duration::from_millis(1_100)).await;
}
