//! Provider calls issued under one owner operation generation.
//!
//! Nothing here runs on the reconciliation owner's task. Every call is bound to
//! the slot's cancellation token and reports its completion back carrying the
//! generation it was issued under, so a completion that arrives after its
//! generation was superseded is refused by the owner and whatever it produced
//! is released rather than installed.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use fava_query::{BoundedText, OperationGeneration};
use fava_runtime::{CancellationToken, OperationName, ProviderCompletion, Runtime, TaskName};
use fava_state::RelaySessionKey;
use fava_subscriptions::{PlanRevision, SubscriptionPlan, WithdrawalReason};
use fava_transport::{
    HandoffCorrelation, HandoffOutcome, OpenRelaySession, RelaySession, RelaySessionLease,
    Transport,
};
use fava_wire::{ClientMessage, SubscriptionId, encode_client};

use crate::engine::{Report, Reports};

const ACQUIRE: OperationName = OperationName("transport.acquire_session");
const HANDOFF: OperationName = OperationName("transport.send");
const RELEASE: OperationName = OperationName("transport.close");
const ACQUIRE_TASK: TaskName = TaskName("observe.acquire");
const OPEN_TASK: TaskName = TaskName("observe.open");
const LISTEN_TASK: TaskName = TaskName("observe.listen");
const RELEASE_TASK: TaskName = TaskName("observe.release");
const ADMISSION_TASK: TaskName = TaskName("observe.admission");

/// Acquire a lease on the current session for one relay.
pub(crate) fn acquire(
    runtime: &Runtime,
    transport: &Arc<dyn Transport>,
    reports: &Reports,
    request: OpenRelaySession,
    generation: OperationGeneration,
    cancel: CancellationToken,
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
                detail: BoundedText::new(describe(&other)),
            },
        };
        reports.send(report).await;
    });
}

/// Close one fixed, first-arrival-anchored admission window.
///
/// Arming again while a window is pending never extends it: that decision is
/// the owner's, taken before this is called.
pub(crate) fn arm_admission(
    runtime: &Runtime,
    reports: &Reports,
    relay: RelaySessionKey,
    generation: OperationGeneration,
    window: Duration,
) {
    let reports = reports.clone();
    let _ = runtime.spawn(ADMISSION_TASK, async move {
        tokio::time::sleep(window).await;
        reports.send(Report::Flush { relay, generation }).await;
    });
}

/// Identity of the plan installation one execution carries out.
pub(crate) struct Installing {
    /// Relay session the plan applies to.
    pub(crate) relay: RelaySessionKey,
    /// Owner operation generation the frames are issued under.
    pub(crate) generation: OperationGeneration,
    /// Desired-plan revision being installed.
    pub(crate) revision: PlanRevision,
}

/// Install one plan on the wire: every REQ first, then the CLOSEs it earned.
///
/// A withdrawal whose reason names a successor waits for that successor to be
/// locally accepted. If the successor is refused the predecessor stays live and
/// no CLOSE is sent for it, because an EOSE naming a shared id cannot say which
/// filter generation it completed.
pub(crate) fn install_plan(
    runtime: &Runtime,
    reports: &Reports,
    installing: Installing,
    session: Arc<dyn RelaySession>,
    plan: SubscriptionPlan,
    write_deadline: Duration,
) {
    let Installing {
        relay,
        generation,
        revision,
    } = installing;
    let reports = reports.clone();
    let owner = runtime.clone();
    let _ = runtime.spawn(OPEN_TASK, async move {
        let mut opened = BTreeSet::new();
        let mut correlation = 0_u64;
        for planned in &plan.open {
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
                opened.insert(planned.id.clone());
            }
        }
        let mut closed = BTreeSet::new();
        for entry in &plan.close {
            if let WithdrawalReason::Regrouped { into } = &entry.reason
                && !opened.contains(into)
            {
                // The successor never opened. Keep the predecessor live.
                continue;
            }
            correlation = correlation.saturating_add(1);
            let message = ClientMessage::close(entry.id.clone());
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
                closed.insert(entry.id.clone());
            }
        }
        reports
            .send(Report::Installed {
                relay,
                generation,
                revision,
                plan: Box::new(plan),
                opened,
                closed,
            })
            .await;
    });
}

/// Withdraw everything and release the relay's lease.
pub(crate) fn release(
    runtime: &Runtime,
    lease: Box<RelaySessionLease>,
    closing: Vec<SubscriptionId>,
    generation: OperationGeneration,
    write_deadline: Duration,
    close_deadline: Duration,
) {
    let owner = runtime.clone();
    let _ = runtime.spawn(RELEASE_TASK, async move {
        let mut correlation = u64::MAX / 4;
        for id in closing {
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
    cancel: CancellationToken,
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
    deadline: Duration,
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
        other => Err(BoundedText::new(describe(&other))),
    }
}

/// The bounded, scoped name of one non-completing provider outcome.
fn describe<T>(completion: &ProviderCompletion<T>) -> String {
    match completion {
        ProviderCompletion::Completed { operation, .. } => format!("{operation} completed"),
        ProviderCompletion::TimedOut {
            operation, after, ..
        } => format!("{operation} timed out after {after:?}"),
        ProviderCompletion::Panicked {
            operation, detail, ..
        } => format!("{operation} panicked: {detail}"),
        ProviderCompletion::Cancelled { operation, .. } => format!("{operation} was cancelled"),
        ProviderCompletion::Refused { operation, .. } => {
            format!("{operation} was refused by the runtime")
        }
    }
}
