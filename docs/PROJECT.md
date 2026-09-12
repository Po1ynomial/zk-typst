# Project

## Goal

Build a self-contained, editor-native Zettelkasten whose durable state is plain Typst source.

The archive should support the daily work of creating atomic notes, linking ideas in prose, finding prior notes, following links, inspecting backlinks, and maintaining archive integrity. Rust provides archive semantics through the `zk` executable and its companion language server. Neovim is the primary editor. Tinymist continues to provide ordinary Typst language support.

The Rust tool and Neovim plugin are separate products. They live in
independent `zk` and `zk.nvim` repositories and communicate through a
versioned editor-neutral protocol.

## Users

The version-one user is a single person maintaining one archive on one machine over many years. They work mainly in Neovim and also use shell commands for checks and queries.

External tools may consume a disk-backed JSON graph. Live unsaved state is available only to bundled consumers in the active editor session.

Agent workers may participate through external editor coordination. `zk`
provides optional archive-local operating skills, data queries, and the same
whole-file creation and removal operations available to human users. It does
not orchestrate collaboration.

## Version-one scope

### Archive

- One directory is one archive and one ID namespace.
- `zk.toml` marks the root and declares `format = 1`.
- `zk init` attempts a supplementary Git repository and initialization commit
  when `git` is available, but Git failures do not block archive creation.
- An explicit `zk init` flag may install bundled archive-local agent skills.
  Ordinary initialization remains agent-neutral, and installed skills are
  user-owned.
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
zk init [--agent-skills]
zk --archive <PATH> ...
zk new
zk remove <ID>
zk check
zk query node|links|backlinks|search ...
zk graph --format json
zk lsp
```

### Editor integration

- Tinymist handles ordinary Typst syntax, completion, formatting, diagnostics, compilation, and preview.
- `zk lsp` handles archive metadata, links, searchable reference and category
  completion, navigation, backlinks, queries, and diagnostics.
- The independently released `zk.nvim` plugin presents ZK operations and title
  decorations.
- Neovim starts the configured personal archive provider eagerly and can switch the session fallback.

## Constraints

- One machine writes the archive.
- Paths and source contracts are fixed in archive format 1.
- IDs use local time with minute resolution and a ten-digit `YYMMDDHHmm` representation.
- `zk` pins a supported Typst minor version because `typst-syntax` is not a stable independent protocol.
- The archive remains relocatable and complete beneath its root.
- Git is optional and never becomes canonical archive state.
- `zk` and `zk.nvim` have independent versions and no submodule or umbrella
  repository.
- `zk.nvim` declares a minimum `zk` version and negotiates an explicit ZK
  protocol version and feature flags.
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
- Agent orchestration or autonomous organization owned by `zk`
- Interactive metadata forms
- A custom query language
- A shared daemon or language-neutral live stream
- Persistent derived graph or metadata caches

## Quality expectations

- Source parsing, metadata extraction, and graph construction use one shared implementation across CLI and LSP consumers.
- The Rust repository contains no Neovim-specific code or tests after the
  repository split.
- The Neovim plugin does not duplicate archive parsing or graph semantics.
- Diagnostics point to exact authored source ranges whenever a relevant range exists.
- Open-buffer changes cannot be overwritten by stale disk or parse results.
- Commands are noninteractive and scriptable.
- Metadata search uses one deterministic matching rule across CLI and LSP
  consumers.
- Local archive discovery takes precedence over Neovim's configured fallback.
- Archive migrations are explicit and reviewable.
- The provider does not silently rewrite Zettel bodies or the user-owned Typst library.
- A compact eager graph remains practical at the 50,000-Zettel stress case.

## Success conditions

Version one succeeds when:

- a user can initialize an archive and create a correctly shaped Zettel;
- Neovim can find and create Zettel in a configured personal archive from any buffer;
- shell commands can target an archive explicitly without changing directory;
- Neovim can search titles while completing, display, and follow `@ID`
  references against unsaved state;
- Neovim can restrict direct category completion to keys declared by the
  archive library when its completion frontend uses the ZK filter;
- `zk.nvim` detects incompatible ZK protocol versions and tests against its
  minimum and latest supported `zk` releases;
- backlinks and reference locations update as buffers change;
- `zk check` reports malformed metadata, mismatched IDs, and dangling links;
- removal is blocked when incoming references exist;
- metadata and graph queries work from the shell;
- shell users and workers can search metadata directly without loading the
  complete graph;
- a user can opt into archive-local Zettelkasten operating guidance without
  affecting unrelated agent sessions;
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
