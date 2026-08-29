# 0051 — Fava owns its shipped protocol materializers

**Status:** in progress
**Owner:** `fava` facade selects shipped protocol materializers; applications
select only materializers for application-defined edit semantics.

## Decision

`fava-nip02`, `fava-bookmarks`, and `fava-simple-groups` own their edit codecs,
but Fava owns their integration with its semantic-write lifecycle. Whenever a
Fava assembly selects publication, it installs those shipped materializers
before recovery. An application publishes the protocol crate's typed edit
without materializer wiring.

`ReplaceableEventMaterializer` remains the public neutral extension contract.
The facade names its registration methods `application_materializer` and
`application_materializers`: only an application that adds its own edit
semantics interacts with that low-level contract. A claimed built-in kind is a
duplicate materializer refusal, never an override.

## Proof

- Facade integration tests publish NIP-02, bookmark, and saved-group edits
  with no application materializer registration.
- The public simple-groups API no longer exports
  `saved_group_list_materializer`.
- Application materializer tests retain custom-kind lifecycle coverage and
  prove built-in-kind collisions refuse during assembly.
