use std::sync::Arc;

use fava_query::{
    BoundedText, OpenedQuerySource, Query, QuerySource, QuerySourceClosed, QuerySourceError,
    SourceChangeFuture, SourceChanges, SourceEvent, SourceKind, SourceRevision, SourceSnapshot,
    SourceStatus, SourceTerminationCause,
};
use fava_state::RelayEvent;
use fava_write::{EventValue, LocalWriteEvent};
use tokio::sync::watch;

use super::Fava;

impl QuerySource for Fava {
    fn open(&self, query: &Query) -> Result<OpenedQuerySource, QuerySourceError> {
        tokio::runtime::Handle::try_current().map_err(|_| {
            QuerySourceError::Refused(BoundedText::new(
                "Fava query source requires a running Tokio runtime",
            ))
        })?;
        let initial = SourceSnapshot::empty(SourceKind::EventCache);
        let (latest, receiver) = watch::channel(Arc::new(initial.clone()));
        let (cancel, mut cancel_receiver) = watch::channel(false);
        // The cause is published before the latest-state sender is dropped, so
        // the terminal value the consumer observes names why the observation
        // actually ended instead of assuming a clean provider close.
        let (cause, cause_receiver) = watch::channel(None);
        let fava = self.clone();
        let query = query.clone();
        tokio::spawn(async move {
            let mut observation = match fava.observe(query).await {
                Ok(observation) => observation,
                Err(error) => {
                    cause.send_replace(Some(SourceTerminationCause::ProviderFailed {
                        detail: BoundedText::new(error.to_string()),
                    }));
                    return;
                }
            };
            latest.send_replace(Arc::new(convert(&observation.current())));
            let ended = loop {
                tokio::select! {
                    biased;
                    changed = cancel_receiver.changed() => {
                        if changed.is_err() || *cancel_receiver.borrow_and_update() {
                            break SourceTerminationCause::LocalClose;
                        }
                    }
                    changed = observation.changed() => {
                        match changed {
                            Ok(snapshot) => {
                                latest.send_replace(Arc::new(convert(&snapshot)));
                            }
                            Err(_) => break SourceTerminationCause::ProviderClosed,
                        }
                    }
                }
            };
            cause.send_replace(Some(ended));
            observation.close();
        });
        Ok(OpenedQuerySource {
            initial,
            changes: Box::new(FavaChanges {
                receiver,
                cause: cause_receiver,
                cancel,
                closed: false,
            }),
        })
    }
}

struct FavaChanges {
    receiver: watch::Receiver<Arc<SourceSnapshot>>,
    cause: watch::Receiver<Option<SourceTerminationCause>>,
    cancel: watch::Sender<bool>,
    closed: bool,
}

impl FavaChanges {
    /// The cause the owning task published, or a clean provider close when the
    /// task ended without publishing one.
    fn reported_cause(&self) -> SourceTerminationCause {
        self.cause
            .borrow()
            .clone()
            .unwrap_or(SourceTerminationCause::ProviderClosed)
    }
}

impl SourceChanges for FavaChanges {
    fn next_change(&mut self) -> SourceChangeFuture<'_> {
        Box::pin(async move {
            if self.closed {
                return Err(QuerySourceClosed::local_close());
            }
            if self.receiver.changed().await.is_err() {
                self.closed = true;
                return Err(QuerySourceClosed::new(self.reported_cause()));
            }
            Ok(self.receiver.borrow_and_update().as_ref().clone())
        })
    }

    fn close(&mut self) {
        if !self.closed {
            self.closed = true;
            self.cancel.send_replace(true);
        }
    }
}

impl Drop for FavaChanges {
    fn drop(&mut self) {
        self.close();
    }
}

fn convert(snapshot: &fava_query::QuerySnapshot) -> SourceSnapshot {
    SourceSnapshot {
        kind: SourceKind::EventCache,
        revision: SourceRevision(snapshot.revision.0),
        status: SourceStatus::Open,
        events: snapshot.events.iter().flat_map(convert_record).collect(),
        // Forward the retraction causes the underlying event cache reported.
        // A composed source that drops them turns every removal back into a
        // bare disappearance for the next consumer up the chain.
        retractions: snapshot
            .evidence
            .source(&SourceKind::EventCache)
            .map(|source| source.retractions.clone())
            .unwrap_or_default(),
    }
}

fn convert_record(record: &fava_query::EventRecord) -> Vec<SourceEvent> {
    let mut sources = Vec::new();
    if let Some(publication) = record.publication()
        && let Ok(local) = LocalWriteEvent::new(record.event().clone(), publication.clone())
    {
        sources.push(SourceEvent::Local(local));
    }
    let EventValue::Signed(event) = record.event() else {
        return sources;
    };
    sources.extend(record.relay_occurrences().occurrences().map(|occurrence| {
        SourceEvent::Relay(RelayEvent::new(
            event.clone(),
            occurrence.session.clone(),
            occurrence.observed_at,
        ))
    }));
    sources
}
