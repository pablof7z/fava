//! Application-owned group context and top-level command dispatch.

use std::collections::BTreeMap;

use e2e_support::{CommandResult, E2eSession, ResultValue, ShellError};
use fava::{Fava, Receipt, ReceiptOutcome, RelayDeliveryOutcome, Write};
use fava_simple_groups::SimpleGroup;

use crate::support::{
    AcknowledgementSettlement, TerminalSettlement, settle_acknowledged, settle_terminal,
};

pub(crate) const READ_LIMIT: usize = 16;
pub(crate) const MAX_READ_LIMIT: usize = 64;
pub(crate) const GROUP_LIMIT: usize = 8;
const PRESENTATION_LIST_LIMIT: usize = 16;

/// Stateful NIP-29 command owner for this one runnable application.
pub(crate) struct App {
    pub(crate) fava: Fava,
    pub(crate) groups: BTreeMap<String, SimpleGroup>,
    pub(crate) selected_group: Option<String>,
}

impl App {
    pub(crate) fn new(fava: Fava) -> Self {
        Self {
            fava,
            groups: BTreeMap::new(),
            selected_group: None,
        }
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
            [command] if command == "status" => self.status(session),
            [command, rest @ ..] if command == "routes" => self.routes_command(session, rest),
            [command, rest @ ..] if command == "receipt" => self.receipt_command(rest, prompt),
            [command] if command == "diagnostics" => self.diagnostics(),
            [command, rest @ ..] if command == "group" => self.group_command(session, rest, prompt),
            [command, rest @ ..] if command == "saved-list" => {
                self.saved_list_command(session, rest, prompt)
            }
            _ => Err(ShellError::UnknownCommand {
                command: words.join(" "),
            }),
        }
    }

    pub(crate) fn selected_group(&self) -> Result<&SimpleGroup, ShellError> {
        let id = self
            .selected_group
            .as_deref()
            .ok_or_else(|| ShellError::Domain("no simple group is selected".to_owned()))?;
        self.groups.get(id).ok_or_else(|| {
            ShellError::Domain(format!("selected simple group {id:?} is no longer known"))
        })
    }

    pub(crate) fn publication_result(
        kind: &'static str,
        summary: String,
        write: &Write,
        group: Option<&str>,
        content: Option<&str>,
        relay: Option<&str>,
    ) -> Result<CommandResult, ShellError> {
        match settle_acknowledged(write).map_err(ShellError::Domain)? {
            AcknowledgementSettlement::Acknowledged(receipt) => receipt_result(
                CommandResult::success(kind, summary),
                write.write_id().as_u64(),
                write.receipt_id().as_u64(),
                &receipt,
                group,
                content,
                relay,
            ),
            AcknowledgementSettlement::Terminal(receipt) => {
                let result = receipt_result(
                    CommandResult::failed(kind, "publication was not acknowledged"),
                    write.write_id().as_u64(),
                    write.receipt_id().as_u64(),
                    &receipt,
                    group,
                    content,
                    relay,
                )?;
                Err(ShellError::CommandFailed { result })
            }
            AcknowledgementSettlement::TimedOut(receipt) => {
                let result = receipt_result(
                    CommandResult::failed(kind, "publication acknowledgement timed out"),
                    write.write_id().as_u64(),
                    write.receipt_id().as_u64(),
                    &receipt,
                    group,
                    content,
                    relay,
                )?;
                Err(ShellError::CommandFailed { result })
            }
        }
    }

    pub(crate) fn expected_rejection_result(
        kind: &'static str,
        summary: String,
        write: &Write,
        group: Option<&str>,
        content: Option<&str>,
    ) -> Result<CommandResult, ShellError> {
        match settle_terminal(write).map_err(ShellError::Domain)? {
            TerminalSettlement::Terminal(receipt) if all_desired_rejected(&receipt) => {
                receipt_result(
                    CommandResult::success(kind, summary),
                    write.write_id().as_u64(),
                    write.receipt_id().as_u64(),
                    &receipt,
                    group,
                    content,
                    None,
                )
            }
            TerminalSettlement::Terminal(receipt) => {
                let result = receipt_result(
                    CommandResult::failed(
                        kind,
                        "publication reached terminal outcomes without the expected rejection",
                    ),
                    write.write_id().as_u64(),
                    write.receipt_id().as_u64(),
                    &receipt,
                    group,
                    content,
                    None,
                )?;
                Err(ShellError::CommandFailed { result })
            }
            TerminalSettlement::TimedOut(receipt) => {
                let result = receipt_result(
                    CommandResult::failed(kind, "publication rejection wait timed out"),
                    write.write_id().as_u64(),
                    write.receipt_id().as_u64(),
                    &receipt,
                    group,
                    content,
                    None,
                )?;
                Err(ShellError::CommandFailed { result })
            }
        }
    }

    pub(crate) fn reserve_group(&self, id: &str) -> Result<(), ShellError> {
        if !self.groups.contains_key(id) && self.groups.len() >= GROUP_LIMIT {
            return Err(ShellError::Limit {
                what: "known simple groups",
                maximum: GROUP_LIMIT,
            });
        }
        Ok(())
    }

    fn status(&self, session: &E2eSession) -> Result<CommandResult, ShellError> {
        CommandResult::success("status", "current application selection")
            .with_field(
                "active_account",
                session.selected_account_alias().unwrap_or(""),
            )?
            .with_field("active_group", self.selected_group.as_deref().unwrap_or(""))?
            .with_field(
                "known_groups",
                ResultValue::array(self.groups.keys().cloned().map(ResultValue::text)),
            )
    }

    fn receipt_command<P>(
        &self,
        arguments: &[String],
        prompt: &mut P,
    ) -> Result<CommandResult, ShellError>
    where
        P: FnMut(&str) -> Result<Option<String>, ShellError>,
    {
        match arguments {
            [action] if action == "list" => {
                let receipts = self.fava.open_receipts().map_err(domain_error)?;
                let receipt_ids = receipts
                    .iter()
                    .take(PRESENTATION_LIST_LIMIT)
                    .map(|receipt| ResultValue::from(receipt.receipt_id.as_u64()));
                CommandResult::success("receipt-list", "currently open publication obligations")
                    .with_field("count", receipts.len())?
                    .with_field("receipt_ids", ResultValue::array(receipt_ids))?
                    .with_field(
                        "shortfall",
                        receipts.len().saturating_sub(PRESENTATION_LIST_LIMIT),
                    )
            }
            [action, rest @ ..] if action == "show" => {
                let raw =
                    required_value(rest, 0, "receipt-id", "receipt show <receipt-id>", prompt)?;
                if rest.len() > 1 {
                    return usage("receipt show <receipt-id>");
                }
                let receipt_id_value = raw.parse::<u64>().map_err(|_| ShellError::Usage {
                    usage: "receipt show <receipt-id>",
                })?;
                let receipt_id = receipt_id_value.try_into().map_err(|_| ShellError::Usage {
                    usage: "receipt show <receipt-id>",
                })?;
                let receipt = self.fava.receipt(receipt_id).map_err(domain_error)?;
                match receipt {
                    Some(receipt) => receipt_view(&receipt),
                    None => CommandResult::success("receipt-missing", "receipt is not retained")
                        .with_field("receipt_id", receipt_id_value),
                }
            }
            _ => usage("receipt <list|show> ..."),
        }
    }

    fn diagnostics(&self) -> Result<CommandResult, ShellError> {
        let diagnostics = self.fava.diagnostics();
        let dropped = diagnostics.dropped_facts;
        CommandResult::success("diagnostics", "bounded current Fava diagnostic facts")
            .with_field("relays", diagnostics.relays.len())?
            .with_field("queries", diagnostics.queries.len())?
            .with_field("writes", diagnostics.writes.len())?
            .with_field("providers", diagnostics.providers.len())?
            .with_field("limits", diagnostics.limits.len())?
            .with_field(
                "dropped",
                ResultValue::array([
                    ResultValue::from(dropped.relays),
                    ResultValue::from(dropped.queries),
                    ResultValue::from(dropped.writes),
                    ResultValue::from(dropped.providers),
                    ResultValue::from(dropped.limits),
                ]),
            )
    }
}

