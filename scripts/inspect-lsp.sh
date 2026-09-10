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
workspace=$(mktemp -d "${temp_root%/}/zk-inspect-lsp.XXXXXX")
archive="$workspace/archive"

cleanup() {
  status=$?
  if [[ $keep == 1 || $status != 0 ]]; then
    printf 'fixture workspace retained at %s\n' "$workspace" >&2
  else
    rm -rf "$workspace"
  fi
  exit "$status"
}
trap cleanup EXIT

cd "$repo_root"
printf 'building zk and protocol probe...\n' >&2
cargo build --quiet
zk_bin="${CARGO_TARGET_DIR:-$repo_root/target}/debug/zk"

create_archive() {
  rm -rf "$archive"
  "$zk_bin" init "$archive" >/dev/null
  cat >"$archive/zettel/2603231410.typ" <<'TYP'
#import "../lib/zettel.typ": zettel, abstract, keywords, category
#show: zettel

= Disk source <2603231410>

#abstract[Disk source before opening an overlay.]

#keywords("source")

#category.thoughts

Link @2603231411.
TYP

  cat >"$archive/zettel/2603231411.typ" <<'TYP'
#import "../lib/zettel.typ": zettel, abstract, keywords, category
#show: zettel

= Disk target <2603231411>

#abstract[Target used by the protocol probe.]

#keywords("target")

#category.thoughts

No outgoing links.
TYP
}

for encoding in utf-8 utf-16; do
  create_archive
  printf 'probing %s positions...\n' "$encoding" >&2
  cargo run --quiet --example lsp_probe -- "$zk_bin" "$archive" "$encoding" \
    >"$workspace/$encoding.json"
done

jq -s -e '
  length == 2
  and (map(.encoding) == ["utf-8", "utf-16"])
  and all(.[];
    .completion
    and .referenceCompletion
    and .categoryCompletion
    and .archiveSearch
    and .hover
    and .definition
    and .references
    and .backlinks
    and .diagnostics
    and .unsavedState
    and .staleVersionRejected
    and .watchedFileRefresh
    and .watcherRegistration
    and .shutdown
  )
' "$workspace/utf-8.json" "$workspace/utf-16.json" >/dev/null

printf 'language-server protocol results\n'
jq -s . "$workspace/utf-8.json" "$workspace/utf-16.json"
printf '\ninspection assertions passed\n' >&2
