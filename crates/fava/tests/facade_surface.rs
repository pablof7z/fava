//! Public facade inventory and neutral-contract boundary evidence.

use std::fs;
use std::path::Path;

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
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let facade = fs::read_to_string(root.join("src/lib.rs")).expect("facade source reads");
    let publication =
        fs::read_to_string(root.join("src/publication.rs")).expect("publication source reads");

    let write_exports = public_use_block(&facade, "pub use fava_write::{");
    for removed in ["WriteIntent", "WritePayload"] {
        assert!(
            !write_exports
                .split(|character: char| !character.is_alphanumeric())
                .any(|symbol| symbol == removed),
            "facade still exports {removed}: {write_exports}"
        );
    }
    assert!(
        write_exports
            .split(|character: char| !character.is_alphanumeric())
            .any(|symbol| symbol == "WriteRouting"),
        "Receipt::routing requires the facade WriteRouting export"
    );

    let store_exports = public_use_block(&facade, "pub use fava_write_store::{");
    assert!(
        !store_exports
            .split(|character: char| !character.is_alphanumeric())
            .any(|symbol| symbol == "AcceptedWrite"),
        "facade still exports AcceptedWrite: {store_exports}"
    );
    assert!(
        !publication.contains("impl PublishPayload for WriteIntent"),
        "old publish(WriteIntent) overload remains"
    );
    assert!(
        !facade.contains("pub async fn wait_terminal(&self"),
        "facade-level terminal wait remains"
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
