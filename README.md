# zk

`zk` is a local Zettelkasten engine for atomic notes written in Typst. It creates notes, tracks links and backlinks, searches metadata, checks archive integrity, manages attached files, and exports a JSON graph.

The archive is ordinary files beneath one root. Notes and templates are canonical source; assets remain ordinary files. There is no persistent metadata database or graph cache. The intended user is one person maintaining an archive on one machine.

The Rust executable provides a scriptable CLI and an editor-neutral language server through `zk lsp`.

## Basic use

Install from a local checkout:

```sh
cargo install --locked --path .
```

Then:

```sh
zk init ~/notes
zk --archive ~/notes new
zk --archive ~/notes query search "untitled"
zk --archive ~/notes check
```

`new` prints the created note's archive-relative path. Edit that Typst file in your editor. Its timestamp filename is its permanent ID; use literal `@ID` references to connect notes and explain each connection in prose.

`templates/zettel.typ` controls new-note content and tracked metadata. Initialization supplies only a labelled title and reference presentation, with no built-in abstract, keyword, or category fields. Search covers IDs, titles, and declared metadata, not arbitrary body text. CLI queries read saved files; `zk lsp` also sees its own unsaved buffers. Initialization does not invoke Git.

## Documentation

- [Philosophy](docs/PHILOSOPHY.md): thinking practice, central doctrine, and the relationship to Typst.
- [Glossary](GLOSSARY.md): domain terms and their distinctions.
- [Project](docs/PROJECT.md): goals, users, scope, constraints, and success conditions.
- [System](docs/SYSTEM.md): implemented capabilities, workflows, limitations, code entry points, and development checks.
- [Design](docs/DESIGN.md): shared design, responsibilities, and invariants.
- [External contracts](docs/contract/README.md): authoritative contracts and current interface versions.
- [Decisions](docs/adr/README.md): accepted choices and their rationale.
- [Research](docs/research/README.md): recorded experiments, evidence, and limitations.
- [Contributing](CONTRIBUTING.md): reporting problems, requesting changes, and code expectations.

## Usage

Keep one principal thought per Zettel, write in your own words, and explain connections in prose. Search provides entry points; links, backlinks, and structure notes help revisit and develop prior thinking. [Philosophy](docs/PHILOSOPHY.md) elaborates this practice, its sources, and its adaptation to Typst.
