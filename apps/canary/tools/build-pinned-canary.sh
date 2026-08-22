#!/bin/sh
set -eu
export LC_ALL=C

readonly_paths='Cargo.toml Cargo.lock rust-toolchain.toml .cargo apps/canary crates'
archive_paths='Cargo.toml Cargo.lock rust-toolchain.toml apps/canary crates'
build_command='cargo build --frozen --offline --release --manifest-path apps/canary/Cargo.toml --bin canary'
build_command_sha256=8e010e7b68d708e96ebc25f34935b42d8e6198436a65cf41e27a60c7765bae08
rust_image_tag=rust:1.90-bookworm
temporary=
image_tag=
container_prefix=
staging=

sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

remove_container() {
  if [ -n "$1" ] && docker container inspect "$1" >/dev/null 2>&1; then
    docker container rm --force "$1" >/dev/null
  fi
}

cleanup() {
  status=$?
  trap - EXIT HUP INT TERM
  if [ -n "$container_prefix" ]; then
    remove_container "$container_prefix-readonly"
    remove_container "$container_prefix-break"
    remove_container "$container_prefix-green"
  fi
  if [ -n "$image_tag" ] && docker image inspect "$image_tag" >/dev/null 2>&1; then
    if ! docker image rm "$image_tag" >/dev/null; then
      echo "failed to remove exact pinned source image tag: $image_tag" >&2
      [ "$status" -ne 0 ] || status=74
    fi
  fi
  if [ -n "$staging" ] && [ -d "$staging" ]; then
    case "$staging" in
      */.fava-pinned-build-staging-*) find "$staging" -depth -delete ;;
      *)
        echo "refusing unsafe pinned output staging cleanup: $staging" >&2
        [ "$status" -ne 0 ] || status=75
        ;;
    esac
  fi
  if [ -n "$temporary" ] && [ -d "$temporary" ]; then
    case "$temporary" in
      "${TMPDIR:-/tmp}"/fava-pinned-build.*) find "$temporary" -depth -delete ;;
      *)
        echo "refusing unsafe pinned build cleanup: $temporary" >&2
        [ "$status" -ne 0 ] || status=76
        ;;
    esac
  fi
  exit "$status"
}
trap cleanup EXIT HUP INT TERM

if [ "$#" -ne 2 ]; then
  echo "usage: build-pinned-canary.sh SOURCE_CHECKOUT EMPTY_OUTPUT_DIRECTORY" >&2
  exit 64
fi
command -v git >/dev/null 2>&1
command -v docker >/dev/null 2>&1
command -v python3 >/dev/null 2>&1
command -v tar >/dev/null 2>&1
observed_build_command_sha256=$(printf '%s' "$build_command" | sha256_file /dev/stdin)
if [ "$observed_build_command_sha256" != "$build_command_sha256" ]; then
  echo "canonical pinned build command digest disagreed with its command" >&2
  exit 64
fi

source_argument=$(cd "$1" && pwd -P)
source_checkout=$(git -C "$source_argument" rev-parse --show-toplevel)
source_checkout=$(cd "$source_checkout" && pwd -P)
if [ "$source_argument" != "$source_checkout" ]; then
  echo "pinned build source must name the exact repository root" >&2
  exit 65
fi

output_argument=$2
mkdir -p "$output_argument"
output_directory=$(cd "$output_argument" && pwd -P)
if find "$output_directory" -mindepth 1 -print -quit | grep -q .; then
  echo "pinned build output directory was not empty" >&2
  exit 66
fi

# The compiler-input scope must be a clean view of one exact committed HEAD. The
# Docker context below is materialized from Git objects, never from these paths.
if [ -n "$(git -C "$source_checkout" status --porcelain=v1 --untracked-files=all -- $readonly_paths)" ]; then
  echo "pinned build compiler inputs differed from HEAD" >&2
  exit 67
fi
revision=$(git -C "$source_checkout" rev-parse --verify HEAD)
tree=$(git -C "$source_checkout" rev-parse --verify 'HEAD^{tree}')
case "$revision:$tree" in *[!0-9a-f:]*|'') exit 68 ;; esac
if [ "${#revision}" -ne 40 ] || [ "${#tree}" -ne 40 ]; then
  echo "pinned build Git identity was not canonical SHA-1" >&2
  exit 68
fi

temporary=$(mktemp -d "${TMPDIR:-/tmp}/fava-pinned-build.XXXXXX")
mkdir "$temporary/source" "$temporary/control"
manifest=$temporary/control/source.manifest

# Parse the NUL-delimited object inventory so hostile Git path bytes cannot
# change manifest rows. Hash the exact blob objects before materialization.
python3 - "$source_checkout" "$revision" "$tree" "$manifest" <<'PY'
import hashlib
import os
import re
import subprocess
import sys

