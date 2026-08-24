# Fava internals

`vocabulary.toml` is the source of truth for architectural concepts, workspace
crate names, implemented public Rust symbols, and public symbols declared by
the specifications.

`vocabulary-candidates.jsonl` is the explicit research packet for names the
registry currently hides under another term. It is not approval. Every record
must be anchored to an exact repository line, and the approval tooling refuses
to synthesize a signable row for a discovered name without that record.

`approvals.jsonl` is append-only signed history. One kind-9999 event approves
only the single `name` tag and exact canonical markdown in its content. Older
events remain preserved after a candidate changes; only an event whose content
exactly matches the final current candidate is authoritative. A parent term's
signature never approves a child candidate.

Protocol terms retain their established Nostr meaning. Fava terms exist only
where Fava owns behavior or state not named by the protocol. Every Fava term
states the nearest Nostr concept and the exact additional distinction.

Run:

```sh
python3 tools/check_vocabulary.py
python3 -m unittest tools.tests.test_vocabulary_check tools.tests.test_vocabulary_approval
cargo build -p fava --bin vocab-verify
python3 tools/approve_vocabulary.py
```

The check fails when a workspace or specified crate, an implemented public
nominal Rust symbol, or a public nominal symbol in `docs/spec/`
lacks a definition. Vocabulary changes require a separate, human-approved
architecture change.
