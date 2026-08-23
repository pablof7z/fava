//! Owner-level evidence for task ownership, cancellation, and shutdown joins.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use fava_runtime::{Cancellation, Runtime, SpawnRefusal};

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
    let runtime = Runtime::new();
    let ticks = Arc::new(AtomicUsize::new(0));

    for _ in 0..3 {
        let cancel = runtime.cancellation().child();
        let ticks = Arc::clone(&ticks);
        runtime
            .spawn("cooperative", async move {
                loop {
                    ticks.fetch_add(1, Ordering::SeqCst);
                    tokio::select! {
                        () = cancel.cancelled() => break,
                        () = tokio::time::sleep(Duration::from_millis(1)) => {}
                    }
                }
            })
            .expect("runtime accepts owned work");
    }

    assert!(settle(|| ticks.load(Ordering::SeqCst) > 3).await);
    assert_eq!(runtime.live_tasks(), 3);

    let report = runtime.shutdown(Duration::from_secs(2)).await;
    assert_eq!(report.joined(), 3, "every owned task is joined");
    assert_eq!(report.unjoined(), 0);
    assert!(report.is_clean());
    assert_eq!(runtime.live_tasks(), 0);

    let after = ticks.load(Ordering::SeqCst);
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(
        ticks.load(Ordering::SeqCst),
        after,
        "no owned task runs after shutdown returns"
    );
}

#[tokio::test]
async fn repeated_shutdown_is_harmless() {
    let runtime = Runtime::new();
    runtime.spawn("brief", async {}).expect("spawn accepted");

    let first = runtime.shutdown(Duration::from_secs(2)).await;
    assert!(first.is_clean());
    let second = runtime.shutdown(Duration::from_secs(2)).await;
    assert!(second.is_clean());
    assert_eq!(second.joined(), 0);
}

#[tokio::test]
async fn shutdown_attributes_the_exact_task_that_outlived_the_deadline() {
    let runtime = Runtime::new();

    let cancel = runtime.cancellation().child();
    runtime
        .spawn("cooperative", async move { cancel.cancelled().await })
        .expect("spawn accepted");
    runtime
        .spawn("stubborn", std::future::pending::<()>())
        .expect("spawn accepted");

    let report = runtime.shutdown(Duration::from_millis(100)).await;
    assert_eq!(report.joined(), 1);
    assert_eq!(report.unjoined(), 1);
    assert!(!report.is_clean());
    let unjoined: Vec<&'static str> = report.unjoined_tasks().iter().map(|id| id.label()).collect();
    assert_eq!(unjoined, vec!["stubborn"]);
}

#[tokio::test]
async fn spawn_after_shutdown_is_refused() {
    let runtime = Runtime::new();
    runtime.shutdown(Duration::from_secs(1)).await;
    assert_eq!(
        runtime.spawn("late", async {}).unwrap_err(),
        SpawnRefusal::Closed
    );
}

#[tokio::test]
async fn spawn_beyond_the_declared_task_capacity_is_refused() {
    let runtime = Runtime::with_task_capacity(1);
    let cancel = runtime.cancellation().child();
    runtime
        .spawn("first", async move { cancel.cancelled().await })
        .expect("spawn accepted");
    assert_eq!(
        runtime.spawn("second", std::future::pending::<()>()).unwrap_err(),
        SpawnRefusal::AtCapacity { capacity: 1 }
    );
    runtime.shutdown(Duration::from_secs(1)).await;
}

#[tokio::test]
async fn panicking_task_is_attributed_and_leaves_unrelated_work_running() {
    let runtime = Runtime::new();
    let done = Arc::new(AtomicUsize::new(0));

    runtime
        .spawn("panicking", async { panic!("provider task exploded") })
        .expect("spawn accepted");

    let unrelated = Arc::clone(&done);
    runtime
        .spawn("unrelated", async move {
            unrelated.fetch_add(1, Ordering::SeqCst);
        })
        .expect("spawn accepted");

    assert!(settle(|| runtime.panicked_tasks().len() == 1).await);
    let panicked = runtime.panicked_tasks();
    assert_eq!(panicked[0].label(), "panicking");

    assert!(settle(|| done.load(Ordering::SeqCst) == 1).await);

    let report = runtime.shutdown(Duration::from_secs(2)).await;
    assert_eq!(report.panicked(), 1);
    assert_eq!(report.unjoined(), 0, "a panicking task never blocks shutdown");
}

#[tokio::test]
async fn cancellation_propagates_from_the_owner_token_to_its_children() {
    let owner = Cancellation::new();
    let child = owner.child();
    let grandchild = child.child();

    assert!(!grandchild.is_cancelled());
    owner.cancel();
    assert!(child.is_cancelled());
    assert!(grandchild.is_cancelled());

    tokio::time::timeout(Duration::from_secs(1), grandchild.cancelled())
        .await
        .expect("a cancelled token wakes its waiters");
}

#[tokio::test]
async fn a_child_created_after_cancellation_is_already_cancelled() {
    let owner = Cancellation::new();
    owner.cancel();
    assert!(owner.child().is_cancelled());
}

#[tokio::test]
async fn cancelling_a_child_leaves_the_owner_and_its_siblings_running() {
    let owner = Cancellation::new();
    let first = owner.child();
    let second = owner.child();

    first.cancel();
    assert!(first.is_cancelled());
    assert!(!second.is_cancelled());
    assert!(!owner.is_cancelled());
}

#[tokio::test]
async fn shutdown_cancels_the_runtime_root_token() {
    let runtime = Runtime::new();
    let token = runtime.cancellation().child();
    assert!(!token.is_cancelled());
    runtime.shutdown(Duration::from_secs(1)).await;
    assert!(token.is_cancelled());
}
