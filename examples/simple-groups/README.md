# Simple-groups E2E shell

`simple-groups` is the first real consumer of the private
`examples/crates/e2e-support` command shell. It uses only public Fava/provider
APIs and keeps all NIP-29 grammar and publication decisions in this app.

Run it interactively against a controlled NIP-29 relay:

```sh
cargo run --manifest-path examples/simple-groups/Cargo.toml
```

Or pipe a normal command file without a PTY:

```sh
cargo run --manifest-path examples/simple-groups/Cargo.toml -- --jsonl <<'COMMANDS'
relay add group ws://127.0.0.1:18101
account use alice
group create room group
capture room_event event_id
group event publish --kind 12345 "hello from ${room_event}"
dump
quit
COMMANDS
```

`--script path/to/commands.txt` reads the same command lines from a file.
`--jsonl` emits one typed result object per line; the default is human output.

Shared commands own only application-shell state:

```text
relay add <alias> <ws-url>
account use <alice|bob>
capture <alias> <last-result-field>
dump
quit
```

Group commands are intentionally local to this app:

```text
group create <id> <relay-alias>
group use <id>
group event publish --kind <kind> [content]
group delete <id>
group list
```

Creating a group selects it; `group use` changes the selected group. Event
publication accepts every `u16` Nostr kind: neither the shell nor this app
privileges a group-content kind. In interactive use, omitting `--kind` prompts
for it. A command-file replay renders that refusal and exits unsuccessfully
without consuming a later command as its value.

Each create/event publish/delete waits at most 20 seconds for
`fava::all_acknowledged()`. The result records exact `author`, `event_id`, and
`write_id`.
The app starts with disposable Alice and Bob signers registered in Fava; account
selection chooses the unsigned-event author, never exposes a secret.

Secrets may only enter future commands through `e2e_support::Secret::prompt`.
It refuses non-terminal use, never enters command parsing/history, and cannot
be rendered, captured, or dumped. Command-line `nsec`, PEM, and common secret
assignment forms are rejected before history retention or script rendering.

The support package bounds account aliases, relay aliases, captures, history,
command and expanded-command bytes, command arguments, capture values, result
field count, and the last typed result. It intentionally has no domain
registry, plugin protocol, or Fava provider/profile abstraction.
