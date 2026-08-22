//! Exact-generation relay ingress admission owned by one opened relay.

use std::collections::{BTreeMap, BTreeSet};

use fava_diagnostics::Diagnostics;
use fava_event_cache::EventCache;
use fava_ingest::admit_subscription_event;
use fava_state::{RelaySessionKey, Timestamp};
use fava_wire::{RelayMessage, SubscriptionId};
use nostr::filter::Filter;

/// Admission and terminal state for one exact opened relay generation.
pub(super) struct RelayIngress {
    generation: u64,
    attribution: BTreeMap<SubscriptionId, Filter>,
    terminated: BTreeSet<SubscriptionId>,
}

impl RelayIngress {
    pub(super) fn new(generation: u64, attribution: BTreeMap<SubscriptionId, Filter>) -> Self {
        Self {
            generation,
            attribution,
            terminated: BTreeSet::new(),
        }
    }

    pub(super) fn attribution(&self) -> &BTreeMap<SubscriptionId, Filter> {
        &self.attribution
    }

    /// Reopening the same accepted demand deliberately clears terminal state.
    pub(super) fn restore_generation(&mut self, generation: u64) -> bool {
        if generation != self.generation {
            return false;
        }
        self.terminated.clear();
        true
    }

    /// Apply one relay message only to the generation and request that own it.
    pub(super) fn handle(
        &mut self,
        cache: &dyn EventCache,
        diagnostics: &Diagnostics,
        session_key: &RelaySessionKey,
        generation: u64,
        message: RelayMessage<'static>,
    ) {
        if generation != self.generation {
            diagnostics.failed(
                session_key.clone(),
                generation,
                format!(
                    "stale relay generation {generation}; current generation is {}",
                    self.generation
                ),
            );
            return;
        }
        let key = session_key.clone();
        match message {
            RelayMessage::Event {
                subscription_id,
                event,
            } => {
                let id = subscription_id.into_owned();
                if self.terminated.contains(&id) {
                    diagnostics.failed(
                        key,
                        generation,
                        format!("EVENT after CLOSED for {id} is inert"),
                    );
                    return;
                }
                let Some(filter) = self.attribution.get(&id) else {
                    diagnostics.failed(key, generation, format!("unattributed EVENT for {id}"));
                    return;
                };
                if let Err(error) = admit_subscription_event(
                    cache,
                    session_key,
                    &id,
                    &id,
                    filter,
                    event.into_owned(),
                    Timestamp::now(),
                ) {
                    diagnostics.failed(key, generation, error.to_string());
                }
            }
            RelayMessage::EndOfStoredEvents(subscription) => {
                let id = subscription.into_owned();
                if self.terminated.contains(&id) {
                    diagnostics.failed(
                        key,
                        generation,
                        format!("EOSE after CLOSED for {id} is inert"),
                    );
                } else if self.attribution.contains_key(&id) {
                    diagnostics.eose(key, generation, id);
                } else {
                    diagnostics.failed(key, generation, format!("unattributed EOSE for {id}"));
                }
            }
            RelayMessage::Closed {
                subscription_id,
                message,
            } => {
                let id = subscription_id.into_owned();
                if self.attribution.contains_key(&id) {
                    self.terminated.insert(id.clone());
                    diagnostics.closed(key, generation, id, message.into_owned());
                } else {
                    diagnostics.failed(key, generation, format!("unattributed CLOSED for {id}"));
                }
            }
            RelayMessage::Notice(message) => {
                diagnostics.failed(key, generation, format!("relay NOTICE: {message}"));
            }
            RelayMessage::Auth { .. }
            | RelayMessage::Ok { .. }
            | RelayMessage::Count { .. }
            | RelayMessage::NegMsg { .. }
            | RelayMessage::NegErr { .. } => {}
        }
    }
}