repository, revision, tree, destination = sys.argv[1:]
scopes = ("Cargo.toml", "Cargo.lock", "rust-toolchain.toml", ".cargo", "apps/canary", "crates")
listing = subprocess.run(
    ["git", "-C", repository, "ls-tree", "-r", "-z", "--full-tree", revision, "--", *scopes],
    check=True,
    stdout=subprocess.PIPE,
).stdout
rows = []
total = 0
for record in listing.split(b"\0"):
    if not record:
        continue
    try:
        identity, raw_path = record.split(b"\t", 1)
        mode, kind, object_id = identity.decode("ascii").split(" ")
        path = raw_path.decode("ascii")
    except (UnicodeDecodeError, ValueError) as error:
        raise SystemExit(f"noncanonical pinned Git tree row: {error}")
    if kind != "blob" or mode not in {"100644", "100755"}:
        raise SystemExit(f"unsupported pinned compiler input mode/type: {path}")
    if len(path) > 512 or not re.fullmatch(r"[A-Za-z0-9._/+@=-]+", path):
        raise SystemExit(f"unsafe pinned compiler input path: {path!r}")
    parts = path.split("/")
    if any(part in {"", ".", ".."} for part in parts):
        raise SystemExit(f"noncanonical pinned compiler input path: {path}")
    if not (
        path in scopes[:3]
        or path.startswith(".cargo/")
        or path.startswith("apps/canary/")
        or path.startswith("crates/")
    ):
        raise SystemExit(f"out-of-scope pinned compiler input: {path}")
    content = subprocess.run(
        ["git", "-C", repository, "cat-file", "blob", object_id],
        check=True,
        stdout=subprocess.PIPE,
    ).stdout
    size = len(content)
    if size > 8_388_608:
        raise SystemExit(f"pinned compiler input exceeded 8 MiB: {path}")
    total += size
    if total > 67_108_864:
        raise SystemExit("pinned compiler inputs exceeded 64 MiB")
    rows.append((path, mode, hashlib.sha256(content).hexdigest(), size))

rows.sort(key=lambda row: row[0].encode("ascii"))
if not rows or len(rows) > 4_096 or len({row[0] for row in rows}) != len(rows):
    raise SystemExit("pinned compiler-input inventory count was invalid")
lines = [
    "format=fava-pinned-source-v1",
    f"revision={revision}",
    f"tree={tree}",
    f"file_count={len(rows)}",
    f"total_bytes={total}",
]
lines.extend(f"file={mode}\t{digest}\t{size}\t{path}" for path, mode, digest, size in rows)
payload = ("\n".join(lines) + "\n").encode("ascii")
if len(payload) > 1_048_576:
    raise SystemExit("pinned source manifest exceeded 1 MiB")
descriptor = os.open(destination, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
with os.fdopen(descriptor, "wb") as output:
    output.write(payload)
    output.flush()
    os.fsync(output.fileno())
PY

manifest_sha256=$(sha256_file "$manifest")
tree_listing=$temporary/control/source-tree.list
git -C "$source_checkout" ls-tree -r --full-tree "$revision" -- $readonly_paths > "$tree_listing"
source_tree_sha256=$(sha256_file "$tree_listing")

archive=$temporary/control/source.tar
if git -C "$source_checkout" cat-file -e "$revision:.cargo" 2>/dev/null; then
  archive_paths="$archive_paths .cargo"
fi
git -C "$source_checkout" archive --format=tar --output="$archive" "$revision" -- $archive_paths
tar -xf "$archive" -C "$temporary/source"
find "$archive" "$tree_listing" -type f -delete
find "$temporary/source" ! -type f ! -type d -print -quit | grep -q . && {
  echo "pinned Git archive contained a non-regular compiler input" >&2
  exit 69
}
"$temporary/source/apps/canary/tools/verify-pinned-source.sh" \
  "$temporary/source" "$manifest" "$manifest_sha256"

base_image_id=$(docker image inspect "$rust_image_tag" --format '{{.Id}}')
case "$base_image_id" in sha256:*) ;; *) exit 70 ;; esac
base_image_sha256=${base_image_id#sha256:}
case "$base_image_sha256" in *[!0-9a-f]*|'') exit 70 ;; esac
if [ "${#base_image_sha256}" -ne 64 ]; then
  echo "local Rust base image did not have an exact engine SHA-256 ID" >&2
  exit 70
fi

