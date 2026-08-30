//! Account-reactive commands expressed only through public Fava APIs.

use std::collections::BTreeMap;
use std::time::Duration;

use e2e_support::{CommandResult, E2eSession, ResultValue, ShellError};
use fava::{
    EventBuilder, EventValue, Fava, Kind, Observation, Query, QuerySnapshot, Receipt, ReceiptId,
    all_terminal,
};

const OPERATION_TIMEOUT: Duration = Duration::from_secs(20);

pub(crate) struct App {
    fava: Fava,
    observations: BTreeMap<String, Observation>,
}

impl App {
    pub(crate) fn new(fava: Fava) -> Self {
        Self {
            fava,
            observations: BTreeMap::new(),
        }
    }

    pub(crate) fn query_count(&self) -> usize {
        self.observations.len()
    }

    pub(crate) fn execute<P>(
        &mut self,
        session: &E2eSession,
        words: &[String],
        prompt: &mut P,
    ) -> Result<CommandResult, ShellError>
    where
        P: FnMut(&str) -> Result<Option<String>, ShellError>,
    {
        match words {
            [command, arguments @ ..] if command == "publish" => {
                const USAGE: &str = "publish <kind> <content> <relay> [relay ...]";
                let kind = required(arguments, 0, "event-kind", USAGE, prompt)?;
                let content = required(arguments, 1, "event-content", USAGE, prompt)?;
                let first_relay = required(arguments, 2, "relay-alias", USAGE, prompt)?;
                let mut relays = vec![first_relay];
                relays.extend(arguments.iter().skip(3).cloned());
                self.publish(session, &kind, &content, &relays)
            }
            [command, action, arguments @ ..] if command == "query" => {
                let arguments = query_arguments(action, arguments, prompt)?;
                self.query(session, action, &arguments)
            }
            [command, action, arguments @ ..] if command == "receipt" => {
                let arguments = receipt_arguments(action, arguments, prompt)?;
                self.receipt(action, &arguments)
            }
            [command] if command == "diagnostics" => self.diagnostics(),
            [command] if command == "routes" => self.routes(),
            [command] if command == "help" => Ok(help()),
            _ => Err(ShellError::UnknownCommand {
                command: words.join(" "),
            }),
        }
    }

    fn publish(
        &self,
        session: &E2eSession,
        kind: &str,
        content: &str,
        relay_aliases: &[String],
    ) -> Result<CommandResult, ShellError> {
        const USAGE: &str = "publish <kind> <content> <relay> [relay ...]";
        if relay_aliases.is_empty() {
            return Err(ShellError::Usage { usage: USAGE });
        }
        let kind = parse_kind(kind, USAGE)?;
        let relays = relay_aliases
            .iter()
            .map(|alias| session.relay(alias).cloned())
            .collect::<Result<Vec<_>, _>>()?;
        let write = self
            .fava
            .to(relays)
            .map_err(domain)?
            .publish(EventBuilder::new(kind).content(content))
            .map_err(domain)?;
        let receipt = block_on(async {
            match tokio::time::timeout(OPERATION_TIMEOUT, write.settled(all_terminal())).await {
                Ok(Ok(receipt) | Err(fava::PublishError::NotReached { receipt })) => Ok(receipt),
                Ok(Err(error)) => Err(error.to_string()),
                Err(_) => write.receipt().map_err(|error| error.to_string()),
            }
        })
        .map_err(ShellError::Domain)?;
        receipt_result("published", "publication settled", &receipt)
    }

