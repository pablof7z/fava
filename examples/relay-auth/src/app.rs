//! NIP-42 relay-authentication commands expressed only through public Fava
//! and `fava-auth` APIs.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use e2e_support::{CommandResult, E2eSession, ResultValue, ShellError};
use fava::{EventBuilder, Fava, Observation, Query};
use fava_auth::{AnswerOutcome, Authenticator};

use crate::render::{
    AUTH_ANSWER_USAGE, AUTH_STATE_USAGE, AUTH_USAGE, AccessSpec, POLICY_USAGE, PUBLISH_USAGE,
    QUERY_OPEN_USAGE, QUERY_WAIT_USAGE, answer_domain, block_on, decision_label,
    domain, help, parse_access, parse_decision, parse_demand_id, parse_kind, parse_receipt_id,
    receipt_result, required, resolve_access, resolve_relays, snapshot_result, state_result,
};
use crate::support::SwitchablePolicy;

const OPERATION_TIMEOUT: Duration = Duration::from_secs(20);

pub(crate) struct App {
    fava: Fava,
    policy: Arc<SwitchablePolicy>,
    observations: BTreeMap<String, Observation>,
}

impl App {
    pub(crate) fn new(fava: Fava, policy: Arc<SwitchablePolicy>) -> Self {
        Self {
            fava,
            policy,
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
            [command, action, arguments @ ..] if command == "policy" && action == "set" => {
                let decision = required(arguments, 0, "decision", POLICY_USAGE, prompt)?;
                self.policy_set(session, &decision)
            }
            [command] if command == "auth" => Err(ShellError::Usage { usage: AUTH_USAGE }),
            [command, action] if command == "auth" && action == "pending" => self.auth_pending(),
            [command, action, arguments @ ..] if command == "auth" && action == "answer" => {
                let id = required(arguments, 0, "demand-id", AUTH_ANSWER_USAGE, prompt)?;
                let decision = required(arguments, 1, "decision", AUTH_ANSWER_USAGE, prompt)?;
                self.auth_answer(session, &id, &decision)
            }
            [command, action, arguments @ ..] if command == "auth" && action == "state" => {
                let relay = required(arguments, 0, "relay-alias", AUTH_STATE_USAGE, prompt)?;
                let access = required(
                    arguments,
                    1,
                    "public|as:<account>",
                    AUTH_STATE_USAGE,
                    prompt,
                )?;
                self.auth_state(session, &relay, &access)
            }
            [command, action, arguments @ ..] if command == "query" && action == "open" => {
                let name = required(arguments, 0, "query-name", QUERY_OPEN_USAGE, prompt)?;
                let access = required(
                    arguments,
                    1,
                    "public|as:<account>",
                    QUERY_OPEN_USAGE,
                    prompt,
                )?;
                let kind = required(arguments, 2, "event-kind", QUERY_OPEN_USAGE, prompt)?;
                let first_relay = required(arguments, 3, "relay-alias", QUERY_OPEN_USAGE, prompt)?;
                let mut relays = vec![first_relay];
                relays.extend(arguments.iter().skip(4).cloned());
                self.query_open(session, &name, &access, &kind, &relays)
            }
            [command, action, name] if command == "query" && action == "snapshot" => {
                let observation = self.observation(name)?;
                snapshot_result(name, observation.id().get().get(), &observation.current())
            }
            [command, action, arguments @ ..] if command == "query" && action == "wait" => {
                let name = required(arguments, 0, "query-name", QUERY_WAIT_USAGE, prompt)?;
                let count = required(arguments, 1, "minimum-count", QUERY_WAIT_USAGE, prompt)?;
                self.query_wait(&name, &count)
            }
            [command, action, name] if command == "query" && action == "close" => {
                self.query_close(name)
            }
            [command] if command == "query" => Err(ShellError::Usage {
                usage: "query <open|snapshot|wait|close> ...",
            }),
            [command, arguments @ ..] if command == "publish" => {
                let access = required(arguments, 0, "public|as:<account>", PUBLISH_USAGE, prompt)?;
                let rest = &arguments[1.min(arguments.len())..];
                let (author, rest) = match rest.first().map(String::as_str) {
                    Some("for") => (
                        Some(required(rest, 1, "author-account", PUBLISH_USAGE, prompt)?),
                        &rest[2.min(rest.len())..],
                    ),
                    _ => (None, rest),
                };
                let kind = required(rest, 0, "event-kind", PUBLISH_USAGE, prompt)?;
                let content = required(rest, 1, "event-content", PUBLISH_USAGE, prompt)?;
                let first_relay = required(rest, 2, "relay-alias", PUBLISH_USAGE, prompt)?;
                let mut relays = vec![first_relay];
                relays.extend(rest.iter().skip(3).cloned());
                self.publish(
                    session,
                    &access,
                    author.as_deref(),
                    &kind,
                    &content,
                    &relays,
                )
            }
            [command, action] if command == "receipt" && action == "list" => self.receipt_list(),
            [command, action, id] if command == "receipt" && action == "show" => {
                self.receipt_show(id)
            }
            [command, action, id] if command == "receipt" && action == "wait" => {
                self.receipt_wait(id)
            }
            [command] if command == "receipt" => Err(ShellError::Usage {
                usage: "receipt <list|show|wait> ...",
            }),
            [command] if command == "diagnostics" => self.diagnostics(),
            [command] if command == "routes" => self.routes(),
            [command] if command == "help" => Ok(help()),
            _ => Err(ShellError::UnknownCommand {
                command: words.join(" "),
            }),
        }
    }

