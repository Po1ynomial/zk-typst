# Design

This guide describes the shared implemented design. Generic user-declared metadata, data schema 2, and ZK protocol 2 are implemented in zk 0.3.0. [System](SYSTEM.md) describes flows and limitations. The authoritative external definitions live under [Contract](contract/README.md); this guide explains their shared semantics and ownership rather than duplicating wire schemas.

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
format = 3
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
  zettel.typ
```

`templates/zettel.typ` is the fixed schema and creation input. Archive loading creates a minimal core template if absent and fails on a present invalid template. Inspection may therefore write this file. The library is optional for extraction. Both installed files are user-owned.

An opt-in initialization may also create user-owned agent skills:

```text
.agents/
  skills/
    zettelkasten/
      SKILL.md
```

Other directories are allowed. `assets/ID/` namespaces have explicit file-lifecycle commands but no graph semantics.
After optional skill installation, `zk` does not manage `.agents/` or other
additional files.

## Asset storage

Assets use `assets/ID/name`, with nested namespace-relative names permitted. Directories are created lazily. The engine copies opaque bytes, lists regular files deterministically, and removes an explicitly named file. Asset commands operate on saved filename identity without evaluating or loading the note graph.

Authors use native Typst paths such as `/assets/2603231410/tiger.jpg`. No contextual resolver, generated ID binding, source insertion, dependency graph, or loader wrapper is installed. [The asset-resolution research](research/asset-resolution.md) explains the authoring trade-off; [Explicit-path asset management](decisions/asset-management.md) records the accepted policy. Exact CLI behavior and payloads live under [Contract](contract/cli.md#asset-management).

Namespace association is not exclusive ownership. Note removal retains assets; listing and explicit removal work for orphan namespaces. Symlinks in stored namespaces, traversal, special files, and clobbering are rejected. Copying stages outside ID namespaces before no-clobber publication. Direct file edits remain valid canonical state. Asset bytes and references within them do not enter the graph.

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

Filename identity, a single labelled title, and literal ten-digit links are the stable core. Initialization supplies a regular Typst starter with no additional fields. The fixed `templates/zettel.typ` owns both starter text and metadata declarations. `zk.toml` only declares the archive format.

Standalone top-level declaration comments name a metadata field and output kind. The following element supplies a direct selector and a supported literal shape. The provider compiles these into independent matchers rather than matching the entire document skeleton. Notes can reorder declarations and change unrelated prose or presentation without losing tracking. Comments in notes do not define schema.

Creation replaces the parsed title label and strips only recognized declaration comments, leaving all other bytes intact. A comment that fails declaration parsing remains ordinary Typst. A parsed declaration with unsupported or ambiguous semantics is a template error. The exact grammar and source forms live in the [CLI contract](contract/cli.md#initialization-and-source-declarations).

No abstract, keyword, or category field is built in. [The descriptive example](../examples/templates/descriptive.typ) is an optional convenience with its own helpers. Adding a field changes the template, not the public node structure. The rationale is recorded in [Template-declared metadata](decisions/template-schema.md).

Matching does not evaluate imports, bindings, aliases, qualified calls, nested declarations, conditionals, or generated values. Unrestricted Typst remains available inside markup values and ordinary prose, but tracked values are authored literals rather than evaluated results. Named-argument and dictionary capture remain deferred.

## Identity

The filename stem is the node ID. It uses local time in `YYMMDDHHmm` form.

If the current minute is occupied, allocation advances one minute at a time until it finds a free filename. Titles may change; IDs do not.

A canonical filename creates a node even when parsing or metadata validation fails. The heading label is a declaration that Rust validates against the filename, not the source of node existence.

## Metadata model

The public node has a stable core of filename ID, archive-relative path, recovered title, and a generic metadata map. Configured field names are data, not an engine enumeration. A name such as abstract, keywords, category, or status has no implicit semantic role. Internal source generations remain implementation state rather than persistent public note identity.

Values distinguish exact markup content, literal strings, and authored string lists. Markup retains source, deterministic projected text, and its inner UTF-8 byte range without evaluation. Omitted configured fields are null; malformed or repeated declarations are null with field-specific diagnostics; unconfigured fields are not emitted. Empty authored values remain distinct from omission.

The authoritative node, typed-value, absence, projection, range, diagnostic, and ordering definitions are in [Shared data schema 2](contract/data.md). Search examines all textual declared values using the shared deterministic matcher. It does not privilege metadata names or search arbitrary document bodies.

## Links and graph

Each literal ten-digit reference is an authored occurrence. The provider groups occurrences by ordered source/target pair, preserves every byte range, and derives incoming and outgoing adjacency. A target resolves when its node exists, including a malformed node; missing targets do not create synthetic nodes.

References remain archive links independently of whether their surrounding source is recognized as metadata. Strings, raw blocks, comments, imported code, and generated references do not contribute authored links. The public link shape and array ordering are defined once in the [shared data contract](contract/data.md#link).

## Provider session

The provider is a reusable Rust component. Each consumer creates its own session.

A CLI command creates a short-lived disk-backed session. `zk lsp` owns a long-lived session with open-buffer overlays. Version one has no shared daemon.

### Loading

At startup the provider:

1. locates and validates `zk.toml` and the schema template, creating a missing template;
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

Saved template changes re-extract all closed notes and open sources under the new metadata rules in one revision. All fallible reads complete before live state changes. Open source and versions survive, while all results prepared under old rules become stale. Invalid templates, invalid manifests, or failed reads preserve the previous rules and graph and visibly report errors. Unsaved manifest/template text is not a schema overlay. Starter-text changes that preserve compiled rules do not change the graph revision.

## Position conversion

The provider stores byte ranges only.

For open files, the LSP adapter converts ranges using the retained source. For closed files, it reads the saved text for each location conversion and scans that text to derive positions. It does not group reads or refresh graph state during conversion.

The model remains independent of LSP position encoding and stores no duplicate coordinates. Closed-file graph state depends on watched-file notifications; positions may be stale if saved text changes without a corresponding notification.

## Rust and Typst boundary

Rust is authoritative for archive semantics. It uses `typst-syntax` but does not compile or evaluate Typst.

Rust owns:

- root discovery, manifest loading, and template declaration compilation;
- filename identity;
- metadata extraction and validation;
- literal links and ranges;
- graph construction and revisions;
- disk and buffer overlays;
- archive diagnostics and queries;
- CLI and ZK language-server behavior.

`lib/zettel.typ` owns:

- core reference presentation; archive-specific metadata rendering belongs to user-owned Typst code;
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

The current executable accepts archive format 3 only. Release 0.3.0 intentionally replaces the undeployed format-2 source contract without adding a migration command. It never silently rewrites source, templates, or libraries. Future migrations for deployed archives must be explicit and reviewable.

`lib/zettel.typ` is user-owned. `zk init` supplies its initial version but never silently replaces it. Library changes currently require manual edits.

## Deferred design

- Orphan asset-namespace reporting and asset-specific live queries
- Controlled vocabulary and category derivation
- Structural node roles
- Aggregate or selected-subgraph Typst compilation
- Publishing and export
- Authoritative evaluated Typst metadata
- Graph visualization
- Shared live provider service
- Language-neutral streaming updates
