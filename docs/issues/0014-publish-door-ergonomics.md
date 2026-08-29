# The publish door: scope the publication, then name the payload

**Status:** accepted and completed
**Implementation:** `756fc20` (synchronous payload door and `Write`), `ee38b6d`
(`by`/`to` scopes), `1ec544e` (complete receipt summaries), `771d8f7`
(`Write::settled`)
**Authority:** WRITE-002 through WRITE-005, WRITE-011, WRITE-013, WRITE-018;
`docs/spec/ARCHITECTURE.md` `fava` facade
**Depends on:** `0015` for `PublishAs` and `PublishTo`
**Companion:** `0013` for accepted edit-author ownership

## Adopted application surface

```rust
let write = fava.publish(unsigned_or_presigned)?;
let write = fava.by(alice).publish(edit)?;
let write = fava.to([r1, r2])?.publish(payload)?;
let write = fava.by(alice).to([r1, r2])?.publish(edit)?;

let receipt = write.receipt()?;
let receipt = write.settled(fava::all_terminal()).await?;
let receipt = write.settled(fava::all_acknowledged()).await?;
let receipt = write.settled(fava::at_least(2)?).await?;
let receipt = write.settled(|r| r.acknowledged() >= 2).await?;
```

`publish` is synchronous. It returns `Write` only after durable local acceptance
and immediate query-source visibility; it does not wait for signing, routing,
transport, or relay acknowledgement. Delivery sufficiency is the separate
asynchronous `Write::settled` operation.

An edit requires `by(author)`. `PublishAs::publish` accepts only
`ReplaceableEventEdit`, making a signer scope on an unsigned or pre-signed event
unwritable. `to(...)` accepts every payload form, validates and normalizes the
route before custody, preserves first-occurrence order, and refuses empty or
over-bound routes.

## Current signatures

```rust
impl Fava {
    pub fn publish<P>(&self, payload: P) -> Result<Write, PublishError>;
    pub fn by(&self, author: PublicKey) -> PublishAs<'_>;
    pub fn to(
        &self,
        relays: impl IntoIterator<Item = RelayUrl>,
    ) -> Result<PublishTo<'_>, PublishError>;
}

impl PublishAs<'_> {
    pub fn to(
        self,
        relays: impl IntoIterator<Item = RelayUrl>,
    ) -> Result<Self, PublishError>;
    pub fn publish(self, edit: ReplaceableEventEdit)
        -> Result<Write, PublishError>;
}

impl PublishTo<'_> {
    pub fn by(self, author: PublicKey) -> PublishAs<'_>;
    pub fn publish<P>(self, payload: P) -> Result<Write, PublishError>;
}

impl Write {
    pub const fn write_id(&self) -> WriteId;
    pub const fn receipt_id(&self) -> ReceiptId;
    pub fn receipt(&self) -> Result<Receipt, PublishError>;
    pub async fn settled<F>(&self, predicate: F)
        -> Result<Receipt, PublishError>;
}
```

The payload trait is private. Applications use concrete unsigned, pre-signed,
or edit values and cannot establish another publication door through it.

## Internal boundary

`WriteIntent`, `WritePayload`, and `AcceptedWrite` remain public neutral
cross-crate contracts owned by `fava-write` and `WriteStore`. The `fava` facade
does not re-export or accept them as application payloads. `WriteRouting`
remains visible because receipts expose the durable routing fact.

## Executable evidence

```sh
cargo test -p fava --test publication_door
cargo test -p fava --test publication_scopes
cargo test -p fava --test write_settlement
cargo test -p fava --doc
python3 -m unittest tools.tests.test_vocabulary_check
```

The implementation was causally checked with three restored-source breaks:

- making `publish` wait for receipt terminality caused
  `publish_returns_after_local_acceptance` to miss its 250 ms deadline;
- moving route validation after acceptance made
  `publication_scopes_are_inert_before_valid_payload` observe one custody row;
- erasing destination facts from terminal `NotReached` made
  `settlement_preserves_terminal_receipt_evidence` fail.

Each source checksum was restored before the green gates.

## Decision rationale

The payload is the ordinary vocabulary. `by` and `to` narrow the expression
only when needed, and `Write` is the durable value applications inspect or
await. This keeps acceptance local and immediate while leaving signer, route,
publisher, delivery, and store authority with their existing owners.