    fn query(
        &mut self,
        session: &E2eSession,
        action: &str,
        arguments: &[String],
    ) -> Result<CommandResult, ShellError> {
        match (action, arguments) {
            ("open", [name, binding, kind, relay_aliases @ ..])
                if binding == "$currentPubkey" && !relay_aliases.is_empty() =>
            {
                if self.observations.contains_key(name) {
                    return Err(ShellError::Domain(format!(
                        "query {name:?} is already open"
                    )));
                }
                let kind = parse_kind(
                    kind,
                    "query open <name> $currentPubkey <kind> <relay> [relay ...]",
                )?;
                let relays = relay_aliases
                    .iter()
                    .map(|alias| session.relay(alias).cloned())
                    .collect::<Result<Vec<_>, _>>()?;
                let query = Query::events()
                    .authors_current_account()
                    .kinds([kind])
                    .map_err(domain)?
                    .only_from_relays(relays)
                    .map_err(domain)?;
                let observation = block_on(self.fava.observe(query)).map_err(domain)?;
                let id = observation.id().get().get();
                let snapshot = observation.current();
                self.observations.insert(name.clone(), observation);
                CommandResult::success("query-opened", format!("opened {name}"))
                    .with_field("query", name.as_str())?
                    .with_field("observation_id", id)?
                    .with_field("revision", snapshot.revision.0)?
                    .with_field("event_count", snapshot.events.len())
            }
            ("snapshot", [name]) => {
                let observation = self.observation(name)?;
                snapshot_result(name, observation.id().get().get(), &observation.current())
            }
            ("sync", [name]) => {
                let observation = self.observation_mut(name)?;
                let snapshot = block_on(observation.synchronize_current_account(OPERATION_TIMEOUT))
                    .map_err(domain)?
                    .ok_or_else(|| ShellError::Domain(format!("query {name:?} sync timed out")))?;
                snapshot_result(name, observation.id().get().get(), &snapshot)
            }
            ("wait", [name, count]) => {
                let count = count.parse::<usize>().map_err(|_| ShellError::Usage {
                    usage: "query wait <name> <minimum-count>",
                })?;
                let observation = self.observation_mut(name)?;
                let snapshot = block_on(
                    observation
                        .wait_until(OPERATION_TIMEOUT, |snapshot| snapshot.events.len() >= count),
                )
                .map_err(domain)?
                .ok_or_else(|| ShellError::Domain(format!("query {name:?} wait timed out")))?;
                snapshot_result(name, observation.id().get().get(), &snapshot)
            }
            ("close", [name]) => {
                let observation = self
                    .observations
                    .remove(name)
                    .ok_or_else(|| ShellError::Domain(format!("unknown query {name:?}")))?;
                let id = observation.id().get().get();
                observation.close();
                CommandResult::success("query-closed", format!("closed {name}"))
                    .with_field("query", name.as_str())?
                    .with_field("observation_id", id)
            }
            _ => Err(ShellError::Usage {
                usage: "query open <name> $currentPubkey <kind> <relay>... | query <snapshot|sync|wait|close> ...",
            }),
        }
    }

    fn receipt(&self, action: &str, arguments: &[String]) -> Result<CommandResult, ShellError> {
        match (action, arguments) {
            ("list", []) => {
                let receipts = self.fava.open_receipts().map_err(domain)?;
                CommandResult::success("receipt-list", "open receipts").with_field(
                    "receipt_ids",
                    ResultValue::array(
                        receipts
                            .iter()
                            .map(|receipt| ResultValue::from(receipt.receipt_id.as_u64())),
                    ),
                )
            }
            ("show", [id]) => {
                let id = parse_receipt_id(id)?;
                let receipt = self.fava.receipt(id).map_err(domain)?.ok_or_else(|| {
                    ShellError::Domain(format!("unknown receipt {}", id.as_u64()))
                })?;
                receipt_result("receipt", "receipt state", &receipt)
            }
            _ => Err(ShellError::Usage {
                usage: "receipt <list|show> ...",
            }),
        }
    }

    fn diagnostics(&self) -> Result<CommandResult, ShellError> {
        let (current, revision) = self.fava.current_account_snapshot();
        let diagnostics = self.fava.diagnostics();
        let query_ids = diagnostics
            .queries
            .iter()
            .map(|query| ResultValue::from(query.observation.get().get()));
        let accounts = self.fava.accounts();
        let account_keys = accounts.iter().map(|key| ResultValue::text(key.to_hex()));
        let signer_statuses: Vec<_> = accounts
            .iter()
            .map(|key| self.fava.signer_status(*key))
            .collect();
        let signer_generations = signer_statuses.iter().map(|status| {
            status.map_or_else(
                || ResultValue::text(""),
                |(generation, _)| ResultValue::from(generation),
            )
        });
        let signer_availability = signer_statuses.iter().map(|status| {
            status.map_or_else(
                || ResultValue::text("pubkey-only"),
                |(_, availability)| ResultValue::text(format!("{availability:?}")),
            )
        });
        CommandResult::success("diagnostics", "current Fava ownership facts")
            .with_field(
                "current_pubkey",
                current.map_or_else(String::new, |key| key.to_hex()),
            )?
            .with_field("selection_revision", revision)?
            .with_field("session_revision", self.fava.session_revision())?
            .with_field("account_pubkeys", ResultValue::array(account_keys))?
            .with_field("signer_generations", ResultValue::array(signer_generations))?
            .with_field(
                "signer_availability",
                ResultValue::array(signer_availability),
            )?
            .with_field("query_ids", ResultValue::array(query_ids))?
            .with_field("query_count", diagnostics.queries.len())?
            .with_field("relay_count", diagnostics.relays.len())?
            .with_field("write_count", diagnostics.writes.len())
    }