    fn policy_set(
        &self,
        session: &E2eSession,
        decision: &str,
    ) -> Result<CommandResult, ShellError> {
        let decision = parse_decision(session, decision)?;
        self.policy.set(decision);
        CommandResult::success(
            "policy-set",
            format!("policy now {}", decision_label(decision)),
        )
        .with_field("decision", decision_label(decision))
    }

    fn auth_pending(&self) -> Result<CommandResult, ShellError> {
        let authenticator = self.authenticator()?;
        let pending = authenticator.pending();
        let ids = pending
            .iter()
            .map(|demand| ResultValue::from(demand.id.get().get()));
        let relays = pending
            .iter()
            .map(|demand| ResultValue::text(demand.session.relay.to_string()));
        let connections = pending
            .iter()
            .map(|demand| ResultValue::from(demand.session.connection.get()));
        // A bounded scalar convenience alongside `ids`: a scenario that knows
        // exactly one demand is pending can `capture` this directly, since
        // `capture` only accepts scalar fields and cannot read an array
        // element. Zero when nothing is pending.
        let first_id = pending.first().map_or(0, |demand| demand.id.get().get());
        CommandResult::success(
            "auth-pending",
            format!("{} demand(s) awaiting a person", pending.len()),
        )
        .with_field("count", pending.len())?
        .with_field("first_id", first_id)?
        .with_field("ids", ResultValue::array(ids))?
        .with_field("relays", ResultValue::array(relays))?
        .with_field("connections", ResultValue::array(connections))
    }

    fn auth_answer(
        &self,
        session: &E2eSession,
        id: &str,
        decision: &str,
    ) -> Result<CommandResult, ShellError> {
        let id = parse_demand_id(id)?;
        let decision = parse_decision(session, decision)?;
        let authenticator = self.authenticator()?;
        let outcome =
            block_on(authenticator.answer(id, decision)).map_err(|error| answer_domain(&error))?;
        let label = match outcome {
            AnswerOutcome::Applied => "applied",
            AnswerOutcome::NoLongerApplicable => "no-longer-applicable",
        };
        CommandResult::success(
            "auth-answered",
            format!("demand {} {label}", id.get().get()),
        )
        .with_field("id", id.get().get())?
        .with_field("decision", decision_label(decision))?
        .with_field("outcome", label)
    }

    /// What one relay connection has proved for the named authority.
    ///
    /// Authentication is a fact about a connection, not a lookup this
    /// application can address directly: it reads the same diagnostics record
    /// every other relay-status question does, and picks the one connection
    /// (if any) currently reachable for the requested authority.
    fn auth_state(
        &self,
        session: &E2eSession,
        relay_alias: &str,
        access_token: &str,
    ) -> Result<CommandResult, ShellError> {
        let relay = session.relay(relay_alias)?.clone();
        let authority = resolve_access(session, &parse_access(access_token)?)?;
        let diagnostics = self.fava.diagnostics();
        let authentication = diagnostics
            .relays
            .into_iter()
            .filter(|entry| entry.session == relay)
            .find(|entry| entry.authentication.can_serve(&authority))
            .map(|entry| entry.authentication);
        state_result(relay_alias, access_token, authentication)
    }

