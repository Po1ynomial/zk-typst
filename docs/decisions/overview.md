# Overview

## What this is

This project implements a plain-source Zettelkasten engine. The source language is Typst. On disk, one archive is a flat directory of linked Zettel with configurable bounded metadata source forms and fixed identity and reference conventions.

One Rust executable provides the command-line interface, reusable provider, JSON graph snapshots, and companion language server:

```text
zk
zk lsp
```

Editor clients are independent projects. `zk.nvim` is the initial client but is not part of this repository.

## Source workflow

The design adapts the workflow described in the introduction to the Zettelkasten Method. It treats the archive as a personal tool for thinking and writing, built from connected atomic notes rather than arbitrary documents.

| Method concept              | This project                             |
| --------------------------- | ---------------------------------------- |
| One atomic thought per note | One Zettel per `zettel/*.typ` file       |
| Stable address              | Filename and heading label, `YYMMDDHHmm` |
| Knowledge in your own words | Ordinary Typst markup in the body        |
| Connection over collection  | Ten-digit `@ID` references               |
| Link context                | Surrounding prose, by discipline         |
| Structure notes             | Zettel whose content arranges links      |
| Entry points                | Metadata search and navigation           |

## Components

- `zk` initializes, creates, checks, removes, queries, and snapshots archives.
- Asset commands copy, list, and remove opaque files in note-ID namespaces with ordinary Typst paths.
- The provider parses source, retains the graph, and manages live overlays.
- `zk lsp` exposes live archive semantics through a versioned editor protocol.
- `lib/zettel.typ` renders archive constructs and handles ten-digit references.
- Archive-local skills optionally explain the method and command workflow.

## Design status

The archive, provider, CLI, and language server are implemented as described in [System](../SYSTEM.md). Release 0.3.0 uses archive format 3: a regular Typst template declares tracked fields through comments, initialization supplies only the stable core, and saved schema changes reload atomically. [Template-declared metadata](template-schema.md) records the current decision. [Extensible metadata](external-contracts.md) and the authoritative [external contracts](../contract/README.md) define generic fields, data schema 2, and ZK protocol 2. Earlier source-contract decisions remain historical. [Supplementary Git initialization](git-lifecycle.md) is accepted but unimplemented. Format migration has no command yet. See [Version-one architecture](v1-architecture.md) and [Repository boundaries](repository-boundaries.md) for ownership decisions.

- [External CLI, LSP, and shared data contracts](../contract/README.md)
- [Explicit-path asset management](asset-management.md)
- [Template-declared metadata](template-schema.md)
- [Extensible metadata and external contracts](external-contracts.md)
- [Configurable source contracts and user-owned templates](source-contracts.md)
- [Archive](archive.md)
- [Zettel](zettel.md)
- [Metadata](metadata.md)
- [References and graph](references.md)
- [CLI](cli.md)
- [Language server](lsp.md)
- [Git lifecycle](git-lifecycle.md)
- [Agent workers](agent-workers.md)
- [Repository boundaries](repository-boundaries.md)
- [Implementation and migration](implementation.md)

## References

- [Introduction to the Zettelkasten Method](https://zettelkasten.de/introduction/)
- [Communicating with Slip Boxes](https://zettelkasten.de/communications-with-zettelkastens/)
- [Typst documentation](https://typst.app/docs/)
- [typstyle](https://github.com/Enter-tainer/typstyle)
