//! Public facade inventory and neutral-contract boundary evidence.

use fava::{Receipt, WriteRouting};
use fava_write::{WriteIntent, WritePayload};
use fava_write_store::AcceptedWrite;

#[test]
fn neutral_contracts_remain_available_to_providers() {
    let _ = std::mem::size_of::<WriteIntent>();
    let _ = std::mem::size_of::<WritePayload>();
    let _ = std::mem::size_of::<AcceptedWrite>();

    fn routing(receipt: &Receipt) -> &WriteRouting {
        &receipt.routing
    }
    let _: fn(&Receipt) -> &WriteRouting = routing;
}

#[test]
fn facade_has_no_write_intent_compatibility_door() {
    let facade = include_str!("../src/lib.rs");
    let publication = include_str!("../src/publication.rs");

    let write_exports = public_use_block(facade, "pub use fava_write::{");
    for removed in ["WriteIntent", "WritePayload"] {
        assert!(
            !write_exports
                .split(|character: char| !character.is_alphanumeric())
                .any(|symbol| symbol == removed),
            "facade still exports {removed}: {write_exports}"
        );
        assert!(
            !facade.contains(&format!("pub use fava_write::{removed}")),
            "facade still singly exports {removed}"
        );
    }
    assert!(
        write_exports
            .split(|character: char| !character.is_alphanumeric())
            .any(|symbol| symbol == "WriteRouting"),
        "Receipt::routing requires the facade WriteRouting export"
    );

    assert!(
        !facade.contains("pub use fava_write_store::AcceptedWrite")
            && !facade.contains("pub use fava_write_store::{AcceptedWrite"),
        "facade still exports AcceptedWrite"
    );
    assert!(
        !publication.contains("impl PublishPayload for WriteIntent"),
        "old publish(WriteIntent) overload remains"
    );
    assert!(
        !facade.contains("pub async fn wait_terminal(&self"),
        "facade-level terminal wait remains"
    );
    for removed in ["pub fn accept_event(", "pub fn preview_write_routes("] {
        assert!(
            !facade.contains(removed),
            "facade still exposes neutral custody method {removed}"
        );
    }
}

#[test]
fn facade_root_stays_below_the_repository_soft_limit() {
    let lines = include_str!("../src/lib.rs").lines().count();
    assert!(
        lines < 500,
        "facade root grew to {lines} lines; split the new cohesive owner before merging"
    );
}

fn public_use_block<'a>(source: &'a str, start: &str) -> &'a str {
    let remainder = source
        .split_once(start)
        .unwrap_or_else(|| panic!("missing public export block {start}"))
        .1;
    remainder
        .split_once("};")
        .unwrap_or_else(|| panic!("unterminated public export block {start}"))
        .0
}
