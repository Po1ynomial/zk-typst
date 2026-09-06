# CLI

## Packaging

One executable named `zk` provides all archive operations as subcommands:

```text
zk init
zk new
zk remove <ID>
zk check
zk query ...
zk graph --format json
zk lsp
```

`zk lsp` runs the companion language server over stdio. All subcommands share the parser, archive model, diagnostics, and graph operations.

## Commands

### init

Initialize the fixed archive layout.

### new

Allocate an ID and create a Zettel from the standard template.

### remove

Remove a Zettel. Refuse while incoming references exist and print their locations.

### check

Verify archive-wide invariants:

- dangling and stale references;
- duplicate IDs;
- filename and heading-label mismatches;
- malformed or missing metadata constructs;
- orphan Zettel as warnings.

### query

Return targeted metadata, links, and backlinks for shell use.

### graph

Emit one complete disk-backed archive snapshot. Version one requires JSON output. The provider schema has its own version independent of the archive format declared in `zk.toml`.

The snapshot contains nodes, grouped links with occurrence ranges and resolution status, diagnostics, and the session revision. Source ranges are half-open UTF-8 byte offsets. Each CLI invocation creates a short-lived provider session, so its revision is not a durable archive identifier.

### lsp

Run the language server.

`export` remains reserved for later link resolution, selected-subgraph extraction, and dependency graph output.

## Responsibility

`zk` is a noninteractive, scriptable archive manager.

It should not:

- provide a TUI or picker;
- launch or control Neovim;
- silently rewrite Zettel bodies;
- maintain canonical graph state outside the source;
- perform publishing or compilation.

Later AST transformations can be explicit commands that produce reviewable edits.

## Runtime model

- CLI commands operate on saved files.
- `zk lsp` also tracks unsaved editor buffers.
- CLI commands do not discover or communicate with a running LSP.
- There is no daemon discovery or session state between CLI invocations.

## Loading

At startup, `zk` reads all Zettel and builds the relation graph and metadata table in memory. There are no centralized artifacts to keep in sync.

For ordinary CLI commands, loading is synchronous. For `zk lsp`, initialization returns promptly while a background task builds the model. ZK features become available when that task completes.

The parser extracts metadata and references, then discards closed-file syntax trees. Open buffers keep their current syntax trees.

This is a shallow syntax pass, not deep Typst resolution. Each source file is parsed once without evaluating imports, functions, or referenced Zettel.
