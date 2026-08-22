#!/bin/sh
set -eu
export LC_ALL=C

readonly_paths='Cargo.toml Cargo.lock rust-toolchain.toml .cargo apps/canary crates'
archive_paths='Cargo.toml Cargo.lock rust-toolchain.toml apps/canary crates'
build_command='cargo build --frozen --offline --release --manifest-path apps/canary/Cargo.toml --bin canary'
build_command_sha256=8e010e7b68d708e96ebc25f34935b42d8e6198436a65cf41e27a60c7765bae08
rust_image_tag=rust:1.90-bookworm
registry_image_ref='registry:2@sha256:a3d8aaa63ed8681a604f1dea0aa03f100d5895b6a58ace528858a7b332415373'
green_target_maximum_bytes=4294967296
docker_deadline_seconds=1200
docker_output_maximum_bytes=8388608
temporary=
image_tag=
probe_image_tag=
subject_image_tag=
probe_image_id=
subject_image_id=
container_prefix=
staging=
readonly_container_id=
registry_container_id=
registry_image_was_present=1

sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

remove_container_id() {
  if [ -z "$1" ]; then
    return 0
  fi
  if container_inspect=$(docker container inspect "$1" --format '{{.Id}}' 2>&1); then
    [ "$container_inspect" = "$1" ] || return 1
    docker container rm --force "$1" >/dev/null 2>&1 || return 1
  else
    case "$container_inspect" in *'No such container'*) return 0 ;; *) return 1 ;; esac
  fi
  if container_inspect=$(docker container inspect "$1" --format '{{.Id}}' 2>&1); then
    return 1
  fi
  case "$container_inspect" in *'No such container'*) return 0 ;; *) return 1 ;; esac
}

remove_cidfile_container() {
  cidfile=$1
  if [ ! -f "$cidfile" ]; then
    return 0
  fi
  container_id=$(tr -d '\r\n' < "$cidfile")
  case "$container_id" in *[!0-9a-f]*|'') return 1 ;; esac
  [ "${#container_id}" -eq 64 ] || return 1
  remove_container_id "$container_id"
}

remove_image_reference() {
  if [ -z "$1" ]; then
    return 0
  fi
  if image_inspect=$(docker image inspect "$1" --format '{{.Id}}' 2>&1); then
    docker image rm "$1" >/dev/null 2>&1 || return 1
  else
    case "$image_inspect" in *'No such image'*) return 0 ;; *) return 1 ;; esac
  fi
  if image_inspect=$(docker image inspect "$1" --format '{{.Id}}' 2>&1); then
    return 1
  fi
  case "$image_inspect" in *'No such image'*) return 0 ;; *) return 1 ;; esac
}

cleanup() {
  status=$?
  trap - EXIT HUP INT TERM
  cleanup_failed=0
  if ! remove_container_id "$readonly_container_id"; then
    cleanup_failed=1
  fi
  if ! remove_container_id "$registry_container_id"; then
    cleanup_failed=1
  fi
  if [ -n "$temporary" ] && [ -d "$temporary/control" ]; then
    for cidfile in "$temporary"/control/*.cid; do
      if [ -e "$cidfile" ] && ! remove_cidfile_container "$cidfile"; then
        cleanup_failed=1
      fi
    done
  fi
  for reference in "$subject_image_tag" "$subject_image_id" "$probe_image_tag" "$probe_image_id" "$image_tag"; do
    if ! remove_image_reference "$reference"; then
      cleanup_failed=1
    fi
  done
  if [ "$registry_image_was_present" -eq 0 ] \
    && ! remove_image_reference "$registry_image_ref"; then
    cleanup_failed=1
  fi
  if [ -n "$staging" ] && [ -d "$staging" ]; then
    case "$staging" in
      */.fava-pinned-build-staging-*)
        if ! find "$staging" -depth -delete; then cleanup_failed=1; fi
        ;;
      *)
        echo "refusing unsafe pinned output staging cleanup: $staging" >&2
        [ "$status" -ne 0 ] || status=75
        ;;
    esac
  fi
  if [ -n "$temporary" ] && [ -d "$temporary" ]; then
    case "$temporary" in
      "${TMPDIR:-/tmp}"/fava-pinned-build.*)
        if ! find "$temporary" -depth -delete; then cleanup_failed=1; fi
        ;;
      *)
        echo "refusing unsafe pinned build cleanup: $temporary" >&2
        [ "$status" -ne 0 ] || status=76
        ;;
    esac
  fi
  if [ "$cleanup_failed" -ne 0 ]; then
    echo "one or more exact pinned build resources could not be cleaned" >&2
    [ "$status" -ne 0 ] || status=77
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
docker buildx version >/dev/null 2>&1
command -v python3 >/dev/null 2>&1
command -v tar >/dev/null 2>&1
command -v curl >/dev/null 2>&1
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

