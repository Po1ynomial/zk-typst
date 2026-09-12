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

`zk init --agent-skills [PATH]` also installs the complete bundled skill set under `.agents/skills/`. The current set contains `.agents/skills/zettelkasten/SKILL.md`, which documents the writing method, source contract, query workflow, whole-file lifecycle commands, and archive checks. Ordinary initialization does not create `.agents/`.

Skill installation is best-effort. An existing same-name skill remains untouched and produces a warning. Other filesystem failures also warn without failing canonical archive creation. Once installed, skills are user-owned; later `zk` commands do not validate or update `.agents/`.

Commands that require an existing archive accept the global `--archive PATH` option. An explicit path resolves relative to the process working directory, must itself be a valid archive root, and takes precedence over current-directory discovery. Without the option, commands walk upward to the nearest `zk.toml`. `zk init [PATH]` rejects `--archive`.

`zk new` creates a Zettel under `zettel/` using the current local minute as its `YYMMDDHHmm` ID. If that path exists, allocation advances one minute at a time. The command writes the required metadata template and prints the archive-relative path.

### Disk-backed provider

`zk graph --format json` loads saved canonical `zettel/ID.typ` files and writes a complete JSON snapshot. Noncanonical entries under `zettel/` produce archive diagnostics and do not enter the graph.

The provider parses files concurrently with `typst-syntax` 0.15.1. It extracts the restricted direct metadata forms without evaluating Typst. Title and abstract values contain exact inner source, deterministic text projection, and half-open UTF-8 byte ranges. Malformed fields remain absent and produce diagnostics.

Literal ten-digit Typst references become directed links. The provider groups repeated occurrences by source-target pair, preserves every authored byte range, and records whether the target resolves. References in raw text, strings, and comments do not become links.

The retained graph interns IDs as `u32` indexes and keeps incoming and outgoing adjacency lists. Parsed source and syntax trees are discarded after extraction.

Provider schema 1 contains `schema_version`, `revision`, `nodes`, `links`, and `diagnostics`. Nodes sort by ID, links by source and target, and diagnostics by path and range. A new disk-backed provider starts at revision 1, so revisions are not durable archive identifiers.

### Integrity and shell operations

`zk check` prints diagnostics and exits with status 1 when any error exists. Warnings do not fail the command. `zk check --format json` emits the same diagnostic array used in graph snapshots.

Checks cover:

- noncanonical entries under `zettel/`;
- Typst syntax errors;
- missing, malformed, repeated, or misplaced metadata;
- filename and heading-label mismatches;
- dangling reference occurrences.

The query commands write JSON:

```text
zk query node <ID>
zk query links <ID>
zk query backlinks <ID>
zk query search <QUERY>
```

Metadata search compares the query case-insensitively with IDs, projected titles, projected abstracts, keywords, and categories. It returns every matching node in provider ID order without an implicit cap. The language server uses the same matcher.

`zk remove <ID>` deletes a canonical Zettel only when it has no incoming references. A blocked removal leaves the file untouched and prints every incoming source path and byte range.

### Live provider sessions

The same `Provider` type supports long-lived clients. `open_buffer` installs full text and a document version. `change_buffer` accepts only newer versions and uses `typst_syntax::Source::replace` for incremental reparsing.

An open overlay replaces the disk node and outgoing links in one graph revision. Incoming adjacency, target resolution, and diagnostics update before consumers observe the revision. Opening a canonical path absent from disk creates a session node.

`save_buffer` retains the overlay. `refresh_disk` ignores open paths. `close_buffer` drops the overlay and reloads disk or removes the node when no disk file exists.

Every scheduled source state receives a generation. Prepared disk updates apply only when their generation remains current. Stale document versions, stale generations, saves, and ignored disk events do not increment the graph revision.

### Language server

`zk lsp` runs over standard input and output. It loads either an explicit `--archive PATH` or the archive discovered above its working directory, then owns one live provider session.

