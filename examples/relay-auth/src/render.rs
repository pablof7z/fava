//! Command-argument parsing and result rendering for the relay-auth app.
//!
//! Pure functions only: nothing here touches `Fava` or `Authenticator`. This
//! module exists to keep `app.rs` under the repository's file-length bound
//! without splitting one coherent command surface across crates.

use e2e_support::{CommandResult, E2eSession, ResultValue, ShellError};
use fava::{Kind, PublicKey, QuerySnapshot, Receipt, ReceiptId, RelayDeliveryOutcome};
use fava_auth::{AnswerError, AuthenticationDecision, AuthenticationDemandId};
use fava_relay::{Authentication, Authority, BoundedText};
use std::num::NonZeroU64;

/// Which relay-access lane a command names: the unauthenticated public lane,
/// or one authenticated session, named by its local account alias.
pub(crate) enum AccessSpec {
    Public,
    As(String),
}

pub(crate) const POLICY_USAGE: &str = "policy set <authenticate:<account-alias>|decline|defer>";
pub(crate) const AUTH_USAGE: &str = "auth <pending|answer|state> ...";
pub(crate) const AUTH_ANSWER_USAGE: &str =
    "auth answer <demand-id> <authenticate:<account-alias>|decline>";
pub(crate) const AUTH_STATE_USAGE: &str = "auth state <relay-alias> <public|as:<account-alias>>";
pub(crate) const QUERY_OPEN_USAGE: &str =
    "query open <name> <public|as:<account>> <kind> <relay> [relay ...]";
pub(crate) const QUERY_WAIT_USAGE: &str = "query wait <name> <minimum-count>";
pub(crate) const PUBLISH_USAGE: &str =
    "publish <public|as:<account>> [for <author-account>] <kind> <content> <relay> [relay ...]";

pub(crate) fn resolve_relays(
    session: &E2eSession,
    aliases: &[String],
) -> Result<Vec<fava::RelayUrl>, ShellError> {
    aliases
        .iter()
        .map(|alias| session.relay(alias).cloned())
        .collect()
}

pub(crate) fn resolve_access(
    session: &E2eSession,
    spec: &AccessSpec,
) -> Result<Authority, ShellError> {
    match spec {
        AccessSpec::Public => Ok(Authority::Unauthenticated),
        AccessSpec::As(alias) => Ok(Authority::As(session.account(alias)?.public_key())),
    }
}

pub(crate) fn parse_access(token: &str) -> Result<AccessSpec, ShellError> {
    if token == "public" {
        Ok(AccessSpec::Public)
    } else if let Some(alias) = token.strip_prefix("as:").filter(|alias| !alias.is_empty()) {
        Ok(AccessSpec::As(alias.to_owned()))
    } else {
        Err(ShellError::Usage {
            usage: "public | as:<account-alias>",
        })
    }
}

/// Parse a policy or answer decision. Authenticating names the account in the
/// same token, `authenticate:<account-alias>`: deciding to authenticate and
/// deciding as whom are one decision (`fava_auth::AuthenticationDecision`
/// carries no separate "as whom" elsewhere).
pub(crate) fn parse_decision(
    session: &E2eSession,
    value: &str,
) -> Result<AuthenticationDecision, ShellError> {
    if let Some(alias) = value
        .strip_prefix("authenticate:")
        .filter(|alias| !alias.is_empty())
    {
        let as_of = session.account(alias)?.public_key();
        return Ok(AuthenticationDecision::Authenticate { as_of });
    }
    match value {
        "decline" => Ok(AuthenticationDecision::Decline),
        "defer" => Ok(AuthenticationDecision::Defer),
        _ => Err(ShellError::Usage {
            usage: "authenticate:<account-alias> | decline | defer",
        }),
    }
}

pub(crate) fn decision_label(decision: AuthenticationDecision) -> String {
    match decision {
        AuthenticationDecision::Authenticate { as_of } => {
            format!("authenticate:{}", as_of.to_hex())
        }
        AuthenticationDecision::Decline => "decline".to_owned(),
        AuthenticationDecision::Defer => "defer".to_owned(),
    }
}

pub(crate) fn parse_demand_id(value: &str) -> Result<AuthenticationDemandId, ShellError> {
    value
        .parse::<u64>()
        .ok()
        .and_then(NonZeroU64::new)
        .map(AuthenticationDemandId::from_nonzero)
        .ok_or(ShellError::Usage {
            usage: AUTH_ANSWER_USAGE,
        })
}

pub(crate) fn access_label(access: &Authority) -> String {
    match access {
        Authority::Unauthenticated => "public".to_owned(),
        Authority::As(key) => format!("authenticated:{}", key.to_hex()),
    }
}

pub(crate) fn state_result(
    relay: &str,
    access: &str,
    state: Option<Authentication>,
) -> Result<CommandResult, ShellError> {
    let (name, message, truncated) = state_fields(state);
    CommandResult::success("auth-state", format!("{relay} {access} is {name}"))
        .with_field("relay", relay)?
        .with_field("access", access)?
        .with_field("state", name)?
        .with_field("message", message)?
        .with_field("truncated_bytes", truncated)
}

