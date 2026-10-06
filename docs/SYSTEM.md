# System

This guide describes implemented behavior. The [CLI contract](contract/cli.md), [LSP JSON contract](contract/lsp.md), and [shared data schema 2](contract/data.md) define the interfaces implemented by zk 0.2.0. Metadata is a map of user-declared fields, initialized with explicit seed rules and without hidden runtime field defaults. CLI and LSP archive queries use schema-2 envelopes, and the server advertises ZK protocol 2 with independent data-schema discovery. See the [version matrix](contract/README.md#status) and [decision](decisions/external-contracts.md).

## Implemented capabilities

The repository builds one Rust executable named `zk`, currently version 0.2.0.

### Archive authoring

`zk init [PATH]` creates an archive-format-2 layout without overwriting an existing manifest, note directory, library, or default template:

```text
zk.toml
zettel/
lib/
  zettel.typ
templates/
  zettel.typ.tpl
```

The default path is the current directory. The generated manifest declares `format = 2`, `[new] template = "templates/zettel.typ.tpl"`, and explicit initial rules for abstract, keywords, and category. The bundled Typst library renders the default metadata forms and intercepts ten-digit references. The library and template immediately become user-owned. Initialization does not invoke Git, stage files, or create a commit. Only the manifest and `zettel/` are required for inspecting an existing archive; the presentation library is not an extraction dependency.

`zk init --agent-skills [PATH]` also installs the complete bundled skill set under `.agents/skills/`. The current set contains `.agents/skills/zettelkasten/SKILL.md`, which documents the writing method, source contract, query workflow, whole-file lifecycle commands, and archive checks. Ordinary initialization does not create `.agents/`.

Skill installation is best-effort. An existing same-name skill remains untouched and produces a warning. Other filesystem failures also warn without failing canonical archive creation. Once installed, skills are user-owned; later `zk` commands do not validate or update `.agents/`.

Commands that require an existing archive accept the global `--archive PATH` option. An explicit path resolves relative to the process working directory, must itself be a valid archive root, and takes precedence over current-directory discovery. Without the option, commands walk upward to the nearest `zk.toml`. `zk init [PATH]` rejects `--archive`.

`zk new` creates a Zettel under `zettel/` using the current local minute as its `YYMMDDHHmm` ID. If that path exists, allocation advances one minute at a time. It reads the configured archive-local UTF-8 template, replaces every literal `{{id}}`, preserves all other bytes, and prints the archive-relative path. The rendered title and recognized metadata are validated before creation; Typst compilation and imports are not checked. Missing templates, absent ID markers, and invalid rendered metadata fail without creating a note. Template paths must stay beneath the archive root, including through symlinks. Relative Typst imports are written for the destination Zettel's location.

### Metadata source configuration

Initialization writes the following metadata rules explicitly. They are user-owned seeds, not runtime defaults. A field declaration requires both `form` and `name`; any nonempty field name can select any supported form. Removing a declaration disables its extraction. A missing or empty metadata table produces `metadata: {}` for every node.

```toml
[metadata.abstract]
form = "content-call"
name = "abstract"

[metadata.keywords]
form = "string-arguments-call"
name = "keywords"

[metadata.category]
form = "field-access"
name = "category"

[new]
template = "templates/zettel.typ.tpl"
```

The provider recognizes direct top-level declarations anywhere in a note, in any order, ignoring unrelated imports, styles, show rules, and prose. One level-one title heading with a matching ID label is required. Every configured metadata field is optional and may occur at most once; duplicates and malformed recognized declarations are errors. `content-call` produces a markup value from one literal content block with arbitrary Typst inside it. `string-arguments-call` produces an authored string list from positional string literals. `string-array-call` produces the same kind from one literal array, such as `#tags(("one", "two"))`. `field-access` produces a string containing its member name without validating a presentation dictionary. Field names do not imply engine roles.

Names must be direct Typst identifiers. They do not imply binding resolution, aliases, or recognition of nested or generated metadata. Invalid settings, unsupported forms, empty field names, overlapping call selectors, and overlapping field-access selectors fail configuration loading. Changing a map key changes the retrieved field name; changing its selector changes which source is read. Neither operation rewrites the template or library. The complete source and creation contract is in the [CLI contract](contract/cli.md#initialization-and-source-declarations).

### Disk-backed provider

`zk graph --format json` loads saved canonical `zettel/ID.typ` files and writes a complete JSON snapshot. Noncanonical entries under `zettel/` produce archive diagnostics and do not enter the graph.

The provider parses files concurrently with `typst-syntax` 0.15.1. It extracts the configured direct metadata forms without evaluating Typst. Titles and markup metadata contain exact inner source, deterministic text projection, and half-open UTF-8 byte ranges. The public node retains a generic metadata map with `kind`/`value` entries. Every configured field is present; absent, malformed, or repeated declarations have a `null` value, with diagnostics for the latter two. Unconfigured fields are not emitted. Authored empty markup and string-list values remain non-null. Source generations remain internal and are not serialized.

Literal ten-digit Typst references become directed links. The provider groups repeated occurrences by source-target pair, preserves every authored byte range, and records whether the target resolves. References in raw text, strings, and comments do not become links.

The retained graph interns IDs as `u32` indexes and keeps incoming and outgoing adjacency lists. Parsed source and syntax trees are discarded after extraction.

A shared schema-2 envelope contains `schema_version` and `data`. Graph `data` contains `revision`, `nodes`, `links`, and `diagnostics`; targeted results contain their node or result array directly under `data`. Nodes sort by ID, links by source and target, and diagnostics by path, range, code, field, and message. A new disk-backed provider starts at revision 1, so revisions are not durable archive identifiers. The shared [data contract](contract/data.md) defines all values, ordering, and range meanings.

### Integrity and shell operations

`zk check` prints diagnostics and exits with status 1 when any error exists. Warnings do not fail the command. `zk check --format json` wraps the same diagnostic array used in graph snapshots in a schema-2 envelope. A completed check may return this envelope with exit status 1; fatal inspection failures need not return JSON.

Checks cover:

- noncanonical entries under `zettel/`;
- Typst syntax errors;
- missing or repeated titles and malformed or repeated recognized metadata;
- filename and heading-label mismatches;
- dangling reference occurrences.

The query commands write JSON:

```text
zk query node <ID>
zk query links <ID>
zk query backlinks <ID>
zk query search <QUERY>
```

Metadata search compares the query case-insensitively with IDs, projected titles, and all textual configured values: markup projections, strings, and string-list items. It does not search field names, unrecognized calls, or body-only content. It returns every matching node in provider ID order without an implicit cap. The language server uses the same matcher.

`zk remove <ID>` deletes a canonical Zettel only when it has no incoming references. A blocked removal leaves the file untouched and prints every incoming source path and byte range.

### Live provider sessions

The same `Provider` type supports long-lived clients. `open_buffer` installs full text and a document version. `change_buffer` accepts only newer versions and uses `typst_syntax::Source::replace` for incremental reparsing.

An open overlay replaces the disk node and outgoing links in one graph revision. Incoming adjacency, target resolution, and diagnostics update before consumers observe the revision. Opening a canonical path absent from disk creates a session node.

`save_buffer` retains the overlay. `refresh_disk` ignores open paths. `close_buffer` drops the overlay and reloads disk or removes the node when no disk file exists.

Every scheduled source state receives a generation. Prepared disk updates apply only when their generation remains current. Stale document versions, stale generations, saves, and ignored disk events do not increment the graph revision.

`reload_manifest` reads saved configuration and re-extracts disk notes and open sources together when metadata rules change. It preserves open source text and document versions, invalidates all older prepared results, and publishes one graph revision. Invalid configuration or failed reads leave the previous graph and rules intact. A template-only manifest change does not change graph semantics or its revision.

### Language server

`zk lsp` runs over standard input and output. It loads either an explicit `--archive PATH` or the archive discovered above its working directory, then owns one live provider session.

The server advertises full-text synchronization. Open, change, save, and close notifications map to the provider overlay lifecycle. When the client supports dynamic registration, the server registers `**/zettel/*.typ` and `**/zk.toml` for create, change, and delete events. Watched-file notifications refresh closed Zettel or reload saved metadata rules and cannot replace open overlays. Failed manifest reloads log an error while retaining the last valid state. Unsaved manifest text is not a configuration overlay. Ordinary library buffers and document requests outside canonical Zettel paths are ignored.

The server prefers UTF-8 positions when offered and otherwise uses UTF-16. It converts retained byte ranges with open-buffer text or a saved-text read for each closed-file location.

Initialization advertises ZK protocol version 2 and `dataSchemaVersion: 2` under `capabilities.experimental.zk` with these boolean features. All are true except `categoryCompletion`:

- `archiveQueries`
- `categoryCompletion`, currently false
- `referenceCompletion`
- `referenceTitleDecorations`

The editor protocol version is independent of the executable version, archive format, and provider schema.

Implemented requests:

- `textDocument/completion` supports numeric ID prefixes and case-insensitive title queries after `@`. It advertises only `@` as a trigger and leaves dictionary-member completion to Tinymist.
- `textDocument/hover` returns title and ID followed by all non-null metadata in UTF-8 lexical field-name order, or a missing-target message. Completion depends on core title and ID only; no metadata field is privileged for documentation.
- `textDocument/definition` opens the target, including unsaved session nodes.
- `textDocument/references` returns incoming authored occurrences and optionally the declaration.
- `workspace/symbol` searches live metadata.
- `workspace/executeCommand` supports `zk.queryNode`, `zk.links`, and `zk.backlinks` with exactly one string ID and schema-2 result envelopes. Invalid parameters or missing nodes return `-32602`; unsupported commands return `-32601` before node lookup. Workspace symbols do not infer grouping from category or any other field.

The server pushes archive-specific diagnostics for open Zettel after accepted source updates and watched-file changes. It filters generic Typst syntax diagnostics, leaving those and ordinary bindings, imports, types, and compilation to Tinymist. Resolving or creating a target republishes affected open-buffer diagnostics. Generic syntax diagnostics remain in graph snapshots and `zk check`. Configured-field errors use stable codes `metadata.invalid_shape` or `metadata.duplicate` and a separate field identifier. LSP diagnostic `data` carries the schema version, archive-relative path, field or null, and authored UTF-8 byte range or null independently of negotiated standard positions. Normal shutdown/exit returns process status 0; exit without shutdown returns 1.

## External interface documentation

The authoritative implemented interface definitions are maintained under [docs/contract](contract/README.md). [Design](DESIGN.md) links those definitions rather than maintaining competing wire schemas. Schema-2 fixtures under `tests/fixtures/schema2/` are shared by CLI and LSP tests; the normal integration suites cover generic fields, removal/reload semantics, stable diagnostics, versioned results, request errors, and process lifecycle.

## Code entry points

- `src/main.rs` defines the CLI, JSON output, and process exit behavior.
- `src/archive.rs` implements initialization, discovery, validation, creation, skill installation, and removal.
- `src/config.rs` defines and validates the manifest, bounded metadata matchers, and template-path settings.
- `src/extract.rs` extracts configured metadata, references, ranges, and syntax diagnostics.
- `src/model.rs` defines public node, link, diagnostic, and snapshot shapes.
- `src/provider.rs` loads files, owns graph state and overlays, rejects stale updates, and produces snapshots.
- `src/lsp.rs` implements protocol capabilities, synchronization, diagnostics, navigation, search, and archive commands.
- `src/templates.rs` contains the initial manifest, user-owned template and library defaults, and bundled skill registry.
- `skills/zettelkasten/SKILL.md` is the inspectable bundled skill source.
- `tests/cli.rs` covers authoring, graph output and authored byte ranges, integrity checks, queries, skills, search, and removal blocked by all incoming occurrences, including those in malformed notes.
- `tests/provider.rs` exercises the complete overlay lifecycle through coherent revisions and a final graph snapshot. Unit tests in `src/provider.rs` cover individual transitions and stale-update rejection.
- `tests/lsp.rs` tests the language server over framed standard-input and standard-output JSON-RPC with isolated temporary archives and both UTF-8 and UTF-16 positions.

## Development tools

Repository guidance is in [AGENTS.md](../AGENTS.md). Development checks use Rust 1.94.0 with Rustfmt and Clippy, `just` 1.58.0, and `rumdl` 0.2.78. `rust-toolchain.toml` pins the development toolchain; this is not a declaration of the executable's minimum supported Rust version.

With Rustup installed, set up the pinned tools from the repository root:

```sh
rustup show active-toolchain
cargo install --locked just --version 1.58.0
cargo install --locked rumdl --version 0.2.78
```

Rustup installs the configured toolchain and components when first used in the repository. The recipes use a POSIX shell and support development on Linux and macOS.

## Inspection

Run the same non-mutating checks used by CI:

```sh
just check
```

Individual recipes are `just fmt-check`, `just lint`, `just test`, and `just docs`. `just` without arguments also runs `check`. Each check captures stdout and stderr once, prints a short success line, or prints the complete captured output on failure and returns a failing status. Temporary command logs are removed after the command finishes.

The underlying commands remain available for detailed output:

```sh
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked
rumdl check AGENTS.md docs skills
```

Checks do not rewrite Rust or Markdown source. Cargo commands use `--locked` where supported so verification does not update dependency resolution. Run `cargo fmt` explicitly when formatting changes are intended. Package inspection remains available separately through `cargo package --list --allow-dirty`.

Inspect generated skills and metadata search:

```sh
archive="$(mktemp -d)"
cargo run -- init --agent-skills "$archive"
cargo run -- --archive "$archive" new
cargo run -- --archive "$archive" query search "untitled"
```

Run focused integration suites:

```sh
just test --test cli
just test --test provider
just test --test lsp
```

All suites run under `cargo test` without shell scripts or `jq`. The LSP suite covers protocol-2 and data-schema discovery, shared schema-2 fixtures, strict query errors, ID and title completion, navigation, archive-only diagnostic data, generic metadata hover, atomic saved-manifest reloads and field removal, live search and queries, unsaved state, stale versions, watched files, registration, Tinymist responsibility boundaries, and shutdown/exit statuses. Requests have receive timeouts, shutdown has an exit timeout, and the client reaps the server on failures.

## Continuous integration

[CI](../.github/workflows/ci.yml) runs `just check` on Ubuntu 24.04 for pull requests, pushes to `main`, and manual dispatch. It uses the repository's Rust toolchain file and the same pinned `just` and `rumdl` versions documented above.

The job has read-only repository permissions, does not persist checkout credentials, caches Rust dependencies and installed Cargo tools, cancels superseded runs for the same event and ref, and has a 20-minute timeout. Actions are pinned to full commit SHAs with their release tags noted alongside them. Cache writes are restricted to `main`.

Tool updates should keep the workflow pins, toolchain file, and setup instructions synchronized. CI does not publish releases, change branch protection, or run a macOS or Windows matrix.

## Current limitations

The server loads its initial graph synchronously before serving requests. Clients without dynamic watched-file registration must arrange file notifications. Diagnostics are pushed for open Zettel; archive-wide closed-file inspection remains available through `zk check`.

Closed-file LSP location conversions read saved text for each location. They do not group file reads or refresh graph state automatically. A saved-file change without a watched-file notification can leave graph ranges stale relative to the text used for conversion.

The executable accepts archive format 2 only and has no migration command or format-1 compatibility layer. This is an intentional pre-deployment breaking change. Supplementary Git initialization is accepted but unimplemented; see [Git lifecycle](decisions/git-lifecycle.md).

Queries emit JSON only. CLI locations use UTF-8 byte ranges. Removal does not edit incoming references. Metadata search has no result cap, so broad queries can produce large arrays.

The initial presentation dictionary contains `thoughts`, `physics`, and `coding`. Tinymist owns ordinary dictionary-member completion; `zk lsp` does not inspect the library for categories. Category and keyword vocabulary policy remain deferred.

Source matching supports direct identifiers and the documented finite AST forms only. It does not follow imported declarations, aliases, arbitrary qualified calls, computed values, or nested metadata. Template expansion substitutes only `{{id}}`; it does not evaluate code or infer presentation from the matching rules.
