//! Exact custom-tag Query-to-wire evidence.

use std::num::NonZeroU64;

use fava_query::{ObservationId, Query, QueryBranchId, SingleLetterTag};
use fava_subscriptions::demand_for_query;
use fava_wire::{ClientMessage, SubscriptionId, encode_client};

fn tag(key: char) -> SingleLetterTag {
    SingleLetterTag::from_char(key).expect("ASCII letter tag key")
}

fn encode_query(id: &str, query: &Query) -> String {
    let observation =
        ObservationId::new(NonZeroU64::new(1).expect("non-zero observation identity"));
    let demand = demand_for_query(observation, QueryBranchId::ROOT, query);
    encode_client(&ClientMessage::req(SubscriptionId::new(id), demand.filter))
        .expect("REQ encodes")
}

#[test]
fn lowercase_uppercase_and_utf8_tag_values_encode_exactly() {
    let query = Query::events()
        .tag_values(tag('e'), ["café", "alpha"])
        .tag_values(tag('E'), ["東京"]);

    assert_eq!(
        encode_query("tag-case", &query),
        r##"["REQ","tag-case",{"#e":["alpha","café"],"#E":["東京"]}]"##
    );
}

#[test]
fn present_empty_tag_axis_remains_on_the_wire() {
    let query = Query::events().tag_values(tag('p'), std::iter::empty::<String>());

    assert_eq!(
        encode_query("tag-empty", &query),
        r##"["REQ","tag-empty",{"#p":[]}]"##
    );
}

#[test]
fn duplicate_and_reordered_values_encode_canonically() {
    let left = Query::events()
        .tag_values(tag('e'), ["omega", "alpha", "omega"])
        .tag_values(tag('e'), ["café"]);
    let right = Query::events()
        .tag_values(tag('e'), ["café", "omega"])
        .tag_values(tag('e'), ["alpha", "café"]);

    let left = encode_query("tag-canonical", &left);
    let right = encode_query("tag-canonical", &right);

    assert_eq!(left, right);
    assert_eq!(
        left,
        r##"["REQ","tag-canonical",{"#e":["alpha","café","omega"]}]"##
    );
}
