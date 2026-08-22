#!/bin/sh
set -eu
export LC_ALL=C

if [ "$#" -lt 1 ]; then
  echo "pinned TOCTOU wrapper requires the real rustc path" >&2
  exit 64
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
  prime) exec "$real_rustc" "$@" ;;
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
