# Design

This guide describes the shared implemented design. Generic user-declared metadata, data schema 2, and ZK protocol 2 are implemented in zk 0.2.0. [System](SYSTEM.md) describes flows and limitations. The authoritative external definitions live under [Contract](contract/README.md); this guide explains their shared semantics and ownership rather than duplicating wire schemas.

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

The server uses ZK protocol 2 with independently advertised data schema 2 under `capabilities.experimental.zk`. The exact capabilities, standard method behavior, custom commands, and diagnostic data are defined in the [LSP contract](contract/lsp.md). Feature flags permit compatible servers to omit optional behavior without clients inferring support from an executable version. CLI and LSP archive queries share the [data schema](contract/data.md).

Local development uses sibling checkouts and explicit executable paths. A
protocol addition lands compatibly in `zk` before `zk.nvim` requires it.

Each repository has its own `PROJECT.md`, `DESIGN.md`, `SYSTEM.md`, decisions, and research. Plugin knowledge from the former mixed documents is factored into focused `zk.nvim` artifacts. Git records implementation history; the repositories do not share a work-state database.

## Archive root and layout

`zk.toml` marks the archive root:

```toml
format = 2
```

Tools walk upward from the current path until they find this file. LSP clients
use the archive root as the workspace root. This keeps
`../lib/zettel.typ` inside Typst's project sandbox without requiring a Git
repository.

Canonical note paths are fixed. The default initialized layout is:

```text
zk.toml
zettel/
  2603231410.typ
lib/
  zettel.typ
templates/
  zettel.typ.tpl
```

Only `zk.toml` and `zettel/` are required to inspect an archive. The template path is configured in the manifest. The initial library and template are user-owned presentation and authoring inputs.

An opt-in initialization may also create user-owned agent skills:

```text
.agents/
  skills/
    zettelkasten/
      SKILL.md
```

Other directories are allowed; only configured source inputs and canonical notes have archive semantics.
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

This is an initializer template, not an enforced header. Archive format 2 recognizes direct top-level declarations in any order and position among imports, styling, and prose. One level-one title with its ID label is required. Other fields are optional user-declared metadata, each with at most one occurrence. Abstract content and ordinary document content may contain arbitrary Typst. Imports and show rules are presentation choices, not extraction requirements. Only literal ten-digit reference syntax contributes links.

