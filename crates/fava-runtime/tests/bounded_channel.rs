//! Owner-level evidence that command and completion traffic is bounded and that
//! exceeding the declared bound is a typed refusal rather than an unbounded queue.

use fava_runtime::{Backpressure, Completion, Generation, ProviderCall, ProviderFailure, bounded_channel};

#[derive(Debug, Eq, PartialEq)]
struct Command(u8);

#[tokio::test]
async fn a_command_channel_refuses_work_beyond_its_declared_capacity() {
    let (sender, mut receiver) = bounded_channel::<Command>(2);
    assert_eq!(sender.capacity(), 2);

    sender.try_send(Command(1)).expect("first command admitted");
    sender.try_send(Command(2)).expect("second command admitted");
    assert_eq!(
        sender.try_send(Command(3)).unwrap_err(),
        Backpressure::Full { capacity: 2 }
    );

    assert_eq!(receiver.recv().await, Some(Command(1)));
    sender.try_send(Command(3)).expect("capacity freed by the owner");
}

#[tokio::test]
async fn a_closed_receiver_is_a_typed_refusal_not_a_silent_drop() {
    let (sender, receiver) = bounded_channel::<Command>(1);
    drop(receiver);
    assert_eq!(sender.try_send(Command(1)).unwrap_err(), Backpressure::Closed);
    assert!(sender.is_closed());
}

#[tokio::test]
async fn a_dropped_sender_ends_the_owner_receive_loop() {
    let (sender, mut receiver) = bounded_channel::<Command>(1);
    sender.try_send(Command(4)).expect("admitted");
    drop(sender);
    assert_eq!(receiver.recv().await, Some(Command(4)));
    assert_eq!(receiver.recv().await, None);
}

#[tokio::test]
async fn completions_travel_on_the_same_bounded_primitive() {
    let (sender, mut receiver) = bounded_channel::<Completion<u8>>(1);
    let call = ProviderCall::new(
        "test-publisher",
        "publish",
        std::time::Duration::from_secs(1),
        Generation::FIRST,
    );
    sender
        .try_send(Completion::failed(&call, ProviderFailure::Cancelled))
        .expect("admitted");
    let completion = receiver.recv().await.expect("a completion arrives");
    assert_eq!(completion.operation(), "publish");
    assert_eq!(completion.into_outcome(), Err(ProviderFailure::Cancelled));
}

#[tokio::test]
async fn a_zero_capacity_request_still_yields_a_usable_bound() {
    let (sender, mut receiver) = bounded_channel::<Command>(0);
    assert_eq!(sender.capacity(), 1);
    sender.try_send(Command(0)).expect("admitted");
    assert_eq!(
        sender.try_send(Command(1)).unwrap_err(),
        Backpressure::Full { capacity: 1 }
    );
    assert_eq!(receiver.recv().await, Some(Command(0)));
}

#[tokio::test]
async fn senders_are_cloneable_and_share_one_bound() {
    let (sender, mut receiver) = bounded_channel::<Command>(1);
    let second = sender.clone();
    sender.try_send(Command(1)).expect("admitted");
    assert_eq!(
        second.try_send(Command(2)).unwrap_err(),
        Backpressure::Full { capacity: 1 }
    );
    assert_eq!(receiver.recv().await, Some(Command(1)));
}
