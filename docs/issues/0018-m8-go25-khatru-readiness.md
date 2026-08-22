# Go 1.25 qualifies the pinned Khatru relay fixture

**Status:** resolved
**Requirements:** `HARD-04`, `HARD-10`
**Authority:** `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md`,
`docs/spec/ARCHITECTURE.md`, and
`docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md`

## Behavior

The checked-in `github.com/fava/canary-khatru-relay` module remains pinned to
Go 1.25.0 and its existing dependency checksums. With a host-selected Go 1.25.x
toolchain, one bounded probe must:

- verify the module checksums and run every Go test and build target with
  `GOTOOLCHAIN=local`;
- start one real Khatru process on an operating-system-selected loopback port
  with nonzero NIP-11 limits;
- observe and validate the live NIP-11 HTTP document within ten seconds; and
- terminate and reap that exact PID within five seconds, leaving no generated
  repository residue.

This is readiness evidence for the later `relay-limit-shortfall` and
second-relay canary closure. It does not claim those later scenarios are built.

## Preservation boundary

The manual toolchain action changed host state only. At continuation start:

- `go version` and `GOTOOLCHAIN=local go version` both reported
  `go1.25.14 darwin/arm64`;
- `go.mod` remained pinned at `go 1.25.0`;
- `stash@{0}` remained
  `5faecf42c0ec903507e3faeb04962f4680a9cb44`;
- `crates/fava/tests/hostile_ingress.rs` remained blob
  `7b9270a3c255a00a8a42e5d1d90294bd662e82ae`; and
- the reconciled post-Plan-02 tracked Rust patch remained SHA-256
  `5c6da3a98c5dbbc074e3f33ff33f1bfc6fae77a52658bf3e4024e93c1bbbc604`.

The plan's older `e7710b...` witness described the pre-Plan-02 dirty patch and
is stale. No source reset or reversion was appropriate.

## Causal RED

Before the probe implementation, the exact module command failed with status
127 because `apps/canary/relays/khatru/probe.sh` did not exist. The missing
executable witness was the intended initial RED; the pinned Go module and relay
source were not edited to manufacture it.

## Automated evidence

The focused command was run from `apps/canary/relays/khatru`:

```text
./probe.sh
all modules verified
? github.com/fava/canary-khatru-relay [no test files]
GO25_KHATRU_TOOLCHAIN: go version go1.25.14 darwin/arm64
GO25_KHATRU_MODULE: PASS go_mod=1.25.0 checksums=verified tests=passed build=passed
GO25_KHATRU_PROCESS: pid=99374 port=57225 stdout_sha256=1b9a55a36324dc2e549e2d3f5937188e6c8d5c0898e863ca5f5688894ad9f14f
GO25_KHATRU_NIP11: ready_ms=14 content_type=application/nostr+json bytes=476 sha256=27bb00d7f7e869748bafe4ae8364d10e76a3a1af133a1eba498039f1f2bf654c
GO25_KHATRU_TEARDOWN: term=sent wait_status=143 wait_ms=25 reaped=true
GO25_KHATRU_READINESS: PASS pid=99374 port=57225 readiness_limit_s=10 stop_limit_s=5
```

The bounded NIP-11 document advertised the exact configured limitations:

```json
{"auth_required":false,"max_limit":17,"max_message_length":4096,"max_subid_length":64,"max_subscriptions":3,"payment_required":false,"restricted_writes":false}
```

The probe also stored the exact PID, port, response headers, bounded document,
stdout, and stderr in its private `/tmp/fava-khatru-probe.*` directory while it
ran. Its exit trap removed that directory after the child and watchdog were
reaped. A repository status check found no generated module, binary, or probe
artifact.

## Named deliberate break

`DELIBERATE_BREAK_GO25_KHATRU_NIP11` temporarily launched the relay with
`max_limit=18` while the witness continued to require 17. The probe failed with
status 1 for the exact causal reason:

```text
invalid NIP-11 max_limit: expected 17, got 18
GO25_KHATRU_READINESS: FAIL relay failed bounded readiness or NIP-11 validation
DELIBERATE_BREAK_GO25_KHATRU_NIP11: EXPECTED FAIL status=1 cause=max_limit-mismatch
```

Restoration reproduced the pre-break `probe.sh` SHA-256
`33a62caf5b2b30f9e4a1b4f2c14c8a881d48a0814c19178e675a083cfe594f3e`
and the same probe passed again.

DELIBERATE_BREAK_GO25_KHATRU_NIP11: PASS the live NIP-11 mismatch killed the bounded readiness witness; restoration matched the pre-break checksum

## Exit gates

- Go 1.25.14 evaluates the unchanged `go 1.25.0` module with
  `GOTOOLCHAIN=local`.
- `go mod verify`, `go test ./...`, and both cache-only and temporary-output
  builds pass.
- One real Khatru process publishes exact nonzero NIP-11 limits within the
  ten-second readiness ceiling.
- `SIGTERM` plus `wait` reaps the recorded child within the five-second stop
  ceiling; the watchdog and temporary directory are also reaped.
- The reconciled Rust WIP hash, hostile-ingress blob, and preserved stash object
  remain unchanged.

GO25_KHATRU_READINESS: PASS Go 1.25.14 verified the pinned module and a bounded live Khatru process
