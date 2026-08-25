# State-foundation Bazel evidence

**Result:** PASS — exact `//...` aggregate executed 87/87 tests; 87 passed.

## Source binding

- Tested commit: `cc45b6eda6247ef84680b0704583d7ac8d2d90da`
- Tested tree: `ec0a8286bf9e2be566c92ef954dbc2a55ef133e6`
- Current-main base, verified against `origin/main` at the run:
  `1fe3fd6d5b3ca531182b2524d63fbbcc5aa633a1`
- `crates/fava-routing/BUILD.bazel` SHA-256:
  `670e6c91048e2b566e7207f3763ce11799beee9e053d4cf762a41091f22a4527`
- Required integration edge:
  `//crates/fava-routing:failure_isolation` depends on
  `//crates/fava-relay:lib` at `crates/fava-routing/BUILD.bazel:51`.
- The only tracked working-tree difference during the run is this issue/evidence
  correction. The BEP is an output in this non-package evidence directory.
  Rust, Cargo, Bazel, lock, and toolchain inputs match the tested commit/tree.

## Toolchain binding

- Bazelisk `v1.29.0`; Bazel `9.2.0`
- Rust `1.90.0` (`1159e78c4747b02ef996e55082b704c09b970588`),
  `aarch64-apple-darwin`, LLVM `20.1.8`
- Cargo `1.90.0` (`840b83a10`)
- `rust-toolchain.toml` SHA-256:
  `4e4ea26ca143c3f38dd90c1c1f05a35129e87d677a4046dfae64cace99e43d94`
- `MODULE.bazel.lock` SHA-256:
  `7c52f25384c773a7858b379335c9d5844a44feda6338d34675ae57daa02ebe32`
- `.bazelrc` SHA-256:
  `fe08b2e5bb2175340fa8ce7e16b5d959b9e84fa4b4df9154092e8ae945a029c6`

## Invocation and retained result

- Invocation ID: `b27bd189-813f-4941-bad2-196e665acc9f`
- Command:

```sh
task_bazel_root=$(mktemp -d /tmp/fava-state-current-main-bazel.XXXXXX)
task_bazel_cache=$(mktemp -d /tmp/fava-state-current-main-cache.XXXXXX)
env -i HOME=/Users/pablofernandez \
  PATH=/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin \
  TMPDIR=/tmp USER=pablofernandez \
  /opt/homebrew/bin/bazel --output_user_root="$task_bazel_root" test \
  --disk_cache="$task_bazel_cache" \
  --build_event_binary_file="$PWD/docs/evidence/state-foundation/bazel-87.bep" \
  --noshow_progress --color=no --curses=no --show_result=0 \
  --test_output=errors --test_summary=short //...
```

- Analysis: 171 targets, 339 packages, 9,779 configured targets.
- Execution: 1,169 actions; 87/87 test targets passed; elapsed 136.429 s;
  critical path 36.07 s.
- Binary BEP: [`bazel-87.bep`](bazel-87.bep), 378,008 bytes, SHA-256
  `472e671e46d1d7cc13aca09018409581ff9d16e20fe044ae6182d76bb1f539d7`.
- The BEP contains the invocation, `failure_isolation`, and `fava-relay:lib`
  records. A string audit found no credential assignments; its client
  environment is limited to Bazelisk/Darwin process metadata plus the four
  explicitly supplied variables above.

This evidence closes only the Bazel integration-review rerun. It does not close
or replace the separate human-signature gate.