# Keep the bootstrap programs in this shell's memory after reading their exact
# committed Git objects. Later pathname replacement cannot select executable
# helper bytes.
tree_digest_file=$temporary/control/source-tree.sha256
manifest_program=$(git -C "$source_checkout" cat-file blob \
  "$revision:apps/canary/tools/build-pinned-manifest.py")
bounded_runner_program=$(git -C "$source_checkout" cat-file blob \
  "$revision:apps/canary/tools/run-bounded-command.py")
pinned_input_program=$(git -C "$source_checkout" cat-file blob \
  "$revision:apps/canary/tools/run-pinned-input-command.py")
promotion_program=$(git -C "$source_checkout" cat-file blob \
  "$revision:apps/canary/tools/promote-pinned-output.py")
for program_path in \
  build-pinned-manifest.py run-bounded-command.py run-pinned-input-command.py \
  promote-pinned-output.py
do
  program_bytes=$(git -C "$source_checkout" cat-file -s \
    "$revision:apps/canary/tools/$program_path")
  case "$program_bytes" in *[!0-9]*|'') exit 69 ;; esac
  if [ "$program_bytes" -gt 1048576 ]; then
    echo "pinned build helper exceeded 1 MiB: $program_path" >&2
    exit 69
  fi
done
if [ "$(printf '%s\n' "$manifest_program" | wc -c | tr -d ' ')" \
      != "$(git -C "$source_checkout" cat-file -s "$revision:apps/canary/tools/build-pinned-manifest.py")" ] \
  || [ "$(printf '%s\n' "$bounded_runner_program" | wc -c | tr -d ' ')" \
      != "$(git -C "$source_checkout" cat-file -s "$revision:apps/canary/tools/run-bounded-command.py")" ] \
  || [ "$(printf '%s\n' "$pinned_input_program" | wc -c | tr -d ' ')" \
      != "$(git -C "$source_checkout" cat-file -s "$revision:apps/canary/tools/run-pinned-input-command.py")" ] \
  || [ "$(printf '%s\n' "$promotion_program" | wc -c | tr -d ' ')" \
      != "$(git -C "$source_checkout" cat-file -s "$revision:apps/canary/tools/promote-pinned-output.py")" ]; then
  echo "pinned build helper was not canonical single-final-LF text" >&2
  exit 69
fi
python3 -c "$bounded_runner_program" --seconds 120 --bytes 1048576 -- \
  python3 -c "$manifest_program" \
    "$source_checkout" "$revision" "$tree" "$manifest" "$tree_digest_file"

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
python3 -c "$bounded_runner_program" --help >/dev/null

base_image_id=$(docker image inspect "$rust_image_tag" --format '{{.Id}}')
case "$base_image_id" in sha256:*) ;; *) exit 70 ;; esac
base_image_sha256=${base_image_id#sha256:}
case "$base_image_sha256" in *[!0-9a-f]*|'') exit 70 ;; esac
if [ "${#base_image_sha256}" -ne 64 ]; then
  echo "local Rust base image did not have an exact engine SHA-256 ID" >&2
  exit 70
fi
base_image_reference="${rust_image_tag%%:*}@$base_image_id"
if [ "$(docker image inspect "$base_image_id" --format '{{json .RepoDigests}}')" \
  != "[\"$base_image_reference\"]" ]; then
  echo "Rust base image lacked its exact content-addressed repository identity" >&2
  exit 70
fi

