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

`zk graph --format json` loads saved canonical `zettel/ID.typ` files and writes a complete JSON snapshot to standard output. Files with noncanonical names do not enter the provider graph.

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

## Code entry points

- `src/main.rs` defines the command-line interface, JSON output, and process exit behavior.
- `src/archive.rs` implements initialization, root discovery, manifest validation, layout validation, and collision-safe Zettel creation.
- `src/extract.rs` extracts metadata, literal references, source ranges, and syntax diagnostics from one parsed Zettel.
- `src/model.rs` defines the public node, link, diagnostic, and snapshot data shapes.
- `src/provider.rs` loads files concurrently, interns IDs, builds grouped links and adjacency, and produces snapshots.
- `src/templates.rs` contains the canonical manifest, Typst library, and Zettel templates.
- `tests/cli.rs` exercises archive initialization, nested-directory authoring, and JSON graph output through the executable.

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

The script builds `zk`, creates five canonical Zettel plus one ignored noncanonical file, and checks metadata extraction, malformed metadata, syntax recovery, resolved and missing links, grouped occurrences, ignored raw and commented references, and exact byte ranges. It prints the complete JSON snapshot after the assertions pass.

Preserve the generated archive and snapshot for manual inspection:

```sh
scripts/inspect-graph.sh --keep
```

The script prints the retained archive path to standard error. `KEEP_TMP=1 scripts/inspect-graph.sh` provides the same behavior.

## Current limitations

The provider reads saved files only. It has no filesystem watcher or open-buffer overlays. The executable does not yet expose `check`, targeted queries, guarded removal, or a language server. The Neovim adapter is also not implemented.

Snapshot diagnostics currently cover Typst parse errors and the direct metadata source contract. Dangling links have `resolution: "missing"` but do not yet emit integrity diagnostics. Noncanonical files are ignored rather than diagnosed. Orphan checks and removal policy belong to the next implementation slice.

The initial category dictionary contains `thoughts`, `physics`, and `coding`. Category and keyword policy remain deferred product decisions. Archives may edit their user-owned `lib/zettel.typ`, but `zk` does not migrate it yet.
