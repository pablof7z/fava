//! Owner-level evidence that provider calls are deadline-bounded, panic-isolated,
//! and carry enough identity for a late completion to be rejected.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use fava_runtime::{Completion, Generation, ProviderCall, ProviderFailure, Runtime};

fn call(operation: &'static str, deadline: Duration, generation: Generation) -> ProviderCall {
    ProviderCall::new("test-signer", operation, deadline, generation)
}

#[tokio::test]
async fn a_blocked_provider_completes_as_a_typed_timeout_within_its_deadline() {
    let runtime = Runtime::new();
    let started = Instant::now();

    let completion: Completion<u8> = runtime
        .invoke(
            call("sign_event", Duration::from_millis(60), Generation::FIRST),
            std::future::pending::<u8>(),
        )
        .await;

    assert!(started.elapsed() < Duration::from_secs(2), "deadline enforced");
    assert_eq!(
        completion.outcome().err(),
        Some(&ProviderFailure::TimedOut {
            deadline: Duration::from_millis(60)
        })
    );
    assert_eq!(completion.provider(), "test-signer");
    assert_eq!(completion.operation(), "sign_event");
    assert_eq!(completion.generation(), Generation::FIRST);
}

#[tokio::test]
async fn a_blocked_provider_cannot_block_unrelated_owner_progress() {
    let runtime = Runtime::new();

    let blocked = runtime.spawn("blocked-lane", {
        let runtime = runtime.clone();
        async move {
            let _: Completion<u8> = runtime
                .invoke(
                    call("sign_event", Duration::from_secs(3_600), Generation::FIRST),
                    std::future::pending::<u8>(),
                )
                .await;
        }
    });
    blocked.expect("spawn accepted");

    let unrelated: Completion<u8> = runtime
        .invoke(
            call("get_public_key", Duration::from_secs(1), Generation::FIRST),
            async { 7 },
        )
        .await;
    assert_eq!(unrelated.into_outcome(), Ok(7));
}

#[tokio::test]
async fn a_blocked_provider_cannot_block_shutdown() {
    let runtime = Runtime::new();

    runtime
        .spawn("blocked-lane", {
            let runtime = runtime.clone();
            async move {
                let _: Completion<u8> = runtime
                    .invoke(
                        call("sign_event", Duration::from_secs(3_600), Generation::FIRST),
                        std::future::pending::<u8>(),
                    )
                    .await;
            }
        })
        .expect("spawn accepted");

    let started = Instant::now();
    let report = runtime.shutdown(Duration::from_millis(500)).await;
    assert!(started.elapsed() < Duration::from_secs(2));
    assert_eq!(report.unjoined(), 0, "the stalled provider call is cancelled, not awaited");
    assert!(report.is_clean());
}

#[tokio::test]
async fn a_cancelled_provider_call_is_typed_and_distinct_from_a_timeout() {
    let runtime = Runtime::new();
    let handle = tokio::spawn({
        let runtime = runtime.clone();
        async move {
            runtime
                .invoke(
                    call("sign_event", Duration::from_secs(3_600), Generation::FIRST),
                    std::future::pending::<u8>(),
                )
                .await
        }
    });

    tokio::time::sleep(Duration::from_millis(20)).await;
    runtime.shutdown(Duration::from_secs(1)).await;

    let completion = handle.await.expect("call returns a completion");
    assert_eq!(completion.into_outcome(), Err(ProviderFailure::Cancelled));
}

#[tokio::test]
async fn a_panicking_provider_is_isolated_and_attributed() {
    let runtime = Runtime::new();

    let completion: Completion<u8> = runtime
        .invoke(
            call("sign_event", Duration::from_secs(1), Generation::FIRST),
            async { panic!("substituted signer exploded") },
        )
        .await;

    match completion.outcome() {
        Err(ProviderFailure::Panicked { detail }) => {
            assert!(detail.contains("substituted signer exploded"), "detail: {detail}");
        }
        other => panic!("expected an attributed panic, got {other:?}"),
    }

    let next: Completion<u8> = runtime
        .invoke(
            call("sign_event", Duration::from_secs(1), Generation::FIRST.next()),
            async { 3 },
        )
        .await;
    assert_eq!(next.into_outcome(), Ok(3), "the runtime survives a provider panic");
}

#[tokio::test]
async fn a_panicking_provider_does_not_poison_the_owner_lock_or_shutdown() {
    let runtime = Runtime::new();
    for _ in 0..4 {
        let _: Completion<u8> = runtime
            .invoke(
                call("sign_event", Duration::from_secs(1), Generation::FIRST),
                async { panic!("boom") },
            )
            .await;
    }
    let report = runtime.shutdown(Duration::from_secs(1)).await;
    assert!(report.is_clean());
}

#[tokio::test]
async fn a_stale_completion_is_rejected_rather_than_installed() {
    let runtime = Runtime::new();
    let opened = Generation::FIRST;
    let current = opened.next();

    let completion: Completion<u8> = runtime
        .invoke(
            call("sign_event", Duration::from_secs(1), opened),
            async { 9 },
        )
        .await;

    assert!(!completion.is_current(current));
    assert!(completion.is_current(opened));
    assert_eq!(
        completion.accept_if_current(current),
        None,
        "a completion from a superseded generation is refused"
    );
}

#[tokio::test]
async fn a_current_completion_is_accepted() {
    let runtime = Runtime::new();
    let current = Generation::FIRST.next().next();
    let completion: Completion<u8> = runtime
        .invoke(call("sign_event", Duration::from_secs(1), current), async { 11 })
        .await;
    assert_eq!(completion.accept_if_current(current), Some(Ok(11)));
}

#[tokio::test]
async fn a_late_timeout_from_a_superseded_generation_is_rejectable() {
    let runtime = Runtime::new();
    let superseded = Generation::FIRST;
    let completion: Completion<u8> = runtime
        .invoke(
            call("sign_event", Duration::from_millis(30), superseded),
            std::future::pending::<u8>(),
        )
        .await;
    assert!(completion.outcome().is_err());
    assert_eq!(completion.accept_if_current(superseded.next()), None);
}

#[tokio::test]
async fn generations_are_monotonic_and_comparable() {
    let first = Generation::FIRST;
    let second = first.next();
    assert!(second > first);
    assert_eq!(second.value(), first.value() + 1);
}

#[tokio::test]
async fn a_provider_call_observes_cancellation_before_its_deadline() {
    let runtime = Runtime::new();
    let observed = Arc::new(AtomicBool::new(false));

    let handle = tokio::spawn({
        let runtime = runtime.clone();
        let observed = Arc::clone(&observed);
        async move {
            let completion: Completion<u8> = runtime
                .invoke(
                    call("open_session", Duration::from_secs(3_600), Generation::FIRST),
                    std::future::pending::<u8>(),
                )
                .await;
            observed.store(completion.outcome().is_err(), Ordering::SeqCst);
        }
    });

    tokio::time::sleep(Duration::from_millis(20)).await;
    runtime.cancellation().cancel();
    tokio::time::timeout(Duration::from_secs(1), handle)
        .await
        .expect("cancellation ends the call")
        .expect("task completes");
    assert!(observed.load(Ordering::SeqCst));
}