unique=$(basename "$temporary" | tr -cd 'A-Za-z0-9_.-' | tr '[:upper:]' '[:lower:]')
container_prefix="fava-pinned-$unique"
if ! docker image inspect "$registry_image_ref" >/dev/null 2>&1; then
  registry_image_was_present=0
  python3 -c "$bounded_runner_program" --seconds 120 --bytes 1048576 -- \
    docker pull "$registry_image_ref"
fi
registry_image_id=$(docker image inspect "$registry_image_ref" --format '{{.Id}}')
case "$registry_image_id" in sha256:*) ;; *) exit 70 ;; esac
registry_cidfile=$temporary/control/registry.cid
registry_container_id=$(python3 -c "$bounded_runner_program" --seconds 120 --bytes 1024 -- \
  docker run --detach --name "$container_prefix-registry" --cidfile "$registry_cidfile" \
    --network bridge --cap-drop ALL --security-opt no-new-privileges \
    --pids-limit 128 --memory 512m --cpus 1 --read-only \
    --tmpfs "/var/lib/registry:rw,nosuid,nodev,size=$green_target_maximum_bytes" \
    --log-driver local --log-opt max-size=1m --log-opt max-file=1 \
    --publish 127.0.0.1::5000 "$registry_image_id")
case "$registry_container_id" in *[!0-9a-f]*|'') exit 70 ;; esac
if [ "${#registry_container_id}" -ne 64 ] \
  || [ "$(tr -d '\r\n' < "$registry_cidfile")" != "$registry_container_id" ] \
  || [ "$(docker container inspect "$registry_container_id" --format '{{.Id}}')" \
    != "$registry_container_id" ] \
  || [ "$(docker container inspect "$registry_container_id" --format '{{.Image}}')" \
    != "$registry_image_id" ]; then
  echo "owned content-addressed registry identity disagreed" >&2
  exit 70
fi
registry_port=$(docker port "$registry_container_id" 5000/tcp | sed -n 's/.*://p')
case "$registry_port" in *[!0-9]*|'') exit 70 ;; esac
if [ "$registry_port" -lt 1024 ] || [ "$registry_port" -gt 65535 ]; then
  echo "owned registry port was outside its exact bound" >&2
  exit 70
fi
registry_ready=0
registry_waits=0
while [ "$registry_waits" -lt 60 ]; do
  if [ "$(curl --fail --silent --show-error --max-time 1 \
    "http://127.0.0.1:$registry_port/v2/" 2>/dev/null || true)" = '{}' ]; then
    registry_ready=1
    break
  fi
  if [ "$(docker container inspect "$registry_container_id" --format '{{.State.Running}}')" \
    != true ]; then
    echo "owned content-addressed registry exited before readiness" >&2
    exit 70
  fi
  registry_waits=$((registry_waits + 1))
  sleep 1
done
if [ "$registry_ready" -ne 1 ]; then
  echo "owned content-addressed registry exceeded its readiness deadline" >&2
  exit 70
fi

source_registry_tag="127.0.0.1:$registry_port/fava-pinned-source:$revision-$unique"
iidfile=$temporary/control/source-image.id
source_dockerfile_path=apps/canary/pinned-source.Dockerfile
source_dockerfile_sha256=$(awk -F '\t' -v path="$source_dockerfile_path" \
  '$4 == path { print $2 }' "$manifest")
case "$source_dockerfile_sha256" in *[!0-9a-f]*|'') exit 71 ;; esac
if [ "${#source_dockerfile_sha256}" -ne 64 ]; then
  echo "pinned source Dockerfile manifest identity was invalid" >&2
  exit 71
fi
python3 -c "$pinned_input_program" \
  --repository "$source_checkout" --revision "$revision" \
  --kind archive --path "$source_dockerfile_path" \
  --archive-prefix source/ --archive-path $archive_paths \
  --extra-file "$manifest" --extra-name control/source.manifest \
  --extra-sha256 "$manifest_sha256" --maximum-input-bytes 83886080 \
  --seconds "$docker_deadline_seconds" \
  --bytes "$docker_output_maximum_bytes" -- \
  docker buildx build --builder colima --push --progress plain \
  --platform linux/arm64 --provenance=false --sbom=false \
  --pull=false --no-cache --network default --iidfile "$iidfile" \
  --resource memory=6g --resource cpu-quota=400000 \
  --ulimit nproc=512 --ulimit nofile=4096:4096 --shm-size 67108864 \
  --build-arg "RUST_IMAGE=$base_image_reference" \
  --build-arg "FAVA_REVISION=$revision" \
  --build-arg "FAVA_TREE=$tree" \
  --build-arg "FAVA_SOURCE_MANIFEST_SHA256=$manifest_sha256" \
  --file source/apps/canary/pinned-source.Dockerfile \
  --tag "$source_registry_tag" -
