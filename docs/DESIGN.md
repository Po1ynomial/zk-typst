# Design

## System shape

```text
Editor buffers --------------------+
                                    v
Disk files ----> provider session ----> zk lsp
                     |
                     +----> CLI commands
                     +----> JSON snapshot
```

Each consumer owns a provider session. Editor clients send buffer state through
LSP and may run other Typst language servers independently.

## Repository boundaries

`zk` and `zk.nvim` are independent repositories with independent semantic
versions. They have no submodule or umbrella repository.

The `zk` repository owns archive semantics, the CLI, provider, language
server, protocol contract, Typst templates, and archive-local skills. It tests
the LSP over its real transport but contains no Neovim-specific code or tests.

The `zk.nvim` repository owns Lua, Neovim and Tinymist integration, user-facing
plugin documentation, and headless Neovim tests. It owns presentation and
editor workflow but does not extract metadata or retain archive relations.

`zk lsp` reports an integer ZK protocol version and feature flags in its
initialization capabilities. The plugin declares a minimum supported `zk`
release, rejects unsupported protocol versions, and checks feature flags for
optional behavior. Plugin integration tests run against the minimum and latest
supported engine releases.

Protocol version 1 uses `capabilities.experimental.zk`:

```json
{
  "protocolVersion": 1,
  "features": {
    "archiveQueries": true,
    "categoryCompletion": true,
    "referenceCompletion": true,
    "referenceTitleDecorations": true
  }
}
```

The integer versions the editor contract rather than the archive format or
graph snapshot schema. Feature flags allow compatible servers to omit optional
behavior without making clients infer support from an executable version.

Local development uses sibling checkouts and explicit executable paths. A
protocol addition lands compatibly in `zk` before `zk.nvim` requires it.

Each repository has its own `PROJECT.md`, `DESIGN.md`, `SYSTEM.md`, decisions, and research. Plugin knowledge from the former mixed documents is factored into focused `zk.nvim` artifacts. Git records implementation history; the repositories do not share a work-state database.

## Archive root and layout

`zk.toml` marks the archive root:

```toml
format = 1
```

Tools walk upward from the current path until they find this file. LSP clients
use the archive root as the workspace root. This keeps
`../lib/zettel.typ` inside Typst's project sandbox without requiring a Git
repository.

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
permissions, and live editor collaboration remain external to `zk`.

## Git lifecycle

`zk init` currently creates source files and directories only. It does not invoke Git, stage files, or create a commit. Users manage archive version control themselves.

Best-effort Git initialization and a path-limited initial commit are accepted but unimplemented. The planned behavior is preserved in [Git lifecycle](decisions/git-lifecycle.md).

## Archive selection

Filesystem discovery remains the default. Tools walk upward from their current path to the nearest `zk.toml`.

The CLI has no persistent fallback. `--archive PATH` explicitly selects an existing archive and takes precedence over current-directory discovery. `zk init [PATH]` rejects that option.

CLI relative paths resolve against the process working directory. An LSP client
may launch one server process per selected archive with `--archive PATH` or
with that archive as the process working directory.

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
```

Diagnostics are retained separately by the provider and appear in the snapshot's top-level `diagnostics` array, keyed by archive-relative path.

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

Keywords are an optional list of strings; category is an optional string. Missing or malformed fields remain absent and produce diagnostics. Category completion reads keys from the saved direct
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

Both CLI and LSP startup load the initial graph synchronously. `zk lsp` starts serving requests only after `Provider::load` succeeds; there is no background-loading readiness phase.

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

For open files, the LSP adapter converts ranges using the retained source. For closed files, it reads the saved text for each location conversion and scans that text to derive positions. It does not group reads or refresh graph state during conversion.

The model remains independent of LSP position encoding and stores no duplicate coordinates. Closed-file graph state depends on watched-file notifications; positions may be stale if saved text changes without a corresponding notification.

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

It does not launch an editor, provide a TUI, publish documents, compile Typst,
coordinate workers, or silently rewrite Zettel bodies.

## Language-server responsibilities

`zk lsp` provides metadata diagnostics, searchable Zettel completion, category
completion, hover, definitions, references, backlinks, and archive queries.
It advertises ZK protocol version 1 and its feature flags under the standard
experimental server-capability field.
`workspace/symbol` searches live metadata. The `zk.queryNode`, `zk.links`, and
`zk.backlinks` execute commands expose the provider's targeted JSON values to
editor clients.

Reference completion treats numeric text after `@` as an ID prefix and other
text as a case-insensitive title query. It returns at most 100 items, marks the
result incomplete for continued server filtering, and replaces the temporary
query with the selected ID. Empty queries show the newest Zettel. The title
matching and ranking rules remain provisional pending inspection with a large
archive.

For a direct top-level `#category.<query>`, the server returns only keys from a
direct saved `#let category = (...)` dictionary in `lib/zettel.typ`. It does
not evaluate computed library code.

Protocol version 1 advertises archive queries, category completion, reference
completion, and reference-title decoration data as feature flags. Clients
decide how to present those capabilities.

## Tinymist integration

Tinymist handles ordinary Typst language features. The archive library transforms ten-digit references before Typst's default reference realization, preventing unresolved-label diagnostics. `zk` alone determines whether the target Zettel exists.

Tinymist integration is optional and never becomes canonical archive state.

## Diagnostics

`zk check` and `zk lsp` share validation logic. Checks include:

- noncanonical filenames and entries under `zettel/`;
- filename and heading-label mismatch;
- missing, repeated, malformed, or misplaced metadata;
- invalid metadata body structure;
- dangling references.

Incoming references also block removal, but their reported locations are not provider diagnostics. Diagnostics use the current provider revision and exact byte ranges where available.

## Migration

The current executable accepts archive format 1 only. It has no `zk migrate` command and never silently rewrites source for a different format. A future format change requires an explicit, reviewable migration mechanism.

`lib/zettel.typ` is user-owned. `zk init` supplies its initial version but never silently replaces it. Library changes currently require manual edits.

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
