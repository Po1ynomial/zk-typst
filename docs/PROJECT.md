# Project

## Goal and motivation

Build a self-contained Zettelkasten engine whose durable state is plain Typst source. The archive supports developing atomic thoughts, explaining connections in prose, revisiting prior thinking, and keeping references valid over many years.

The owner supplies the intellectual judgment. The engine supplies creation, retrieval, navigation, and integrity checks without becoming an editor, an autonomous organizer, or a publisher. [Philosophy](PHILOSOPHY.md) elaborates this doctrine and its relationship to Typst; the [glossary](../GLOSSARY.md) defines the domain terms.

## Users

The initial user is a single person maintaining one archive on one machine. They may work through shell commands, an LSP client, or another consumer of the saved JSON graph.

Agent workers use the same saved-state operations and may receive optional archive-local operating skills. Unsaved source belongs to the client session that owns its language-server process.

## Scope

- A relocatable archive of Typst Zettel and ordinary associated assets beneath one root.
- Stable note identity, authored directed links, backlinks, and archive integrity checks.
- A user-owned template for creation and optional tracked metadata, without engine-defined category or keyword policy.
- A noninteractive CLI for note and asset lifecycle, metadata queries, and saved graph snapshots.
- A reusable Rust provider and editor-neutral language server for live archive semantics.
- Complementary use with Tinymist for general Typst language intelligence.
- Optional, user-owned agent operating skills rather than engine-owned orchestration.

[System](SYSTEM.md) describes implemented capabilities and limitations. [Design](DESIGN.md) owns shared responsibilities and invariants; [contracts](contract/README.md) own exact source and interface definitions.

## Constraints

- One machine writes the archive.
- Copying the archive root preserves all canonical state.
- Source is canonical; Git and derived graph state are not.
- The engine remains useful while files and buffers are incomplete or malformed.
- Archive semantics come from bounded authored source forms, not Typst evaluation.
- The supported Typst parser generation is pinned because its syntax API is not an independent stable protocol.
- Editors and their releases remain separate from the engine.
- Executable, archive, data, and editor-protocol versions have independent meanings.
- Breaking changes to deployed archives require explicit, reviewable migration.

## Non-goals for the initial scope

- Publishing, compiling, or visualizing the archive
- Full dependency resolution or aggregate Typst compilation
- Evaluated Typst metadata as authoritative archive state
- A TUI, picker, or editor-specific presentation owned by `zk`
- Transclusion or structure-note-specific tooling
- Multiple structural node types
- Agent orchestration, autonomous organization, or interactive metadata forms
- A custom query language
- A shared daemon or language-neutral live stream
- Persistent derived graph or metadata caches

## Quality expectations

- CLI and LSP share extraction, validation, graph construction, and metadata matching.
- Diagnostics identify exact authored ranges when a relevant range exists.
- Stale disk or parse work cannot overwrite open source.
- Commands remain noninteractive and scriptable.
- Creation, removal, and configuration changes do not silently rewrite existing Zettel or presentation code.
- The repository contains no editor-specific client code or tests.
- A compact eager graph remains practical at the 50,000-Zettel stress case. Performance claims about the executable require measurements of the executable, not just prototypes.

## Success conditions

The initial scope succeeds when a user can:

- initialize an archive and create a correctly shaped Zettel from its editable template;
- discover an archive from a working directory or select it explicitly;
- retrieve metadata, follow links, and inspect backlinks through saved-state commands;
- search, complete, and navigate references against unsaved editor state;
- change tracked metadata rules without losing open source or accepting stale results;
- inspect malformed metadata, identity mismatches, and dangling references;
- remove a Zettel safely when no incoming references remain;
- consume a versioned graph snapshot and reject incompatible interfaces;
- attach files without clobbering, inspect them deterministically, and remove them explicitly after their Zettel disappears;
- copy the root to preserve the complete archive.

Writing quality remains the owner's responsibility, distinct from passing integrity checks.

## Deferred questions

- Controlled vocabulary and category policy
- Structural roles inferred from or declared alongside graph position
- Publication and selected-subgraph compilation
- Authoritative evaluated metadata and archive-wide Typst values
- Shared live access outside one LSP process
- Orphan asset warnings and asset-specific live queries
- Performance changes justified by measurements on a real archive
