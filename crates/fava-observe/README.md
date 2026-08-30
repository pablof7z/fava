# fava-observe

`Observation::wait_until(timeout, predicate)` is the bounded predicate wait
for an already-open query. It examines the installed current snapshot before
awaiting later delivery from that exact handle. `Some` is the matching
snapshot, `None` is expiry of the caller-supplied bound, and
`ObservationClosed` remains the error; timing out leaves the observation, its
demand, and a later completion intact.
