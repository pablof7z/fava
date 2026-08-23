//! Owner-level evidence that command traffic is bounded and that exceeding the
//! declared depth is a typed refusal that hands the work back.

mod support;

use std::time::{Duration, Instant};

use fava_runtime::SendRefusal;

use support::{nonzero, runtime};

#[derive(Debug, Eq, PartialEq)]
struct Command(u8);

#[tokio::test]
async fn a_full_command_channel_refuses_and_returns_the_item() {
    let runtime = runtime();
    let (sender, mut receiver) = runtime.channel::<Command>(nonzero(2));

    sender.try_send(Command(1)).expect("first command admitted");
    sender
        .try_send(Command(2))
        .expect("second command admitted");
    assert_eq!(sender.len(), 2);

    let refused = sender
        .try_send(Command(3))
        .expect_err("the declared depth holds");
    assert_eq!(refused.reason, SendRefusal::Full { depth: 2 });
    assert_eq!(
        refused.value,
        Command(3),
        "the item is handed back, not dropped"
    );

    assert_eq!(receiver.recv().await, Some(Command(1)));
    sender
        .try_send(Command(3))
        .expect("capacity freed by the owner");

    runtime
        .shutdown(Duration::from_secs(1))
        .await
        .expect("close");
}

#[tokio::test]
async fn a_full_channel_refuses_instead_of_parking() {
    let runtime = runtime();
    let (sender, _receiver) = runtime.channel::<Command>(nonzero(1));
    sender.try_send(Command(1)).expect("admitted");

    let started = Instant::now();
    assert!(sender.try_send(Command(2)).is_err());
    assert!(
        started.elapsed() < Duration::from_millis(50),
        "a full channel must not park its caller"
    );

    runtime
        .shutdown(Duration::from_secs(1))
        .await
        .expect("close");
}

#[tokio::test]
async fn send_before_waits_for_capacity_then_gives_up_with_the_item() {
    let runtime = runtime();
    let (sender, mut receiver) = runtime.channel::<Command>(nonzero(1));
    sender.try_send(Command(1)).expect("admitted");

    let refused = sender
        .send_before(Command(2), Duration::from_millis(40))
        .await
        .expect_err("no capacity appears");
    assert_eq!(refused.reason, SendRefusal::DeadlineExpired);
    assert_eq!(refused.value, Command(2));

    assert_eq!(receiver.recv().await, Some(Command(1)));
    sender
        .send_before(Command(2), Duration::from_millis(200))
        .await
        .expect("capacity appeared");
    assert_eq!(receiver.recv().await, Some(Command(2)));

    runtime
        .shutdown(Duration::from_secs(1))
        .await
        .expect("close");
}

#[tokio::test]
async fn a_closed_channel_refuses_and_returns_the_item() {
    let runtime = runtime();
    let (sender, receiver) = runtime.channel::<Command>(nonzero(1));
    drop(receiver);

    let refused = sender
        .try_send(Command(1))
        .expect_err("no receiver remains");
    assert_eq!(refused.reason, SendRefusal::Closed);
    assert_eq!(refused.value, Command(1));
    assert!(sender.is_closed());

    let refused = sender
        .send_before(Command(2), Duration::from_millis(200))
        .await
        .expect_err("no receiver remains");
    assert_eq!(refused.reason, SendRefusal::Closed);

    runtime
        .shutdown(Duration::from_secs(1))
        .await
        .expect("close");
}

#[tokio::test]
async fn a_dropped_sender_ends_the_owner_receive_loop() {
    let runtime = runtime();
    let (sender, mut receiver) = runtime.channel::<Command>(nonzero(1));
    sender.try_send(Command(4)).expect("admitted");
    drop(sender);
    assert_eq!(receiver.recv().await, Some(Command(4)));
    assert_eq!(receiver.recv().await, None);

    runtime
        .shutdown(Duration::from_secs(1))
        .await
        .expect("close");
}

#[tokio::test]
async fn clones_of_one_sender_share_one_declared_depth() {
    let runtime = runtime();
    let (sender, mut receiver) = runtime.channel::<Command>(nonzero(1));
    let second = sender.clone();
    sender.try_send(Command(1)).expect("admitted");
    assert_eq!(
        second
            .try_send(Command(2))
            .expect_err("one depth, not one per sender")
            .reason,
        SendRefusal::Full { depth: 1 }
    );
    assert_eq!(receiver.recv().await, Some(Command(1)));

    runtime
        .shutdown(Duration::from_secs(1))
        .await
        .expect("close");
}

#[tokio::test]
async fn the_default_channel_uses_the_configured_depth() {
    let runtime = runtime();
    let (sender, _receiver) = runtime.default_channel::<Command>();
    assert_eq!(sender.depth(), runtime.config().default_channel_depth.get());
    assert!(sender.is_empty());

    runtime
        .shutdown(Duration::from_secs(1))
        .await
        .expect("close");
}

#[tokio::test]
async fn a_channel_created_after_shutdown_admits_no_command_traffic() {
    let runtime = runtime();
    runtime
        .shutdown(Duration::from_secs(1))
        .await
        .expect("close");
    let (sender, _receiver) = runtime.channel::<Command>(nonzero(4));
    assert_eq!(
        sender
            .try_send(Command(1))
            .expect_err("closed runtime")
            .reason,
        SendRefusal::Closed
    );
}

#[tokio::test]
async fn queued_commands_stay_readable_after_the_owner_stops_admitting() {
    let runtime = runtime();
    let (sender, mut receiver) = runtime.channel::<Command>(nonzero(4));
    sender.try_send(Command(1)).expect("admitted");
    receiver.close();
    assert_eq!(
        sender
            .try_send(Command(2))
            .expect_err("no longer admitting")
            .reason,
        SendRefusal::Closed
    );
    assert_eq!(receiver.try_recv(), Some(Command(1)));
    assert_eq!(receiver.try_recv(), None);

    runtime
        .shutdown(Duration::from_secs(1))
        .await
        .expect("close");
}
