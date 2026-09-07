# System

## Implemented capabilities

The repository builds one Rust executable named `zk`.

### Archive authoring

`zk init [PATH]` creates the version-one archive layout without overwriting an existing manifest or canonical path:

```text
zk.toml
zettel/
lib/
  zettel.typ
```

The default path is the current directory. The generated manifest declares `format = 1`. The bundled Typst library renders the fixed metadata forms and intercepts ten-digit references.

`zk new` walks upward from the current directory to find `zk.toml`, validates archive format 1 and the canonical layout, then creates a Zettel under `zettel/`. The filename uses the current local minute in `YYMMDDHHmm.typ` form. If that ID exists, allocation advances by one minute until a free name is available. The command prints the new path relative to the archive root.

The generated Zettel has the required import, show rule, labelled level-one heading, abstract, keyword list, and category before an empty body.

### Disk-backed provider

`zk graph --format json` loads saved canonical `zettel/ID.typ` files and writes a complete JSON snapshot to standard output. Files with noncanonical names do not enter the provider graph and produce archive diagnostics.

The provider parses files concurrently with `typst-syntax` 0.15.1. It extracts the restricted direct metadata forms without evaluating Typst. Title and abstract values contain their exact inner source, deterministic text projection, and half-open UTF-8 byte range. Malformed fields remain `null` and produce snapshot diagnostics.

Literal ten-digit Typst references become directed links. References in raw text, strings, and comments do not appear as Typst reference nodes and do not become links. The provider groups repeated occurrences by source-target pair, preserves every authored byte range, and records whether each target resolves to a canonical node.

The retained graph interns IDs as `u32` indexes and keeps incoming and outgoing adjacency lists. Parsed source and syntax trees are discarded after extraction. A JSON snapshot expands the internal IDs back to strings.

Provider schema 1 has this top-level shape:

```json
{
  "schema_version": 1,
  "revision": 1,
  "nodes": [],
  "links": [],
  "diagnostics": []
}
```

Nodes are ordered by ID. Links are ordered by source and target ID. Diagnostics are ordered by path and source range. A fresh disk-backed provider starts at revision 1, so this revision is not a durable archive identifier.

### Integrity and shell operations

`zk check` prints diagnostics and exits with status 1 when any error exists. Warnings do not fail the command. The default text output includes the path, byte range when available, stable diagnostic code, and message. `zk check --format json` emits the diagnostic array used in graph snapshots.

Checks cover:

- noncanonical files or entries under `zettel/`;
- Typst syntax errors;
- missing, malformed, repeated, or misplaced metadata;
- filename and heading-label mismatches;
- dangling reference occurrences;
- Zettel with no incoming or outgoing links, reported as warnings.

The query commands write JSON to standard output:

```text
zk query node <ID>
zk query links <ID>
zk query backlinks <ID>
```

A node query returns metadata for one Zettel. Link and backlink queries return grouped links with every authored byte range. Missing node IDs fail the command.

`zk remove <ID>` deletes a canonical Zettel only when it has no incoming references. A blocked removal exits with status 1, leaves the file untouched, and prints every incoming source path and byte range. A successful removal prints the deleted archive-relative path.

### Live provider sessions

The same `Provider` type supports long-lived editor sessions. `open_buffer` installs full text and a document version. `change_buffer` accepts only newer versions and calls `typst_syntax::Source::replace` for incremental reparsing. Open sources remain in memory; closed-file source and syntax trees do not.

An overlay replaces the corresponding disk node and outgoing links in one graph revision. Incoming adjacency, target resolution, dangling diagnostics, and orphan diagnostics update before consumers see that revision. Opening a canonical path that does not exist on disk creates a session node.

`save_buffer` retains the overlay without reading disk. `refresh_disk` ignores open paths. `close_buffer` drops the overlay, then reloads disk or removes the session node when no disk file exists.

Every scheduled source state receives a generation. `prepare_disk_update` returns parsed work tagged with that generation, and `apply_prepared` rejects it if a newer disk or buffer state has already been scheduled. Accepted replacements increment the graph revision once. Stale document versions, stale generations, saves, and ignored disk events do not increment it.

## Code entry points

- `src/main.rs` defines the command-line interface, JSON output, and process exit behavior.
- `src/archive.rs` implements initialization, root discovery, manifest validation, layout validation, timestamp-ID validation, collision-safe creation, and file removal.
- `src/extract.rs` extracts metadata, literal references, source ranges, and syntax diagnostics from one parsed Zettel.
- `src/model.rs` defines the public node, link, diagnostic, and snapshot data shapes.
- `src/provider.rs` loads files concurrently, owns mutable graph state, retains open overlays, rejects stale updates, derives integrity diagnostics, and produces snapshots.
- `src/templates.rs` contains the canonical manifest, Typst library, and Zettel templates.
- `tests/cli.rs` exercises authoring, graph output, checking, queries, and guarded removal through the executable.
- `examples/inspect_overlays.rs` drives the in-process live-provider lifecycle used by the overlay inspection script.

## Inspection

Run the automated checks:

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
rumdl check docs
```

Inspect the provider with a generated archive:

```sh
scripts/inspect-graph.sh
```

The script builds `zk`, creates five canonical Zettel plus one diagnosed noncanonical file, and checks metadata extraction, malformed metadata, syntax recovery, resolved and missing links, grouped occurrences, ignored raw and commented references, and exact byte ranges. It prints the complete JSON snapshot after the assertions pass.

Preserve the generated archive and snapshot for manual inspection:

```sh
scripts/inspect-graph.sh --keep
```

The script prints the retained archive path to standard error. `KEEP_TMP=1 scripts/inspect-graph.sh` provides the same behavior.

Inspect checking, queries, and removal with another generated archive:

```sh
scripts/inspect-integrity.sh
```

This script demonstrates failing and repaired checks, node and relation queries, blocked removal with incoming locations, and successful removal. Pass `--keep` or set `KEEP_TMP=1` to retain its archive.

Inspect a live provider session:

```sh
scripts/inspect-overlays.sh
```

The script creates a disk archive and runs the Rust inspection example through overlay installation, incremental full-text changes, stale version and generation rejection, unsaved-node creation, save precedence, close reload, disk deletion, and disk restoration. It prints the final revision-seven snapshot. Pass `--keep` or set `KEEP_TMP=1` to retain the resulting archive.

## Current limitations

No filesystem watcher calls the disk-refresh API yet. The executable does not yet run a language server, and the Neovim adapter is not implemented.

Queries currently emit JSON only. CLI locations use UTF-8 byte ranges rather than line and column coordinates. Removal does not edit incoming references and never rewrites Zettel bodies.

The initial category dictionary contains `thoughts`, `physics`, and `coding`. Category and keyword policy remain deferred product decisions. Archives may edit their user-owned `lib/zettel.typ`, but `zk` does not migrate it yet.
