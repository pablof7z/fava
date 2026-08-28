//! Bounded public query, decoder, route, and saved-list workflows.

use std::collections::BTreeMap;
use std::sync::Arc;

use e2e_support::{CommandResult, E2eSession, ResultValue, ShellError};
use fava::{EventValue, Query, QuerySnapshot};
use fava_simple_groups::{
    SimpleGroupAdmins, SimpleGroupLivekitParticipants, SimpleGroupMembers, SimpleGroupMetadata,
    SimpleGroupPins, SimpleGroupRoles, SimpleGroupStateEventKind,
};

use crate::app::{App, domain_error, parse_read_limit, usage};
use crate::support::OPERATION_TIMEOUT;

const EVENTS_USAGE: &str = "group events [limit]";
const STATE_USAGE: &str = "group state [limit]";
const PRESENTATION_LIST_LIMIT: usize = 16;
const LIMITED_REQUEST_EOSE: &str = "LimitedRequest";

impl App {
    pub(crate) fn routes_command(
        &self,
        _session: &E2eSession,
        arguments: &[String],
    ) -> Result<CommandResult, ShellError> {
        if arguments.len() > 1 {
            return usage("routes [limit]");
        }
        let limit = parse_read_limit(arguments.first(), "routes [limit]")?;
        let group = self.selected_group()?;
        let query = group
            .events(Query::events().limit(limit).map_err(domain_error)?)
            .map_err(domain_error)?;
        let plan = self.fava.preview_routes(&query).map_err(domain_error)?;
        CommandResult::success("routes", "current query route preview")
            .with_field("group", group.id())?
            .with_field("settled", plan.settled())?
            .with_field("destinations", plan.destinations.len())?
            .with_field("unresolved", plan.unresolved.len())?
            .with_field("shortfalls", plan.shortfalls.len())
    }

    pub(crate) fn events_command(
        &self,
        _session: &E2eSession,
        arguments: &[String],
    ) -> Result<CommandResult, ShellError> {
        if arguments.len() > 1 {
            return usage(EVENTS_USAGE);
        }
        let limit = parse_read_limit(arguments.first(), EVENTS_USAGE)?;
        let group = self.selected_group()?;
        let query = group
            .events(Query::events().limit(limit).map_err(domain_error)?)
            .map_err(domain_error)?;
        let snapshot = self.read_limited_eose(query)?;
        let kinds = kind_counts(snapshot.as_ref());
        CommandResult::success("group-events", "bounded group event snapshot")
            .with_field("group", group.id())?
            .with_field("count", snapshot.events.len())?
            .with_field("kinds", count_names(&kinds))?
            .with_field("kind_counts", count_numbers(&kinds))?
            .with_field(
                "kind_shortfall",
                kinds.len().saturating_sub(PRESENTATION_LIST_LIMIT),
            )?
            .with_field(
                "event_id",
                snapshot
                    .events
                    .first()
                    .map(|event| event.id().to_hex())
                    .unwrap_or_default(),
            )?
            .with_field("relay_eose", true)?
            .with_field("stored_events_complete", false)
    }

    pub(crate) fn state_command(
        &self,
        _session: &E2eSession,
        arguments: &[String],
    ) -> Result<CommandResult, ShellError> {
        if arguments.len() > 1 {
            return usage(STATE_USAGE);
        }
        let limit = parse_read_limit(arguments.first(), STATE_USAGE)?;
        let group = self.selected_group()?;
        let query = group
            .meta_events(SimpleGroupStateEventKind::ALL)
            .map_err(domain_error)?
            .limit(limit)
            .map_err(domain_error)?;
        let snapshot = self.read_limited_eose(query)?;
        let mut decoded = BTreeMap::new();
        let mut failures = 0usize;
        for event in snapshot.events.iter() {
            match decode_state(event.event()) {
                Ok(kind) => *decoded.entry(kind).or_insert(0usize) += 1,
                Err(()) => failures += 1,
            }
        }
        CommandResult::success("group-state", "bounded decoded group state snapshot")
            .with_field("group", group.id())?
            .with_field("count", snapshot.events.len())?
            .with_field("decoded", count_names(&decoded))?
            .with_field("decoded_counts", count_numbers(&decoded))?
            .with_field(
                "decoded_shortfall",
                decoded.len().saturating_sub(PRESENTATION_LIST_LIMIT),
            )?
            .with_field("decode_failures", failures)?
            .with_field("relay_eose", true)?
            .with_field("stored_events_complete", false)
    }

