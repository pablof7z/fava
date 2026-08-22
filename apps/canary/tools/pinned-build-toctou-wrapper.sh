#!/bin/sh
set -eu
export LC_ALL=C

if [ "$#" -lt 1 ]; then
  echo "pinned TOCTOU wrapper requires the real rustc path" >&2
  exit 64
fi

if [ "$1" = --replay ]; then
  if [ "$#" -ne 2 ] || [ "$0" != /source/apps/canary/tools/pinned-build-toctou-wrapper.sh ]; then exit 64; fi
  exec /usr/bin/python3 - "$0" "$2" <<'PY'
import json
import os
import re
import sys

wrapper, path = sys.argv[1:]
with open(path, "rb") as source:
    raw = source.read(65_537)
claim = json.loads(raw)
if len(raw) > 65_536 or set(claim) != {"argv", "cwd", "env"}:
    raise SystemExit("sampled compiler claim was invalid")
argv, cwd, environment = claim["argv"], claim["cwd"], claim["env"]
if cwd != "/source/apps/canary" or not isinstance(argv, list) or not argv:
    raise SystemExit("sampled compiler cwd/argv was invalid")
if not all(isinstance(value, str) and "\0" not in value for value in argv):
    raise SystemExit("sampled compiler argv was invalid")
if not isinstance(environment, dict) or len(environment) > 128:
    raise SystemExit("sampled compiler environment was invalid")
allowed = re.compile(r"(?:CARGO_[A-Z0-9_]+|FAVA_BUILD_[A-Z0-9_]+|FAVA_CANARY_PINNED_BUILD|PATH|HOME|LD_LIBRARY_PATH|RUSTUP_HOME|RUSTUP_TOOLCHAIN|TMPDIR)")
if any(not isinstance(key, str) or not allowed.fullmatch(key) or not isinstance(value, str) or len(value) > 4096 for key, value in environment.items()):
    raise SystemExit("sampled compiler environment exceeded its allowlist")
for key in ("FAVA_BUILD_REVISION", "FAVA_BUILD_TREE", "FAVA_BUILD_SOURCE_TREE_SHA256", "FAVA_BUILD_SOURCE_MANIFEST_SHA256", "FAVA_BUILD_SOURCE_IMAGE_SHA256", "FAVA_BUILD_RUST_BASE_IMAGE_SHA256"):
    if environment.get(key) != os.environ.get(key): raise SystemExit("sampled compiler identity changed")
if environment.get("CARGO_MANIFEST_DIR") != cwd or environment.get("FAVA_BUILD_SOURCE_CLEAN") != "true" or environment.get("FAVA_BUILD_SOURCE_IMMUTABLE") != "true":
    raise SystemExit("sampled compiler source claim changed")
environment["FAVA_PINNED_TOCTOU_MODE"] = "writable-break"
os.chdir(cwd)
os.execve(wrapper, [wrapper, *argv], environment)
PY
fi

real_rustc=$1
shift
main=/source/apps/canary/src/main.rs
saw_canary=0
saw_main=0
previous=
for argument do
  if [ "$previous" = --crate-name ] && [ "$argument" = canary ]; then
    saw_canary=1
  fi
  case "$argument" in
    /source/apps/canary/src/main.rs|apps/canary/src/main.rs)
      saw_main=1
      ;;
    src/main.rs)
      if [ "$(pwd -P)" = /source/apps/canary ]; then
        saw_main=1
      fi
      ;;
  esac
  previous=$argument
done

if [ "$saw_canary" -ne 1 ] || [ "$saw_main" -ne 1 ]; then
  exec "$real_rustc" "$@"
fi

case "${FAVA_PINNED_TOCTOU_MODE:-}" in
  prime)
    mkdir -p /target/toctou-prime
    /usr/bin/python3 - /target/toctou-prime/rustc.json "$real_rustc" "$@" <<'PY'
import json
import os
import sys

path, *arguments = sys.argv[1:]
allowed = {key: value for key, value in os.environ.items() if key.startswith("CARGO_") or key.startswith("FAVA_BUILD_") or key in {"FAVA_CANARY_PINNED_BUILD", "PATH", "HOME", "LD_LIBRARY_PATH", "RUSTUP_HOME", "RUSTUP_TOOLCHAIN", "TMPDIR"}}
claim = {"argv": arguments, "cwd": os.getcwd(), "env": allowed}
encoded = (json.dumps(claim, separators=(",", ":"), sort_keys=True) + "\n").encode("utf-8")
if not arguments or len(allowed) > 128 or len(encoded) > 65_536:
    raise SystemExit("prime rustc invocation exceeded its bound")
descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
with os.fdopen(descriptor, "wb") as output:
    output.write(encoded)
    output.flush()
    os.fsync(output.fileno())
PY
    exec "$real_rustc" "$@"
    ;;
  readonly|writable-break) mode=$FAVA_PINNED_TOCTOU_MODE ;;
  *)
    echo "pinned TOCTOU wrapper mode was absent or invalid" >&2
    exit 65
    ;;
esac

proof=/target/toctou-$mode
mkdir -p "$proof"
if [ -e "$proof/attempted" ]; then
  echo "pinned TOCTOU wrapper reached the canary input more than once" >&2
  exit 66
fi
: > "$proof/attempted"

set +e
/usr/bin/python3 - "$mode" "$main" "$proof/original-main.rs" "$proof/result" <<'PY'
import errno
import hashlib
import os
import sys

mode, source_path, backup_path, result_path = sys.argv[1:]
needle = b"canary failed:"
replacement = b"canary forged:"
with open(source_path, "rb") as source:
    original = source.read()
if original.count(needle) != 1 or len(needle) != len(replacement):
    raise SystemExit("canary main did not contain the one exact same-size hostile marker")
hostile = original.replace(needle, replacement, 1)
if len(hostile) != len(original):
    raise SystemExit("hostile canary mutation changed the source size")
with open(backup_path, "xb") as backup:
    backup.write(original)
    backup.flush()
    os.fsync(backup.fileno())

try:
    descriptor = os.open(source_path, os.O_WRONLY)
except OSError as error:
    if mode != "readonly" or error.errno != errno.EROFS:
        raise
    with open(result_path, "x", encoding="ascii", newline="\n") as result:
        result.write("outcome=EROFS\n")
        result.write(f"bytes={len(original)}\n")
        result.write(f"original_sha256={hashlib.sha256(original).hexdigest()}\n")
        result.write("wrapper_status=86\n")
        result.flush()
        os.fsync(result.fileno())
    raise SystemExit(86)

try:
    if mode != "writable-break":
        raise SystemExit("read-only proof unexpectedly opened the compiler input for writing")
    written = os.write(descriptor, hostile)
    if written != len(hostile):
        raise SystemExit("hostile canary mutation was short")
    os.fsync(descriptor)
finally:
    os.close(descriptor)

with open(source_path, "rb") as source:
    observed = source.read()
if observed != hostile:
    raise SystemExit("hostile canary bytes were not installed exactly")
with open(result_path, "x", encoding="ascii", newline="\n") as result:
    result.write("outcome=hostile-bytes-installed\n")
    result.write(f"bytes={len(original)}\n")
    result.write(f"original_sha256={hashlib.sha256(original).hexdigest()}\n")
    result.write(f"hostile_sha256={hashlib.sha256(hostile).hexdigest()}\n")
    result.flush()
    os.fsync(result.fileno())
PY
mutation_status=$?
set -e

if [ "$mode" = readonly ]; then
  if [ "$mutation_status" -ne 86 ]; then
    echo "pinned read-only proof did not receive EROFS" >&2
    exit 67
  fi
  echo "pinned build refused the post-build.rs mutation with EROFS" >&2
  exit 86
fi
if [ "$mutation_status" -ne 0 ]; then
  echo "writable-root deliberate break did not install hostile bytes" >&2
  exit 68
fi

restore_source() {
  /usr/bin/python3 - "$main" "$proof/original-main.rs" "$proof/result" <<'PY'
import hashlib
import os
import sys

source_path, backup_path, result_path = sys.argv[1:]
with open(backup_path, "rb") as backup:
    original = backup.read()
with open(source_path, "r+b", buffering=0) as source:
    source.write(original)
    source.flush()
    os.fsync(source.fileno())
    source.seek(0)
    restored = source.read()
if restored != original:
    raise SystemExit("writable-root deliberate break did not restore source bytes")
with open(result_path, "a", encoding="ascii", newline="\n") as result:
    result.write(f"restored_sha256={hashlib.sha256(restored).hexdigest()}\n")
    result.flush()
    os.fsync(result.fileno())
PY
}

restored=0
restore_on_exit() {
  if [ "$restored" -eq 0 ]; then
    restore_source
    restored=1
  fi
}
trap restore_on_exit EXIT HUP INT TERM
set +e
"$real_rustc" "$@"
rustc_status=$?
set -e
restore_on_exit
trap - EXIT HUP INT TERM
if [ "$rustc_status" -ne 0 ]; then
  echo "hostile canary bytes did not compile" >&2
  exit "$rustc_status"
fi
printf '%s\n' 'outcome=compiled-hostile-bytes' >> "$proof/result"
