# Overview

## What this is

This project adapts the Zettelkasten Method into a plain-text, editor-native knowledge archive.

The source language is Typst. The editor is Neovim. On disk the archive is a flat directory of linked Typst files with some conventions. Two tools support it: a Rust CLI named `zk` and a companion language server run as `zk lsp`. A thin Neovim plugin wires both into the editing loop.

## Source workflow

The design follows the workflow described in the introduction to the Zettelkasten Method. That article presents the method as a personal tool for thinking and writing, built from connected atomic notes rather than a collection of notes.

The article emphasizes connection over collection, atomic notes, explicit link context, and structure notes. It does not prescribe the popular fleeting notes, literature notes, permanent notes pipeline.

## How this project maps that workflow

| Method concept              | This project                             |
| --------------------------- | ---------------------------------------- |
| One atomic thought per note | One Zettel per `zettel/*.typ` file       |
| Stable address              | Filename and heading label, `YYMMDDHHmm` |
| Knowledge in your own words | Ordinary Typst markup in the body        |
| Connection over collection  | Ten-digit `@ID` references               |
| Link context                | Surrounding prose, by discipline         |
| Structure notes             | Zettel whose content arranges links      |
| Entry points                | Full-text search and navigation          |

## Components

- `zk`: provides the live archive model and creates, checks, removes, queries, and initializes the archive.
- `zk lsp`: companion language server for metadata, references, navigation, backlinks, and diagnostics.
- Neovim plugin: conceals `@ID`, follows links, completes IDs, and shows backlinks and diagnostics.
- `lib/zettel.typ`: user-owned presentation library for archive markup.

Tinymist remains the Typst language server for syntax, formatting, completion, compilation, and preview. The ZK layer adds archive semantics on top.

## Design status

The version-one architecture and global archive access are implemented across the Rust CLI, live provider, companion language server, and Neovim adapter. See [Version-one architecture](v1-architecture.md) for the central design and its research basis, and [System](../SYSTEM.md) for inspection paths and current limitations.

- [Archive](archive.md)
- [Zettel](zettel.md)
- [Metadata](metadata.md)
- [References and graph](references.md)
- [CLI](cli.md)
- [Language server and editor](lsp.md)
- [Global archive access](global-archive-access.md)
- [Git lifecycle](git-lifecycle.md)
- [Implementation and migration](implementation.md)

## References

- [Introduction to the Zettelkasten Method](https://zettelkasten.de/introduction/)
- [Communicating with Slip Boxes](https://zettelkasten.de/communications-with-zettelkastens/)
- [Typst documentation](https://typst.app/docs/)
- [Tinymist](https://github.com/Myriad-Dreamin/tinymist)
- [typstyle](https://github.com/Enter-tainer/typstyle)
- [Neovim](https://neovim.io/)
