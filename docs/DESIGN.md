# Design

## System shape

```text
                              +--------------------+
                              | Tinymist           |
                              | Typst intelligence |
                              +--------------------+
                                        ^
                                        |
Neovim buffers ----+--------------------+
                   |
                   v
             +-----------+       +----------------+
Disk files ->| provider  |------>| zk lsp         |
             | session   |       +----------------+
             +-----------+               |
                   |                      v
                   |               Neovim adapter
                   |
                   +----> CLI commands
                   +----> JSON snapshot
```

Tinymist and `zk lsp` are separate language servers. They receive the same editor buffers but do not exchange state.

## Archive root and layout

`zk.toml` marks the archive root:

```toml
format = 1
```

Tools walk upward from the current path until they find this file. The Neovim plugin extends Tinymist's root markers so `zk.toml` selects the same workspace root used by `zk lsp`. This keeps `../lib/zettel.typ` inside Typst's project sandbox without requiring a Git repository.

Canonical paths are fixed:

```text
zk.toml
zettel/
  2603231410.typ
lib/
  zettel.typ
```

An opt-in initialization may also create user-owned agent skills:

```text
.agents/
  skills/
    zettelkasten/
      SKILL.md
```

Other directories are allowed but have no version-one archive semantics.
After optional skill installation, `zk` does not manage `.agents/` or other
additional files.

## Agent operating skills

Ordinary `zk init` does not assume an agent workflow. The boolean
`--agent-skills` flag installs the complete bundled skill set under
`.agents/skills/`. The initial set contains the `zettelkasten` skill.

Installation is best-effort. Existing same-name destinations remain untouched
and produce warnings. Failure to install one skill does not stop other skill
installations, fail canonical archive creation, or roll back files.

Installed skills immediately become user-owned. `zk` does not validate or
refresh them. The initial skill gives workers concise Zettelkasten writing
discipline, the fixed source contract, command recipes, and guidance for
checking their work. Its source is an ordinary
`skills/zettelkasten/SKILL.md` file included in the executable at build time,
so reviewers can inspect it without reading a Rust string literal. It does not
settle category or keyword policy.

Skills provide operating knowledge only. Agent orchestration, review,
permissions, and live Neovim-buffer collaboration remain external to `zk`.

## Git lifecycle

After writing the fixed layout, `zk init` attempts to initialize Git when the
`git` executable is available in `PATH`. It stages and commits only `zk.toml`
and `lib/zettel.typ`, using
`chore: initialize zettelkasten archive` as the commit message. A path-limited
commit preserves unrelated staged changes in an existing repository.

Git setup is best-effort. A missing executable is silent. A failed Git command
produces a warning but does not fail archive initialization or roll back files
and Git state.

The command does not inspect parent repositories, choose a branch name, set
identity or signing options, bypass hooks, create ignore files, or preserve the
empty `zettel/` directory in Git. These remain under user and Git
configuration control.

## Archive selection

Filesystem discovery remains the default. Tools walk upward from their current path to the nearest `zk.toml`.

The Neovim adapter may also receive `archive` in `require("zk").setup`. The nearest archive above the current buffer wins; otherwise the adapter uses its session fallback. A valid fallback starts one eager, initially unattached `zk lsp` client so search and other archive-level commands work before a Zettel buffer opens.

If a command selects a local archive without a running client, the adapter starts one on demand without attaching the unrelated current buffer.

`:ZkSetArchive [PATH]` reports or replaces the session fallback. A successful switch stops only the previous fallback client. Clients serving open buffers from other local archives remain active. Invalid switches leave current state untouched.

The CLI has no persistent fallback. `--archive PATH` explicitly selects an existing archive and takes precedence over current-directory discovery. `zk init [PATH]` rejects that option.

Neovim expands user and environment expressions in configured paths, resolves relative paths against its current working directory, and canonicalizes valid archives. CLI relative paths resolve against the process working directory.

## Zettel source contract

A valid Zettel uses direct top-level constructs:

```typst
#import "../lib/zettel.typ": zettel, abstract, keywords, category
#show: zettel

= Path efficiency <2603231410>

#abstract[
Repeated traffic can produce efficient paths.
]

#keywords(
  "networks",
  "optimization",
)

#category.thoughts

The unrestricted body begins here.

This relates to @2603220935 because ...
```

The header order and forms are part of archive format 1. Aliases, loops, conditional metadata, computed keyword arrays, and other equivalent Typst programs do not define archive metadata.

The required import names form an unordered exact set. Typst formatters may reorder them. Missing, repeated, renamed, or additional names are invalid.

The body may contain arbitrary Typst. Only literal ten-digit reference syntax contributes links.

## Identity

The filename stem is the node ID. It uses local time in `YYMMDDHHmm` form.

If the current minute is occupied, allocation advances one minute at a time until it finds a free filename. Titles may change; IDs do not.

