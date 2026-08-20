//! External-provider proof for the public event-cache and query-source contracts.

use fava_event_cache::{EventCache, EventCacheError};
use fava_query::{
    OpenedQuerySource, Query, QuerySource, QuerySourceError, SourceChangeFuture, SourceChanges,
    SourceKind, SourceSnapshot,
};
use fava_state::{CacheMutation, CachedEvent};
use nostr::event::EventId;

/// Deliberately absent event cache implemented outside the Fava workspace.
pub struct NullEventCache;

impl EventCache for NullEventCache {
    fn commit(&self, mutations: Vec<CacheMutation>) -> Result<(), EventCacheError> {
        if mutations.is_empty() {
            Ok(())
        } else {
            Err(EventCacheError::Refused(
                "null cache retains no relay events".to_owned(),
            ))
        }
    }

    fn event(&self, _id: EventId) -> Result<Option<CachedEvent>, EventCacheError> {
        Ok(None)
    }

    fn events(&self) -> Result<Vec<CachedEvent>, EventCacheError> {
        Ok(Vec::new())
    }

    fn len(&self) -> Result<usize, EventCacheError> {
        Ok(0)
    }
}

impl QuerySource for NullEventCache {
    fn open(&self, _query: &Query) -> Result<OpenedQuerySource, QuerySourceError> {
        Ok(OpenedQuerySource {
            initial: SourceSnapshot::empty(SourceKind::EventCache),
            changes: Box::new(NullChanges),
        })
    }
}

struct NullChanges;

impl SourceChanges for NullChanges {
    fn next_change(&mut self) -> SourceChangeFuture<'_> {
        Box::pin(std::future::pending())
    }

    fn close(&mut self) {}
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use fava::Fava;

    use super::*;

    #[tokio::test(flavor = "current_thread")]
    async fn external_cache_assembles_without_private_access() {
        let engine = Fava::builder()
            .event_cache(Arc::new(NullEventCache))
            .write_store(Arc::new(fava_write_store_memory::MemoryWriteStore::default()))
            .query_evaluator(Arc::new(fava_query_standard::StandardQueryEvaluator))
            .build()
            .expect("public contracts are sufficient for external assembly");

        let observation = engine
            .observe(fava::Query::events().cache_only())
            .await
            .expect("external null cache opens through public facade");

        assert!(observation.current().events.is_empty());
    }
}
