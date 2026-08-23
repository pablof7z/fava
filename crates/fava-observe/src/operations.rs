//! Provider calls issued under one owner operation generation.
//!
//! Nothing here runs on the reconciliation owner's task. Every call is bound to
//! the slot's cancellation token and reports its completion back carrying the
//! generation it was issued under, so a completion that arrives after its
//! generation was superseded is refused by the owner and whatever it produced
//! is released rather than installed.

use std::sync::Arc;

use fava_query::{BoundedText, OperationGeneration};
use fava_runtime::{OperationName, ProviderCompletion, Runtime, TaskName};
use fava_state::RelaySessionKey;
use fava_subscriptions::{PlanRevision, PlannedSubscription, WithdrawnSubscription};
use fava_transport::{
    HandoffCorrelation, HandoffOutcome, OpenRelaySession, RelaySession, Transport,
};
use fava_wire::{ClientMessage, encode_client};

use crate::engine::{Reports, Report};

const ACQUIRE: OperationName = OperationName("transport.acquire_session");
const HANDOFF: OperationName = OperationName("transport.send");
const RELEASE: OperationName = OperationName("transport.close");
const ACQUIRE_TASK: TaskName = TaskName("observe.acquire");
const APPLY_TASK: TaskName = TaskName("observe.apply");
const LISTEN_TASK: TaskName = TaskName("observe.listen");
const WITHDRAW_TASK: TaskName = TaskName("observe.withdraw");

/// Acquire a lease on the current session for one relay.
pub(crate) fn acquire(
    runtime: &Runtime,
    transport: &Arc<dyn Transport>,
    reports: &Reports,
    request: OpenRelaySession,
    generation: OperationGeneration,
    cancel: fava_runtime::CancellationToken,
) {
    let transport = Arc::clone(transport);
    let reports = reports.clone();
    let relay = request.key.clone();
    let deadline = request.deadlines.establish;
    let owner = runtime.clone();
    let _ = runtime.spawn_cancellable(ACQUIRE_TASK, cancel, async move {
        let acquired = owner
            .call_provider(ACQUIRE, generation, deadline, async move {
                transport.acquire_session(request).await
            })
            .await;
        let report = match acquired {
            ProviderCompletion::Completed {
                value: Ok(lease), ..
            } => Report::Acquired {
                relay,
                generation,
                lease: Box::new(lease),
            },
            ProviderCompletion::Completed {
                value: Err(error), ..
            } => Report::Refused {
                relay,
                generation,
                detail: BoundedText::new(error.to_string()),
            },
            other => Report::Refused {
                relay,
                generation,
                detail: BoundedText::new(format!("{:?}", ProviderName(&other))),
            },
        };
        reports.send(report).await;
    });
}

/// Identity of the plan installation one apply carries out.
pub(crate) struct Installing {
    /// Relay session the plan applies to.
    pub(crate) relay: RelaySessionKey,
    /// Owner operation generation the frames are issued under.
    pub(crate) generation: OperationGeneration,
    /// Desired-plan revision being installed.
    pub(crate) revision: PlanRevision,
}

/// Hand off the frames one plan delta requires, in add-then-withdraw order.
pub(crate) fn apply(
    runtime: &Runtime,
    reports: &Reports,
    installing: Installing,
    session: Arc<dyn RelaySession>,
    open: Vec<PlannedSubscription>,
    close: Vec<WithdrawnSubscription>,
    write_deadline: std::time::Duration,
) {
    let Installing {
        relay,
        generation,
        revision,
    } = installing;
    let reports = reports.clone();
    let owner = runtime.clone();
    let _ = runtime.spawn(APPLY_TASK, async move {
        let mut correlation = 0_u64;
        for planned in &open {
            let message = ClientMessage::Req {
                subscription_id: std::borrow::Cow::Owned(planned.id.clone()),
                filters: planned
                    .filters
                    .iter()
                    .cloned()
                    .map(std::borrow::Cow::Owned)
                    .collect(),
            };
            correlation = correlation.saturating_add(1);
            if let Err(detail) = hand_off(
                &owner,
                &session,
                &message,
                generation,
                correlation,
                write_deadline,
            )
            .await
            {
                reports
                    .send(Report::Refused {
                        relay,
                        generation,
                        detail,
                    })
                    .await;
                return;
            }
        }
        let mut withdrawn = Vec::with_capacity(close.len());
        for entry in &close {
            let message = ClientMessage::close(entry.id.clone());
            correlation = correlation.saturating_add(1);
            if hand_off(
                &owner,
                &session,
                &message,
                generation,
                correlation,
                write_deadline,
            )
            .await
            .is_ok()
            {
                withdrawn.push(entry.id.clone());
            }
        }
        reports
            .send(Report::Applied {
                relay,
                generation,
                revision,
                withdrawn,
            })
            .await;
    });
}

