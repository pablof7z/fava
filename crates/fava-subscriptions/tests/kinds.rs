//! Exact repeated-kind Query-to-wire evidence.

use std::num::NonZeroU64;

use fava_query::{Kind, ObservationId, Query, QueryBranchId};
use fava_subscriptions::demand_for_query;
use fava_wire::{ClientMessage, SubscriptionId, encode_client};

fn observation(value: u64) -> ObservationId {
    ObservationId::new(NonZeroU64::new(value).expect("non-zero observation identity"))
}

#[test]
fn bounded_kind_set_req_encoding_is_complete() {
    let query = Query::events()
        .kinds([
            Kind::from_u16(30_002),
            Kind::from_u16(30_001),
            Kind::from_u16(30_002),
        ])
        .expect("three kinds are bounded");
    let demand = demand_for_query(observation(1), QueryBranchId::ROOT, &query);
    let encoded = encode_client(&ClientMessage::req(
        SubscriptionId::new("multi-kind"),
        demand.filter,
    ))
    .expect("REQ encodes");

    assert_eq!(encoded, r#"["REQ","multi-kind",{"kinds":[30001,30002]}]"#);
}
