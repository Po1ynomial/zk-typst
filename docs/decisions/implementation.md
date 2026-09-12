# Implementation and migration

## Provider architecture

The core is a reusable live archive provider. Each consumer instantiates its own provider session.

- `zk lsp` owns a long-lived session and applies open-buffer overlays from LSP document events.
- Ordinary CLI commands create short-lived, disk-backed sessions.
- An open-buffer overlay takes precedence over the corresponding disk file within that session until the buffer closes.
- Full-text LSP updates carry document versions; generation checks discard parse results superseded by newer buffer or disk state.
- Version one has no shared daemon, process discovery, or cross-process session synchronization.

A separate consumer therefore does not see unsaved editor buffers unless it
operates through the LSP session. A shared service adapter may be added later
without changing the archive model.

Live bundled consumers use an internal Rust API. Language-neutral consumers use the versioned JSON snapshot emitted by `zk graph --format json`. Version one has no public live event stream. The provider schema version is independent of the on-disk archive format version.

## Rust and Typst boundary

The Rust provider is authoritative for version-one archive semantics. It parses Typst source without compiling or evaluating it. Rust owns root discovery, filename identity, metadata extraction and validation, open-buffer overlays, literal references and ranges, grouped links, graph revisions, diagnostics, queries, CLI behavior, and ZK language-server behavior.

`lib/zettel.typ` owns presentation and intercepts ten-digit references so Typst
tools do not treat them as unresolved document labels. Typst-side metadata
objects and archive-wide business logic are non-authoritative and deferred to
later compilation paths.

## Language

The `zk` executable is implemented in Rust and uses Typst's `typst-syntax` crate directly.

Reasons:

- metadata and references must come from the real Typst AST;
- parser error recovery matters while editing incomplete buffers;
- byte ranges must map accurately to LSP positions;
- the CLI and `zk lsp` share the same parser and archive model;
- generated content must match the installed Typst generation.

## Version coupling

`typst-syntax` is not an independent stable protocol. `zk` pins a supported Typst minor version and tests newer versions before claiming compatibility.

The installed environment at design time:

- Typst 0.15.1
- typstyle 0.15.1

## Archive format version

`zk.toml` declares one integer archive format version:

```toml
format = 1
```

That version fixes:

- the `YYMMDDHHmm.typ` filename convention;
- the ten-digit `@ID` namespace;
- the required import and `#show` declarations;
- the title and ID heading structure;
- the metadata markup contracts.

A newer `zk` may read older formats but must not silently rewrite them. Source migration uses an explicit command:

```text
zk migrate
```

Generated content may rebuild automatically. Version one keeps no generated content, but future generated artifacts may update without migrating source.

## Library migration

`lib/zettel.typ` is user-owned. Changing it requires an explicit migration or manual edit. `zk init` never silently replaces it.

## Loading and retained state

Zettel files are parsed concurrently at startup. The provider eagerly builds the complete node and relation graph, including source ranges for every literal reference occurrence.

The in-memory representation interns Zettel IDs to compact integer indices and maintains incoming and outgoing adjacency. Its compact storage may use a flat occurrence arena, while the logical consumer model groups all occurrences for an ordered source-target pair into one link. It retains metadata and relation records for every Zettel, but discards source text and syntax trees for closed files. Open buffers retain incrementally updated `typst-syntax` sources.

Version one does not use lazy graph indexing or a persistent cache. It stores source positions once as half-open UTF-8 byte ranges; adapters derive other encodings on demand. At the 50,000-file benchmark limit, a graph containing one million reference occurrences and byte ranges used about 96 MiB. See [Graph index performance](../research/graph-index-performance.md).