/// Withdraw every installed subscription and release the lease.
pub(crate) fn withdraw(
    runtime: &Runtime,
    lease: Box<fava_transport::RelaySessionLease>,
    subscriptions: Vec<fava_wire::SubscriptionId>,
    generation: OperationGeneration,
    write_deadline: std::time::Duration,
    close_deadline: std::time::Duration,
) {
    let owner = runtime.clone();
    let _ = runtime.spawn(WITHDRAW_TASK, async move {
        let mut correlation = u64::MAX / 2;
        for id in subscriptions {
            correlation = correlation.saturating_add(1);
            let message = ClientMessage::close(id);
            let _ = hand_off(
                &owner,
                lease.session(),
                &message,
                generation,
                correlation,
                write_deadline,
            )
            .await;
        }
        let _ = owner
            .call_provider(RELEASE, generation, close_deadline, async move {
                lease.release().await
            })
            .await;
    });
}

/// Forward one session's inbound items to the reconciliation owner.
pub(crate) fn listen(
    runtime: &Runtime,
    reports: &Reports,
    relay: RelaySessionKey,
    generation: OperationGeneration,
    session: &Arc<dyn RelaySession>,
    cancel: fava_runtime::CancellationToken,
) {
    let reports = reports.clone();
    let mut stream = session.messages();
    let _ = runtime.spawn_cancellable(LISTEN_TASK, cancel, async move {
        loop {
            match stream.next_inbound().await {
                Ok(item) => {
                    reports
                        .send(Report::Inbound {
                            relay: relay.clone(),
                            generation,
                            item: Box::new(item),
                        })
                        .await;
                }
                Err(error) => {
                    reports
                        .send(Report::Refused {
                            relay,
                            generation,
                            detail: BoundedText::new(error.to_string()),
                        })
                        .await;
                    stream.close();
                    return;
                }
            }
        }
    });
}

async fn hand_off(
    runtime: &Runtime,
    session: &Arc<dyn RelaySession>,
    message: &ClientMessage<'_>,
    generation: OperationGeneration,
    correlation: u64,
    deadline: std::time::Duration,
) -> Result<(), BoundedText> {
    let frame = encode_client(message)
        .map_err(|error| BoundedText::new(error.to_string()))?
        .into_bytes();
    let session = Arc::clone(session);
    let outcome = runtime
        .call_provider(HANDOFF, generation, deadline, async move {
            session.send(frame, HandoffCorrelation(correlation)).await
        })
        .await;
    match outcome {
        ProviderCompletion::Completed {
            value: HandoffOutcome::HandedOff { .. },
            ..
        } => Ok(()),
        ProviderCompletion::Completed {
            value: HandoffOutcome::NotHandedOff { reason, .. },
            ..
        } => Err(BoundedText::new(format!("{reason:?}"))),
        ProviderCompletion::Completed {
            value: HandoffOutcome::Ambiguous { reason, .. },
            ..
        } => Err(BoundedText::new(format!("{reason:?}"))),
        other => Err(BoundedText::new(format!("{:?}", ProviderName(&other)))),
    }
}

/// The scoped, bounded name of a non-completing provider outcome.
struct ProviderName<'a, T>(&'a ProviderCompletion<T>);

impl<T> std::fmt::Debug for ProviderName<'_, T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            ProviderCompletion::Completed { operation, .. } => {
                write!(formatter, "{operation} completed")
            }
            ProviderCompletion::TimedOut {
                operation, after, ..
            } => write!(formatter, "{operation} timed out after {after:?}"),
            ProviderCompletion::Panicked {
                operation, detail, ..
            } => write!(formatter, "{operation} panicked: {detail}"),
            ProviderCompletion::Cancelled { operation, .. } => {
                write!(formatter, "{operation} was cancelled")
            }
            ProviderCompletion::Refused { operation, .. } => {
                write!(formatter, "{operation} was refused by the runtime")
            }
        }
    }
}