A canonical filename creates a node even when parsing or metadata validation fails. The heading label is a declaration that Rust validates against the filename, not the source of node existence.

## Metadata model

A node contains:

```text
ZettelNode
  id
  path
  generation
  title
  abstract
  keywords
  category
  diagnostics
```

Title and abstract use:

```text
MarkupValue
  source
  text
  range
```

`source` is the exact inner Typst fragment. `text` is a deterministic projection for display and search. `range` is the half-open UTF-8 byte range of the inner fragment.

The text projection:

- concatenates text and spaces;
- recurses through emphasis and strong markup;
- uses raw text contents;
- preserves literal `@ID` references;
- preserves equations and code expressions as source;
- omits comments.

Keywords and category are strings. Missing or malformed fields remain absent
and produce diagnostics. Category completion reads keys from the saved direct
top-level `#let category = (...)` dictionary in `lib/zettel.typ` without
making that presentation library authoritative for Zettel metadata.

## Links and graph

Each literal ten-digit reference is an authored occurrence. The logical graph groups occurrences by ordered source-target pair:

```text
Link
  source
  target
  resolution
  spans
```

`resolution` is `resolved` when a node with the target ID exists and `missing` otherwise. Missing targets do not create synthetic nodes.

`spans` contains one half-open UTF-8 byte range for every authored occurrence. Repeated references therefore remain inspectable without creating parallel logical links.

Incoming and outgoing adjacency are derived indexes. A compact implementation may keep a flat occurrence arena internally, but consumers see grouped links.

References generated by functions, loops, strings, raw blocks, comments, imported code, or assembled labels do not create links.

## Provider session

The provider is a reusable Rust component. Each consumer creates its own session.

A CLI command creates a short-lived disk-backed session. `zk lsp` owns a long-lived session with open-buffer overlays. Version one has no shared daemon.

### Loading

At startup the provider:

1. locates and validates `zk.toml`;
2. enumerates canonical Zettel paths;
3. parses files concurrently;
4. extracts metadata, literal references, and diagnostics;
5. builds nodes, grouped links, and adjacency indexes;
6. discards source text and syntax trees for closed files.

The LSP may return initialization before background loading finishes. ZK features become available after the initial graph revision is ready.

### Retained state

The provider eagerly retains all node metadata, diagnostics, links, and occurrence ranges. It interns IDs to compact integer indexes.

It retains `typst-syntax` sources only for open buffers. The 50,000-file benchmark found that the eager span graph remained under 100 MiB with one million references, while retaining closed syntax trees became expensive.

There is no persistent derived cache or lazy graph indexing in version one.

## Overlay lifecycle

Source precedence is:

```text
open buffer overlay
then disk file
```

`zk lsp` uses full-text synchronization:

- `didOpen` installs the complete buffer text and document version.
- A newer `didChange` replaces the overlay text.
- `typst_syntax::Source::replace` incrementally reparses the changed region.
- `didSave` keeps the overlay authoritative.
- `didClose` drops the overlay and reloads disk, or removes the node if the file is absent.

An unsaved canonical `zettel/ID.typ` buffer creates a session-only node. Disk events update only closed files.

Every source state has a generation number. Parse results apply only when their generation is still current. Accepted replacements update the affected node, replace its outgoing links, repair incoming adjacency, and publish one coherent graph revision.

## Position conversion

The provider stores byte ranges only.

For open files, the LSP adapter converts ranges using the retained source. For closed files, it groups requested ranges by path, reads each file once, and builds a temporary line index. If disk contents no longer match the graph generation, the provider refreshes that node before returning positions.

This keeps the model independent of LSP position encoding and avoids retaining duplicate coordinates.

## Rust and Typst boundary

Rust is authoritative for archive semantics. It uses `typst-syntax` but does not compile or evaluate Typst.

Rust owns:

- root discovery and manifest loading;
- filename identity;
- metadata extraction and validation;
- literal links and ranges;
- graph construction and revisions;
- disk and buffer overlays;
- archive diagnostics and queries;
- CLI and ZK language-server behavior.

`lib/zettel.typ` owns:

- rendering the fixed metadata constructs;
- archive presentation;
- intercepting ten-digit `ref` elements so Typst and Tinymist do not report them as missing labels.

Typst-side `metadata` values may exist as non-authoritative projections for later compilation paths. Version one does not inject graph updates into Tinymist or consume Tinymist as a metadata backend.

## Consumer interfaces

### Internal Rust API

Bundled live consumers operate on provider revisions in-process. The provider exposes node and relation reads, full-text buffer lifecycle methods, disk-update preparation and application, current diagnostics, open `typst-syntax` sources, and complete snapshots. Protocol adapters remain outside the provider.

### JSON snapshots

```text
zk graph --format json
```

emits one complete disk-backed snapshot. Provider schema 1 has these top-level fields:

- `schema_version`
- `revision`
- `nodes`
- `links`
- `diagnostics`

