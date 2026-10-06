# CLI

Status: superseded in part

The standard-template creation and strict metadata-validation policies below are superseded by [Configurable source contracts](source-contracts.md). The command interface and other responsibilities remain in force. The authoritative [CLI contract](../contract/cli.md) specifies the implemented schema-2 interface with envelopes and generic metadata. The raw-output descriptions below record the earlier interface.

## Packaging

One executable named `zk` provides all archive operations as subcommands:

```text
zk init [--agent-skills] [PATH]
zk new
zk remove <ID>
zk check
zk query ...
zk graph --format json
zk lsp
```

`zk lsp` runs the companion language server over stdio. All subcommands share the parser, archive model, diagnostics, and graph operations.

Commands that require an existing archive accept `--archive PATH`. This explicit path takes precedence over current-directory discovery. Without it, existing upward discovery remains unchanged. `zk init [PATH]` rejects `--archive`, and the CLI stores no persistent personal archive setting.

## Commands

### init

Initialize the fixed archive layout at `PATH`, defaulting to the current directory. The command does not currently invoke Git; [supplementary Git setup](git-lifecycle.md) is accepted but unimplemented.

The optional boolean `--agent-skills` flag installs the complete bundled
archive-local skill set under `.agents/skills/`. Installation is best-effort.
Existing same-name skills remain untouched and produce warnings. Installed
skills are user-owned and are not updated by later `zk` commands.

### new

Allocate an ID and create a Zettel from the standard template.

### remove

Remove a Zettel. Refuse while incoming references exist and print each source path and UTF-8 byte range. Successful removal prints the deleted archive-relative path.

### check

Verify archive-wide invariants. Text diagnostics are the default; `--format json` emits the provider diagnostic array. Errors fail the command, while warnings do not.

- noncanonical filenames and entries under `zettel/`;
- Typst syntax errors;
- filename and heading-label mismatches;
- missing, malformed, repeated, or misplaced metadata;
- dangling literal reference occurrences.

Filename identity in the flat directory prevents duplicate node IDs. A Zettel with no links is valid and produces no orphan diagnostic.

### query

Return targeted metadata, links, and backlinks as JSON for shell use:

```text
zk query node <ID>
zk query links <ID>
zk query backlinks <ID>
zk query search <QUERY>
```

Search uses deterministic case-insensitive substring matching across IDs,
projected titles, projected abstracts, keywords, and categories. It returns
all matching node objects in provider ID order without an implicit cap. The
CLI and LSP share the matching rule.

### graph

Emit one complete disk-backed archive snapshot. Version one requires JSON output. The provider schema has its own version independent of the archive format declared in `zk.toml`.

The snapshot contains nodes, grouped links with occurrence ranges and resolution status, diagnostics, and the session revision. Source ranges are half-open UTF-8 byte offsets. Each CLI invocation creates a short-lived provider session, so its revision is not a durable archive identifier.

### lsp

Run the language server.

An editor client may launch it as `zk --archive PATH lsp` for an explicitly
selected archive.

`export` remains reserved for later link resolution, selected-subgraph extraction, and dependency graph output.

## Responsibility

`zk` is a noninteractive, scriptable archive manager.

It should not:

- provide a TUI or picker;
- launch or control an editor;
- coordinate agent workers or live collaboration;
- silently rewrite Zettel bodies;
- manage user-owned files under `.agents/`;
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

Provider loading is synchronous for graph-consuming CLI commands and for `zk lsp`. The server finishes loading before serving initialization or other requests. Background loading is not implemented.

The parser extracts metadata and references, then discards closed-file syntax trees. Open buffers keep their current syntax trees.

This is a shallow syntax pass, not deep Typst resolution. Each source file is parsed once without evaluating imports, functions, or referenced Zettel.
