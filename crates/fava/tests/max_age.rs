//! Public MaxAge query identity contract.

use std::time::Duration;

use fava::Query;

#[test]
fn max_age_query_identity() {
    let short = Query::events().max_age(Duration::from_secs(1));
    let same = Query::events().max_age(Duration::from_secs(1));
    let long = Query::events().max_age(Duration::from_secs(2));
    assert_eq!(short, same);
    assert_ne!(short, long);
}
