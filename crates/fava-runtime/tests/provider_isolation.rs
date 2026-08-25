//! Owner-level evidence that provider calls are deadline-bounded, panic-isolated,
//! and carry enough identity for a late completion to be rejected.
//!
//! Falsifier obligations from `FROZEN-CONTRACTS.md` §8:
//! `stalled_provider_yields_timed_out_completion_and_shutdown_still_joins` and
//! `panicking_provider_becomes_a_typed_completion`.

mod support;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use fava_runtime::{
    OperationGeneration, OperationName, ProviderCompletion, Runtime, RuntimeError, TaskName,
};

use support::{config, runtime};

const SIGN: OperationName = OperationName("sign_event");

fn generation(value: u64) -> OperationGeneration {
    OperationGeneration::new(value)
}

#[tokio::test]
async fn stalled_provider_yields_timed_out_completion_and_shutdown_still_joins() {
    let runtime = runtime();
    let unrelated_ran = Arc::new(AtomicUsize::new(0));

    let counter = Arc::clone(&unrelated_ran);
    let token = runtime.cancellation_token();
    runtime
        .spawn(TaskName("unrelated"), async move {
            counter.fetch_add(1, Ordering::SeqCst);
            token.cancelled().await;
        })
        .expect("admitted");

    let started = Instant::now();
    let completion: ProviderCompletion<u8> = runtime
        .call_provider(
            SIGN,
            generation(1),
            Duration::from_millis(60),
            std::future::pending::<u8>(),
        )
        .await;

    assert!(
        started.elapsed() < Duration::from_secs(2),
        "the deadline is enforced"
    );
    assert_eq!(
        completion,
        ProviderCompletion::TimedOut {
            operation: SIGN,
            generation: generation(1),
            after: Duration::from_millis(60),
        }
    );
    assert_eq!(unrelated_ran.load(Ordering::SeqCst), 1);

    // The stalled provider is detached, not aborted: it stays registered, and
    // shutdown either joins it or names it. Either way shutdown returns within
    // its own deadline and unrelated owned work is joined.
    let closing = Instant::now();
    let outcome = runtime.shutdown(Duration::from_millis(300)).await;
    assert!(
        closing.elapsed() < Duration::from_secs(2),
        "shutdown stays bounded"
    );
    assert_eq!(
        outcome,
        Err(RuntimeError::ShutdownIncomplete {
            tasks: vec![TaskName("sign_event")]
        }),
        "the stalled provider is named, and only the stalled provider"
    );
    assert!(runtime.outstanding_tasks().is_empty());
}

#[tokio::test]
async fn panicking_provider_becomes_a_typed_completion() {
    let runtime = runtime();

    let completion: ProviderCompletion<u8> = runtime
        .call_provider(SIGN, generation(1), Duration::from_secs(1), async {
            panic!("substituted signer exploded")
        })
        .await;

    match &completion {
        ProviderCompletion::Panicked {
            operation,
            generation: seen,
            detail,
        } => {
            assert_eq!(*operation, SIGN);
            assert_eq!(*seen, generation(1));
            assert!(
                detail.contains("substituted signer exploded"),
                "detail: {detail}"
            );
        }
        other => panic!("expected an attributed panic completion, got {other:?}"),
    }

    let next: ProviderCompletion<u8> = runtime
        .call_provider(SIGN, generation(2), Duration::from_secs(1), async { 3 })
        .await;
    assert_eq!(
        next.value(),
        Some(3),
        "the runtime survives a provider panic"
    );

    runtime
        .shutdown(Duration::from_secs(1))
        .await
        .expect("close");
}

#[tokio::test]
async fn a_panicking_provider_does_not_poison_the_registry_or_shutdown() {
    let runtime = runtime();
    for attempt in 0..4 {
        let completion: ProviderCompletion<u8> = runtime
            .call_provider(SIGN, generation(attempt), Duration::from_secs(1), async {
                panic!("boom")
            })
            .await;
        assert!(matches!(completion, ProviderCompletion::Panicked { .. }));
    }
    assert_eq!(runtime.running_provider_operations(), 0);
    runtime
        .shutdown(Duration::from_secs(1))
        .await
        .expect("close");
}

#[tokio::test]
async fn a_blocked_provider_cannot_block_unrelated_owner_progress() {
    let runtime = runtime();

    runtime
        .spawn(TaskName("blocked-lane"), {
            let runtime = runtime.clone();
            async move {
                let _: ProviderCompletion<u8> = runtime
                    .call_provider(
                        SIGN,
                        generation(1),
                        Duration::from_secs(3_600),
                        std::future::pending::<u8>(),
                    )
                    .await;
            }
        })
        .expect("admitted");

    let unrelated: ProviderCompletion<u8> = runtime
        .call_provider(
            OperationName("get_public_key"),
            generation(1),
            Duration::from_secs(1),
            async { 7 },
        )
        .await;
    assert_eq!(unrelated.value(), Some(7));

    runtime
        .shutdown(Duration::from_millis(300))
        .await
        .expect_err("the blocked provider is still named");
}