    pub(crate) fn read_limited_eose(&self, query: Query) -> Result<Arc<QuerySnapshot>, ShellError> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let mut observation = self.fava.observe(query).await.map_err(domain_error)?;
                let snapshot = observation
                    .wait_until(OPERATION_TIMEOUT, |snapshot| {
                        snapshot.evidence.relays.iter().all(|relay| {
                            relay.shortfall.as_ref().is_some_and(|shortfall| {
                                shortfall.detail.as_str() == LIMITED_REQUEST_EOSE
                            })
                        })
                    })
                    .await
                    .map_err(domain_error)?
                    .ok_or_else(|| {
                        ShellError::Domain(format!(
                            "bounded query did not reach relay EOSE within {OPERATION_TIMEOUT:?}"
                        ))
                    })?;
                observation.close();
                Ok(snapshot)
            })
        })
    }
}

fn kind_counts(snapshot: &QuerySnapshot) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for event in snapshot.events.iter() {
        *counts
            .entry(event.event().kind().as_u16().to_string())
            .or_insert(0) += 1;
    }
    counts
}

fn count_names(counts: &BTreeMap<String, usize>) -> ResultValue {
    ResultValue::array(
        counts
            .keys()
            .take(PRESENTATION_LIST_LIMIT)
            .cloned()
            .map(ResultValue::text),
    )
}

fn count_numbers(counts: &BTreeMap<String, usize>) -> ResultValue {
    ResultValue::array(
        counts
            .values()
            .take(PRESENTATION_LIST_LIMIT)
            .copied()
            .map(ResultValue::from),
    )
}

fn decode_state(event: &EventValue) -> Result<String, ()> {
    match SimpleGroupStateEventKind::try_from(event.kind()).map_err(|_| ())? {
        SimpleGroupStateEventKind::Metadata => SimpleGroupMetadata::from_event(event)
            .map(|_| "metadata".to_owned())
            .map_err(|_| ()),
        SimpleGroupStateEventKind::Admins => SimpleGroupAdmins::from_event(event)
            .map(|_| "admins".to_owned())
            .map_err(|_| ()),
        SimpleGroupStateEventKind::Members => SimpleGroupMembers::from_event(event)
            .map(|_| "members".to_owned())
            .map_err(|_| ()),
        SimpleGroupStateEventKind::Roles => SimpleGroupRoles::from_event(event)
            .map(|_| "roles".to_owned())
            .map_err(|_| ()),
        SimpleGroupStateEventKind::LivekitParticipants => {
            SimpleGroupLivekitParticipants::from_event(event)
                .map(|_| "livekit-participants".to_owned())
                .map_err(|_| ())
        }
        SimpleGroupStateEventKind::Pins => SimpleGroupPins::from_event(event)
            .map(|_| "pins".to_owned())
            .map_err(|_| ()),
    }
}

#[cfg(test)]
mod tests {
    use fava::{EventValue, Kind, Tag, Timestamp};
    use nostr::event::{EventBuilder as NostrEventBuilder, FinalizeEvent};
    use nostr::key::Keys;

    use super::decode_state;

    #[test]
    fn every_closed_state_kind_uses_its_public_decoder() {
        let author = Keys::generate();
        let participant = Keys::generate().public_key().to_hex();
        let event_id = "a".repeat(64);
        let cases = [
            (39_000, vec![Tag::parse(["d", "room"]).unwrap()], "metadata"),
            (39_001, vec![Tag::parse(["d", "room"]).unwrap()], "admins"),
            (39_002, vec![Tag::parse(["d", "room"]).unwrap()], "members"),
            (39_003, vec![Tag::parse(["d", "room"]).unwrap()], "roles"),
            (
                39_004,
                vec![
                    Tag::parse(["d", "room"]).unwrap(),
                    Tag::parse(["participant", &participant]).unwrap(),
                ],
                "livekit-participants",
            ),
            (
                39_005,
                vec![
                    Tag::parse(["d", "room"]).unwrap(),
                    Tag::parse(["e", &event_id]).unwrap(),
                ],
                "pins",
            ),
        ];
        for (kind, tags, expected) in cases {
            let event = NostrEventBuilder::new(Kind::from_u16(kind), "")
                .tags(tags)
                .custom_created_at(Timestamp::from(1))
                .finalize(&author)
                .unwrap();
            assert_eq!(decode_state(&EventValue::Signed(event)).unwrap(), expected);
        }
    }
}