unique=$(basename "$temporary" | tr -cd 'A-Za-z0-9_.-')
image_tag="fava-pinned-canary:$revision-$unique"
container_prefix="fava-pinned-$unique"
iidfile=$temporary/control/source-image.id
docker build --pull=false --no-cache --iidfile "$iidfile" \
  --build-arg "RUST_IMAGE=$base_image_id" \
  --build-arg "FAVA_REVISION=$revision" \
  --build-arg "FAVA_TREE=$tree" \
  --build-arg "FAVA_SOURCE_MANIFEST_SHA256=$manifest_sha256" \
  --file "$temporary/source/apps/canary/pinned-source.Dockerfile" \
  --tag "$image_tag" "$temporary"
source_image_id=$(tr -d '\r\n' < "$iidfile")
inspected_source_image_id=$(docker image inspect "$image_tag" --format '{{.Id}}')
if [ "$source_image_id" != "$inspected_source_image_id" ]; then
  echo "engine-derived source image identities disagreed" >&2
  exit 71
fi
case "$source_image_id" in sha256:*) ;; *) exit 71 ;; esac
source_image_sha256=${source_image_id#sha256:}
case "$source_image_sha256" in *[!0-9a-f]*|'') exit 71 ;; esac
if [ "${#source_image_sha256}" -ne 64 ]; then
  echo "source image did not have an exact engine SHA-256 ID" >&2
  exit 71
fi

for label_and_expected in \
  "org.opencontainers.image.revision=$revision" \
  "org.fava.source-tree=$tree" \
  "org.fava.source-manifest-sha256=$manifest_sha256" \
  "org.fava.rust-base-image=$base_image_id"
do
  label=${label_and_expected%%=*}
  expected=${label_and_expected#*=}
  observed=$(docker image inspect "$source_image_id" --format "{{ index .Config.Labels \"$label\" }}")
  if [ "$observed" != "$expected" ]; then
    echo "pinned source image label disagreed: $label" >&2
    exit 72
  fi
done

common_run() {
  run_name=$1
  target=$2
  shift 2
  if [ ! -d "$target" ]; then
    mkdir "$target"
  fi
  docker run --rm --name "$run_name" \
    --network none \
    --cap-drop ALL \
    --security-opt no-new-privileges \
    --volume "$target:/target" \
    --tmpfs /target/tmp:rw,nosuid,nodev,size=67108864 \
    --env CARGO_INCREMENTAL=0 \
    --env CARGO_TARGET_DIR=/target \
    --env TMPDIR=/target/tmp \
    --env FAVA_CANARY_PINNED_BUILD=1 \
    --env "FAVA_BUILD_REVISION=$revision" \
    --env "FAVA_BUILD_TREE=$tree" \
    --env "FAVA_BUILD_SOURCE_TREE_SHA256=$source_tree_sha256" \
    --env "FAVA_BUILD_SOURCE_MANIFEST_SHA256=$manifest_sha256" \
    --env "FAVA_BUILD_SOURCE_IMAGE_SHA256=$source_image_sha256" \
    --env "FAVA_BUILD_RUST_BASE_IMAGE_SHA256=$base_image_sha256" \
    "$@" "$source_image_id" \
    cargo build --frozen --offline --release \
      --manifest-path apps/canary/Cargo.toml --bin canary
}

readonly_target=$temporary/target-readonly
set +e
common_run "$container_prefix-readonly" "$readonly_target" \
  --read-only \
  --env RUSTC_WRAPPER=/source/apps/canary/tools/pinned-build-toctou-wrapper.sh \
  --env FAVA_PINNED_TOCTOU_MODE=readonly
readonly_status=$?
set -e
if [ "$readonly_status" -eq 0 ] \
  || [ ! -f "$readonly_target/toctou-readonly/result" ] \
  || [ "$(sed -n '1p' "$readonly_target/toctou-readonly/result")" != outcome=EROFS ] \
  || [ -e "$readonly_target/release/canary" ]; then
  echo "read-only post-build.rs mutation proof was not causally refused" >&2
  exit 73
fi

break_target=$temporary/target-break
# Prime the target while the build-script sample is protected by EROFS. Then
# discard only the canary binary fingerprint and recompile that binary against
# the same cached build-script result with the rootfs protection removed. This
# recreates the reviewed post-sample race without weakening build.rs itself.
common_run "$container_prefix-break" "$break_target" --read-only
bin_fingerprints=$(find "$break_target/release/.fingerprint" -type f -name bin-canary -print)
if [ "$(printf '%s\n' "$bin_fingerprints" | sed '/^$/d' | wc -l | tr -d ' ')" -ne 1 ]; then
  echo "could not resolve the one exact canary binary fingerprint" >&2
  exit 74
fi
bin_fingerprint_directory=$(dirname "$bin_fingerprints")
case "$bin_fingerprint_directory" in
  "$break_target"/release/.fingerprint/canary-*) find "$bin_fingerprint_directory" -depth -delete ;;
  *) echo "refusing unsafe canary fingerprint invalidation" >&2; exit 74 ;;
