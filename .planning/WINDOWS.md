---
schema_version: 1
open_count: 1
waived_count: 0
fixed_count: 0
total_count: 1
last_updated: 2026-08-21T09:20:57.780Z
---

# Broken Windows Ledger

> Cross-phase defect register. With `workflow.windows_enforce` enabled, `/gsd-ship` blocks while `open_count > 0`.
> Waive with `gsd-tools windows waive <id> "<reason>"` (reason required).
> Mark fixed with `gsd-tools windows fixed <id>`.

| id | phase | kind | file | line | description | status | reason | recorded_at | resolved_at |
|----|-------|------|------|------|-------------|--------|--------|-------------|-------------|
| 1 | 07 | deviation | crates/fava/BUILD.bazel |  | Pre-existing //crates/fava:write_bounds target omits direct //crates/fava-routing:lib dependency | open |  | 2026-08-21T09:20:57.780Z |  |

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
  }
]
````
