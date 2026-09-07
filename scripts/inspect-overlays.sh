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
workspace=$(mktemp -d "${temp_root%/}/zk-inspect-overlays.XXXXXX")
archive="$workspace/archive"
snapshot="$workspace/snapshot.json"

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
printf 'building zk and overlay inspection example...\n' >&2
cargo build --quiet
zk_bin="${CARGO_TARGET_DIR:-$repo_root/target}/debug/zk"
"$zk_bin" init "$archive" >/dev/null

cat >"$archive/zettel/2603231410.typ" <<'TYP'
#import "../lib/zettel.typ": zettel, abstract, keywords, category
#show: zettel

= Disk source <2603231410>

#abstract[Initial disk source.]

#keywords("inspection")

#category.thoughts

Link @2603231411.
TYP

cat >"$archive/zettel/2603231411.typ" <<'TYP'
#import "../lib/zettel.typ": zettel, abstract, keywords, category
#show: zettel

= Disk target <2603231411>

#abstract[Initial disk target.]

#keywords("inspection")

#category.thoughts
TYP

cargo run --quiet --example inspect_overlays -- "$archive" >"$snapshot"

jq -e '
  .revision == 7
  and ([.nodes[].id] == ["2603231410", "2603231411", "2603231412"])
  and ((.nodes[] | select(.id == "2603231410") | .title.text) == "Saved disk")
  and ((.nodes[] | select(.id == "2603231412") | .title.text) == "Restored disk target")
  and ([.links[] | select(
    .source == "2603231410"
    and .target == "2603231412"
    and .resolution == "resolved"
  )] | length == 1)
  and ([.links[] | select(
    .source == "2603231412"
    and .target == "2603231411"
    and .resolution == "resolved"
  )] | length == 1)
  and (.diagnostics == [])
' "$snapshot" >/dev/null

printf 'final live-provider snapshot\n'
jq . "$snapshot"
printf '\ninspection assertions passed\n' >&2
