# Project

## Goal

Build a self-contained Zettelkasten engine whose durable state is plain Typst source.

Assets remain ordinary files associated with note IDs; note identity and metadata remain Typst source.

The archive should support creating atomic notes, linking ideas in prose, finding prior notes, following links, inspecting backlinks, and maintaining archive integrity. The `zk` executable provides archive semantics through a scriptable CLI, reusable Rust provider, and editor-neutral language server.

The independently maintained `zk.nvim` repository is one client of this engine. It is not part of this repository or release.

## Users

The version-one user is a single person maintaining one archive on one machine over many years. They may work through shell commands, an LSP client, or another consumer of the JSON graph.

Agent workers may use optional archive-local operating skills and the same saved-state CLI available to the user. Live unsaved state belongs to the client session that owns the corresponding `zk lsp` process.

## Current scope

### Archive

- One directory is one archive and one ID namespace.
- `zk.toml` marks the root and contains only `format = 3`. The fixed `templates/zettel.typ` declares tracked metadata through comments.
- `zk init` creates the minimal core template and reference library without initializing or modifying Git. Optional templates supply additional fields.
- `zk init --agent-skills` may install bundled archive-local skills. Installed skills immediately become user-owned.
- An explicit CLI archive path may select an archive outside the current directory.
- Zettel live in one flat `zettel/` directory.
- Archive-specific Typst presentation code lives in `lib/`.
- `zk asset add/list/remove` manages opaque files under `assets/ID/`, creating directories lazily and retaining files after note removal.
- Authors use explicit Typst paths. The engine does not inject asset helpers or discover transitive dependencies.
- Source files are canonical. There is no persistent graph cache or metadata index.

### Zettel

- One `zettel/YYMMDDHHmm.typ` file is one node.
- The filename establishes identity even when the contents are malformed.
- Metadata uses configurable direct top-level Typst forms in any order and position among other source constructs.
- One title with its ID label is required. Other metadata is a map of optional user-declared fields, with markup, string, and string-list values.
- Removing template declarations disables extraction. There are no built-in additional fields. A missing template is created with only the core; an invalid one raises an error.
- Imports, show rules, and presentation are user choices. Abstract content and ordinary document content are unrestricted Typst.
- `zk new` replaces the parsed title label and strips declaring comments, preserving all other bytes without evaluating Typst.
- Literal ten-digit `@ID` references create directed links.

### Provider

- Rust parses source with `typst-syntax` without evaluating Typst.
- The provider eagerly retains all node metadata and grouped links.
- Every link retains the byte range of each authored occurrence.
- Closed-file source and syntax trees are discarded after extraction.
- Open buffers retain incrementally updated sources and override disk files.
- Public archive JSON results use data schema 2 envelopes shared by CLI and LSP queries.
- `zk graph --format json` emits a versioned disk-backed snapshot.

### Commands

```text
zk init [--agent-skills] [PATH]
zk --archive <PATH> ...
zk new
zk remove <ID>
zk asset add <ID> <SOURCE> [--name <RELATIVE-PATH>]
zk asset list <ID>
zk asset remove <ID> <RELATIVE-PATH>
zk check
zk query node|links|backlinks|search ...
zk graph --format json
zk lsp
```

### Language server

- `zk lsp` handles archive metadata, links, reference completion, navigation, backlinks, queries, and archive-specific diagnostics.
- Tinymist handles general Typst language intelligence and dictionary-member completion. Generic syntax diagnostics remain available in the CLI, not the archive language server.
- Saved template changes atomically re-extract disk and open-buffer metadata using the new rules. Invalid reloads visibly report errors while retaining the previous live state.
- Full-text document synchronization supplies live unsaved overlays.
- LSP initialization advertises ZK protocol 2, data schema 2, and feature flags independently of the executable version.
- Protocol-level tests use the real standard-input and standard-output transport.

## Constraints

- One machine writes the archive.
- Note paths, the template path, title identity, and reference syntax are fixed in archive format 3. Comment declarations select bounded direct metadata forms.
- IDs use local time with minute resolution and a ten-digit `YYMMDDHHmm` representation.
- `zk` pins a supported Typst minor version because `typst-syntax` is not a stable independent protocol.
- The archive remains relocatable and complete beneath its root.
- Git is optional and never becomes canonical archive state.
- The engine must remain useful while buffers and files are incomplete or malformed.
- Editor clients remain separate repositories and releases.
- Protocol versions are independent of executable, archive-format, and graph-schema versions.

## Non-goals for version one

- Publishing or compiling the archive
- Full dependency resolution or aggregate Typst compilation
- Typst-evaluated authoritative metadata
- Graph visualization
- A TUI or picker owned by `zk`
- Editor-specific code or presentation
- Transclusion
- Structure-note-specific tooling
- Multiple structural node types
- Agent orchestration or autonomous organization owned by `zk`
- Interactive metadata forms
- A custom query language
- A shared daemon or language-neutral live stream
- Persistent derived graph or metadata caches

## Quality expectations

- Source parsing, metadata extraction, and graph construction use one shared implementation across CLI and LSP consumers.
- The repository contains no editor-specific client code or tests.
- Diagnostics point to exact authored source ranges whenever a relevant range exists.
- Open-buffer changes cannot be overwritten by stale disk or parse results.
- Commands are noninteractive and scriptable.
- Metadata search uses one deterministic matching rule across CLI and LSP consumers.
- Archive migrations are explicit and reviewable.
- The provider does not silently rewrite Zettel bodies or the user-owned Typst library.
- A compact eager graph remains practical at the 50,000-Zettel stress case.

## Success conditions

Version one succeeds when:

- a user can initialize an archive and create a correctly shaped Zettel;
- shell commands can discover an archive or target it explicitly;
- `zk lsp` can search metadata, complete and resolve references, and report backlinks against unsaved state;
- archive metadata extraction respects the configured rules without enforcing presentation;
- creation uses an editable archive-local template;
- schema reloads preserve open source and reject stale results;
- backlinks and reference locations update as buffers change;
- `zk check` reports malformed metadata, mismatched IDs, and dangling links;
- removal is blocked when incoming references exist;
- metadata and graph queries work from the shell;
- metadata search returns every matching node in deterministic order;
- an external process can consume a versioned JSON graph snapshot;
- clients can reject incompatible ZK protocol versions;
- assets can be copied without clobbering, listed deterministically, and removed explicitly after their note disappears;
- copying the archive root preserves all canonical state.

## Accepted but unimplemented

- [Supplementary Git initialization](decisions/git-lifecycle.md) remains an accepted request. `zk init` does not yet invoke Git.

## Deferred questions

- Controlled vocabulary and category policy
- Structural roles inferred from or declared alongside graph position
- Publication and selected-subgraph compilation
- Authoritative Typst metadata and archive-wide Typst values
- Shared live access for consumers outside one LSP process
- Orphan asset-namespace warnings and asset-specific live queries
- Performance changes justified by measurements on a real archive
