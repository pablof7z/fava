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
    /// The serialized event-state writer, held honestly by a cache that
    /// retains nothing.
    ///
    /// Exclusive write authority is trivially satisfied here: retained state is
    /// permanently empty, so the decision can never be made against a snapshot
    /// another writer has already replaced. The decided batch is then routed
    /// through the same refusal as [`Self::commit`], so admission cannot smuggle
    /// retention past a cache that declares it retains nothing.
    fn transact(
        &self,
        decide: &dyn Fn(&[CachedEvent]) -> Vec<CacheMutation>,
    ) -> Result<usize, EventCacheError> {
        let mutations = decide(&[]);
        let count = mutations.len();
        self.commit(mutations)?;
        Ok(count)
    }

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

    fn transact(
        &self,
        decide: &dyn for<'a> Fn(&'a [CachedEvent]) -> Vec<CacheMutation>,
    ) -> Result<usize, EventCacheError> {
        if decide(&[]).is_empty() {
            Ok(0)
        } else {
            Err(EventCacheError::Refused(
                "null cache retains no relay events".to_owned(),
            ))
        }
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
    use fava_state::Timestamp;

    use super::*;

    /// The serialized-writer contract is implementable from outside the
    /// workspace, and honoured: the decision runs against retained state the
    /// provider itself supplies, and the batch it returns is the only thing
    /// that reaches commit.
    #[test]
    fn transact_is_the_only_event_state_writer_an_external_cache_needs() {
        let cache = NullEventCache;

        let committed = cache
            .transact(&|current| {
                assert!(current.is_empty(), "a null cache retains nothing");
                Vec::new()
            })
            .expect("an empty batch commits nothing");
        assert_eq!(committed, 0);

        assert_eq!(cache.expire(Timestamp::from_secs(0)), Ok(0));
        assert!(cache.is_empty().expect("a null cache is always empty"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn external_cache_assembles_without_private_access() {
        let engine = Fava::builder()
            .event_cache(Arc::new(NullEventCache))
            .write_store(Arc::new(
                fava_write_store_memory::MemoryWriteStore::default(),
            ))
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
