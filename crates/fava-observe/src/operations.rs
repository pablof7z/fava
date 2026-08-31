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
use fava_relay::RelaySessionKey;
use fava_runtime::{CancellationToken, OperationName, ProviderCompletion, Runtime, TaskName};
use fava_subscriptions::{
    DeclaredLimit, PlanRevision, RelayReadConstraints, SubscriptionPlan,
};
use fava_transport::{
    OpenRelaySession, RelaySession, RelaySessionLease,
    Transport,
};
use fava_wire::SubscriptionId;

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
    cancel: CancellationToken,
) {
    let Installing {
        relay,
        generation,
        revision,
    } = installing;
    let reports = reports.clone();
    let owner = runtime.clone();
    let _ = runtime.spawn(OPEN_TASK, async move {
        // The session names each subscription as it sends the REQ. A position
        // that stays `None` is one the transport refused.
        let mut opened: Vec<Option<SubscriptionId>> = Vec::with_capacity(plan.open.len());
        let mut attending: Vec<(SubscriptionId, CancellationToken)> = Vec::new();
        for planned in &plan.open {
            let outcome = open_subscription(
                    &owner,
                    &reports,
                    &relay,
                    &session,
                    planned.filters.clone(),
                    generation,
                    write_deadline,
                    &cancel,
                )
                .await;
            if let Some((id, token)) = outcome {
                opened.push(Some(id.clone()));
                attending.push((id, token));
            } else {
                opened.push(None);
            }
        }
        // Closing is the handle's own act: the owner cancels the task holding
        // it, and its Drop sends the CLOSE. Sending one here as well would
        // close the same subscription twice.
        let closed: BTreeSet<SubscriptionId> = plan.close.iter().cloned().collect();
        reports
            .send(Report::Installed {
                relay,
                generation,
                revision,
                plan: Box::new(plan),
                opened,
                attending,
                closed,
            })
            .await;
    });
}

/// Withdraw everything and release the relay's lease.
pub(crate) fn release(
    runtime: &Runtime,
    lease: Box<RelaySessionLease>,
    generation: OperationGeneration,
    close_deadline: Duration,
) {
    let owner = runtime.clone();
    let _ = runtime.spawn(RELEASE_TASK, async move {
        // Every subscription was closed by its own handle when the slot's token
        // fired. Closing them again here would close each one twice.
        let _ = owner
            .call_provider(RELEASE, generation, close_deadline, async move {
                lease.release().await
            })
            .await;
    });
}

/// Forward one session's connection state to the reconciliation owner.
///
/// This owner reads connection state, not frames. Each subscription's own
/// traffic arrives on that subscription's handle, so there is nothing here to
/// attribute and nothing to sift.
pub(crate) fn listen(
    runtime: &Runtime,
    reports: &Reports,
    relay: RelaySessionKey,
    generation: OperationGeneration,
    session: &Arc<dyn RelaySession>,
    cancel: CancellationToken,
) {
    let reports = reports.clone();
    let connection = fava_transport::RelaySessionExt::connection(session);
    let _ = runtime.spawn_cancellable(LISTEN_TASK, cancel, async move {
        loop {
            let changed = connection.notified();
            while let Some(state) = connection.take() {
                reports
                    .send(Report::Connection {
                        relay: relay.clone(),
                        generation,
                        state: Box::new(state),
                    })
                    .await;
            }
            if connection.is_closed() {
                return;
            }
            changed.await;
        }
    });
}

/// Forward one subscription's own traffic to the reconciliation owner.
pub(crate) fn attend(
    runtime: &Runtime,
    reports: &Reports,
    relay: RelaySessionKey,
    generation: OperationGeneration,
    mut subscription: Box<dyn fava_transport::Subscription>,
    cancel: CancellationToken,
) {
    let reports = reports.clone();
    let id = subscription.id().clone();
    let _ = runtime.spawn_cancellable(LISTEN_TASK, cancel, async move {
        loop {
            let item = subscription.next().await;
            let ended = matches!(item, fava_transport::SubscriptionItem::Ended(_));
            reports
                .send(Report::Carried {
                    relay: relay.clone(),
                    generation,
                    subscription: id.clone(),
                    item: Box::new(item),
                })
                .await;
            if ended {
                return;
            }
        }
    });
}

