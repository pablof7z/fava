# 0052 — Polished simple-groups terminal shell

**Status:** implemented; focused PTY, ANSI-stripped golden, and actual-binary
screenshots recorded
**Owner:** `examples/simple-groups` owns interactive presentation, while
`e2e-support` remains the bounded grammar, ingress, and result owner.
**Related:** [0049 full simple-groups REPL](0049-simple-groups-full-repl.md)

## Decision

The application has two presentation paths over its one existing dispatcher.
Script and non-TTY use continue through the existing buffered runner and retain
their human/JSONL records unchanged. A real TTY uses `reedline` as the public
line editor. It owns line editing, in-process capped history navigation,
completion menus, usage hints, and syntax highlighting; it is not a generic
shell/plugin layer.

The interactive presenter owns only prompt and rendering facts. Its prompt
shows selected account, selected group, and retained relay count. Its renderer
uses ANSI only when stdout is a TTY and neither `NO_COLOR` nor `--no-color`
disables it. It renders result DTOs as aligned facts, compact paired tables,
and delivery routes, emphasizing acknowledgement/count and elapsed time.
Prompted ordinary values use the same explicit `Limits` policy before the
existing domain dispatcher receives them. The protected account-import prompt
remains the support-owned no-echo path. A terminal-local history adapter drops
secret-shaped lines before `reedline` can retain them.

No vocabulary change is required: this adds neither a Fava public symbol nor a
new cross-crate nominal concept, contract, lifecycle owner, or persisted
entity. `Terminal` is private application presentation.

## Evidence and falsifiers

- A deterministic non-PTY JSONL golden proves the ordinary dispatcher keeps
  the exact bytes of its typed records; existing black-box tests cover every
  command family.
- A focused PTY test drives the actual binary, strips terminal control
  sequences, and compares the plain interactive transcript with a golden
  record; it also proves `NO_COLOR` removes SGR color codes.
- Unit tests prove command/option completions and protected-shape history
  exclusion.
- [Terminal session](0052/terminal-session.png) and
  [completion menu](0052/terminal-completion.png) are captures from the actual
  debug binary in a 100-column PTY, not hand-authored mockups.

Removing `reedline`, allowing secret-looking text into editor history, routing
non-TTY through ANSI rendering, changing a JSONL byte, or bypassing the
existing prompt-value policy must fail focused tests.
