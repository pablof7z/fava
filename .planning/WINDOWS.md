---
schema_version: 1
open_count: 5
waived_count: 0
fixed_count: 0
total_count: 5
last_updated: 2026-08-22T09:43:55.793Z
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
  }
]
````
