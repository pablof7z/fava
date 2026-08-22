//! Exact repeated-kind Query-to-wire evidence.

use fava_query::{Kind, Query};
use fava_subscriptions::demand_for_query;
use fava_wire::{ClientMessage, SubscriptionId, encode_client};

#[test]
fn repeated_kind_req_encoding_is_complete() {
    let query = Query::events()
        .kind(Kind::from_u16(30_002))
        .kind(Kind::from_u16(30_001))
        .kind(Kind::from_u16(30_002));
    let demand = demand_for_query(SubscriptionId::new("multi-kind"), &query);
    let encoded = encode_client(&ClientMessage::req(demand.subscription_id, demand.filter))
        .expect("REQ encodes");

    assert_eq!(encoded, r#"["REQ","multi-kind",{"kinds":[30001,30002]}]"#);
}
