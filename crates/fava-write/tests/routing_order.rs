//! Neutral ordered explicit-route evidence.

use fava_state::RelayUrl;
use fava_write::{WriteIntentError, WriteRouting};

#[test]
fn explicit_route_preserves_first_occurrences() {
    let first = relay("first");
    let second = relay("second");

    let routing = WriteRouting::explicit([first.clone(), second.clone(), first.clone()])
        .expect("duplicate identities normalize");

    assert_eq!(routing, WriteRouting::Explicit(vec![first, second]));
}

#[test]
fn explicit_route_refuses_empty_and_over_bound_inputs() {
    assert_eq!(
        WriteRouting::explicit(std::iter::empty::<RelayUrl>()),
        Err(WriteIntentError::EmptyExplicitRelays)
    );
    assert_eq!(
        WriteRouting::explicit((0..257).map(|index| relay(&format!("relay-{index}")))),
        Err(WriteIntentError::TooManyExplicitRelays {
            actual: 257,
            maximum: 256,
        })
    );

    let repeated = relay("repeated");
    assert_eq!(
        WriteRouting::explicit(std::iter::repeat(repeated)),
        Err(WriteIntentError::TooManyExplicitRelays {
            actual: 257,
            maximum: 256,
        })
    );
}

#[test]
fn ordered_route_has_a_stable_serde_shape() {
    let routing =
        WriteRouting::explicit([relay("second"), relay("first")]).expect("route validates");
    let encoded = serde_json::to_string(&routing).expect("route serializes");
    let decoded: WriteRouting = serde_json::from_str(&encoded).expect("route deserializes");

    assert_eq!(decoded, routing);
    assert!(encoded.find("second").unwrap() < encoded.find("first").unwrap());
}

fn relay(name: &str) -> RelayUrl {
    RelayUrl::parse(&format!("wss://{name}.example")).expect("relay URL")
}