source_image_id=$(tr -d '\r\n' < "$iidfile")
registry_source_image_id=$(docker buildx imagetools inspect "$source_registry_tag" \
  --format '{{.Manifest.Digest}}')
if [ "$source_image_id" != "$registry_source_image_id" ]; then
  echo "registry-derived source image identities disagreed" >&2
  exit 71
fi
case "$source_image_id" in sha256:*) ;; *) exit 71 ;; esac
source_image_sha256=${source_image_id#sha256:}
case "$source_image_sha256" in *[!0-9a-f]*|'') exit 71 ;; esac
if [ "${#source_image_sha256}" -ne 64 ]; then
  echo "source image did not have an exact engine SHA-256 ID" >&2
  exit 71
fi
source_image_reference="$source_registry_tag@$source_image_id"
python3 -c "$bounded_runner_program" --seconds 120 --bytes 1048576 -- \
  docker pull "$source_image_reference"
image_tag=$source_image_reference
source_manifest_file=$temporary/control/source-registry-manifest.json
python3 -c "$bounded_runner_program" --seconds 120 --bytes 65536 -- \
  docker buildx imagetools inspect "$source_image_reference" --raw > "$source_manifest_file"
source_manifest_claims=$(python3 - "$source_image_id" "$source_manifest_file" <<'PY'
import hashlib
import json
import re
import sys

expected, path = sys.argv[1:]
with open(path, "rb") as source:
    raw = source.read(65_537)
if len(raw) > 65_536 or hashlib.sha256(raw).hexdigest() != expected.removeprefix("sha256:"):
    raise SystemExit("registry manifest bytes disagreed with their digest")
manifest = json.loads(raw)
config = manifest.get("config", {}).get("digest")
if not isinstance(config, str) or not re.fullmatch(r"sha256:[0-9a-f]{64}", config):
    raise SystemExit("registry manifest config identity was invalid")
print(config)
PY
)
source_config_id=$(printf '%s\n' "$source_manifest_claims" | sed -n '1p')
source_engine_id=$(docker image inspect "$source_image_reference" --format '{{.Id}}')
if [ "$source_engine_id" != "$source_config_id" ] \
  && [ "$source_engine_id" != "$source_image_id" ]; then
  echo "pulled source engine identity disagreed with manifest/config identities" >&2
  exit 71
fi

for label_and_expected in \
  "org.opencontainers.image.revision=$revision" \
  "org.fava.source-tree=$tree" \
  "org.fava.source-manifest-sha256=$manifest_sha256" \
  "org.fava.rust-base-image=$base_image_reference"