    fn routes(&self) -> Result<CommandResult, ShellError> {
        let diagnostics = self.fava.diagnostics();
        let mut route_observations = Vec::new();
        let mut route_relays = Vec::new();
        let mut route_revisions = Vec::new();
        let mut demand_observations = Vec::new();
        let mut demand_relays = Vec::new();
        let mut demand_states = Vec::new();
        let mut wire_observations = Vec::new();
        let mut wire_relays = Vec::new();
        let mut wire_subscriptions = Vec::new();
        for query in diagnostics.queries {
            let observation = query.observation.get().get();
            for session in query.route_relays {
                route_observations.push(ResultValue::from(observation));
                route_relays.push(ResultValue::text(session.relay.to_string()));
                route_revisions.push(
                    query
                        .route_revision
                        .map_or_else(|| ResultValue::text("explicit"), ResultValue::from),
                );
            }
            for demand in query.demand {
                demand_observations.push(ResultValue::from(observation));
                demand_relays.push(ResultValue::text(demand.session.relay.to_string()));
                demand_states.push(ResultValue::text(format!("{:?}", demand.state)));
            }
            for wire in query.wire {
                wire_observations.push(ResultValue::from(observation));
                wire_relays.push(ResultValue::text(wire.session.relay.to_string()));
                wire_subscriptions.push(ResultValue::text(wire.subscription.to_string()));
            }
        }
        CommandResult::success("routes", "active public query routing facts")
            .with_field("route_observations", ResultValue::array(route_observations))?
            .with_field("route_relays", ResultValue::array(route_relays))?
            .with_field("route_revisions", ResultValue::array(route_revisions))?
            .with_field(
                "demand_observations",
                ResultValue::array(demand_observations),
            )?
            .with_field("demand_relays", ResultValue::array(demand_relays))?
            .with_field("demand_states", ResultValue::array(demand_states))?
            .with_field("wire_observations", ResultValue::array(wire_observations))?
            .with_field("wire_relays", ResultValue::array(wire_relays))?
            .with_field("wire_subscriptions", ResultValue::array(wire_subscriptions))
    }

    fn observation(&self, name: &str) -> Result<&Observation, ShellError> {
        self.observations
            .get(name)
            .ok_or_else(|| ShellError::Domain(format!("unknown query {name:?}")))
    }

    fn observation_mut(&mut self, name: &str) -> Result<&mut Observation, ShellError> {
        self.observations
            .get_mut(name)
            .ok_or_else(|| ShellError::Domain(format!("unknown query {name:?}")))
    }
}

fn query_arguments<P>(
    action: &str,
    arguments: &[String],
    prompt: &mut P,
) -> Result<Vec<String>, ShellError>
where
    P: FnMut(&str) -> Result<Option<String>, ShellError>,
{
    let (labels, usage): (&[&str], &'static str) = match action {
        "open" => (
            &["query-name", "query-author", "event-kind", "relay-alias"],
            "query open <name> $currentPubkey <kind> <relay> [relay ...]",
        ),
        "snapshot" | "sync" | "close" => (&["query-name"], "query <action> <name>"),
        "wait" => (
            &["query-name", "minimum-count"],
            "query wait <name> <minimum-count>",
        ),
        _ => return Ok(arguments.to_vec()),
    };
    let mut values = Vec::with_capacity(arguments.len().max(labels.len()));
    for (index, label) in labels.iter().enumerate() {
        values.push(required(arguments, index, label, usage, prompt)?);
    }
    values.extend(arguments.iter().skip(labels.len()).cloned());
    Ok(values)
}