fn state_fields(state: Option<Authentication>) -> (&'static str, String, u64) {
    match state {
        None => ("unknown", String::new(), 0),
        Some(Authentication::None) => ("none", String::new(), 0),
        Some(Authentication::Requested { .. }) => ("requested", String::new(), 0),
        Some(Authentication::Authenticating { .. }) => ("authenticating", String::new(), 0),
        Some(Authentication::Authenticated { .. }) => ("authenticated", String::new(), 0),
        Some(Authentication::Declined) => ("declined", String::new(), 0),
        Some(Authentication::Failed { reason }) => bounded_text("failed", &reason),
    }
}

fn bounded_text(name: &'static str, text: &BoundedText) -> (&'static str, String, u64) {
    (
        name,
        text.as_str().to_owned(),
        text.truncated_bytes() as u64,
    )
}

pub(crate) fn answer_domain(error: &AnswerError) -> ShellError {
    ShellError::Domain(error.to_string())
}

pub(crate) fn required<P>(
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

pub(crate) fn parse_kind(value: &str, usage: &'static str) -> Result<Kind, ShellError> {
    value
        .parse::<u16>()
        .map(Kind::from_u16)
        .map_err(|_| ShellError::Usage { usage })
}

pub(crate) fn parse_receipt_id(value: &str) -> Result<ReceiptId, ShellError> {
    value
        .parse::<u64>()
        .ok()
        .and_then(|value| ReceiptId::try_from(value).ok())
        .ok_or(ShellError::Usage {
            usage: "receipt show <nonzero-id>",
        })
}

pub(crate) fn receipt_result(
    kind: &'static str,
    summary: &'static str,
    receipt: &Receipt,
) -> Result<CommandResult, ShellError> {
    let mut destination_relays = Vec::new();
    let mut destination_outcomes = Vec::new();
    for (relay, outcome) in receipt.destinations() {
        destination_relays.push(ResultValue::text(relay.to_string()));
        destination_outcomes.push(ResultValue::text(outcome_label(outcome)));
    }
    CommandResult::success(kind, summary)
        .with_field("write_id", receipt.write_id.as_u64())?
        .with_field("receipt_id", receipt.receipt_id.as_u64())?
        .with_field("event_id", receipt.current.id().to_string())?
        .with_field("author", receipt.current.event.author().to_hex())?
        .with_field("access", access_label(&receipt.access))?
        .with_field("acknowledged", receipt.acknowledged())?
        .with_field("route_settled", receipt.route_settled)?
        .with_field("outcome", format!("{:?}", receipt.outcome))?
        .with_field("destination_relays", ResultValue::array(destination_relays))?
        .with_field(
            "destination_outcomes",
            ResultValue::array(destination_outcomes),
        )
}

fn outcome_label(outcome: &RelayDeliveryOutcome) -> String {
    match outcome {
        RelayDeliveryOutcome::Pending => "pending".to_owned(),
        RelayDeliveryOutcome::Attempting => "attempting".to_owned(),
        RelayDeliveryOutcome::CancelledBeforeHandoff => "cancelled-before-handoff".to_owned(),
        RelayDeliveryOutcome::Retryable { reason } => format!("retryable:{reason}"),
        RelayDeliveryOutcome::Acknowledged { message } => format!("acknowledged:{message}"),
        RelayDeliveryOutcome::Rejected { message } => format!("rejected:{message}"),
        RelayDeliveryOutcome::AuthenticationDenied { reason } => {
            format!("authentication-denied:{reason}")
        }
        RelayDeliveryOutcome::GivenUp { reason } => format!("given-up:{reason}"),
        RelayDeliveryOutcome::Unknown { reason } => format!("unknown:{reason}"),
    }
}

pub(crate) fn snapshot_result(
    name: &str,
    observation_id: u64,
    snapshot: &QuerySnapshot,
) -> Result<CommandResult, ShellError> {
    let event_ids = snapshot
        .events
        .iter()
        .map(|record| ResultValue::text(record.id().to_string()));
    let authors: Vec<PublicKey> = snapshot
        .events
        .iter()
        .map(|record| record.event().author())
        .collect();
    let authors = authors
        .into_iter()
        .map(|author| ResultValue::text(author.to_hex()));
    CommandResult::success("query-snapshot", format!("snapshot {name}"))
        .with_field("query", name)?
        .with_field("observation_id", observation_id)?
        .with_field("revision", snapshot.revision.0)?
        .with_field("event_count", snapshot.events.len())?
        .with_field("event_ids", ResultValue::array(event_ids))?
        .with_field("authors", ResultValue::array(authors))
}

pub(crate) fn domain(error: impl std::fmt::Display) -> ShellError {
    ShellError::Domain(error.to_string())
}

pub(crate) fn block_on<F: std::future::Future>(future: F) -> F::Output {
    tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(future))
}

pub(crate) fn help() -> CommandResult {
    CommandResult::success("help", "relay-auth commands")
        .with_field(
            "commands",
            ResultValue::array(
                [
                    "account new|import|add-pubkey|list|switch|replace|remove|clear",
                    "relay add|list|remove",
                    "policy set <authenticate:<account-alias>|decline|defer>",
                    "auth pending",
                    "auth answer <demand-id> <authenticate:<account-alias>|decline>",
                    "auth state <relay> <public|as:<account>>",
                    "query open <name> <public|as:<account>> <kind> <relay>...",
                    "query snapshot|wait|close <name> ...",
                    "publish <public|as:<account>> [for <author>] <kind> <content> <relay>...",
                    "receipt list|show|wait",
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