do
  label=${label_and_expected%%=*}
  expected=${label_and_expected#*=}
  observed=$(docker image inspect "$source_image_reference" --format "{{ index .Config.Labels \"$label\" }}")
  if [ "$observed" != "$expected" ]; then
    echo "pinned source image label disagreed: $label" >&2
    exit 72
  fi
done

common_run() {
  run_name=$1
  target=$2
  cidfile=$temporary/control/$run_name.cid
  shift 2
  if [ ! -d "$target" ]; then
    mkdir "$target"
  fi
  python3 -c "$bounded_runner_program" \
    --seconds "$docker_deadline_seconds" \
    --bytes "$docker_output_maximum_bytes" -- \
    docker run --rm --name "$run_name" --cidfile "$cidfile" \
    --user 0:0 \
    --network none \
    --cap-drop ALL \
    --security-opt no-new-privileges \
    --pids-limit 512 \
    --memory 6g \
    --cpus 4 \
    --log-driver local \
    --log-opt max-size=1m \
    --log-opt max-file=1 \
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
    "$@" "$source_image_reference" \
    cargo build --frozen --offline --release \
      --manifest-path apps/canary/Cargo.toml --bin canary
}

readonly_name=$container_prefix-readonly
readonly_cidfile=$temporary/control/readonly.cid
readonly_container_id=$(python3 -c "$bounded_runner_program" --seconds 120 --bytes 1024 -- \
  docker run --detach --name "$readonly_name" --cidfile "$readonly_cidfile" \
  --user 0:0 \
  --network none \
  --cap-drop ALL \
  --security-opt no-new-privileges \
  --pids-limit 512 \
  --memory 6g \
  --cpus 4 \
  --log-driver local \
  --log-opt max-size=1m \
  --log-opt max-file=1 \
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
  --env RUSTC_WRAPPER=/source/apps/canary/tools/pinned-build-toctou-wrapper.sh \
  --env FAVA_PINNED_TOCTOU_MODE=readonly \
  "$source_image_reference" /bin/sh -c \
  "mkdir -p /target/tmp; $build_command; status=\$?; printf '%s\\n' \"\$status\" > /target/readonly-status; printf '%s\\n' complete > /target/readonly-complete; exec tail -f /dev/null" \
)
case "$readonly_container_id" in *[!0-9a-f]*|'') exit 73 ;; esac
if [ "${#readonly_container_id}" -ne 64 ] \
  || [ "$(tr -d '\r\n' < "$readonly_cidfile")" != "$readonly_container_id" ] \
  || [ "$(docker container inspect "$readonly_container_id" --format '{{.Id}}')" != "$readonly_container_id" ]; then
  echo "read-only proof container identity was not exact" >&2
  exit 73
fi
readonly_complete=0
readonly_waits=0
while [ "$readonly_waits" -lt 1200 ]; do
  if docker exec "$readonly_container_id" test -f /target/readonly-complete >/dev/null 2>&1; then
    readonly_complete=1
    break
  fi
  if [ "$(docker container inspect "$readonly_container_id" --format '{{.State.Running}}')" != true ]; then
    docker logs --tail 8 "$readonly_container_id" >&2 || true
    echo "read-only post-build.rs mutation exited before recording its result" >&2
    exit 73
  fi
  readonly_waits=$((readonly_waits + 1))
  sleep 1
done
if [ "$readonly_complete" -ne 1 ]; then
  echo "read-only post-build.rs mutation exceeded its readiness bound" >&2
  exit 73
fi
readonly_status=$(docker exec "$readonly_container_id" sed -n '1p' /target/readonly-status)
case "$readonly_status" in *[!0-9]*|'') exit 73 ;; esac
if [ "$readonly_status" -eq 0 ]; then
  echo "read-only post-build.rs mutation unexpectedly compiled" >&2
  exit 73
fi
if ! docker exec "$readonly_container_id" test -f /target/toctou-readonly/result; then
  echo "read-only post-build.rs mutation did not retain its result" >&2
  exit 73
fi
if ! docker exec "$readonly_container_id" test -f /target/toctou-readonly/attempted; then
  echo "read-only post-build.rs mutation did not retain its attempt marker" >&2
  exit 73
fi
readonly_result_lines=$(docker exec "$readonly_container_id" wc -l /target/toctou-readonly/result | sed 's/ .*//')
readonly_expected_bytes=$(wc -c < "$temporary/source/apps/canary/src/main.rs" | tr -d ' ')
readonly_expected_sha256=$(sha256_file "$temporary/source/apps/canary/src/main.rs")
readonly_outcome=$(docker exec "$readonly_container_id" sed -n '1p' /target/toctou-readonly/result)
readonly_bytes=$(docker exec "$readonly_container_id" sed -n '2s/^bytes=//p' /target/toctou-readonly/result)
readonly_sha256=$(docker exec "$readonly_container_id" sed -n '3s/^original_sha256=//p' /target/toctou-readonly/result)
if [ "$readonly_result_lines" != 4 ] \
  || [ "$readonly_outcome" != outcome=EROFS ] \
  || [ "$readonly_bytes" != "$readonly_expected_bytes" ] \
  || [ "$readonly_sha256" != "$readonly_expected_sha256" ]; then
  echo "read-only post-build.rs mutation record did not bind exact compiler input" >&2
  exit 73
