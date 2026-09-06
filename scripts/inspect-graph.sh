#!/usr/bin/env bash
set -euo pipefail

usage() {
  printf 'Usage: %s [--keep]\n' "${0##*/}"
}

keep=${KEEP_TMP:-0}
case ${1:-} in
  --keep)
    keep=1
    ;;
  "") ;;
  *)
    usage >&2
    exit 2
    ;;
esac

for command in cargo jq; do
  if ! command -v "$command" >/dev/null 2>&1; then
    printf 'required command not found: %s\n' "$command" >&2
    exit 1
  fi
done

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
temp_root=${TMPDIR:-/tmp}
workspace=$(mktemp -d "${temp_root%/}/zk-inspect-graph.XXXXXX")
archive="$workspace/archive"
graph="$workspace/graph.json"

cleanup() {
  status=$?
  if [[ $keep == 1 || $status != 0 ]]; then
    printf 'fixture archive retained at %s\n' "$archive" >&2
  else
    rm -rf "$workspace"
  fi
  exit "$status"
}
trap cleanup EXIT

cd "$repo_root"
printf 'building zk...\n' >&2
cargo build --quiet
zk_bin="${CARGO_TARGET_DIR:-$repo_root/target}/debug/zk"

"$zk_bin" init "$archive" >/dev/null

cat >"$archive/zettel/2603231410.typ" <<'TYP'
#import "../lib/zettel.typ": zettel, abstract, keywords, category
#show: zettel

= Network _paths_ <2603231410>

#abstract[
Connects to @2603231411.
]

#keywords(
  "networks",
  "graph",
)

#category.thoughts

Unicode before the range: café. Again @2603231411 and missing @9999999999.
The raw value `@8888888888` is not a link.
#let ignored = "@7777777777"
// @6666666666 is also not a link.
TYP

cat >"$archive/zettel/2603231411.typ" <<'TYP'
#import "../lib/zettel.typ": zettel, abstract, keywords, category
#show: zettel

= Backlink target <2603231411>

#abstract[]

#keywords("target")

#category.thoughts

Back to @2603231410.
TYP

cat >"$archive/zettel/2603231412.typ" <<'TYP'
#import "../lib/zettel.typ": zettel, abstract, keywords, category
#show: zettel

= Malformed metadata <2603231499>

#abstract[The syntax parses, but two metadata fields are invalid.]

#keywords(
  "valid",
  computed,
)

#category.thoughts

Still links to @2603231411.
TYP

cat >"$archive/zettel/2603231413.typ" <<'TYP'
#import "../lib/zettel.typ": zettel, abstract, keywords, category
#show: zettel

= Isolated note <2603231413>

#abstract[]

#keywords()

#category.thoughts
TYP

cat >"$archive/zettel/2603231414.typ" <<'TYP'
#import "../lib/zettel.typ": zettel, abstract, keywords, category
#show: zettel

= Broken Typst <2603231414>

#abstract[
This block is never closed.
TYP

printf 'not a canonical Zettel\n' >"$archive/zettel/readme.typ"

(
  cd "$archive"
  "$zk_bin" graph --format json
) >"$graph"

jq -e '
  .schema_version == 1
  and .revision == 1
  and ([.nodes[].id] == [
    "2603231410",
    "2603231411",
    "2603231412",
    "2603231413",
    "2603231414"
  ])
  and ((.nodes[] | select(.id == "2603231410") | .title.text) == "Network paths")
  and ((.nodes[] | select(.id == "2603231412") | .keywords) == null)
  and ([.links[] | select(
    .source == "2603231410"
    and .target == "2603231411"
    and .resolution == "resolved"
    and (.spans | length) == 2
  )] | length == 1)
  and ([.links[] | select(
    .source == "2603231410"
    and .target == "9999999999"
    and .resolution == "missing"
    and (.spans | length) == 1
  )] | length == 1)
  and ([.links[] | select(
    .source == "2603231411"
    and .target == "2603231410"
    and .resolution == "resolved"
  )] | length == 1)
  and ([.links[] | select(
    .source == "2603231412"
    and .target == "2603231411"
    and .resolution == "resolved"
  )] | length == 1)
  and all(.links[]; .target != "8888888888" and .target != "7777777777" and .target != "6666666666")
  and any(.diagnostics[];
    .path == "zettel/2603231412.typ" and .code == "metadata.id_mismatch"
  )
  and any(.diagnostics[];
    .path == "zettel/2603231412.typ" and .code == "metadata.keywords"
  )
  and any(.diagnostics[];
    .path == "zettel/2603231414.typ" and .code == "syntax.error"
  )
' "$graph" >/dev/null

while IFS=$'\t' read -r source target start end; do
  count=$((end - start))
  authored=$(dd if="$archive/zettel/$source.typ" bs=1 skip="$start" count="$count" 2>/dev/null)
  if [[ $authored != "@$target" ]]; then
    printf 'range mismatch in %s: expected @%s, found %q\n' "$source" "$target" "$authored" >&2
    exit 1
  fi
done < <(
  jq -r '.links[] | .source as $source | .target as $target | .spans[] | [$source, $target, .start, .end] | @tsv' "$graph"
)

printf 'inspection assertions passed\n' >&2
jq . "$graph"
