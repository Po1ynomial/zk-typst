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
workspace=$(mktemp -d "${temp_root%/}/zk-inspect-integrity.XXXXXX")
archive="$workspace/archive"

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

write_valid_zettel() {
  id=$1
  title=$2
  body=$3
  cat >"$archive/zettel/$id.typ" <<TYP
#import "../lib/zettel.typ": zettel, abstract, keywords, category
#show: zettel

= $title <$id>

#abstract[Summary for $title.]

#keywords("inspection")

#category.thoughts

$body
TYP
}

write_valid_zettel 2603231410 "Source" "Links to @2603231411."
write_valid_zettel 2603231411 "Target" "No outgoing links."
write_valid_zettel 2603231412 "Orphan" "No links."
write_valid_zettel 2603231413 "Dangling" "Missing @9999999999."

cat >"$archive/zettel/2603231414.typ" <<'TYP'
#import "../lib/zettel.typ": zettel, abstract, keywords, category
#show: zettel

= Malformed <2603231499>

#abstract[This note still contributes its authored link.]

#keywords("valid", computed)

#category.thoughts

Also links to @2603231411.
TYP

printf 'not a canonical Zettel\n' >"$archive/zettel/bad-name.typ"

set +e
(
  cd "$archive"
  "$zk_bin" check --format json
) >"$workspace/initial-check.json"
check_status=$?
set -e
if [[ $check_status != 1 ]]; then
  printf 'expected initial check to exit 1, got %s\n' "$check_status" >&2
  exit 1
fi

jq -e '
  any(.[]; .code == "archive.filename" and .path == "zettel/bad-name.typ")
  and any(.[]; .code == "metadata.id_mismatch" and .path == "zettel/2603231414.typ")
  and any(.[]; .code == "metadata.keywords" and .path == "zettel/2603231414.typ")
  and any(.[]; .code == "reference.dangling" and .path == "zettel/2603231413.typ")
  and all(.[]; .code != "graph.orphan")
' "$workspace/initial-check.json" >/dev/null

(
  cd "$archive"
  "$zk_bin" query node 2603231410
) >"$workspace/node.json"
jq -e '.id == "2603231410" and .title.text == "Source"' "$workspace/node.json" >/dev/null

(
  cd "$archive"
  "$zk_bin" query links 2603231410
) >"$workspace/links.json"
jq -e 'length == 1 and .[0].target == "2603231411" and .[0].resolution == "resolved"' "$workspace/links.json" >/dev/null

(
  cd "$archive"
  "$zk_bin" query backlinks 2603231411
) >"$workspace/backlinks.json"
jq -e '[.[].source] == ["2603231410", "2603231414"]' "$workspace/backlinks.json" >/dev/null

set +e
(
  cd "$archive"
  "$zk_bin" remove 2603231411
) >"$workspace/blocked-remove.stdout" 2>"$workspace/blocked-remove.stderr"
remove_status=$?
set -e
if [[ $remove_status != 1 ]]; then
  printf 'expected blocked removal to exit 1, got %s\n' "$remove_status" >&2
  exit 1
fi
blocked=$(<"$workspace/blocked-remove.stderr")
if [[ $blocked != *"incoming references exist"* || $blocked != *"zettel/2603231410.typ:"* || $blocked != *"zettel/2603231414.typ:"* ]]; then
  printf 'blocked removal did not report every incoming location\n' >&2
  exit 1
fi
test -f "$archive/zettel/2603231411.typ"

(
  cd "$archive"
  "$zk_bin" remove 2603231412
) >"$workspace/removed.stdout"
test ! -e "$archive/zettel/2603231412.typ"
test "$(<"$workspace/removed.stdout")" = "zettel/2603231412.typ"

write_valid_zettel 2603231413 "Repaired" "Now links to @2603231411."
write_valid_zettel 2603231414 "Repaired metadata" "Still links to @2603231411."
rm "$archive/zettel/bad-name.typ"

(
  cd "$archive"
  "$zk_bin" check
) >"$workspace/final-check.txt"
test "$(<"$workspace/final-check.txt")" = "0 error(s), 0 warning(s)"

printf '\nInitial diagnostics\n'
jq '[.[] | {severity, path, code, range}]' "$workspace/initial-check.json"
printf '\nNode query\n'
jq . "$workspace/node.json"
printf '\nOutgoing links\n'
jq . "$workspace/links.json"
printf '\nBacklinks\n'
jq . "$workspace/backlinks.json"
printf '\nBlocked removal\n'
printf '%s\n' "$blocked"
printf '\nSuccessful removal\n'
printf '%s\n' "$(<"$workspace/removed.stdout")"
printf '\nFinal check\n'
printf '%s\n' "$(<"$workspace/final-check.txt")"
printf '\ninspection assertions passed\n' >&2