    fn query_open(
        &mut self,
        session: &E2eSession,
        name: &str,
        access_token: &str,
        kind: &str,
        relay_aliases: &[String],
    ) -> Result<CommandResult, ShellError> {
        if self.observations.contains_key(name) {
            return Err(ShellError::Domain(format!(
                "query {name:?} is already open"
            )));
        }
        let kind = parse_kind(kind, QUERY_OPEN_USAGE)?;
        let relays = resolve_relays(session, relay_aliases)?;
        let spec = parse_access(access_token)?;
        let account = match &spec {
            AccessSpec::Public => None,
            AccessSpec::As(alias) => Some(session.account(alias)?.public_key()),
        };
        let query = Query::events()
            .kinds([kind])
            .map_err(domain)?
            .only_from_relays(relays)
            .map_err(domain)?;
        let observation = block_on(async {
            match account {
                Some(account) => self.fava.with_account(account).observe(query).await,
                None => self.fava.observe(query).await,
            }
        })
        .map_err(domain)?;
        let id = observation.id().get().get();
        let snapshot = observation.current();
        self.observations.insert(name.to_owned(), observation);
        CommandResult::success("query-opened", format!("opened {name}"))
            .with_field("query", name)?
            .with_field("access", access_token)?
            .with_field("observation_id", id)?
            .with_field("revision", snapshot.revision.0)?
            .with_field("event_count", snapshot.events.len())
    }

    fn query_wait(&mut self, name: &str, count: &str) -> Result<CommandResult, ShellError> {
        let count = count.parse::<usize>().map_err(|_| ShellError::Usage {
            usage: QUERY_WAIT_USAGE,
        })?;
        let observation = self.observation_mut(name)?;
        let snapshot = block_on(
            observation.wait_until(OPERATION_TIMEOUT, |snapshot| snapshot.events.len() >= count),
        )
        .map_err(domain)?
        .ok_or_else(|| ShellError::Domain(format!("query {name:?} wait timed out")))?;
        snapshot_result(name, observation.id().get().get(), &snapshot)
    }

    fn query_close(&mut self, name: &str) -> Result<CommandResult, ShellError> {
        let observation = self
            .observations
            .remove(name)
            .ok_or_else(|| ShellError::Domain(format!("unknown query {name:?}")))?;
        let id = observation.id().get().get();
        observation.close();
        CommandResult::success("query-closed", format!("closed {name}"))
            .with_field("query", name)?
            .with_field("observation_id", id)
    }

    #[allow(clippy::too_many_arguments)]
    fn publish(
        &mut self,
        session: &E2eSession,
        access_token: &str,
        author_alias: Option<&str>,
        kind: &str,
        content: &str,
        relay_aliases: &[String],
    ) -> Result<CommandResult, ShellError> {
        if relay_aliases.is_empty() {
            return Err(ShellError::Usage {
                usage: PUBLISH_USAGE,
            });
        }
        let kind = parse_kind(kind, PUBLISH_USAGE)?;
        let relays = resolve_relays(session, relay_aliases)?;
        let spec = parse_access(access_token)?;
        let author = author_alias
            .map(|alias| session.account(alias).map(e2e_support::Account::public_key))
            .transpose()?;
        // Publishing as an account performs no ceremony of its own: the
        // transport announces every connection whose relay asks to
        // authenticate, and the one component that answers hears about it
        // without this application arranging a watch first.
        let write = match (spec, author) {
            (AccessSpec::Public, None) => self
                .fava
                .to(relays)
                .map_err(domain)?
                .publish(EventBuilder::new(kind).content(content))
                .map_err(domain)?,
            (AccessSpec::Public, Some(author)) => self
                .fava
                .to(relays)
                .map_err(domain)?
                .publish(EventBuilder::new(kind).content(content).by(author))
                .map_err(domain)?,
            (AccessSpec::As(alias), None) => {
                let account = session.account(&alias)?.public_key();
                self.fava
                    .with_account(account)
                    .to(relays)
                    .map_err(domain)?
                    .publish(EventBuilder::new(kind).content(content))
                    .map_err(domain)?
            }
            (AccessSpec::As(alias), Some(author)) => {
                let account = session.account(&alias)?.public_key();
                self.fava
                    .with_account(account)
                    .to(relays)
                    .map_err(domain)?
                    .publish(EventBuilder::new(kind).content(content).by(author))
                    .map_err(domain)?
            }
        };
        // Publication returns as soon as the write is durably accepted, not
        // once it settles: a deferred NIP-42 demand can leave a receipt open
        // for as long as it takes a person to answer `auth pending`, and this
        // one REPL command stream is the only thing that can run `auth
        // answer` next. Blocking here would make that impossible to drive
        // from one session. Use `receipt wait` to block for terminality when
        // a command line does not need to interleave with a demand.
        let receipt = write.receipt().map_err(domain)?;
        receipt_result("published", "publication accepted", &receipt)
    }

