//! Owner-level evidence for task ownership, cancellation, and shutdown joins.

mod support;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use fava_runtime::{Runtime, RuntimeError, TaskFailure, TaskName};

use support::{config, runtime};

/// Poll a condition until it holds or a bounded wall-clock budget expires.
async fn settle(mut condition: impl FnMut() -> bool) -> bool {
    for _ in 0..200 {
        if condition() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    condition()
}

#[tokio::test]
async fn shutdown_joins_every_owned_task_within_the_deadline() {
    let runtime = runtime();
    let ticks = Arc::new(AtomicUsize::new(0));

    for _ in 0..3 {
        let token = runtime.cancellation_token();
        let ticks = Arc::clone(&ticks);
        runtime
            .spawn(TaskName("cooperative"), async move {
                loop {
                    ticks.fetch_add(1, Ordering::SeqCst);
                    tokio::select! {
                        () = token.cancelled() => break,
                        () = tokio::time::sleep(Duration::from_millis(1)) => {}
                    }
                }
            })
            .expect("runtime admits owned work");
    }

    assert!(settle(|| ticks.load(Ordering::SeqCst) > 3).await);
    assert_eq!(runtime.outstanding_tasks().len(), 3);

    runtime
        .shutdown(Duration::from_secs(2))
        .await
        .expect("every owned task joins");
    assert!(runtime.outstanding_tasks().is_empty());

    let after = ticks.load(Ordering::SeqCst);
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(
        ticks.load(Ordering::SeqCst),
        after,
        "no owned task runs after shutdown returns"
    );
}

#[tokio::test]
async fn a_task_whose_handle_was_dropped_is_still_joined() {
    let runtime = runtime();
    let token = runtime.cancellation_token();
    drop(
        runtime
            .spawn(TaskName("orphan"), async move { token.cancelled().await })
            .expect("admitted"),
    );
    assert_eq!(runtime.outstanding_tasks(), vec![TaskName("orphan")]);
    runtime
        .shutdown(Duration::from_secs(2))
        .await
        .expect("the registry keeps its own grip");
}

#[tokio::test]
async fn repeated_shutdown_is_harmless() {
    let runtime = runtime();
    runtime
        .spawn(TaskName("brief"), async {})
        .expect("admitted");

    runtime
        .shutdown(Duration::from_secs(2))
        .await
        .expect("first close");
    runtime
        .shutdown(Duration::from_secs(2))
        .await
        .expect("repeat close");
}

#[tokio::test]
async fn shutdown_names_the_exact_task_that_outlived_the_deadline() {
    let runtime = runtime();

    let token = runtime.cancellation_token();
    runtime
        .spawn(
            TaskName("cooperative"),
            async move { token.cancelled().await },
        )
        .expect("admitted");
    runtime
        .spawn(TaskName("stubborn"), std::future::pending::<()>())
        .expect("admitted");

    let started = Instant::now();
    let failure = runtime
        .shutdown(Duration::from_millis(100))
        .await
        .expect_err("a task that ignores cancellation is reported");
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "shutdown stays bounded"
    );
    assert_eq!(
        failure,
        RuntimeError::ShutdownIncomplete {
            tasks: vec![TaskName("stubborn")]
        }
    );
}

#[tokio::test]
async fn spawn_after_shutdown_is_refused() {
    let runtime = runtime();
    runtime
        .shutdown(Duration::from_secs(1))
        .await
        .expect("close");
    assert_eq!(
        runtime.spawn(TaskName("late"), async {}).unwrap_err(),
        RuntimeError::ShuttingDown
    );
}

#[tokio::test]
async fn spawn_beyond_the_declared_task_limit_is_refused() {
    let runtime = Runtime::new(config(4, 1, 4));
    let token = runtime.cancellation_token();
    runtime
        .spawn(TaskName("first"), async move { token.cancelled().await })
        .expect("admitted");
    assert_eq!(
        runtime
            .spawn(TaskName("second"), std::future::pending::<()>())
            .unwrap_err(),
        RuntimeError::TaskLimit { limit: 1 }
    );
    runtime
        .shutdown(Duration::from_secs(1))
        .await
        .expect("close");
}

#[tokio::test]
async fn a_completed_task_frees_its_registry_slot() {
    let runtime = Runtime::new(config(4, 1, 4));
    let handle = runtime
        .spawn(TaskName("brief"), async { 5_u8 })
        .expect("admitted");
    assert_eq!(handle.join().await, Ok(5));
    assert!(settle(|| runtime.outstanding_tasks().is_empty()).await);
    runtime
        .spawn(TaskName("next"), async {})
        .expect("the freed slot is reusable");
    runtime
        .shutdown(Duration::from_secs(1))
        .await
        .expect("close");
}