fi
readonly_wrapper_status=$(docker exec "$readonly_container_id" sed -n '4s/^wrapper_status=//p' /target/toctou-readonly/result)
if [ "$readonly_wrapper_status" != 86 ]; then
  echo "read-only post-build.rs mutation did not record wrapper exit 86" >&2
  exit 73
fi
if docker exec "$readonly_container_id" test -e /target/release/canary; then
  echo "read-only post-build.rs mutation left a promoted executable" >&2
  exit 73
fi
docker logs --tail 8 "$readonly_container_id" >&2
remove_container_id "$readonly_container_id"
readonly_container_id=

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

output_parent=$(dirname "$output_directory")
staging=$output_parent/.fava-pinned-build-staging-$unique
mkdir -m 700 "$staging"
output_dockerfile_path=apps/canary/pinned-output.Dockerfile
extractor_path=apps/canary/tools/extract-pinned-image.py
output_dockerfile_sha256=$(awk -F '\t' -v path="$output_dockerfile_path" \
  '$4 == path { print $2 }' "$manifest")
extractor_sha256=$(awk -F '\t' -v path="$extractor_path" \
  '$4 == path { print $2 }' "$manifest")
case "$output_dockerfile_sha256:$extractor_sha256" in *[!0-9a-f:]*|'') exit 75 ;; esac
if [ "${#output_dockerfile_sha256}" -ne 64 ] || [ "${#extractor_sha256}" -ne 64 ]; then
  echo "pinned output recipe/helper manifest identity was invalid" >&2
  exit 75
fi

# Prove BuildKit can mount the exact content-addressed source stage read-only.
probe_image_tag="fava-pinned-probe:$revision-$unique"
probe_iidfile=$temporary/control/probe-image.id
python3 -c "$pinned_input_program" \
  --repository "$source_checkout" --revision "$revision" \
  --path "$output_dockerfile_path" --expected-sha256 "$output_dockerfile_sha256" \
  --maximum-input-bytes 1048576 --seconds 120 --bytes 1048576 -- \
  docker buildx build --builder colima --load --progress plain --pull=false \
    --platform linux/arm64 --provenance=false --sbom=false \
    --no-cache --network none --target buildkit_probe \
    --resource memory=6g --resource cpu-quota=400000 \
    --ulimit nproc=512 --ulimit nofile=4096:4096 --shm-size 67108864 \
    --iidfile "$probe_iidfile" --tag "$probe_image_tag" \
    --build-arg "SOURCE_IMAGE=$source_image_reference" \
    --build-arg "FAVA_SOURCE_MANIFEST_SHA256=$manifest_sha256" \
    --file - \
    "$temporary/source"
probe_image_id=$(tr -d '\r\n' < "$probe_iidfile")
case "$probe_image_id" in sha256:*) ;; *) exit 75 ;; esac
if [ "${#probe_image_id}" -ne 71 ] \
  || [ "$probe_image_id" != "$(docker image inspect "$probe_image_tag" --format '{{.Id}}')" ]; then
  echo "BuildKit read-only mount probe image identity disagreed" >&2
  exit 75
fi
remove_image_reference "$probe_image_tag"
probe_image_tag=
remove_image_reference "$probe_image_id"
probe_image_id=

# The authoritative GREEN subject is a one-layer scratch image. The compiler
# consumes the exact source image through a read-only BuildKit stage mount.
subject_image_tag="fava-pinned-subject:$revision-$unique"
subject_iidfile=$temporary/control/subject-image.id
python3 -c "$pinned_input_program" \
  --repository "$source_checkout" --revision "$revision" \
  --path "$output_dockerfile_path" --expected-sha256 "$output_dockerfile_sha256" \
  --maximum-input-bytes 1048576 --seconds "$docker_deadline_seconds" \
  --bytes "$docker_output_maximum_bytes" -- \
  docker buildx build --builder colima --load --progress plain --pull=false \
    --platform linux/arm64 --provenance=false --sbom=false \
    --no-cache --network none \
    --resource memory=6g --resource cpu-quota=400000 \
    --ulimit nproc=512 --ulimit nofile=4096:4096 --shm-size 67108864 \
    --iidfile "$subject_iidfile" --tag "$subject_image_tag" \
    --build-arg "SOURCE_IMAGE=$source_image_reference" \
    --build-arg "FAVA_REVISION=$revision" \
    --build-arg "FAVA_TREE=$tree" \
    --build-arg "FAVA_SOURCE_TREE_SHA256=$source_tree_sha256" \
    --build-arg "FAVA_SOURCE_MANIFEST_SHA256=$manifest_sha256" \
    --build-arg "FAVA_SOURCE_IMAGE_SHA256=$source_image_sha256" \
    --build-arg "FAVA_RUST_BASE_IMAGE_SHA256=$base_image_sha256" \
    --file - \
    "$temporary/source"
