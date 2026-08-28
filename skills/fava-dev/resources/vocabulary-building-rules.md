# Vocabulary description rules

## Describe only owned behavior

A symbol description covers only what that symbol owns.

- State its purpose, inputs, outputs, behavior, and owned refusals.
- Do not describe dependency limits, policies, lifecycles, or error cases.
- If a dependency can fail, say the symbol forwards its typed error unchanged.
- Name a dependency detail only when the symbol validates, translates, or owns it.
- Keep examples and falsifiers focused on promises made by that symbol.

Wrong:

> `SimpleGroup::events` returns `QueryError` when more than 4,096 relays are supplied.

Right:

> `SimpleGroup::events` narrows a query to this group and forwards `QueryError` unchanged.

The 4,096 limit belongs to `Query`, not `SimpleGroup`.

## Explain the identity plainly

- Lead with purpose and observable behavior.
- Keep the opening description under 20 words when possible.
- Use plain application language.
- Do not restate the symbol name as its description.
- Avoid architecture, ownership, governance, and implementation jargon unless essential to understanding behavior.
- Source claims from goals, specs, code, and tests.

## Explain members by meaning

- Enum variant: say what condition or choice it represents.
- Field: say what value it holds and what that value means.
- Parameter: say what the caller supplies and any rule this symbol applies to it.
- Receiver: mention only when ownership or mutation matters.
- Return value: say what the caller receives.
- Error: include only refusals owned or deliberately translated by this symbol.
- Edge case: include only surprising behavior caused by this symbol.

## Make reviews skimmable

Use this hierarchy:

~~~~markdown
# crate_name (module)
Short module description.

## TypeOrFunction
Short identity description.

### method_or_variant
Short member description.

#### parameters
- **name** `Type` — meaning

#### errors
- **ErrorType** — owned refusal

#### usage sample
```rust
// ordinary application code
```
~~~~

- Omit filenames, governance essays, record counts, implementation archaeology, and dependency internals.
- Prefer bullets over paragraphs.
- Avoid repeating information already clear from the signature.
- Use ordinary application code.
- Label arguments through nearby text.
- Use valid Rust syntax, double-quoted strings, and explicit `vec![...]` values.
- Keep samples focused on the symbol’s normal use.

## Keep a usage sample to the lines that show the thing

A sample is read next to the declaration, by someone deciding whether the name
and shape are right. Every line that is not the thing being shown costs them
attention and buys nothing.

Show the calls. Nothing else:

```rust
let relay = RelayUrl::parse("wss://relay.example")?;
let group = SimpleGroup::new("cats", vec![relay])?;
let keys = Keys::generate();

let event = create_group(keys.public_key(), &group)?;
```

Not this — the imports, the assertion and the helper are scaffolding for the
compiler, not information for the reader:

```rust
use fava_simple_groups::{SimpleGroup, create_group};
use nostr::key::Keys;
use nostr::types::RelayUrl;

let relay = RelayUrl::parse("wss://relay.example")?;
let group = SimpleGroup::new("cats", vec![relay])?;
let keys = Keys::generate();

let event = create_group(keys.public_key(), &group)?;

assert!(has_h_tag(&event, "cats"));
# fn has_h_tag(e: &fava_write::UnsignedEvent, id: &str) -> bool { … }
# Ok::<(), Box<dyn std::error::Error>>(())
```

- **No `use` lines.** Hide them with `#` so the doctest still compiles.
- **No assertions.** An `assert!` proves the sample to the test runner, not to
  the reader; a reader wanting proof reads the tests. If a return value matters,
  the binding shows it.
- **No helper functions.** If a sample needs one to make its point, the sample
  is showing too much.
- **No `Ok::<(), _>(())` tail** in view; hide it.

The sample must still be **real** — taken from code that runs, in the e2e app or
the crate's own tests, and still compiling as a doctest. Minimal and true, not
minimal and invented. Cutting a line that the compiler needs means hiding it
with `#`, never deleting it.