fn receipt_arguments<P>(
    action: &str,
    arguments: &[String],
    prompt: &mut P,
) -> Result<Vec<String>, ShellError>
where
    P: FnMut(&str) -> Result<Option<String>, ShellError>,
{
    if action == "show" {
        Ok(vec![required(
            arguments,
            0,
            "receipt-id",
            "receipt show <nonzero-id>",
            prompt,
        )?])
    } else {
        Ok(arguments.to_vec())
    }
}

fn required<P>(
    arguments: &[String],
    index: usize,
    label: &str,
    usage: &'static str,
    prompt: &mut P,
) -> Result<String, ShellError>
where
    P: FnMut(&str) -> Result<Option<String>, ShellError>,
{
    if let Some(value) = arguments.get(index) {
        return Ok(value.clone());
    }
    prompt(label)?.ok_or(ShellError::Usage { usage })
}

fn parse_kind(value: &str, usage: &'static str) -> Result<Kind, ShellError> {
    value
        .parse::<u16>()
        .map(Kind::from_u16)
        .map_err(|_| ShellError::Usage { usage })
}

fn parse_receipt_id(value: &str) -> Result<ReceiptId, ShellError> {
    value
        .parse::<u64>()
        .ok()
        .and_then(|value| ReceiptId::try_from(value).ok())
        .ok_or(ShellError::Usage {
            usage: "receipt show <nonzero-id>",
        })
}

fn receipt_result(
    kind: &'static str,
    summary: &'static str,
    receipt: &Receipt,
) -> Result<CommandResult, ShellError> {
    CommandResult::success(kind, summary)
        .with_field("write_id", receipt.write_id.as_u64())?
        .with_field("receipt_id", receipt.receipt_id.as_u64())?
        .with_field("event_id", receipt.current.id().to_string())?
        .with_field("author", receipt.current.event.author().to_hex())?
        .with_field("acknowledged", receipt.acknowledged())?
        .with_field("route_settled", receipt.route_settled)?
        .with_field("outcome", format!("{:?}", receipt.outcome))
}

fn snapshot_result(
    name: &str,
    observation_id: u64,
    snapshot: &QuerySnapshot,
) -> Result<CommandResult, ShellError> {
    let event_ids = snapshot
        .events
        .iter()
        .map(|record| ResultValue::text(record.id().to_string()));
    let authors = snapshot
        .events
        .iter()
        .map(|record| ResultValue::text(record.event().author().to_hex()));
    let contents = snapshot.events.iter().map(|record| {
        ResultValue::text(match record.event() {
            EventValue::Unsigned(event) => event.content.clone(),
            EventValue::Signed(event) => event.content.clone(),
        })
    });
    CommandResult::success("query-snapshot", format!("snapshot {name}"))
        .with_field("query", name)?
        .with_field("observation_id", observation_id)?
        .with_field("revision", snapshot.revision.0)?
        .with_field("event_count", snapshot.events.len())?
        .with_field("event_ids", ResultValue::array(event_ids))?
        .with_field("authors", ResultValue::array(authors))?
        .with_field("contents", ResultValue::array(contents))
}

fn help() -> CommandResult {
    CommandResult::success("help", "account-reactive commands")
        .with_field(
            "commands",
            ResultValue::array(
                [
                    "account new|import|add-pubkey|list|switch|replace|remove|clear",
                    "relay add|list|remove",
                    "publish <kind> <content> <relay>...",
                    "query open <name> $currentPubkey <kind> <relay>... | snapshot|sync|wait|close",
                    "receipt list|show",
                    "diagnostics",
                    "routes",
                    "capture <name> <field>",
                    "dump",
                    "quit",
                ]
                .into_iter()
                .map(ResultValue::text),
            ),
        )
        .expect("constant help is bounded")
}

fn block_on<F: std::future::Future>(future: F) -> F::Output {
    tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(future))
}

fn domain(error: impl std::fmt::Display) -> ShellError {
    ShellError::Domain(error.to_string())
}
