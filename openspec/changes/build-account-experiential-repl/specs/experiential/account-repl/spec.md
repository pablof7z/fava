## Purpose

Defines one focused account application that proves selected-account publication and reactive current-account queries are automatic, exact, and pleasant through Fava’s public API.

## ADDED Requirements

### Requirement: Account lifecycle is usable through one grammar
The application SHALL let users create, import, list, select, replace, and remove bounded local account aliases through public Fava signer and current-account lifecycle APIs. Interactive and replay input SHALL use one parser and dispatcher, and private keys SHALL remain ordinary bounded test data.

#### Scenario: Switch between two accounts
- **WHEN** a user creates or imports two accounts and selects the other alias
- **THEN** the application reports the newly selected public key and later account-dependent work observes that selection

#### Scenario: Replay imports and selects an account
- **WHEN** a command file supplies an inline private key and selects its alias
- **THEN** the real account lifecycle runs without a TTY-only path or content-based suppression

### Requirement: Current-account writes resolve exactly once
A write submitted through the current-account convenience API SHALL resolve the selected public key before acceptance and commit that author to the accepted write. A later switch, signer replacement, or account removal SHALL NOT retarget accepted work.

#### Scenario: Switch after write acceptance
- **WHEN** account A accepts a write and the application selects account B before delivery settles
- **THEN** the resulting event is signed by A and its receipt remains attributed to A

#### Scenario: Publish after switching
- **WHEN** the application selects account B and accepts a later write
- **THEN** the later event is signed by B without the application threading B’s public key into the publication call

#### Scenario: Publish with no current account
- **WHEN** no account is selected and a current-account write is requested
- **THEN** Fava returns an immediate typed refusal before accepting work

### Requirement: `$currentPubkey` is a reactive query input
A declarative query author or tag filter using `$currentPubkey` SHALL bind to the currently selected public key. The same open observation SHALL automatically recompile, reroute, update relay subscriptions, and produce a new current snapshot when account selection changes. Applications SHALL NOT close, rebuild, or reopen the query.

#### Scenario: Open query follows account switch
- **WHEN** an observation using `$currentPubkey` is open for account A and the application selects account B
- **THEN** that observation transitions to B’s dependency graph and current result without a new application observation

#### Scenario: No current account matches nothing
- **WHEN** the selected account is removed or cleared
- **THEN** `$currentPubkey` becomes empty, the open query matches nothing, and it never broadens by dropping the filter

#### Scenario: Rapid switches coalesce to current truth
- **WHEN** selection changes A to B to C while earlier query work is still completing
- **THEN** the observation’s current snapshot and active relay demand reflect C, and late A or B completions cannot overwrite it

### Requirement: Account and signer generations isolate late work
Selection revision and exact signer attachment generation SHALL attribute every asynchronous completion. Replacing a signer for the selected public key SHALL preserve account identity while retiring the old signer generation. Removing or switching an account SHALL retire account-dependent query work without deleting cached public events, accepted writes, or receipts.

#### Scenario: Replace selected signer
- **WHEN** the signer attached to the selected public key is replaced while an old invocation is pending
- **THEN** new writes use the replacement generation and the retired generation cannot produce a current completion

#### Scenario: Remove selected account
- **WHEN** the selected account is removed
- **THEN** current-account writes refuse, reactive queries match nothing, and previously accepted receipts remain inspectable

### Requirement: Developer experience is the primary acceptance gate
Normal application code SHALL only express account commands, test-event intent, and presentation. Manual author propagation, query reconstruction, observation reopening, subscription mutation, route recomputation, signer-generation filtering, or stale-completion filtering SHALL be treated as a blocking public Fava defect.

#### Scenario: App code is audited
- **WHEN** the complete app is reviewed
- **THEN** every account-reactive lifecycle operation is owned by Fava and any remaining app adaptation has an exact falsifiable gap rather than a helper workaround

### Requirement: Output and proof are deterministic
Noninteractive output SHALL be deterministic typed JSONL with bounded scalar interpolation and captures. A bounded harness SHALL run ordinary app commands against real relays and independently inspect exact event authors and query-visible transitions through direct `REQ` and matching `EOSE`; it SHALL NOT construct, sign, route, or publish action events.

#### Scenario: Two-account live proof
- **WHEN** the scenario publishes through A, switches to B, publishes through B, and observes `$currentPubkey`
- **THEN** app evidence reports each accepted author and independent relay evidence proves both exact events while the observation reports only the selected account’s current view
