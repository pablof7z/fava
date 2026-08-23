//! Owner-level evidence for the bounded reconnect policy primitive: growth,
//! ceiling, jitter, and an attempt bound that terminates in a typed shortfall.

use std::time::Duration;

use fava_runtime::Backoff;

fn millis(value: u64) -> Duration {
    Duration::from_millis(value)
}

fn delays(mut backoff: Backoff, count: usize) -> Vec<Duration> {
    (0..count)
        .map(|_| backoff.next_delay().expect("attempt bound not reached"))
        .collect()
}

#[tokio::test]
async fn a_reconnect_delay_grows_and_stops_at_the_ceiling() {
    let backoff = Backoff::new(millis(10), millis(40), 8);
    assert_eq!(
        delays(backoff, 5),
        vec![millis(10), millis(20), millis(40), millis(40), millis(40)]
    );
}

#[tokio::test]
async fn a_declared_growth_factor_is_honoured() {
    let backoff = Backoff::new(millis(10), millis(1_000), 8).with_growth(3);
    assert_eq!(delays(backoff, 4), vec![millis(10), millis(30), millis(90), millis(270)]);
}

#[tokio::test]
async fn the_attempt_bound_terminates_in_a_typed_shortfall() {
    let mut backoff = Backoff::new(millis(10), millis(40), 3);
    assert_eq!(backoff.attempts(), 0);
    for _ in 0..3 {
        backoff.next_delay().expect("within the attempt bound");
    }
    assert_eq!(backoff.attempts(), 3);

    let shortfall = backoff.next_delay().expect_err("the attempt bound is enforced");
    assert_eq!(shortfall.attempts(), 3);
    assert_eq!(shortfall.ceiling(), millis(40));
    assert!(!shortfall.to_string().is_empty());

    backoff
        .next_delay()
        .expect_err("an exhausted policy stays exhausted");
}

#[tokio::test]
async fn resetting_returns_the_policy_to_its_initial_delay() {
    let mut backoff = Backoff::new(millis(10), millis(80), 2);
    assert_eq!(backoff.next_delay(), Ok(millis(10)));
    assert_eq!(backoff.next_delay(), Ok(millis(20)));
    backoff.next_delay().expect_err("bound reached");
    backoff.reset();
    assert_eq!(backoff.attempts(), 0);
    assert_eq!(backoff.next_delay(), Ok(millis(10)));
}

#[tokio::test]
async fn jitter_decorrelates_two_dialers_of_the_same_relay() {
    let first = delays(
        Backoff::new(millis(100), millis(1_000), 8).with_jitter(50, 1),
        6,
    );
    let second = delays(
        Backoff::new(millis(100), millis(1_000), 8).with_jitter(50, 2),
        6,
    );

    assert_ne!(first, second, "distinct seeds must not dial in lockstep");
    for (attempt, delay) in first.iter().enumerate() {
        let base = millis(100 * 2u64.pow(u32::try_from(attempt).unwrap())).min(millis(1_000));
        assert!(*delay <= base, "jitter never exceeds the undithered delay");
        assert!(*delay >= base / 2, "jitter stays within the declared fraction");
    }
}

#[tokio::test]
async fn zero_jitter_is_exact() {
    let first = delays(Backoff::new(millis(10), millis(80), 4).with_jitter(0, 1), 4);
    let second = delays(Backoff::new(millis(10), millis(80), 4).with_jitter(0, 2), 4);
    assert_eq!(first, second);
}

#[tokio::test]
async fn a_ceiling_below_the_initial_delay_is_normalised_to_the_initial_delay() {
    let backoff = Backoff::new(millis(50), millis(10), 3);
    assert_eq!(backoff.ceiling(), millis(50));
    assert_eq!(delays(backoff, 3), vec![millis(50), millis(50), millis(50)]);
}

#[tokio::test]
async fn a_growth_factor_below_one_is_normalised_to_a_constant_delay() {
    let backoff = Backoff::new(millis(10), millis(80), 3).with_growth(0);
    assert_eq!(delays(backoff, 3), vec![millis(10), millis(10), millis(10)]);
}

#[tokio::test]
async fn a_zero_attempt_bound_refuses_the_first_dial() {
    let mut backoff = Backoff::new(millis(10), millis(80), 0);
    let shortfall = backoff.next_delay().expect_err("no attempt is authorised");
    assert_eq!(shortfall.attempts(), 0);
}
