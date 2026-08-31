---
schema_version: 1
open_count: 13
waived_count: 0
fixed_count: 0
total_count: 13
last_updated: 2026-08-22T13:04:27.770Z
---

# Broken Windows Ledger

> Cross-phase defect register. With `workflow.windows_enforce` enabled, `/gsd-ship` blocks while `open_count > 0`.
> Waive with `gsd-tools windows waive <id> "<reason>"` (reason required).
> Mark fixed with `gsd-tools windows fixed <id>`.

| id | phase | kind | file | line | description | status | reason | recorded_at | resolved_at |
|----|-------|------|------|------|-------------|--------|--------|-------------|-------------|
| 1 | 07 | deviation | crates/fava/BUILD.bazel |  | Pre-existing //crates/fava:write_bounds target omits direct //crates/fava-routing:lib dependency | open |  | 2026-08-21T09:20:57.780Z |  |
| 2 | 07.1.1 | deviation | tools/check_vocabulary.py |  | Quoted checker diagnostics required candidate-local classification to keep the live vocabulary gate green | open |  | 2026-08-22T09:25:38.891Z |  |
| 3 | 07.1.1 | lint-warning | crates/fava/src/publication.rs | 21 | Pre-existing strict-Clippy struct_field_names warning on Write.write_id blocks Plan 02 full touched-crate Clippy gate | open |  | 2026-08-22T09:43:55.607Z |  |
| 4 | 07.1.1 | lint-warning | crates/fava/tests/write_settlement.rs | 369 | Pre-existing strict-Clippy similar_names warning blocks Plan 02 full touched-crate Clippy gate | open |  | 2026-08-22T09:43:55.699Z |  |
| 5 | 07.1.1 | lint-warning | crates/fava/tests/semantic_write_store.rs | 129 | Pre-existing strict-Clippy needless_pass_by_value warnings block Plan 02 full touched-crate Clippy gate | open |  | 2026-08-22T09:43:55.793Z |  |
| 6 | 07.1.1 | unrun-verify | crates/fava/BUILD.bazel |  | Bazel simple_groups target could not run because neither bazelisk nor bazel is installed; Cargo target and manifest boundary checks passed | open |  | 2026-08-22T10:35:46.242Z |  |
| 7 | 07.1.1 | deviation | crates/fava-simple-groups/Cargo.toml |  | Added existing workspace nostr as a test-only dependency to finalize signed parser fixtures | open |  | 2026-08-22T11:02:33.912Z |  |
| 8 | 07.1.1 | deviation | docs/internals/vocabulary.toml |  | Promoted the nine issue-0019-approved parser symbols required by the live vocabulary gate | open |  | 2026-08-22T11:02:34.003Z |  |
| 9 | 07.1.1 | deviation | docs/internals/vocabulary.toml |  | Promoted the issue-0019-approved SimpleGroups symbol required by the live vocabulary gate | open |  | 2026-08-22T11:51:14.325Z |  |
| 10 | 07.1.1 | deviation | crates/fava-simple-groups/src/records.rs |  | Exposed bound-first source validation privately for lossless saved-list revision | open |  | 2026-08-22T11:51:14.417Z |  |
| 11 | 07.1.1 | deviation | crates/fava/tests/simple_groups/saved.rs |  | Split root-exact facade evidence to remain below the 800-line hard limit | open |  | 2026-08-22T11:51:14.508Z |  |
| 12 | 07.1.1 | deviation | crates/fava/tests/simple_groups/saved.rs |  | Used distinct author coordinates for concurrent edit isolation evidence | open |  | 2026-08-22T11:51:14.603Z |  |
| 13 | 07.1.1 | unrun-verify | (removed) downstream acceptance application `src/artifacts.rs` | 241 | Plan 11 exact all-target strict Clippy is blocked by eight pre-existing warnings outside Plan 11 ownership; exact files and lints are in phase deferred-items.md. The referenced file was deleted with the downstream acceptance application on 2026-08-31; this window needs a human decision on whether it is now moot. | open |  | 2026-08-22T13:04:27.770Z |  |