pub(crate) fn required_value<P>(
    arguments: &[String],
    index: usize,
    label: &str,
    usage_text: &'static str,
    prompt: &mut P,
) -> Result<String, ShellError>
where
    P: FnMut(&str) -> Result<Option<String>, ShellError>,
{
    match arguments.get(index) {
        Some(value) => Ok(value.clone()),
        None => prompt(label)?.ok_or(ShellError::Usage { usage: usage_text }),
    }
}

pub(crate) fn parse_read_limit(
    value: Option<&String>,
    usage_text: &'static str,
) -> Result<usize, ShellError> {
    let value = value.map_or(Ok(READ_LIMIT), |value| {
        value
            .parse::<usize>()
            .map_err(|_| ShellError::Usage { usage: usage_text })
    })?;
    if (1..=MAX_READ_LIMIT).contains(&value) {
        Ok(value)
    } else {
        Err(ShellError::Usage { usage: usage_text })
    }
}

pub(crate) fn usage<T>(usage_text: &'static str) -> Result<T, ShellError> {
    Err(ShellError::Usage { usage: usage_text })
}

pub(crate) fn domain_error(error: impl std::fmt::Display) -> ShellError {
    ShellError::Domain(error.to_string())
}

fn receipt_result(
    result: CommandResult,
    write_id: u64,
    receipt_id: u64,
    receipt: &Receipt,
    group: Option<&str>,
    content: Option<&str>,
    relay: Option<&str>,
) -> Result<CommandResult, ShellError> {
    let destination_relays = ResultValue::array(
        receipt
            .destinations()
            .keys()
            .take(PRESENTATION_LIST_LIMIT)
            .map(|destination| ResultValue::public_text(destination.relay.to_string())),
    );
    let delivery_outcomes = ResultValue::array(
        receipt
            .destinations()
            .values()
            .take(PRESENTATION_LIST_LIMIT)
            .map(|outcome| ResultValue::text(delivery_outcome_name(outcome))),
    );
    let delivery_reasons = ResultValue::array(
        receipt
            .destinations()
            .values()
            .take(PRESENTATION_LIST_LIMIT)
            .map(|outcome| ResultValue::public_text(delivery_reason(outcome).unwrap_or_default())),
    );
    let mut result = result
        .with_field("author", receipt.current.event.author().to_hex())?
        .with_field("event_id", receipt.current.id().to_hex())?
        .with_field("write_id", write_id)?
        .with_field("receipt_id", receipt_id)?
        .with_field("kind", u64::from(receipt.current.event.kind().as_u16()))?
        .with_field("outcome", receipt_outcome_name(receipt.outcome))?
        .with_field("acknowledged", receipt.acknowledged())?
        .with_field("rejected", receipt.rejected())?
        .with_field("desired", receipt.desired())?
        .with_field("destination_relays", destination_relays)?
        .with_field("delivery_outcomes", delivery_outcomes)?
        .with_field("delivery_reasons", delivery_reasons)?
        .with_field(
            "delivery_shortfall",
            receipt
                .destinations()
                .len()
                .saturating_sub(PRESENTATION_LIST_LIMIT),
        )?;
    if let Some(group) = group {
        result = result.with_field("group", group)?;
    }
    if let Some(content) = content {
        result = result.with_field("content", content)?;
    }
    if let Some(relay) = relay {
        result = result.with_field("relay", relay)?;
    }
    Ok(result)
}

