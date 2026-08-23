use std::collections::BTreeMap;
use std::sync::Arc;

use fava_query::{
    OpenedQuerySource, Query, SourceEvent, SourceKind, SourceSnapshot, SourceStatus,
    SourceTerminationCause,
};
use fava_routing::{RoutePlan, RouteRequest};
use fava_state::{EventCoordinate, RelayAccess, event_coordinate};
use fava_write::{
    Event, EventValue, Kind, PublicKey, ReceiptId, ReplaceableEventEdit,
    ReplaceableEventMaterializer, Timestamp, UnsignedEvent, WriteIntent, WritePayload,
    WriteRouting,
};

use super::{Publication, PublicationError};

pub(super) const MAX_MATERIALIZERS: usize = 64;

pub(super) struct OpenedSemanticSources {
    pub(super) cache: OpenedQuerySource,
    pub(super) writes: OpenedQuerySource,
    snapshots: [SourceSnapshot; 2],
    live: [bool; 2],
}

impl OpenedSemanticSources {
    fn new(cache: OpenedQuerySource, writes: OpenedQuerySource) -> Self {
        let snapshots = [cache.initial.clone(), writes.initial.clone()];
        Self {
            cache,
            writes,
            snapshots,
            live: [true; 2],
        }
    }

    pub(super) fn snapshots(&self) -> &[SourceSnapshot; 2] {
        &self.snapshots
    }

    pub(super) async fn next_change(&mut self) -> Option<Result<SourceKind, SourceKind>> {
        let (index, changed) = match self.live {
            [true, true] => tokio::select! {
                biased;
                changed = self.cache.changes.next_change() => (0, changed),
                changed = self.writes.changes.next_change() => (1, changed),
            },
            [true, false] => (0, self.cache.changes.next_change().await),
            [false, true] => (1, self.writes.changes.next_change().await),
            [false, false] => return None,
        };
        let kind = if index == 0 {
            SourceKind::EventCache
        } else {
            SourceKind::WriteStore
        };
        if let Ok(snapshot) = changed {
            self.snapshots[index] = snapshot;
            Some(Ok(kind))
        } else {
            self.live[index] = false;
            self.snapshots[index].status = SourceStatus::Closed {
                cause: SourceTerminationCause::ProviderClosed,
            };
            Some(Err(kind))
        }
    }

    pub(super) fn selected(&self, selected_id: Option<fava_write::EventId>) -> Option<Event> {
        let selected_id = selected_id?;
        self.snapshots.iter().find_map(|snapshot| {
            snapshot.events.iter().find_map(|event| match event {
                SourceEvent::Cached(cached) if cached.event.id == selected_id => {
                    Some(cached.event.clone())
                }
                SourceEvent::Local(local) => match &local.event {
                    EventValue::Signed(event) if event.id == selected_id => Some(event.clone()),
                    EventValue::Unsigned(_) | EventValue::Signed(_) => None,
                },
                SourceEvent::Cached(_) => None,
            })
        })
    }

    pub(super) fn close(&mut self) {
        self.cache.changes.close();
        self.writes.changes.close();
    }
}

pub(super) struct PreparedSemantic {
    pub(super) event: UnsignedEvent,
    pub(super) source: Option<Event>,
    pub(super) route: RoutePlan,
    pub(super) sources: OpenedSemanticSources,
}

pub(super) struct SemanticState {
    pub(super) edit: ReplaceableEventEdit,
    pub(super) author: PublicKey,
    pub(super) selected_id: Option<fava_write::EventId>,
    pub(super) source_floor: Option<Timestamp>,
    pub(super) failed_id: Option<fava_write::EventId>,
    pub(super) sources: OpenedSemanticSources,
}

impl SemanticState {
    pub(super) fn accepted(
        edit: ReplaceableEventEdit,
        author: PublicKey,
        selected: Option<&Event>,
        sources: OpenedSemanticSources,
    ) -> Self {
        let selected_id = selected.map(|event| event.id);
        let source_floor = selected.map(|event| event.created_at);
        Self {
            edit,
            author,
            selected_id,
            source_floor,
            failed_id: None,
            sources,
        }
    }

    pub(super) fn recovered(
        edit: ReplaceableEventEdit,
        author: PublicKey,
        selected_id: Option<fava_write::EventId>,
        source_floor: Option<Timestamp>,
        _failed_id: Option<fava_write::EventId>,
        sources: OpenedSemanticSources,
    ) -> Self {
        Self {
            edit,
            author,
            selected_id,
            source_floor,
            // Recovery authorizes exactly one retry of the persisted failed
            // source. A repeated live failure sets this again and suppresses spin.
            failed_id: None,
            sources,
        }
    }

    pub(super) fn close(&mut self) {
        self.sources.close();
    }
}