#[tokio::test]
async fn a_provider_call_in_flight_at_shutdown_completes_as_cancelled() {
    let runtime = runtime();
    let inflight = tokio::spawn({
        let runtime = runtime.clone();
        async move {
            runtime
                .call_provider(
                    SIGN,
                    generation(1),
                    Duration::from_secs(3_600),
                    std::future::pending::<u8>(),
                )
                .await
        }
    });

    tokio::time::sleep(Duration::from_millis(20)).await;
    runtime
        .shutdown(Duration::from_millis(200))
        .await
        .expect_err("the stalled provider is named");

    let completion = inflight.await.expect("the call answers");
    assert_eq!(
        completion,
        ProviderCompletion::Cancelled {
            operation: SIGN,
            generation: generation(1),
        }
    );
}

#[tokio::test]
async fn a_provider_call_after_shutdown_is_refused() {
    let runtime = runtime();
    runtime
        .shutdown(Duration::from_secs(1))
        .await
        .expect("close");
    let completion: ProviderCompletion<u8> = runtime
        .call_provider(SIGN, generation(4), Duration::from_secs(1), async { 1 })
        .await;
    assert_eq!(
        completion,
        ProviderCompletion::Refused {
            operation: SIGN,
            generation: generation(4),
        }
    );
}

#[tokio::test]
async fn provider_calls_beyond_the_declared_operation_bound_are_refused() {
    let runtime = Runtime::new(config(4, 16, 1));

    let holder = tokio::spawn({
        let runtime = runtime.clone();
        async move {
            runtime
                .call_provider(
                    SIGN,
                    generation(1),
                    Duration::from_secs(3_600),
                    std::future::pending::<u8>(),
                )
                .await
        }
    });
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert_eq!(runtime.running_provider_operations(), 1);

    let refused: ProviderCompletion<u8> = runtime
        .call_provider(SIGN, generation(2), Duration::from_secs(1), async { 1 })
        .await;
    assert_eq!(
        refused,
        ProviderCompletion::Refused {
            operation: SIGN,
            generation: generation(2),
        }
    );

    runtime
        .shutdown(Duration::from_millis(200))
        .await
        .expect_err("the holder is named");
    drop(holder.await);
}

#[tokio::test]
async fn a_stale_completion_is_rejectable_by_generation() {
    let runtime = runtime();
    let superseded = generation(1);
    let current = superseded.next();

    let completion: ProviderCompletion<u8> = runtime
        .call_provider(SIGN, superseded, Duration::from_secs(1), async { 9 })
        .await;

    assert_eq!(completion.generation(), superseded);
    assert_ne!(
        completion.generation(),
        current,
        "an owner that moved on can tell this completion is stale"
    );
    assert_eq!(completion.operation(), SIGN);

    let fresh: ProviderCompletion<u8> = runtime
        .call_provider(SIGN, current, Duration::from_secs(1), async { 10 })
        .await;
    assert_eq!(fresh.generation(), current);
    assert_eq!(fresh.value(), Some(10));

    runtime
        .shutdown(Duration::from_secs(1))
        .await
        .expect("close");
}

#[tokio::test]
async fn every_completion_variant_carries_its_authorising_identity() {
    let runtime = runtime();
    let asked = generation(7);

    let completed: ProviderCompletion<u8> = runtime
        .call_provider(SIGN, asked, Duration::from_secs(1), async { 1 })
        .await;
    let timed_out: ProviderCompletion<u8> = runtime
        .call_provider(
            SIGN,
            asked,
            Duration::from_millis(40),
            std::future::pending::<u8>(),
        )
        .await;
    let panicked: ProviderCompletion<u8> = runtime
        .call_provider(SIGN, asked, Duration::from_secs(1), async {
            panic!("boom")
        })
        .await;

    for completion in [&completed, &timed_out, &panicked] {
        assert_eq!(completion.generation(), asked);
        assert_eq!(completion.operation(), SIGN);
    }
    assert_eq!(timed_out.value(), None);
    assert_eq!(panicked.value(), None);

    runtime
        .shutdown(Duration::from_millis(200))
        .await
        .expect_err("the detached stalled provider is named");
}

#[tokio::test]
async fn generations_advance_monotonically_and_saturate() {
    assert_eq!(OperationGeneration::default(), generation(0));
    assert!(generation(1).next() > generation(1));
    assert_eq!(
        OperationGeneration::new(u64::MAX).next(),
        OperationGeneration::new(u64::MAX)
    );
}

#[tokio::test]
async fn the_runtime_clock_sleeps_for_owners() {
    let runtime = runtime();
    let started = Instant::now();
    runtime.sleep(Duration::from_millis(20)).await;
    assert!(started.elapsed() >= Duration::from_millis(15));
    runtime
        .shutdown(Duration::from_secs(1))
        .await
        .expect("close");
}
