#!/bin/sh
set -eu
export LC_ALL=C

readonly_paths='Cargo.toml Cargo.lock rust-toolchain.toml .cargo apps/canary crates'
archive_paths='Cargo.toml Cargo.lock rust-toolchain.toml apps/canary crates'
build_command='cargo build --frozen --offline --release --manifest-path apps/canary/Cargo.toml --bin canary'
build_command_sha256=8e010e7b68d708e96ebc25f34935b42d8e6198436a65cf41e27a60c7765bae08
rust_image_tag=rust:1.90-bookworm
green_target_maximum_bytes=4294967296
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
tree_digest_file=$temporary/control/source-tree.sha256
python3 - "$source_checkout" "$revision" "$tree" "$manifest" "$tree_digest_file" <<'PY'
import hashlib
import os
import re
import subprocess
import sys
import tempfile

repository, revision, tree, destination, tree_digest_destination = sys.argv[1:]
scopes = ("Cargo.toml", "Cargo.lock", "rust-toolchain.toml", ".cargo", "apps/canary", "crates")
listing = subprocess.Popen(
    ["git", "-C", repository, "ls-tree", "-r", "-z", "--full-tree", revision, "--", *scopes],
    stdout=subprocess.PIPE,
)
assert listing.stdout is not None
row_spool = tempfile.TemporaryFile()
total = 0
file_count = 0
previous_path = None
buffer = b""
while True:
    chunk = listing.stdout.read(4_096)
    if not chunk:
        break
    buffer += chunk
    while b"\0" in buffer:
        record, buffer = buffer.split(b"\0", 1)
        if not record or len(record) > 1_024:
            raise SystemExit("pinned Git tree row exceeded its bound")
        file_count += 1
        if file_count > 4_096:
            raise SystemExit("pinned compiler-input inventory exceeded 4096 files")
        try:
            identity, raw_path = record.split(b"\t", 1)
            mode, kind, object_id = identity.decode("ascii").split(" ")
            path = raw_path.decode("ascii")
        except (UnicodeDecodeError, ValueError) as error:
            raise SystemExit(f"noncanonical pinned Git tree row: {error}")
        if kind != "blob" or mode not in {"100644", "100755"}:
            raise SystemExit(f"unsupported pinned compiler input mode/type: {path}")
        if not re.fullmatch(r"[0-9a-f]{40}", object_id):
            raise SystemExit(f"noncanonical pinned Git object identity: {path}")
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
        encoded_path = path.encode("ascii")
        if previous_path is not None and encoded_path <= previous_path:
            raise SystemExit("pinned compiler-input paths were not strictly ordered")
        previous_path = encoded_path
        size_output = subprocess.run(
            ["git", "-C", repository, "cat-file", "-s", object_id],
            check=True,
            stdout=subprocess.PIPE,
        ).stdout
        if len(size_output) > 32 or not re.fullmatch(rb"[0-9]+\n", size_output):
            raise SystemExit(f"noncanonical pinned compiler input size: {path}")
        size = int(size_output)
        if size > 8_388_608:
            raise SystemExit(f"pinned compiler input exceeded 8 MiB: {path}")
        total += size
        if total > 67_108_864:
            raise SystemExit("pinned compiler inputs exceeded 64 MiB")
        blob = subprocess.Popen(
            ["git", "-C", repository, "cat-file", "blob", object_id],
            stdout=subprocess.PIPE,
        )
        assert blob.stdout is not None
        digest = hashlib.sha256()
        observed_size = 0
        while True:
            content = blob.stdout.read(65_536)
            if not content:
                break
            observed_size += len(content)
            if observed_size > size:
                blob.kill()
                raise SystemExit(f"pinned compiler input exceeded declared size: {path}")
            digest.update(content)
        if blob.wait() != 0 or observed_size != size:
            raise SystemExit(f"pinned compiler input blob read failed: {path}")
        row_spool.write(
            f"file={mode}\t{digest.hexdigest()}\t{size}\t{path}\n".encode("ascii")
        )
        if row_spool.tell() > 1_048_576:
            raise SystemExit("pinned source manifest rows exceeded 1 MiB")
    if len(buffer) > 1_024:
        listing.kill()
        raise SystemExit("unterminated pinned Git tree row exceeded its bound")