impl Publication {
    pub(super) fn index_materializers(
        materializers: impl IntoIterator<Item = Arc<dyn ReplaceableEventMaterializer>>,
    ) -> Result<BTreeMap<Kind, Arc<dyn ReplaceableEventMaterializer>>, PublicationError> {
        let mut indexed = BTreeMap::new();
        for materializer in materializers {
            if indexed.len() == MAX_MATERIALIZERS {
                return Err(PublicationError::Routing(format!(
                    "materializer selection exceeds bound: {} > {MAX_MATERIALIZERS}",
                    indexed.len() + 1
                )));
            }
            let kind = materializer.kind();
            if indexed.insert(kind, materializer).is_some() {
                return Err(PublicationError::Routing(format!(
                    "duplicate materializer for kind {}",
                    kind.as_u16()
                )));
            }
        }
        Ok(indexed)
    }

    pub(super) fn prepare_semantic(
        &self,
        intent: &WriteIntent,
        exclude: Option<(ReceiptId, fava_write::MaterializationId)>,
        current: Option<&EventValue>,
    ) -> Result<PreparedSemantic, PublicationError> {
        let WritePayload::Edit { edit, author } = intent.payload() else {
            return Err(PublicationError::Routing(
                "semantic preparation requires a replaceable-event edit".to_owned(),
            ));
        };
        self.materializer(edit)?;
        let mut sources = self.open_semantic_sources(edit, *author)?;
        let source = match self.select_source(edit, *author, sources.snapshots(), exclude) {
            Ok(source) => source,
            Err(error) => {
                sources.close();
                return Err(error);
            }
        };
        let (event, route) = match self.materialize_and_route(intent, source.as_ref(), current) {
            Ok(prepared) => prepared,
            Err(error) => {
                sources.close();
                return Err(error);
            }
        };
        Ok(PreparedSemantic {
            event,
            source,
            route,
            sources,
        })
    }

    pub(super) fn materialize_and_route(
        &self,
        intent: &WriteIntent,
        source: Option<&Event>,
        current: Option<&EventValue>,
    ) -> Result<(UnsignedEvent, RoutePlan), PublicationError> {
        let WritePayload::Edit { edit, author } = intent.payload() else {
            return Err(PublicationError::Routing(
                "semantic preparation requires a replaceable-event edit".to_owned(),
            ));
        };
        let materializer = self.materializer(edit)?;
        let created_at = injected_timestamp(source, current)?;
        let invocation = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            materializer.materialize(edit, *author, source, created_at)
        }));
        let event = match invocation {
            Ok(Ok(event)) => event,
            Ok(Err(error)) => {
                return Err(PublicationError::Routing(format!(
                    "semantic materialization refused: {error}"
                )));
            }
            Err(_) => {
                return Err(PublicationError::Routing(
                    "semantic materializer panicked".to_owned(),
                ));
            }
        };
        validate_materialization(edit, *author, &event, intent.routing(), created_at)?;
        let route = self.route_for(&event, intent.routing())?;
        Ok((event, route))
    }

    pub(super) fn materializer(
        &self,
        edit: &ReplaceableEventEdit,
    ) -> Result<Arc<dyn ReplaceableEventMaterializer>, PublicationError> {
        let kind = edit_kind(edit);
        let materializer = self.materializers.get(&kind).ok_or_else(|| {
            PublicationError::Routing(format!(
                "no selected materializer for kind {}",
                kind.as_u16()
            ))
        })?;
        if !materializer.supports(edit) {
            return Err(PublicationError::Routing(format!(
                "selected materializer does not support kind {} edit",
                kind.as_u16()
            )));
        }
        Ok(Arc::clone(materializer))
    }

    pub(super) fn open_semantic_sources(
        &self,
        edit: &ReplaceableEventEdit,
        author: PublicKey,
    ) -> Result<OpenedSemanticSources, PublicationError> {
        let query = exact_query(edit, author);
        let cache = self
            .event_source
            .open(&query)
            .map_err(|error| PublicationError::Routing(error.to_string()))?;
        let writes = match self.store.open(&query) {
            Ok(writes) => writes,
            Err(error) => {
                let mut changes = cache.changes;
                changes.close();
                return Err(PublicationError::Routing(error.to_string()));
            }
        };
        Ok(OpenedSemanticSources::new(cache, writes))
    }

    pub(super) fn select_source(
        &self,
        edit: &ReplaceableEventEdit,
        author: PublicKey,
        sources: &[SourceSnapshot],
        exclude: Option<(ReceiptId, fava_write::MaterializationId)>,
    ) -> Result<Option<Event>, PublicationError> {
        let mut qualified = sources.to_vec();
        let coordinate = edit_coordinate(edit, author);
        for source in &mut qualified {
            source.events.retain(|event| match event {
                SourceEvent::Cached(cached) => {
                    event_coordinate(
                        cached.event.id,
                        cached.event.pubkey,
                        cached.event.kind,
                        cached.event.tags.as_slice(),
                    ) == coordinate
                }
                SourceEvent::Local(local) => {
                    matches!(&local.event, EventValue::Signed(event) if event_coordinate(
                        event.id,
                        event.pubkey,
                        event.kind,
                        event.tags.as_slice(),
                    ) == coordinate)
                        && exclude.is_none_or(|(receipt_id, materialization_id)| {
                            local.publication.receipt_id != receipt_id
                                || local.publication.materialization_id != materialization_id
                        })
                }
            });
        }
        let snapshot = self
            .evaluator
            .evaluate(&exact_query(edit, author), &qualified)
            .map_err(|error| PublicationError::Routing(error.to_string()))?;
        Ok(snapshot
            .events
            .first()
            .and_then(|record| match &record.event {
                EventValue::Signed(event) => Some(event.clone()),
                EventValue::Unsigned(_) => None,
            }))
    }

    pub(super) fn route_for(
        &self,
        event: &UnsignedEvent,
        routing: &WriteRouting,
    ) -> Result<RoutePlan, PublicationError> {
        let request = RouteRequest::Write(EventValue::Unsigned(event.clone()));
        match routing {
            WriteRouting::Explicit(relays) => RoutePlan::explicit(
                relays.iter().cloned(),
                &RelayAccess::public(),
                &request.targets(),
            )
            .map_err(|error| PublicationError::Routing(error.to_string())),
            WriteRouting::Automatic => fava_routing::preview(self.routers.as_slice(), &request)
                .map_err(|error| PublicationError::Routing(error.to_string())),
        }
    }

    pub(super) fn semantic_successor(
        &self,
        state: &SemanticState,
        receipt_id: ReceiptId,
        materialization_id: fava_write::MaterializationId,
    ) -> Result<(bool, Option<Event>), PublicationError> {
        let candidate = self.select_source(
            &state.edit,
            state.author,
            state.sources.snapshots(),
            Some((receipt_id, materialization_id)),
        )?;
        if candidate.as_ref().map(|event| event.id) == state.selected_id {
            return Ok((false, None));
        }
        if candidate.as_ref().map(|event| event.id) == state.failed_id {
            return Ok((false, None));
        }
        let selected_still_present = state
            .selected_id
            .is_some_and(|selected_id| source_is_present(state.sources.snapshots(), selected_id));
        match candidate {
            Some(candidate)
                if state.source_floor.is_none_or(|floor| {
                    candidate.created_at > floor
                        || (candidate.created_at == floor
                            && state
                                .selected_id
                                .is_some_and(|selected_id| candidate.id < selected_id))
                }) =>
            {
                Ok((true, Some(candidate)))
            }
            Some(_) if state.selected_id.is_some() && !selected_still_present => Ok((true, None)),
            None if state.selected_id.is_some() => Ok((true, None)),
            Some(_) | None => Ok((false, None)),
        }
    }
}

