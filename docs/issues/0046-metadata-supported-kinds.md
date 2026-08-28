# MetadataEdit supported kinds

**Status:** implemented
**Branch:** `issue/0046-metadata-supported-kinds`
**Related:** `docs/issues/0031-simple-groups-real-relay-demo.md`

## Decision

`MetadataEdit::supported_kinds` is `Option<Vec<Kind>>`. `None` omits the
NIP-29 `supported_kinds` tag and declares all kinds supported. `Some(vec![])`
emits its explicit one-cell tag and declares no kinds supported. Every supplied
`Kind` is rendered as its exact decimal kind number in caller order, including
repetitions. The metadata constructor owns this protocol tag; applications do
not reopen its event body merely to add it.

## Scope

The slice changes only the typed kind-9002 metadata body. It retains generic
event-building bounds and adds no protocol-specific count, deduplication,
ordering, routing, signing, publication, or relay-state policy.

## Evidence

`management::tests::edit_metadata_kind_and_tags` proves a repeated ordered
kind sequence becomes one exact `supported_kinds` tag. The adjacent tests prove
the empty case remains an explicit tag and `None` omits the tag. The runnable
real-relay demo builds its metadata event through the field with no raw tag
reconstruction.

## Falsifier

The slice is false if `Some(vec![])` omits the tag, if a repeated supplied kind
is removed or reordered, or if the demo again rebuilds metadata solely to add
`supported_kinds`.