if buffer or listing.wait() != 0:
    raise SystemExit("pinned Git tree listing was incomplete")
if file_count == 0:
    raise SystemExit("pinned compiler-input inventory was empty")
header = (
    "format=fava-pinned-source-v1\n"
    f"revision={revision}\n"
    f"tree={tree}\n"
    f"file_count={file_count}\n"
    f"total_bytes={total}\n"
).encode("ascii")
if len(header) + row_spool.tell() > 1_048_576:
    raise SystemExit("pinned source manifest exceeded 1 MiB")
descriptor = os.open(destination, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
with os.fdopen(descriptor, "wb") as output:
    output.write(header)
    row_spool.seek(0)
    while True:
        chunk = row_spool.read(65_536)
        if not chunk:
            break
        output.write(chunk)
    output.flush()
    os.fsync(output.fileno())

tree_listing = subprocess.Popen(
    ["git", "-C", repository, "ls-tree", "-r", "--full-tree", revision, "--", *scopes],
    stdout=subprocess.PIPE,
)
assert tree_listing.stdout is not None
tree_digest = hashlib.sha256()
tree_listing_bytes = 0
while True:
    chunk = tree_listing.stdout.read(65_536)
    if not chunk:
        break
    tree_listing_bytes += len(chunk)
    if tree_listing_bytes > 1_048_576:
        tree_listing.kill()
        raise SystemExit("pinned source tree listing exceeded 1 MiB")
    tree_digest.update(chunk)
if tree_listing.wait() != 0:
    raise SystemExit("pinned source tree listing failed")
descriptor = os.open(tree_digest_destination, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
with os.fdopen(descriptor, "w", encoding="ascii", newline="\n") as output:
    output.write(tree_digest.hexdigest() + "\n")
    output.flush()
    os.fsync(output.fileno())
PY

manifest_sha256=$(sha256_file "$manifest")
source_tree_sha256=$(tr -d '\r\n' < "$tree_digest_file")
case "$source_tree_sha256" in *[!0-9a-f]*|'') exit 69 ;; esac
if [ "${#source_tree_sha256}" -ne 64 ]; then
  echo "pinned source tree digest was not canonical" >&2
  exit 69
fi

archive=$temporary/control/source.tar
if git -C "$source_checkout" cat-file -e "$revision:.cargo" 2>/dev/null; then
  archive_paths="$archive_paths .cargo"
fi
git -C "$source_checkout" archive --format=tar --output="$archive" "$revision" -- $archive_paths
source_file_count=$(sed -n '4s/^file_count=//p' "$manifest")
source_total_bytes=$(sed -n '5s/^total_bytes=//p' "$manifest")
archive_maximum_bytes=$((source_total_bytes + source_file_count * 2048 + 1048576))
archive_bytes=$(wc -c < "$archive" | tr -d ' ')
if [ "$archive_bytes" -gt "$archive_maximum_bytes" ]; then
  echo "pinned Git archive exceeded its derived bound" >&2
  exit 69
fi
tar -xf "$archive" -C "$temporary/source"
find "$archive" "$tree_digest_file" -type f -delete
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
if [ "$readonly_status" -eq 0 ]; then
  echo "read-only post-build.rs mutation unexpectedly compiled" >&2
  exit 73
fi
if [ ! -f "$readonly_target/toctou-readonly/result" ]; then
  echo "read-only post-build.rs mutation did not retain its result" >&2
  exit 73
fi
if [ "$(sed -n '1p' "$readonly_target/toctou-readonly/result")" != outcome=EROFS ]; then
  echo "read-only post-build.rs mutation did not record EROFS" >&2
  exit 73
fi
if [ -e "$readonly_target/release/canary" ]; then
  echo "read-only post-build.rs mutation left a promoted executable" >&2
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
bin_fingerprint_name=$(basename "$bin_fingerprint_directory")
bin_fingerprint_hash=${bin_fingerprint_name#canary-}
case "$bin_fingerprint_hash" in *[!0-9a-f]*|'') exit 74 ;; esac
if [ "$(dirname "$bin_fingerprint_directory")" != "$break_target/release/.fingerprint" ] \
  || [ "${#bin_fingerprint_hash}" -ne 16 ]; then
  echo "refusing unsafe canary fingerprint invalidation" >&2
  exit 74
fi
find "$bin_fingerprint_directory" -depth -delete
if [ ! -f "$break_target/release/canary" ] || [ -L "$break_target/release/canary" ]; then
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

staging=$output_directory/.fava-pinned-build-staging-$unique
mkdir "$staging"

# The authoritative GREEN subject never occupies a host bind. It is compiled
# into one bounded container-owned tmpfs, measured there, copied while that
# exact container remains alive, then measured again before promotion.
green_name=$container_prefix-green
docker run --detach --name "$green_name" \
  --network none \
  --cap-drop ALL \
  --security-opt no-new-privileges \
  --read-only \
  --tmpfs "/target:rw,exec,nosuid,nodev,size=$green_target_maximum_bytes" \
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
  "$source_image_id" /bin/sh -c \
  "mkdir -p /target/tmp && $build_command && chmod 0500 /target/release/canary && printf '%s\\n' ready > /target/green-ready && exec tail -f /dev/null" \
  >/dev/null
green_ready=0
green_waits=0
while [ "$green_waits" -lt 1200 ]; do
  if docker exec "$green_name" test -f /target/green-ready >/dev/null 2>&1; then
    green_ready=1
    break
  fi
  if [ "$(docker container inspect "$green_name" --format '{{.State.Running}}')" != true ]; then
    docker logs "$green_name" >&2 || true
    echo "final pinned canary build exited before readiness" >&2
    exit 75
  fi
  green_waits=$((green_waits + 1))
  sleep 1
done
if [ "$green_ready" -ne 1 ]; then
  echo "final pinned canary build exceeded its readiness bound" >&2
  exit 75
fi
green_identity_before=$(docker exec "$green_name" /bin/sh -c '
  set -eu
  candidate=/target/release/canary
  test -f "$candidate" && test -x "$candidate" && test ! -L "$candidate"
  grep -a -q "canary failed:" "$candidate"
  ! grep -a -q "canary forged:" "$candidate"
  printf "bytes=%s\n" "$(wc -c < "$candidate" | tr -d " ")"
  printf "sha256=%s\n" "$(sha256sum "$candidate" | sed "s/ .*//")"
')
binary_bytes=$(printf '%s\n' "$green_identity_before" | sed -n '1s/^bytes=//p')
binary_sha256=$(printf '%s\n' "$green_identity_before" | sed -n '2s/^sha256=//p')
case "$binary_bytes" in *[!0-9]*|'') exit 75 ;; esac
case "$binary_sha256" in *[!0-9a-f]*|'') exit 75 ;; esac
if [ "$binary_bytes" -le 0 ] || [ "$binary_bytes" -gt 134217728 ] \
  || [ "${#binary_sha256}" -ne 64 ]; then
  echo "container-owned pinned canary identity exceeded its bound" >&2
  exit 75
fi
docker cp "$green_name:/target/release/canary" "$staging/canary"
green_identity_after=$(docker exec "$green_name" /bin/sh -c '
  set -eu
  candidate=/target/release/canary
  printf "bytes=%s\n" "$(wc -c < "$candidate" | tr -d " ")"
  printf "sha256=%s\n" "$(sha256sum "$candidate" | sed "s/ .*//")"
')
if [ "$green_identity_after" != "$green_identity_before" ] \
  || [ "$(wc -c < "$staging/canary" | tr -d ' ')" != "$binary_bytes" ] \
  || [ "$(sha256_file "$staging/canary")" != "$binary_sha256" ]; then
  echo "copied pinned canary disagreed with its stable container subject" >&2
  exit 75
fi
remove_container "$green_name"

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
  "capabilities": "none",
  "target_storage": "bounded-container-tmpfs",
  "target_maximum_bytes": $green_target_maximum_bytes,
  "subject_digest_origin": "container"
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