````json
[
  {
    "id": 1,
    "kind": "deviation",
    "phase": "07",
    "file": "crates/fava/BUILD.bazel",
    "line": null,
    "description": "Pre-existing //crates/fava:write_bounds target omits direct //crates/fava-routing:lib dependency",
    "status": "open",
    "reason": "",
    "recorded_at": "2026-08-21T09:20:57.780Z",
    "resolved_at": null
  },
  {
    "id": 2,
    "kind": "deviation",
    "phase": "07.1.1",
    "file": "tools/check_vocabulary.py",
    "line": null,
    "description": "Quoted checker diagnostics required candidate-local classification to keep the live vocabulary gate green",
    "status": "open",
    "reason": "",
    "recorded_at": "2026-08-22T09:25:38.891Z",
    "resolved_at": null
  },
  {
    "id": 3,
    "kind": "lint-warning",
    "phase": "07.1.1",
    "file": "crates/fava/src/publication.rs",
    "line": 21,
    "description": "Pre-existing strict-Clippy struct_field_names warning on Write.write_id blocks Plan 02 full touched-crate Clippy gate",
    "status": "open",
    "reason": "",
    "recorded_at": "2026-08-22T09:43:55.607Z",
    "resolved_at": null
  },
  {
    "id": 4,
    "kind": "lint-warning",
    "phase": "07.1.1",
    "file": "crates/fava/tests/write_settlement.rs",
    "line": 369,
    "description": "Pre-existing strict-Clippy similar_names warning blocks Plan 02 full touched-crate Clippy gate",
    "status": "open",
    "reason": "",
    "recorded_at": "2026-08-22T09:43:55.699Z",
    "resolved_at": null
  },
  {
    "id": 5,
    "kind": "lint-warning",
    "phase": "07.1.1",
    "file": "crates/fava/tests/semantic_write_store.rs",
    "line": 129,
    "description": "Pre-existing strict-Clippy needless_pass_by_value warnings block Plan 02 full touched-crate Clippy gate",
    "status": "open",
    "reason": "",
    "recorded_at": "2026-08-22T09:43:55.793Z",
    "resolved_at": null
  },
  {
    "id": 6,
    "kind": "unrun-verify",
    "phase": "07.1.1",
    "file": "crates/fava/BUILD.bazel",
    "line": null,
    "description": "Bazel simple_groups target could not run because neither bazelisk nor bazel is installed; Cargo target and manifest boundary checks passed",
    "status": "open",
    "reason": "",
    "recorded_at": "2026-08-22T10:35:46.242Z",
    "resolved_at": null
  },
  {
    "id": 7,
    "kind": "deviation",
    "phase": "07.1.1",
    "file": "crates/fava-simple-groups/Cargo.toml",
    "line": null,
    "description": "Added existing workspace nostr as a test-only dependency to finalize signed parser fixtures",
    "status": "open",
    "reason": "",
    "recorded_at": "2026-08-22T11:02:33.912Z",
    "resolved_at": null
  },
  {
    "id": 8,
    "kind": "deviation",
    "phase": "07.1.1",
    "file": "docs/internals/vocabulary.toml",
    "line": null,
    "description": "Promoted the nine issue-0019-approved parser symbols required by the live vocabulary gate",
    "status": "open",
    "reason": "",
    "recorded_at": "2026-08-22T11:02:34.003Z",
    "resolved_at": null
  },
  {
    "id": 9,
    "kind": "deviation",
    "phase": "07.1.1",
    "file": "docs/internals/vocabulary.toml",
    "line": null,
    "description": "Promoted the issue-0019-approved SimpleGroups symbol required by the live vocabulary gate",
    "status": "open",
    "reason": "",
    "recorded_at": "2026-08-22T11:51:14.325Z",
    "resolved_at": null
  },
  {
    "id": 10,
    "kind": "deviation",
    "phase": "07.1.1",
    "file": "crates/fava-simple-groups/src/records.rs",
    "line": null,
    "description": "Exposed bound-first source validation privately for lossless saved-list revision",
    "status": "open",
    "reason": "",
    "recorded_at": "2026-08-22T11:51:14.417Z",
    "resolved_at": null
  },
  {
    "id": 11,
    "kind": "deviation",
    "phase": "07.1.1",
    "file": "crates/fava/tests/simple_groups/saved.rs",
    "line": null,
    "description": "Split root-exact facade evidence to remain below the 800-line hard limit",
    "status": "open",
    "reason": "",
    "recorded_at": "2026-08-22T11:51:14.508Z",
    "resolved_at": null
  },
  {
    "id": 12,
    "kind": "deviation",
    "phase": "07.1.1",
    "file": "crates/fava/tests/simple_groups/saved.rs",
    "line": null,
    "description": "Used distinct author coordinates for concurrent edit isolation evidence",
    "status": "open",
    "reason": "",
    "recorded_at": "2026-08-22T11:51:14.603Z",
    "resolved_at": null
  },
  {
    "id": 13,
    "kind": "unrun-verify",
    "phase": "07.1.1",
    "file": "(removed) downstream acceptance application src/artifacts.rs",
    "line": 241,
    "description": "Plan 11 exact all-target strict Clippy is blocked by eight pre-existing warnings outside Plan 11 ownership; exact files and lints are in phase deferred-items.md. The referenced file was deleted with the downstream acceptance application on 2026-08-31; this window needs a human decision on whether it is now moot.",
    "status": "open",
    "reason": "",
    "recorded_at": "2026-08-22T13:04:27.770Z",
    "resolved_at": null
  }
]
````
