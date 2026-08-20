# Fava internals

`vocabulary.toml` is the source of truth for architectural concepts and the
public Rust symbols that express them.

Protocol terms retain their established Nostr meaning. Fava terms exist only
where Fava owns behavior or state not named by the protocol. Every Fava term
states the nearest Nostr concept and the exact additional distinction.

Run:

```sh
python3 tools/check_vocabulary.py
python3 -m unittest tools/tests/test_vocabulary_check.py
```

The check fails when a workspace crate or public nominal Rust symbol lacks a
definition. Vocabulary changes require a separate, human-approved architecture
change.