/// Open one wire subscription through the session's own verb, returning the
/// identifier the session minted, or `None` when the frame did not reach the
/// relay. The caller has no say in the name.
#[allow(
    clippy::too_many_arguments,
    reason = "opening one subscription names the runtime, the reports, the relay, the session, the filters, the generation, the deadline, and the token governing its handle"
)]
async fn open_subscription(
    runtime: &Runtime,
    reports: &Reports,
    relay: &RelaySessionKey,
    session: &Arc<dyn RelaySession>,
    filters: Vec<nostr::filter::Filter>,
    generation: OperationGeneration,
    deadline: Duration,
    cancel: &CancellationToken,
) -> Option<(SubscriptionId, CancellationToken)> {
    let opening = Arc::clone(session);
    let outcome = runtime
        .call_provider(HANDOFF, generation, deadline, async move {
            fava_transport::RelaySessionExt::subscribe(&opening, filters).await
        })
        .await;
    let ProviderCompletion::Completed { value: Ok(handle), .. } = outcome else {
        return None;
    };
    let id = handle.id().clone();
    // Read this subscription's own traffic. Nothing else on the connection can
    // reach it, so there is nothing to attribute here. Cancelling this token
    // drops the handle, and dropping the handle sends the relay its CLOSE --
    // one closure, sent by the thing that owns the subscription.
    let attending = cancel.child();
    attend(
        runtime,
        reports,
        relay.clone(),
        generation,
        handle,
        attending.clone(),
    );
    Some((id, attending))
}

const NIP11_TASK: TaskName = TaskName("observe.nip11");

/// Fetch NIP-11 relay information for one relay and report the constraints back.
///
/// Only plain-HTTP (ws://) relay URLs are attempted; wss:// relays return
/// `RelayReadConstraints::unknown()` without network I/O. Failure at any step
/// falls back to `unknown()` rather than failing loudly, consistent with
/// GOALS:1068 (RELAY-004): missing or unparseable claims stay unknown.
pub(crate) fn fetch_constraints(
    runtime: &Runtime,
    reports: &Reports,
    relay: RelaySessionKey,
    timeout: std::time::Duration,
    cancel: CancellationToken,
) {
    let reports = reports.clone();
    let _ = runtime.spawn_cancellable(NIP11_TASK, cancel, async move {
        let constraints = nip11_fetch(&relay.relay, timeout).await;
        reports
            .send(crate::engine::Report::Constraints { relay, constraints })
            .await;
    });
}

async fn nip11_fetch(
    relay_url: &nostr::types::RelayUrl,
    timeout: std::time::Duration,
) -> RelayReadConstraints {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let url_str = relay_url.as_str();
    // Only plain-HTTP relays are supported; return unknown for TLS relays.
    let Some(rest) = url_str.strip_prefix("ws://") else {
        return RelayReadConstraints::unknown();
    };

    let (host_port, path) = rest
        .split_once('/')
        .map(|(h, p)| (h, format!("/{p}")))
        .unwrap_or((rest.trim_end_matches('/'), "/".to_owned()));

    let (host, port): (String, u16) = if let Some(colon) = host_port.rfind(':') {
        let port_str = &host_port[colon + 1..];
        if let Ok(p) = port_str.parse::<u16>() {
            (host_port[..colon].to_owned(), p)
        } else {
            (host_port.to_owned(), 80)
        }
    } else {
        (host_port.to_owned(), 80)
    };

    let addr = format!("{host}:{port}");
    let request = format!(
        "GET {path} HTTP/1.1\r\nHost: {host}\r\nAccept: application/nostr+json\r\nConnection: close\r\n\r\n"
    );

    let result = tokio::time::timeout(timeout, async move {
        let mut stream = tokio::net::TcpStream::connect(&addr).await?;
        stream.write_all(request.as_bytes()).await?;
        let mut response = Vec::new();
        stream.read_to_end(&mut response).await?;
        std::io::Result::Ok(response)
    })
    .await;

    let body = match result {
        Ok(Ok(response)) => {
            if let Some(pos) = response.windows(4).position(|w| w == b"\r\n\r\n") {
                response[pos + 4..].to_vec()
            } else {
                return RelayReadConstraints::unknown();
            }
        }
        _ => return RelayReadConstraints::unknown(),
    };

    parse_nip11(&body)
}

fn parse_nip11(body: &[u8]) -> RelayReadConstraints {
    use std::num::NonZeroUsize;

    let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) else {
        return RelayReadConstraints::unknown();
    };

    let Some(serde_json::Value::Object(limitation)) = value.get("limitation") else {
        return RelayReadConstraints::unknown();
    };

    let declared = |key: &str| -> DeclaredLimit {
        limitation
            .get(key)
            .and_then(serde_json::Value::as_u64)
            .and_then(|n| usize::try_from(n).ok())
            .and_then(NonZeroUsize::new)
            .map_or(DeclaredLimit::Unknown, DeclaredLimit::Declared)
    };

    RelayReadConstraints {
        max_subscriptions: declared("max_subscriptions"),
        max_message_bytes: declared("max_message_length"),
        max_subscription_id_chars: declared("max_subscription_id_length"),
        max_filter_limit: declared("max_limit"),
        default_filter_limit: DeclaredLimit::Unknown,
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
