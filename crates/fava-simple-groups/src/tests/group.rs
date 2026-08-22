use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use fava_state::RelayUrl;

use crate::{Group, GroupError};

fn relay(url: &str) -> RelayUrl {
    RelayUrl::parse(url).expect("test relay URL")
}

#[test]
fn group_construction_refuses_empty_oversized_and_infinite_hosts() {
    let host = relay("wss://groups.example");

    assert_eq!(Group::on(Vec::<RelayUrl>::new(), "photos"), Err(GroupError::EmptyHosts));

    let duplicate_bound_plus_one = vec![host.clone(); 257];
    assert_eq!(
        Group::on(duplicate_bound_plus_one, "photos"),
        Err(GroupError::TooManyHosts {
            actual: 257,
            maximum: 256,
        })
    );

    let distinct_bound_plus_one = (0..257)
        .map(|index| relay(&format!("wss://host-{index}.example")))
        .collect::<Vec<_>>();
    assert_eq!(
        Group::on(distinct_bound_plus_one, "photos"),
        Err(GroupError::TooManyHosts {
            actual: 257,
            maximum: 256,
        })
    );

    let pulls = Arc::new(AtomicUsize::new(0));
    let observed_pulls = Arc::clone(&pulls);
    let infinite = std::iter::repeat(host.clone()).inspect(move |_| {
        let pull = observed_pulls.fetch_add(1, Ordering::SeqCst) + 1;
        assert!(pull <= 257, "group constructor pulled beyond bound+1");
    });
    assert_eq!(
        Group::on(infinite, "photos"),
        Err(GroupError::TooManyHosts {
            actual: 257,
            maximum: 256,
        })
    );
    assert_eq!(pulls.load(Ordering::SeqCst), 257);

    assert_eq!(Group::on([host.clone()], ""), Err(GroupError::EmptyId));
    assert_eq!(
        Group::on([host.clone()], "x".repeat(4_097)),
        Err(GroupError::GroupIdTooLong {
            bytes: 4_097,
            maximum: 4_096,
        })
    );
    let maximum_id = "x".repeat(4_096);
    assert_eq!(
        Group::on([host], maximum_id.clone())
            .expect("maximum-sized opaque id")
            .id(),
        maximum_id
    );
}

#[test]
fn group_construction_preserves_first_occurrence_order() {
    let first = relay("wss://z.example");
    let second = relay("wss://a.example");
    let third = relay("wss://m.example");
    let group = Group::on(
        [
            first.clone(),
            second.clone(),
            first.clone(),
            third.clone(),
            second.clone(),
        ],
        " photos ",
    )
    .expect("bounded hosts normalize");

    assert_eq!(group.hosts().collect::<Vec<_>>(), vec![first, second, third]);
    assert_eq!(group.id(), " photos ");
}