esac
if [ ! -f "$break_target/release/canary" ]; then
  echo "primed canary binary was absent" >&2
  exit 74
fi
find "$break_target/release/canary" -type f -delete
common_run "$container_prefix-break" "$break_target" \
  --env RUSTC_WRAPPER=/source/apps/canary/tools/pinned-build-toctou-wrapper.sh \
  --env FAVA_PINNED_TOCTOU_MODE=writable-break
break_result=$break_target/toctou-writable-break/result
if [ ! -f "$break_result" ] \
  || ! grep -qx 'outcome=compiled-hostile-bytes' "$break_result" \
  || [ "$(sed -n '2s/^bytes=//p' "$break_result")" != "$(wc -c < "$temporary/source/apps/canary/src/main.rs" | tr -d ' ')" ] \
  || [ "$(sed -n '3s/^original_sha256=//p' "$break_result")" != "$(sha256_file "$temporary/source/apps/canary/src/main.rs")" ] \
  || [ "$(sed -n '5s/^restored_sha256=//p' "$break_result")" != "$(sha256_file "$temporary/source/apps/canary/src/main.rs")" ] \
  || [ ! -x "$break_target/release/canary" ] \
  || ! grep -a -q 'canary forged:' "$break_target/release/canary"; then
  echo "writable-root named deliberate break did not compile restored hostile proof" >&2
  exit 74
fi

green_target=$temporary/target-green
common_run "$container_prefix-green" "$green_target" --read-only
green_binary=$green_target/release/canary
if [ ! -f "$green_binary" ] || [ ! -x "$green_binary" ] \
  || ! grep -a -q 'canary failed:' "$green_binary" \
  || grep -a -q 'canary forged:' "$green_binary"; then
  echo "final pinned canary binary did not contain exact clean source bytes" >&2
  exit 75
fi
binary_sha256=$(sha256_file "$green_binary")
source_file_count=$(sed -n '4s/^file_count=//p' "$manifest")
source_total_bytes=$(sed -n '5s/^total_bytes=//p' "$manifest")

staging=$output_directory/.fava-pinned-build-staging-$unique
mkdir "$staging"
cp "$green_binary" "$staging/canary"
cp "$manifest" "$staging/pinned-source.manifest"
cat > "$staging/pinned-build.json" <<EOF
{
  "schema": "fava-pinned-build-v1",
  "fava_revision": "$revision",
  "fava_build_tree": "$tree",
  "fava_build_source_tree_sha256": "$source_tree_sha256",
  "fava_build_source_manifest_sha256": "$manifest_sha256",
  "fava_build_source_image_sha256": "$source_image_sha256",
  "rust_base_image_sha256": "$base_image_sha256",
  "build_command_sha256": "$build_command_sha256",
  "fava_canary_executable_sha256": "$binary_sha256",
  "source_file_count": $source_file_count,
  "source_total_bytes": $source_total_bytes,
  "toctou_read_only_attempt": "EROFS",
  "toctou_deliberate_break": "compiled-hostile-bytes",
  "source_root": "/source",
  "target_root": "/target",
  "network": "none",
  "root_filesystem": "read-only",
  "capabilities": "none"
}
EOF
chmod 0500 "$staging/canary"
chmod 0400 "$staging/pinned-build.json" "$staging/pinned-source.manifest"
python3 - "$staging/canary" "$staging/pinned-build.json" "$staging/pinned-source.manifest" "$staging" <<'PY'
import os
import sys

for path in sys.argv[1:]:
    descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) if os.path.isdir(path) else os.O_RDONLY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)
PY
mv "$staging/canary" "$output_directory/canary"
mv "$staging/pinned-build.json" "$output_directory/pinned-build.json"
mv "$staging/pinned-source.manifest" "$output_directory/pinned-source.manifest"
rmdir "$staging"
staging=
python3 - "$output_directory" <<'PY'
import os
import sys

descriptor = os.open(sys.argv[1], os.O_RDONLY | getattr(os, "O_DIRECTORY", 0))
try:
    os.fsync(descriptor)
finally:
    os.close(descriptor)
PY

printf '%s\n' \
  "fava_revision=$revision" \
  "source_image_sha256=$source_image_sha256" \
  "source_manifest_sha256=$manifest_sha256" \
  "canary_sha256=$binary_sha256" \
  'toctou_read_only_attempt=EROFS' \
  'toctou_deliberate_break=compiled-hostile-bytes'
