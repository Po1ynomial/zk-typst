# Project

## Goal

Build a self-contained, editor-native Zettelkasten whose durable state is plain Typst source.

The archive should support the daily work of creating atomic notes, linking ideas in prose, finding prior notes, following links, inspecting backlinks, and maintaining archive integrity. Rust provides archive semantics through the `zk` executable and its companion language server. Neovim is the primary editor. Tinymist continues to provide ordinary Typst language support.

## Users

The version-one user is a single person maintaining one archive on one machine over many years. They work mainly in Neovim and also use shell commands for checks and queries.

External tools may consume a disk-backed JSON graph. Live unsaved state is available only to bundled consumers in the active editor session.

## Version-one scope

### Archive

- One directory is one archive and one ID namespace.
- `zk.toml` marks the root and declares `format = 1`.
- Neovim may configure one personal archive as a session fallback.
- An explicit CLI archive path may select an archive outside the current directory.
- Zettel live in one flat `zettel/` directory.
- Archive-specific Typst presentation code lives in `lib/`.
- Source files are canonical. There is no persistent graph cache or metadata index.

### Zettel

- One `zettel/YYMMDDHHmm.typ` file is one node.
- The filename establishes identity even when the contents are malformed.
- Metadata uses a restricted direct top-level Typst form.
- The body after the metadata header is unrestricted Typst.
- Literal ten-digit `@ID` references create directed links.

### Provider

- Rust parses source with `typst-syntax` without evaluating Typst.
- The provider eagerly retains all node metadata and grouped links.
- Every link retains the byte range of each authored occurrence.
- Closed-file source and syntax trees are discarded after extraction.
- Open buffers retain incrementally updated sources and override disk files.
- The internal Rust API supports live bundled consumers.
- `zk graph --format json` emits a versioned disk-backed snapshot for other consumers.

### Commands

```text
zk init
zk --archive <PATH> ...
zk new
zk remove <ID>
zk check
zk query ...
zk graph --format json
zk lsp
```

### Editor integration

- Tinymist handles ordinary Typst syntax, completion, formatting, diagnostics, compilation, and preview.
- `zk lsp` handles archive metadata, links, completion, navigation, backlinks, queries, and diagnostics.
- A thin Neovim plugin presents ZK operations and title decorations.
- Neovim starts the configured personal archive provider eagerly and can switch the session fallback.

## Constraints

- One machine writes the archive.
- Paths and source contracts are fixed in archive format 1.
- IDs use local time with minute resolution and a ten-digit `YYMMDDHHmm` representation.
- `zk` pins a supported Typst minor version because `typst-syntax` is not a stable independent protocol.
- The archive remains relocatable and complete beneath its root.
- Core behavior must work without Tinymist.
- `zk` must remain useful while buffers and files are incomplete or malformed.

## Non-goals for version one

- Publishing or compiling the archive
- Full dependency-resolution or aggregate Typst compilation
- Typst-evaluated authoritative metadata
- Graph visualization
- A TUI or picker owned by `zk`
- Transclusion
- Structure-note-specific tooling
- Multiple structural node types
- Agent-assisted organization
- Interactive metadata forms
- A custom query language
- A shared daemon or language-neutral live stream
- Persistent derived graph or metadata caches

## Quality expectations

- Source parsing, metadata extraction, and graph construction use one shared implementation across CLI and LSP consumers.
- Diagnostics point to exact authored source ranges whenever a relevant range exists.
- Open-buffer changes cannot be overwritten by stale disk or parse results.
- Commands are noninteractive and scriptable.
- Local archive discovery takes precedence over Neovim's configured fallback.
- Archive migrations are explicit and reviewable.
- The provider does not silently rewrite Zettel bodies or the user-owned Typst library.
- A compact eager graph remains practical at the 50,000-Zettel stress case.

## Success conditions

Version one succeeds when:

- a user can initialize an archive and create a correctly shaped Zettel;
- Neovim can find and create Zettel in a configured personal archive from any buffer;
- shell commands can target an archive explicitly without changing directory;
- Neovim can complete, display, and follow `@ID` references against unsaved state;
- backlinks and reference locations update as buffers change;
- `zk check` reports malformed metadata, mismatched IDs, and dangling links;
- removal is blocked when incoming references exist;
- metadata and graph queries work from the shell;
- an external process can consume a versioned JSON graph snapshot;
- Tinymist does not report archive `@ID` references as missing Typst labels;
- copying the archive root preserves all canonical state.

## Deferred questions

- Controlled vocabulary and category policy
- Configurable Zettel templates
- Structural roles inferred from or declared alongside graph position
- Publication and selected-subgraph compilation
- Authoritative Typst metadata and archive-wide Typst values
- Shared live access for consumers outside the editor process
- Performance changes justified by measurements on a real archive
