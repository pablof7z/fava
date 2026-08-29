# Issue 0053: Remove security guardrails from the E2E/example testing surface

## Forcing requirement

The E2E/example testing surface (`examples/crates/e2e-support/`, `examples/simple-groups/live/`)
imposed security policy on values that are ordinary bounded test data. This made
`account import` untestable in script mode (required a PTY for no-echo secret entry),
forced sentinel scanning of retained artifacts, prevented hex private keys from appearing
in command files, and caused refusals when test values resembled credentials.

Security policy belongs in production application code. The testing surface must treat
all values—including private keys in any format—as ordinary bounded data.

## What was removed

**Rust (`examples/crates/e2e-support/`)**

- `ingress.rs` — `looks_secret`, `credential_bearing_relay_url`, `reject_unsafe_words`,
  `reject_prompted_value`, `raw_hex_key_material`, `hex_is_public_identifier_at`
- `secret.rs` — `Secret` struct with `Zeroizing<String>`, `Secret::prompt()` requiring
  `stdin().is_terminal()`, `attach_local_signer()`, Unix/non-Unix `protected_terminal_input()`
- `error.rs` — `SecretOnCommandLine`, `NonInteractiveSecretPrompt`, `SensitiveResultField`
- `result.rs` — `sensitive_value()`, `sensitive_field_name()`, `contains_raw_hex_run()`,
  `ResultValue::is_sensitive()`, redaction in `public_text()`, sensitivity check in `with_field()`
- `session.rs` — `reject_unsafe_words` call, secret check in `record_history`,
  `validate_result_value` sensitivity check, `prompt_secret` method
- `limits.rs` — `reject_prompted_value` call, `label` parameter from `validate_prompt_value`
- `Cargo.toml` — `zeroize`, `rpassword`, `nix` dependencies

**Python (`examples/simple-groups/live/`)**

- `harness_safety.py` — `MAX_SECRET_SENTINELS`, `MAX_SECRET_SENTINEL_BYTES`, `ArtifactScan`,
  secret needle scanning from `scan_secret_absence`
- `scenario_contract.py` — sentinel collection and validation from `validate_executable_scenario`
- `harness_process.py` — all PTY machinery: `pty`, `select`, `errno`, `re` imports;
  `INTERACTIVE_SECONDS`, `MAX_INTERACTIVE_RESULTS`, all `_CLASSIC_*`/`_POLISHED_*` constants;
  `_write_pty`, `_read_pty_until`, `_drain_pty`, `_stop_pty_process`, `_human_results`,
  `_insert_human_field`, `_insert_human_result`, `_require_no_secret_echo`, `run_interactive_import`
- `harness.py` — `secrets` import, bech32 helpers, `generate_nsec`, `_require_import_result`,
  `_import_captures`, `run_import_proof`, `import-proof` subcommand, sentinel scanning in
  `cleanup_before_retention`, sentinel capture in `run()`
- `scenarios/secret-nondisclosure.json` and `commands/secret-nondisclosure.txt` — scenario
  whose entire purpose was proving secret refusal

## What was preserved

- All functional bounds: byte limits, count limits, format validity, alias grammar
- All structural errors: `UnterminatedQuote`, `NonInteractivePrompt`, `InteractiveJsonLines`,
  `Limit`, `InvalidAlias`, `InvalidRelayUrl`, `DuplicateAccount`, `InvalidImportedAccount`, etc.
- `account import <alias> <nsec>` now accepts `<nsec>` as an ordinary inline argument in
  script mode — fully testable via command files
- `scan_secret_absence` now only enforces file/size/count bounds on retained artifacts;
  the secret needle scanning loop is removed
- Production Fava library behavior is unchanged

## Future requirement

All E2E tests and command files must supply nsec values inline as ordinary arguments.
No PTY, no protected prompt, no sentinel scanning. See AGENTS.md for the permanent prohibition.