The manifest's `metadata` table is a map of arbitrary field names to bounded AST rules. Initialization writes the abstract, keywords, and category seed definitions explicitly. Runtime extraction never restores removed fields; an omitted or empty table means no extra metadata extraction. Adding a tag field changes configuration rather than the node's structural schema. Matching forms produce markup, string, or string-list values independently of their field names. Exact forms, validation, initialization, and creation behavior are defined in the [CLI contract](contract/cli.md#initialization-and-source-declarations).

Matching does not evaluate imports, bindings, aliases, qualified calls, nested declarations, conditionals, or generated values. Unsupported or overlapping rules fail clearly instead of guessing. The rationale and compatibility consequences are recorded in [Extensible metadata and external contracts](decisions/external-contracts.md).

`[new] template = "templates/zettel.typ.tpl"` selects an archive-local UTF-8 template. `zk init` supplies its default once. `zk new` replaces only literal `{{id}}` markers and preserves all other bytes. It validates the rendered title and recognized metadata before creating a file without overwriting an existing path. Missing or invalid templates fail rather than falling back. Template paths must remain inside the archive, including their resolved symlink targets. Relative Typst imports refer to the resulting Zettel's location. Changing extraction rules never silently rewrites the template or library.

## Identity

The filename stem is the node ID. It uses local time in `YYMMDDHHmm` form.

If the current minute is occupied, allocation advances one minute at a time until it finds a free filename. Titles may change; IDs do not.

A canonical filename creates a node even when parsing or metadata validation fails. The heading label is a declaration that Rust validates against the filename, not the source of node existence.

## Metadata model

The public node has a stable core of filename ID, archive-relative path, recovered title, and a generic metadata map. Configured field names are data, not an engine enumeration. A name such as abstract, keywords, category, or status has no implicit semantic role. Internal source generations remain implementation state rather than persistent public note identity.

Values distinguish exact markup content, literal strings, and authored string lists. Markup retains source, deterministic projected text, and its inner UTF-8 byte range without evaluation. Omitted configured fields are null; malformed or repeated declarations are null with field-specific diagnostics; unconfigured fields are not emitted. Empty authored values remain distinct from omission.

The authoritative node, typed-value, absence, projection, range, diagnostic, and ordering definitions are in [Shared data schema 2](contract/data.md). Search examines all textual declared values using the shared deterministic matcher. It does not privilege default metadata names or search arbitrary document bodies.

## Links and graph

Each literal ten-digit reference is an authored occurrence. The provider groups occurrences by ordered source/target pair, preserves every byte range, and derives incoming and outgoing adjacency. A target resolves when its node exists, including a malformed node; missing targets do not create synthetic nodes.

References remain archive links independently of whether their surrounding source is recognized as metadata. Strings, raw blocks, comments, imported code, and generated references do not contribute authored links. The public link shape and array ordering are defined once in the [shared data contract](contract/data.md#link).

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

Saved manifest changes re-extract all closed notes and open sources under the new metadata rules in one revision. All fallible reads complete before live state changes. Open source and versions survive, while all results prepared under old rules become stale. Invalid configuration or failed reads preserve the previous rules and graph. Unsaved manifest text is not a configuration overlay.

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

- rendering the archive's chosen metadata constructs;
- archive presentation;
- intercepting ten-digit `ref` elements so Typst and Tinymist do not report them as missing labels.

Typst-side `metadata` values may exist as non-authoritative projections for later compilation paths. Version one does not inject graph updates into Tinymist or consume Tinymist as a metadata backend.

## Consumer interfaces

### Internal Rust API

Bundled live consumers operate on provider revisions in-process. The provider exposes node and relation reads, full-text buffer lifecycle methods, disk-update preparation and application, current diagnostics, open `typst-syntax` sources, and complete snapshots. Protocol adapters remain outside the provider.

### JSON snapshots

The CLI emits a complete saved-state graph through `zk graph --format json`. Targeted CLI queries and LSP archive-query commands share the same versioned node, metadata, link, and diagnostic values. The [data contract](contract/data.md) defines schema-2 envelopes and result shapes; the [CLI contract](contract/cli.md) and [LSP contract](contract/lsp.md) define their transport behavior.

Data schema versions are independent of archive and editor protocol versions. Revisions describe one session's graph state, not durable archive identity. There is no public cross-process change stream or shared unsaved state.

## CLI responsibilities

`zk` is noninteractive and scriptable. It initializes archives, creates notes from user-owned templates, checks invariants, prevents removal while incoming references remain, exposes saved-state queries, emits graph snapshots, and runs the language server. It does not launch an editor, compile or publish Typst, coordinate workers, or silently rewrite source.

Invocation, archive selection, streams, exit statuses, creation validation, JSON envelopes, and command-specific results are specified in the [CLI contract](contract/cli.md). Search applies the shared matcher to IDs, titles, and all textual declared metadata without an implicit result cap. Human-facing diagnostics and error wording are not machine-readable output contracts.

## Language-server responsibilities

`zk lsp` exposes live archive-specific metadata diagnostics, reference completion, hover, navigation, incoming reference locations, workspace search, and targeted archive queries. Open source takes precedence over saved files. Ordinary library buffers and unrelated document requests do not belong to its graph session.

Hover displays declared metadata generically in deterministic field-name order. Reference completion uses only core title and ID without assuming an abstract field. No configurable field implicitly supplies grouping, lifecycle, or presentation roles. Dictionary-member completion remains Tinymist's responsibility.

The [LSP contract](contract/lsp.md) owns protocol-2 discovery, feature flags, source synchronization, file notifications, method behavior, error delivery, standard-position conversion, and diagnostic data. The CLI and custom editor queries use the same shared data schema but different source views.

## Tinymist integration

Tinymist handles ordinary Typst syntax, bindings, imports, types, dictionary-member completion, rendering, and compilation. `zk lsp` does not publish generic Typst syntax diagnostics. The archive library transforms ten-digit references before Typst's default reference realization, preventing unresolved-label diagnostics. `zk` alone determines whether the target Zettel exists.

Tinymist integration is optional and never becomes canonical archive state.

## Diagnostics

`zk check` and `zk lsp` share validation logic. Checks include:

- noncanonical filenames and entries under `zettel/`;
- filename and heading-label mismatch;
- missing or repeated titles;
- repeated or malformed recognized metadata declarations;
- dangling references.

Incoming references also block removal, but their reported locations are not provider diagnostics. Diagnostics use the current provider revision and exact byte ranges where available. Generic syntax diagnostics remain in provider snapshots and `zk check` but are filtered from LSP publications. Imports, show rules, declaration order, and abstract block content are not archive validity requirements.

## Migration

The current executable accepts archive format 2 only. Release 0.2.0 intentionally breaks the pre-deployment format-1 source contract without adding a migration command. It never silently rewrites source, templates, or libraries. Future migrations for deployed archives must be explicit and reviewable.

`lib/zettel.typ` is user-owned. `zk init` supplies its initial version but never silently replaces it. Library changes currently require manual edits.

## Deferred design

- Controlled vocabulary and category derivation
- Structural node roles
- Aggregate or selected-subgraph Typst compilation
- Publishing and export
- Authoritative evaluated Typst metadata
- Graph visualization
- Shared live provider service
- Language-neutral streaming updates
