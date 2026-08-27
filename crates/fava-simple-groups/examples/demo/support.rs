use std::error::Error;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use fava::{
    Fava, Observation, PublicKey, QuerySnapshot, Receipt, RelayUrl, ReplaceableEventEdit,
    UnsignedEvent, at_least,
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
use nostr::key::Keys;

pub(super) type DemoResult<T> = Result<T, Box<dyn Error + Send + Sync>>;
const OPERATION_TIMEOUT: Duration = Duration::from_secs(20);
const OPTIONAL_STATE_TIMEOUT: Duration = Duration::from_secs(3);

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
                        "Usage: demo [--relay ws://127.0.0.1:8080] [--saved-relay ws://127.0.0.1:8080] [--spawn-croissant /path/to/croissant]\n\n--relay connects to an existing group relay.\n--saved-relay selects the user relay for kind-10009 saved lists (defaults to --relay).\n--spawn-croissant starts an isolated NIP-29 group relay owned by generated Alice."
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

pub(super) fn assemble(alice: &Keys, bob: &Keys, carol: &Keys) -> DemoResult<Fava> {
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
        .materializers([saved_group_list_materializer()])
        .build()?)
}

pub(super) async fn publish_event(
    fava: &Fava,
    relay: &RelayUrl,
    label: &str,
    event: UnsignedEvent,
) -> DemoResult<Receipt> {
    let event_id = event.compute_id();
    let kind = event.kind;
    println!("   {label}");
    let write = fava.to([relay.clone()])?.publish(event)?;
    let receipt = tokio::time::timeout(OPERATION_TIMEOUT, write.settled(at_least(1)?)).await??;
    println!(
        "      kind={kind}, event={event_id}, acknowledged={}, rejected={}",
        receipt.acknowledged(),
        receipt.rejected()
    );
    Ok(receipt)
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
    let receipt = tokio::time::timeout(OPERATION_TIMEOUT, write.settled(at_least(1)?)).await??;
    println!(
        "      write={}, acknowledged={}, rejected={}",
        write.write_id().as_u64(),
        receipt.acknowledged(),
        receipt.rejected()
    );
    Ok(receipt)
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
    predicate: impl Fn(&QuerySnapshot) -> bool,
) -> DemoResult<Option<Arc<QuerySnapshot>>> {
    match tokio::time::timeout(OPTIONAL_STATE_TIMEOUT, async {
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