    fn receipt_wait(&self, id: &str) -> Result<CommandResult, ShellError> {
        let receipt_id = parse_receipt_id(id)?;
        let terminal = fava::all_terminal();
        let receipt = block_on(async {
            if let Some(current) = self
                .fava
                .receipt(receipt_id)
                .map_err(|error| error.to_string())?
                && terminal(&current)
            {
                return Ok(current);
            }
            let mut changes = self.fava.receipt_changes();
            tokio::time::timeout(OPERATION_TIMEOUT, async {
                loop {
                    match changes.recv().await {
                        Ok((changed_id, Some(receipt)))
                            if changed_id == receipt_id && terminal(&receipt) =>
                        {
                            return Ok(receipt);
                        }
                        Ok(_) => {}
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                            if let Some(current) = self
                                .fava
                                .receipt(receipt_id)
                                .map_err(|error| error.to_string())?
                                && terminal(&current)
                            {
                                return Ok(current);
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            return Err("receipt-change stream closed".to_owned());
                        }
                    }
                }
            })
            .await
            .map_err(|_| format!("receipt {} wait timed out", receipt_id.as_u64()))?
        })
        .map_err(ShellError::Domain)?;
        receipt_result("receipt", "receipt reached terminal state", &receipt)
    }

    fn receipt_list(&self) -> Result<CommandResult, ShellError> {
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

    fn receipt_show(&self, id: &str) -> Result<CommandResult, ShellError> {
        let id = parse_receipt_id(id)?;
        let receipt = self
            .fava
            .receipt(id)
            .map_err(domain)?
            .ok_or_else(|| ShellError::Domain(format!("unknown receipt {}", id.as_u64())))?;
        receipt_result("receipt", "receipt state", &receipt)
    }

    fn diagnostics(&self) -> Result<CommandResult, ShellError> {
        let (current, revision) = self.fava.current_account_snapshot();
        let diagnostics = self.fava.diagnostics();
        let accounts = self.fava.accounts();
        let account_keys = accounts.iter().map(|key| ResultValue::text(key.to_hex()));
        CommandResult::success("diagnostics", "current Fava ownership facts")
            .with_field(
                "current_pubkey",
                current.map_or_else(String::new, |key| key.to_hex()),
            )?
            .with_field("selection_revision", revision)?
            .with_field("session_revision", self.fava.session_revision())?
            .with_field("account_pubkeys", ResultValue::array(account_keys))?
            .with_field("query_count", diagnostics.queries.len())?
            .with_field("relay_count", diagnostics.relays.len())?
            .with_field("write_count", diagnostics.writes.len())
    }

    fn routes(&self) -> Result<CommandResult, ShellError> {
        let diagnostics = self.fava.diagnostics();
        let mut demand_observations = Vec::new();
        let mut demand_relays = Vec::new();
        let mut demand_states = Vec::new();
        for query in diagnostics.queries {
            let observation = query.observation.get().get();
            for demand in query.demand {
                demand_observations.push(ResultValue::from(observation));
                demand_relays.push(ResultValue::text(demand.session.to_string()));
                demand_states.push(ResultValue::text(format!("{:?}", demand.state)));
            }
        }
        CommandResult::success("routes", "active query demand ownership facts")
            .with_field(
                "demand_observations",
                ResultValue::array(demand_observations),
            )?
            .with_field("demand_relays", ResultValue::array(demand_relays))?
            .with_field("demand_states", ResultValue::array(demand_states))
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

    fn authenticator(&self) -> Result<&Authenticator, ShellError> {
        self.fava
            .authentication()
            .ok_or_else(|| ShellError::Domain("no authentication policy is configured".to_owned()))
    }
}