#[tokio::test]
async fn a_panicking_task_becomes_a_typed_failure_and_frees_shutdown() {
    let runtime = runtime();
    let done = Arc::new(AtomicUsize::new(0));

    let panicking = runtime
        .spawn(TaskName("panicking"), async {
            panic!("owned task exploded")
        })
        .expect("admitted");

    let unrelated = Arc::clone(&done);
    runtime
        .spawn(TaskName("unrelated"), async move {
            unrelated.fetch_add(1, Ordering::SeqCst);
        })
        .expect("admitted");

    match panicking.join().await {
        Err(TaskFailure::Panicked { name, detail }) => {
            assert_eq!(name, TaskName("panicking"));
            assert!(detail.contains("owned task exploded"), "detail: {detail}");
        }
        other => panic!("expected an attributed panic, got {other:?}"),
    }

    assert!(settle(|| done.load(Ordering::SeqCst) == 1).await);
    runtime
        .shutdown(Duration::from_secs(2))
        .await
        .expect("a panicking task never blocks shutdown");
}

#[tokio::test]
async fn a_task_aborted_at_shutdown_reports_abortion_to_its_owner() {
    let runtime = runtime();
    let handle = runtime
        .spawn(TaskName("stubborn"), std::future::pending::<u8>())
        .expect("admitted");
    runtime
        .shutdown(Duration::from_millis(60))
        .await
        .expect_err("the task outlives the deadline");
    assert_eq!(
        handle.join().await,
        Err(TaskFailure::Aborted {
            name: TaskName("stubborn")
        })
    );
}

#[tokio::test]
async fn a_cancellable_task_ends_with_none_when_its_token_fires() {
    let runtime = runtime();
    let token = runtime.cancellation_token();
    let handle = runtime
        .spawn_cancellable(
            TaskName("lane"),
            token.clone(),
            std::future::pending::<u8>(),
        )
        .expect("admitted");
    token.cancel();
    assert_eq!(handle.join().await, Ok(None));
    runtime
        .shutdown(Duration::from_secs(1))
        .await
        .expect("close");
}

#[tokio::test]
async fn a_cancellable_task_that_finishes_first_keeps_its_value() {
    let runtime = runtime();
    let handle = runtime
        .spawn_cancellable(TaskName("lane"), runtime.cancellation_token(), async {
            9_u8
        })
        .expect("admitted");
    assert_eq!(handle.join().await, Ok(Some(9)));
    runtime
        .shutdown(Duration::from_secs(1))
        .await
        .expect("close");
}

#[tokio::test]
async fn is_finished_reports_completion_before_the_owner_joins() {
    let runtime = runtime();
    let handle = runtime
        .spawn(TaskName("brief"), async { 1_u8 })
        .expect("admitted");
    assert!(settle(|| handle.is_finished()).await);
    assert_eq!(handle.join().await, Ok(1));
    runtime
        .shutdown(Duration::from_secs(1))
        .await
        .expect("close");
}

#[tokio::test]
async fn cancellation_propagates_from_an_owner_token_to_its_children() {
    let runtime = runtime();
    let owner = runtime.cancellation_token();
    let child = owner.child();
    let grandchild = child.child();

    assert!(!grandchild.is_cancelled());
    owner.cancel();
    assert!(child.is_cancelled());
    assert!(grandchild.is_cancelled());

    tokio::time::timeout(Duration::from_secs(1), grandchild.cancelled())
        .await
        .expect("a fired token wakes its waiters");
}

#[tokio::test]
async fn a_child_derived_after_cancellation_is_already_cancelled() {
    let runtime = runtime();
    let owner = runtime.cancellation_token();
    owner.cancel();
    assert!(owner.child().is_cancelled());
}

#[tokio::test]
async fn cancelling_a_child_leaves_its_parent_and_siblings_running() {
    let runtime = runtime();
    let owner = runtime.cancellation_token();
    let first = owner.child();
    let second = owner.child();

    first.cancel();
    assert!(first.is_cancelled());
    assert!(!second.is_cancelled());
    assert!(!owner.is_cancelled());
}

#[tokio::test]
async fn propagation_does_not_depend_on_holding_intermediate_tokens() {
    let runtime = runtime();
    let leaf = {
        let intermediate = runtime.cancellation_token();
        intermediate.child()
        // `intermediate` is dropped here; the leaf must still be reachable
        // from the runtime root.
    };
    assert!(!leaf.is_cancelled());
    runtime
        .shutdown(Duration::from_secs(1))
        .await
        .expect("close");
    assert!(leaf.is_cancelled());
}

#[tokio::test]
async fn shutdown_fires_every_token_rooted_in_the_runtime() {
    let runtime = runtime();
    let token = runtime.cancellation_token().child();
    assert!(!token.is_cancelled());
    runtime
        .shutdown(Duration::from_secs(1))
        .await
        .expect("close");
    assert!(token.is_cancelled());
}