fn receipt_view(receipt: &Receipt) -> Result<CommandResult, ShellError> {
    receipt_result(
        CommandResult::success("receipt", "current retained receipt facts"),
        receipt.write_id.as_u64(),
        receipt.receipt_id.as_u64(),
        receipt,
        None,
        None,
        None,
    )
}

fn all_desired_rejected(receipt: &Receipt) -> bool {
    !receipt.desired_destinations.is_empty()
        && receipt.desired_destinations.iter().all(|destination| {
            matches!(
                receipt.destinations().get(destination),
                Some(RelayDeliveryOutcome::Rejected { .. })
            )
        })
}

fn receipt_outcome_name(outcome: ReceiptOutcome) -> &'static str {
    match outcome {
        ReceiptOutcome::Open => "open",
        ReceiptOutcome::Cancelled => "cancelled",
        ReceiptOutcome::Complete => "complete",
        ReceiptOutcome::NoDestination => "no-destination",
    }
}

fn delivery_outcome_name(outcome: &RelayDeliveryOutcome) -> &'static str {
    match outcome {
        RelayDeliveryOutcome::Pending => "pending",
        RelayDeliveryOutcome::Attempting => "attempting",
        RelayDeliveryOutcome::Retryable { .. } => "retryable",
        RelayDeliveryOutcome::Acknowledged { .. } => "acknowledged",
        RelayDeliveryOutcome::Rejected { .. } => "rejected",
        RelayDeliveryOutcome::AuthenticationDenied { .. } => "authentication-denied",
        RelayDeliveryOutcome::GivenUp { .. } => "given-up",
        RelayDeliveryOutcome::Unknown { .. } => "unknown",
        RelayDeliveryOutcome::CancelledBeforeHandoff => "cancelled-before-handoff",
    }
}

fn delivery_reason(outcome: &RelayDeliveryOutcome) -> Option<&str> {
    match outcome {
        RelayDeliveryOutcome::Retryable { reason }
        | RelayDeliveryOutcome::AuthenticationDenied { reason }
        | RelayDeliveryOutcome::GivenUp { reason }
        | RelayDeliveryOutcome::Unknown { reason } => Some(reason),
        RelayDeliveryOutcome::Acknowledged { message }
        | RelayDeliveryOutcome::Rejected { message } => Some(message),
        RelayDeliveryOutcome::Pending
        | RelayDeliveryOutcome::Attempting
        | RelayDeliveryOutcome::CancelledBeforeHandoff => None,
    }
}
