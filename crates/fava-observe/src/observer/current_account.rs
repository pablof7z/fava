//! Stable public delivery across concrete current-account query generations.

use std::sync::Arc;

use fava_query::{ObservationId, Query, QueryRevision, QuerySnapshot};
use fava_session::Session;
use tokio::sync::watch;

use crate::observation::Observation;
use crate::sources::publish_and;

use super::Observer;

pub(super) fn bind(query: Query, current: Option<fava_query::PublicKey>) -> Query {
    let bound = query.bind_current_account(current);
    if bound.matches_nothing() {
        bound.cache_only()
    } else {
        bound
    }
}

pub(super) fn revision(snapshot: &QuerySnapshot, revision: u64) -> Arc<QuerySnapshot> {
    let mut snapshot = snapshot.clone();
    snapshot.revision = QueryRevision(revision);
    Arc::new(snapshot)
}

pub(super) struct Follow {
    pub(super) observer: Observer,
    pub(super) parent: ObservationId,
    pub(super) query: Query,
    pub(super) session: Session,
    pub(super) changes: watch::Receiver<u64>,
    pub(super) child: Observation,
    pub(super) current: Option<fava_query::PublicKey>,
    pub(super) current_revision: u64,
    pub(super) latest: watch::Sender<Arc<QuerySnapshot>>,
}

pub(super) async fn run(follow: Follow) {
    let Follow {
        observer,
        parent,
        query,
        session,
        mut changes,
        mut child,
        mut current,
        mut current_revision,
        latest,
    } = follow;
    let mut delivered = 1_u64;
    loop {
        tokio::select! {
            biased;
            signalled = changes.changed() => {
                if signalled.is_err() {
                    break;
                }
                let (next_account, next_selection_revision) = session.current_account_snapshot();
                if next_selection_revision == current_revision {
                    continue;
                }
                if next_account == current {
                    current_revision = next_selection_revision;
                    continue;
                }
                let Ok((next, bound_account, bound_revision)) =
                    observer.open_stable_current_child(&query, &session, parent)
                else {
                    break;
                };
                let Some(next_revision) = delivered.checked_add(1) else {
                    break;
                };
                let child_current = next.current();
                let outbound = revision(child_current.as_ref(), next_revision);
                let sent = session
                    .if_current_account((bound_account, bound_revision), || {
                        publish_and(
                            observer.diagnostics.as_ref(),
                            &observer.registry,
                            next.id(),
                            parent,
                            &child_current.evidence,
                            || latest.send_replace(outbound),
                        )
                    })
                    .flatten()
                    .is_some();
                if !sent {
                    next.close();
                    continue;
                }
                delivered = next_revision;
                current = bound_account;
                current_revision = bound_revision;
                child = next;
            }
            delivered_snapshot = child.changed() => {
                let Ok(snapshot) = delivered_snapshot else {
                    break;
                };
                let Some(next_revision) = delivered.checked_add(1) else {
                    break;
                };
                let outbound = revision(snapshot.as_ref(), next_revision);
                let sent = session
                    .if_current_account((current, current_revision), || {
                        publish_and(
                            observer.diagnostics.as_ref(),
                            &observer.registry,
                            child.id(),
                            parent,
                            &snapshot.evidence,
                            || latest.send_replace(outbound),
                        )
                    })
                    .flatten()
                    .is_some();
                if sent {
                    delivered = next_revision;
                }
            }
        }
    }
}