Nodes contain the ID, archive-relative path, generation, optional title and abstract `MarkupValue` objects, keywords, and category. Links contain the source, target, resolution, and every occurrence range. Diagnostics contain the path, stable code, severity, message, and an optional range.

Ranges serialize as `start` and `end` UTF-8 byte offsets. Nodes sort by ID. Links sort by source and target. Diagnostics sort by path and range. Missing or malformed metadata serializes as `null`.

The provider schema version is independent of `zk.toml`'s archive format version. A CLI session revision is not a durable archive identifier.

Version one has no public cross-process change stream.

## CLI responsibilities

`zk` is noninteractive and scriptable. It initializes archives, allocates Zettel, checks invariants, prevents unsafe removal, serves targeted queries, emits graph snapshots, and runs the language server.

`zk check` defaults to text diagnostics and supports a JSON diagnostic array
through `--format json`. Errors produce a failing exit status; warnings do
not. `zk query node`, `zk query links`, `zk query backlinks`, and
`zk query search` emit JSON. `zk remove` reports every incoming byte range
when it refuses deletion.

`zk query search <QUERY>` applies deterministic case-insensitive substring
matching to IDs, projected titles, projected abstracts, keywords, and
categories. The matcher is shared with LSP workspace-symbol search. The
command returns every matching node in provider ID order without an implicit
cap. The bundled skill warns that broad searches can produce large JSON
arrays.

It does not launch Neovim, provide a TUI, publish documents, compile Typst,
coordinate workers, or silently rewrite Zettel bodies.

## LSP and Neovim responsibilities

`zk lsp` provides metadata diagnostics, searchable Zettel completion, category
completion, hover, definitions, references, backlinks, and archive queries.
`workspace/symbol` searches live metadata. The `zk.queryNode`, `zk.links`, and
`zk.backlinks` execute commands expose the provider's targeted JSON values to
the editor adapter.

Reference completion treats numeric text after `@` as an ID prefix and other
text as a case-insensitive title query. It returns at most 100 items, marks the
result incomplete for continued server filtering, and replaces the temporary
query with the selected ID. Empty queries show the newest Zettel. The title
matching and ranking rules remain provisional pending inspection with a large
archive.

For a direct top-level `#category.<query>`, the server returns only keys from a
direct saved `#let category = (...)` dictionary in `lib/zettel.typ`. It does
not evaluate computed library code.

The Neovim plugin:

- eagerly starts the configured fallback archive provider;
- selects local archive clients before the session fallback;
- routes Zettel-sensitive actions to `zk lsp`;
- opens created, selected, and definition-target Zettel in the invocation
  window while preserving a modified prior buffer as hidden;
- exposes a completion filter that lets a configured frontend keep only ZK
  items in category context;
- conceals raw `@ID` text with target-title extmarks;
- presents searches, backlinks, and diagnostics;
- invokes explicit archive commands;
- applies LSP workspace edits.

Global archive commands use the selected client from any buffer. Buffer-local backlinks, refresh, implicit removal, and cursor language features still require a current Zettel. `:ZkSetArchive` changes selection only; archive command semantics do not change.

The plugin recognizes narrow source contexts for routing but does not extract
metadata or maintain a graph. It uses `vim.ui.select` for live metadata search
and quickfix for backlinks and diagnostics. Reference extmarks use provider
byte spans, conceal the authored `@ID`, and insert target titles as inline
virtual text. Context definition routes ten-digit references only to `zk lsp`;
other positions retain Neovim's ordinary multi-server definition behavior.

LSP cannot make one server's completion response exclusive. The plugin's
frontend-neutral helper filters merged completion items when the user composes
it into their completion frontend. Blink uses `sources.transform_items` for
this hook. The plugin does not depend on Blink or mutate its configuration.

## Tinymist integration

Tinymist handles ordinary Typst language features. The archive library transforms ten-digit references before Typst's default reference realization, preventing unresolved-label diagnostics. `zk` alone determines whether the target Zettel exists.

Tinymist integration is optional and never becomes canonical archive state.

## Diagnostics

`zk check` and `zk lsp` share validation logic. Checks include:

- malformed canonical filenames;
- filename and heading-label mismatch;
- missing, repeated, malformed, or misplaced metadata;
- invalid metadata body structure;
- dangling references;
- incoming references that block removal.

Diagnostics use the current provider revision and exact byte ranges where available.

## Migration

Archive format changes require an explicit `zk migrate`. A newer executable may read older formats but must not silently rewrite source.

`lib/zettel.typ` is user-owned. `zk init` supplies its initial version but never silently replaces it. Library changes require manual edits or explicit migration.

## Deferred design

- Controlled vocabulary and category derivation
- Configurable templates
- Structural node roles
- Aggregate or selected-subgraph Typst compilation
- Publishing and export
- Authoritative evaluated Typst metadata
- Graph visualization
- Shared live provider service
- Language-neutral streaming updates