subject_image_id=$(tr -d '\r\n' < "$subject_iidfile")
case "$subject_image_id" in sha256:*) ;; *) exit 75 ;; esac
if [ "$subject_image_id" != "$(docker image inspect "$subject_image_tag" --format '{{.Id}}')" ]; then
  echo "content-addressed canary subject image identity disagreed" >&2
  exit 75
fi
subject_image_sha256=${subject_image_id#sha256:}
case "$subject_image_sha256" in *[!0-9a-f]*|'') exit 75 ;; esac
if [ "${#subject_image_sha256}" -ne 64 ]; then
  echo "content-addressed canary subject image identity was not canonical" >&2
  exit 75
fi
subject_identity=$(python3 -c "$pinned_input_program" \
  --repository "$source_checkout" --revision "$revision" \
  --path "$extractor_path" --expected-sha256 "$extractor_sha256" \
  --maximum-input-bytes 1048576 --seconds 180 --bytes 4096 -- \
  python3 - "$subject_image_id" "$staging/canary")
if [ "$(printf '%s\n' "$subject_identity" | wc -l | tr -d ' ')" -ne 3 ] \
  || [ "$(printf '%s\n' "$subject_identity" | sed -n '1s/^subject_image_sha256=//p')" != "$subject_image_sha256" ]; then
  echo "immutable subject extractor did not bind the exact engine image" >&2
  exit 75
fi
binary_bytes=$(printf '%s\n' "$subject_identity" | sed -n '2s/^bytes=//p')
binary_sha256=$(printf '%s\n' "$subject_identity" | sed -n '3s/^sha256=//p')
case "$binary_bytes" in *[!0-9]*|'') exit 75 ;; esac
case "$binary_sha256" in *[!0-9a-f]*|'') exit 75 ;; esac
if [ "$binary_bytes" -le 0 ] || [ "$binary_bytes" -gt 134217728 ] \
  || [ "${#binary_sha256}" -ne 64 ] \
  || [ "$(wc -c < "$staging/canary" | tr -d ' ')" != "$binary_bytes" ] \
  || [ "$(sha256_file "$staging/canary")" != "$binary_sha256" ] \
  || ! grep -a -q 'canary failed:' "$staging/canary" \
  || grep -a -q 'canary forged:' "$staging/canary"; then
  echo "image-derived pinned canary subject disagreed with extracted bytes" >&2
  exit 75
fi

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
  "fava_canary_subject_image_sha256": "$subject_image_sha256",
  "source_file_count": $source_file_count,
  "source_total_bytes": $source_total_bytes,
  "toctou_read_only_attempt": "EROFS",
  "toctou_deliberate_break": "compiled-hostile-bytes",
  "source_root": "/source",
  "target_root": "/target",
  "compiler_network": "none",
  "compiler_source_mount": "read-only",
  "compiler_user": "65532:65532",
  "target_storage": "engine-content-addressed-image",
  "target_maximum_bytes": $green_target_maximum_bytes,
  "subject_digest_origin": "engine-image",
  "source_transport": "owned-loopback-registry",
  "source_transport_image_sha256": "${registry_image_ref##*@}"
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
python3 -c "$promotion_program" "$staging" "$output_directory"
staging=
printf '%s\n' \
  "fava_revision=$revision" \
  "source_image_sha256=$source_image_sha256" \
  "source_manifest_sha256=$manifest_sha256" \
  "canary_subject_image_sha256=$subject_image_sha256" \
  "canary_sha256=$binary_sha256" \
  'toctou_read_only_attempt=EROFS' \
  'toctou_deliberate_break=compiled-hostile-bytes'
