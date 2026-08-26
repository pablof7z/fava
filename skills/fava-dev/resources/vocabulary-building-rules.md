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
