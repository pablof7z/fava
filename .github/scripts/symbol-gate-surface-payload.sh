#!/bin/sh
# Write the whole surface's unsigned declarations where the signing page can
# read them.
#
# The pull request comment carries its payload inline, which caps it at what one
# comment holds. A repository signing its surface for the first time is far past
# that — fava's is 203 KB against a comment's practical 60 KB — so that payload
# goes in a file and the page is pointed at it with `&file=`.
#
# The payload is the same shape either way, and decides the same nothing:
# `symbol-gate verify` re-renders every declaration from source, so a payload
# that misstates one produces a signature matching no declaration and the gate
# stays red. That is why this file being committed, editable, and readable by
# anyone costs nothing.
#
# Environment:
#   SG        path to the symbol-gate binary (default: symbol-gate)
#   REPO_URL  repository URL recorded in the payload
#   OUT       file to write (default: .symbol-gate/unsigned.json)
set -eu

SG="${SG:-symbol-gate}"
OUT="${OUT:-.symbol-gate/unsigned.json}"
REPO_URL="${REPO_URL:-}"

commit=$(git rev-parse HEAD)

listing=$(mktemp)
trap 'rm -f "$listing"' EXIT

# `status` reports; it never gates. A repository with nothing signed yet is the
# normal case here, not a failure.
"$SG" status --json > "$listing"

mkdir -p "$(dirname "$OUT")"

jq \
  --arg repo "$REPO_URL" \
  --arg commit "$commit" \
  '.result as $r
   | ($r.unsigned | sort_by(.symbol)) as $u
   | {
       schema_version: 1,
       repository: $repo,
       commit: $commit,
       # No `pr`: this payload describes a repository at a commit, and naming a
       # pull request would claim a scope it does not have.
       #
       # `total` counts unsigned declarations only, never the whole surface, and
       # `complete` is true because a file has no size limit to truncate at. The
       # signing page enforces `total == (unsigned | length)` when complete.
       total: ($u | length),
       complete: true,
       unsigned: $u
     }' "$listing" > "$OUT"

count=$(jq -r '.total' "$OUT")
bytes=$(wc -c < "$OUT" | tr -d ' ')
echo "wrote $OUT: $count unsigned declaration(s), $bytes bytes"
