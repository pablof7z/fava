#!/bin/sh
set -eu
export LC_ALL=C

if [ "$#" -ne 3 ]; then
  echo "usage: verify-pinned-source.sh SOURCE_ROOT MANIFEST EXPECTED_SHA256" >&2
  exit 64
fi
root=$1 manifest=$2 expected=$3

sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}
size_of() { wc -c < "$1" | tr -d ' '; }
mode_of() {
  if stat -c '%a' "$1" >/dev/null 2>&1; then stat -c '%a' "$1"; else stat -f '%Lp' "$1"; fi
}

case "$expected" in *[!0-9a-f]*|'') exit 65 ;; esac
if [ "${#expected}" -ne 64 ] || [ ! -d "$root" ] || [ -L "$root" ] \
  || [ ! -f "$manifest" ] || [ -L "$manifest" ] \
  || [ "$(size_of "$manifest")" -gt 1048576 ] \
  || [ "$(sha256_file "$manifest")" != "$expected" ]; then
  echo "pinned source manifest identity was invalid" >&2
  exit 65
fi

format=$(sed -n '1s/^format=//p' "$manifest")
revision=$(sed -n '2s/^revision=//p' "$manifest")
tree=$(sed -n '3s/^tree=//p' "$manifest")
declared_count=$(sed -n '4s/^file_count=//p' "$manifest")
declared_total=$(sed -n '5s/^total_bytes=//p' "$manifest")
case "$revision:$tree" in *[!0-9a-f:]*|'') exit 66 ;; esac
case "$declared_count:$declared_total" in *[!0-9:]*|'') exit 66 ;; esac
if [ "$format" != fava-pinned-source-v1 ] || [ "${#revision}" -ne 40 ] \
  || [ "${#tree}" -ne 40 ] || [ "$declared_count" -gt 4096 ] \
  || [ "$declared_total" -gt 67108864 ]; then
  echo "pinned source manifest header was invalid" >&2
  exit 66
fi
if { [ "$declared_count" != 0 ] && [ "${declared_count#0}" != "$declared_count" ]; } \
  || { [ "$declared_total" != 0 ] && [ "${declared_total#0}" != "$declared_total" ]; }; then
  echo "pinned source manifest totals were not canonical decimals" >&2
  exit 66
fi

tab=$(printf '\t') count=0 total=0 previous=
while IFS="$tab" read -r prefix digest size path extra; do
  case "$prefix" in file=100644) mode=644 ;; file=100755) mode=755 ;; *) exit 67 ;; esac
  case "$digest" in *[!0-9a-f]*|'') exit 67 ;; esac
  case "$size" in *[!0-9]*|'') exit 67 ;; esac
  case "$path" in
    *[!A-Za-z0-9._/+@=-]*|/*|.|..|./*|*/./*|*/.|../*|*/../*|*/..) exit 67 ;;
    Cargo.toml|Cargo.lock|rust-toolchain.toml|.cargo/*|apps/canary/*|crates/*) ;;
    *) exit 67 ;;
  esac
  if [ -n "${extra:-}" ] || [ "${#digest}" -ne 64 ] || [ "${#path}" -gt 512 ] \
    || { [ "$size" != 0 ] && [ "${size#0}" != "$size" ]; } \
    || [ "$size" -gt 8388608 ] || { [ -n "$previous" ] && [ "$previous" \> "$path" -o "$previous" = "$path" ]; }; then
    echo "pinned source manifest row was invalid" >&2
    exit 67
  fi
  previous=$path file=$root/$path
  if [ ! -f "$file" ] || [ -L "$file" ] || [ "$(mode_of "$file")" != "$mode" ] \
    || [ "$(size_of "$file")" != "$size" ] || [ "$(sha256_file "$file")" != "$digest" ]; then
    echo "pinned source bytes differed: $path" >&2
    exit 68
  fi
  count=$((count + 1)) total=$((total + size))
done <<EOF
$(sed -n '6,$p' "$manifest")
EOF

set -- "$root/Cargo.toml" "$root/Cargo.lock" "$root/rust-toolchain.toml" \
  "$root/apps/canary" "$root/crates"
if [ -e "$root/.cargo" ]; then set -- "$@" "$root/.cargo"; fi
actual=$(find "$@" -type f | wc -l | tr -d ' ')
if find "$@" ! -type f ! -type d -print -quit | grep -q . \
  || [ "$count" -ne "$declared_count" ] || [ "$total" -ne "$declared_total" ] \
  || [ "$actual" -ne "$declared_count" ]; then
  echo "pinned source inventory was incomplete or unbounded" >&2
  exit 69
fi
printf '%s\n' "verified pinned source manifest $expected"