fn source_is_present(sources: &[SourceSnapshot], selected_id: fava_write::EventId) -> bool {
    sources.iter().any(|source| {
        source.events.iter().any(|event| match event {
            SourceEvent::Cached(cached) => cached.event.id == selected_id,
            SourceEvent::Local(local) => {
                matches!(&local.event, EventValue::Signed(event) if event.id == selected_id)
            }
        })
    })
}

pub(super) const fn edit_kind(edit: &ReplaceableEventEdit) -> Kind {
    edit.kind()
}

fn exact_query(edit: &ReplaceableEventEdit, author: PublicKey) -> Query {
    Query::events()
        .authors([author])
        .kind(edit_kind(edit))
        .cache_only()
}

fn edit_coordinate(edit: &ReplaceableEventEdit, author: PublicKey) -> EventCoordinate {
    EventCoordinate::Replaceable {
        author,
        kind: edit.kind(),
        identifier: edit.identifier().map(ToOwned::to_owned),
    }
}

pub(super) fn injected_timestamp(
    source: Option<&Event>,
    current: Option<&EventValue>,
) -> Result<Timestamp, PublicationError> {
    let newest = source
        .map(|event| event.created_at.as_secs())
        .into_iter()
        .chain(current.map(|event| event.created_at().as_secs()))
        .max();
    let minimum = match newest {
        Some(timestamp) => timestamp.checked_add(1).ok_or_else(|| {
            PublicationError::Routing("materialization timestamp exhausted".to_owned())
        })?,
        None => 0,
    };
    Ok(Timestamp::from(Timestamp::now().as_secs().max(minimum)))
}

pub(super) fn validate_materialization(
    edit: &ReplaceableEventEdit,
    author: PublicKey,
    event: &UnsignedEvent,
    routing: &WriteRouting,
    injected_created_at: Timestamp,
) -> Result<(), PublicationError> {
    WriteIntent::event(event.clone(), routing.clone()).map_err(|error| {
        PublicationError::Routing(format!("semantic materialization refused: {error}"))
    })?;
    let id = event.id.ok_or_else(|| {
        PublicationError::Routing("semantic materialization has no event id".to_owned())
    })?;
    let coordinate = event_coordinate(id, event.pubkey, event.kind, event.tags.as_slice());
    if event.created_at != injected_created_at {
        return Err(PublicationError::Routing(
            "semantic materialization ignored the injected timestamp".to_owned(),
        ));
    }
    if event.pubkey != author || coordinate != edit_coordinate(edit, author) {
        return Err(PublicationError::Routing(
            "semantic materialization author or coordinate does not match accepted write"
                .to_owned(),
        ));
    }
    Ok(())
}
