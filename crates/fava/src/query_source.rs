use std::sync::Arc;

use fava_query::{
    OpenedQuerySource, Query, QuerySource, QuerySourceClosed, QuerySourceError, SourceChangeFuture,
    SourceChanges, SourceEvent, SourceKind, SourceRevision, SourceSnapshot, SourceStatus,
};
use fava_state::CachedEvent;
use fava_write::{EventValue, LocalWriteEvent};
use tokio::sync::watch;

use super::Fava;

impl QuerySource for Fava {
    fn open(&self, query: &Query) -> Result<OpenedQuerySource, QuerySourceError> {
        tokio::runtime::Handle::try_current().map_err(|_| {
            QuerySourceError::Refused(
                "Fava query source requires a running Tokio runtime".to_owned(),
            )
        })?;
        let initial = SourceSnapshot::empty(SourceKind::EventCache);
        let (latest, receiver) = watch::channel(Arc::new(initial.clone()));
        let (cancel, mut cancel_receiver) = watch::channel(false);
        let fava = self.clone();
        let query = query.clone();
        tokio::spawn(async move {
            let Ok(mut observation) = fava.observe(query).await else {
                return;
            };
            latest.send_replace(Arc::new(convert(&observation.current())));
            loop {
                tokio::select! {
                    biased;
                    changed = cancel_receiver.changed() => {
                        if changed.is_err() || *cancel_receiver.borrow_and_update() {
                            break;
                        }
                    }
                    changed = observation.changed() => {
                        let Ok(snapshot) = changed else { break; };
                        latest.send_replace(Arc::new(convert(&snapshot)));
                    }
                }
            }
            observation.close();
        });
        Ok(OpenedQuerySource {
            initial,
            changes: Box::new(FavaChanges {
                receiver,
                cancel,
                closed: false,
            }),
        })
    }
}

struct FavaChanges {
    receiver: watch::Receiver<Arc<SourceSnapshot>>,
    cancel: watch::Sender<bool>,
    closed: bool,
}

impl SourceChanges for FavaChanges {
    fn next_change(&mut self) -> SourceChangeFuture<'_> {
        Box::pin(async move {
            if self.closed || self.receiver.changed().await.is_err() {
                return Err(QuerySourceClosed);
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
        events: snapshot.events.iter().filter_map(convert_record).collect(),
    }
}

fn convert_record(record: &fava_query::EventRecord) -> Option<SourceEvent> {
    if let Some(publication) = &record.publication {
        return LocalWriteEvent::new(record.event.clone(), publication.clone())
            .ok()
            .map(SourceEvent::Local);
    }
    let EventValue::Signed(event) = &record.event else {
        return None;
    };
    Some(SourceEvent::Cached(CachedEvent::new(
        event.clone(),
        record.relay_evidence.clone(),
    )))
}