The server advertises full-text synchronization. Open, change, save, and close notifications map to the provider overlay lifecycle. When the client supports dynamic registration, the server registers `**/zettel/*.typ` for create, change, and delete events. Watched-file notifications refresh closed Zettel and cannot replace open overlays.

The server prefers UTF-8 positions when offered and otherwise uses UTF-16. It converts retained byte ranges with open-buffer text or one read of a closed file.

Initialization advertises ZK protocol version 1 under `capabilities.experimental.zk` with these boolean features:

- `archiveQueries`
- `categoryCompletion`
- `referenceCompletion`
- `referenceTitleDecorations`

The editor protocol version is independent of the executable version, archive format, and provider schema.

Implemented requests:

- `textDocument/completion` supports numeric ID prefixes and case-insensitive title queries after `@`, plus direct category keys from the saved library.
- `textDocument/hover` returns target metadata or a missing-target message.
- `textDocument/definition` opens the target, including unsaved session nodes.
- `textDocument/references` returns incoming authored occurrences and optionally the declaration.
- `workspace/symbol` searches live metadata.
- `workspace/executeCommand` supports `zk.queryNode`, `zk.links`, and `zk.backlinks`.

The server pushes diagnostics for open Zettel after accepted source updates and watched-file changes. Resolving or creating a target republishes affected open-buffer diagnostics.

## Code entry points

- `src/main.rs` defines the CLI, JSON output, and process exit behavior.
- `src/archive.rs` implements initialization, discovery, validation, creation, skill installation, and removal.
- `src/extract.rs` extracts metadata, references, ranges, and syntax diagnostics.
- `src/model.rs` defines public node, link, diagnostic, and snapshot shapes.
- `src/provider.rs` loads files, owns graph state and overlays, rejects stale updates, and produces snapshots.
- `src/lsp.rs` implements protocol capabilities, synchronization, diagnostics, navigation, search, and archive commands.
- `src/templates.rs` contains the canonical templates and bundled skill registry.
- `skills/zettelkasten/SKILL.md` is the inspectable bundled skill source.
- `tests/cli.rs` covers authoring, graph output, checks, queries, skills, search, and guarded removal.
- `examples/inspect_overlays.rs` drives the live-provider lifecycle.
- `examples/lsp_probe.rs` inspects the language server over framed JSON-RPC.

## Inspection

Run automated checks:

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
rumdl check docs skills
cargo package --list --allow-dirty
```

Inspect generated skills and metadata search:

```sh
archive="$(mktemp -d)"
cargo run -- init --agent-skills "$archive"
cargo run -- --archive "$archive" query search "untitled"
```

Inspect each engine layer:

```sh
scripts/inspect-graph.sh
scripts/inspect-integrity.sh
scripts/inspect-overlays.sh
scripts/inspect-lsp.sh
```

The graph script checks extraction, recovery, links, grouped ranges, and diagnostics. The integrity script demonstrates checks, queries, blocked removal, and successful removal. The overlay script exercises incremental changes, stale update rejection, save precedence, disk deletion, and restoration. The LSP script probes UTF-8 and UTF-16 sessions over the real transport, including protocol capabilities, completion, navigation, diagnostics, queries, watched files, and shutdown.

## Current limitations

The server loads its initial graph synchronously. Clients without dynamic watched-file registration must arrange file notifications. Diagnostics are pushed for open Zettel; archive-wide closed-file inspection remains available through `zk check`.

Queries emit JSON only. CLI locations use UTF-8 byte ranges. Removal does not edit incoming references. Metadata search has no result cap, so broad queries can produce large arrays.

The initial category dictionary contains `thoughts`, `physics`, and `coding`. Category completion reads only the saved library. Category and keyword policy remain deferred.

Installed agent skills are snapshots from archive creation and do not receive automatic updates. The independently maintained `zk.nvim` client is outside this repository.
