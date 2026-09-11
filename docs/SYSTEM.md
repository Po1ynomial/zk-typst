# System

## Implemented capabilities

The repository builds one Rust executable named `zk`.

### Archive authoring

`zk init [PATH]` creates the version-one archive layout without overwriting an existing manifest or canonical path:

```text
zk.toml
zettel/
lib/
  zettel.typ
```

The default path is the current directory. The generated manifest declares `format = 1`. The bundled Typst library renders the fixed metadata forms and intercepts ten-digit references.

`zk init --agent-skills [PATH]` also installs the complete bundled skill set
under `.agents/skills/`. The current set contains
`.agents/skills/zettelkasten/SKILL.md`, which documents the writing method,
source contract, query workflow, whole-file lifecycle commands, and archive
checks. Ordinary initialization does not create `.agents/`.

Skill installation is best-effort. An existing same-name skill remains
untouched and produces a warning. Other filesystem failures also warn without
failing canonical archive creation. Once installed, skills are user-owned;
later `zk` commands do not validate or update `.agents/`.

Commands that require an existing archive accept the global `--archive PATH` option. An explicit path resolves relative to the process working directory, must itself be a valid archive root, and takes precedence over current-directory discovery. Without the option, commands retain upward `zk.toml` discovery. `zk init [PATH]` rejects `--archive`.

`zk new` selects and validates an archive, then creates a Zettel under `zettel/`. The filename uses the current local minute in `YYMMDDHHmm.typ` form. If that ID exists, allocation advances by one minute until a free name is available. The command prints the new path relative to the archive root.

The generated Zettel has the required import, show rule, labelled level-one heading, abstract, keyword list, and category before an empty body.

### Disk-backed provider

`zk graph --format json` loads saved canonical `zettel/ID.typ` files and writes a complete JSON snapshot to standard output. Files with noncanonical names do not enter the provider graph and produce archive diagnostics.

The provider parses files concurrently with `typst-syntax` 0.15.1. It extracts the restricted direct metadata forms without evaluating Typst. The required library import accepts its four names in any order so `typstyle` output remains valid. Missing, repeated, renamed, or additional import names are rejected. Title and abstract values contain their exact inner source, deterministic text projection, and half-open UTF-8 byte range. Malformed fields remain `null` and produce snapshot diagnostics.

Literal ten-digit Typst references become directed links. References in raw text, strings, and comments do not appear as Typst reference nodes and do not become links. The provider groups repeated occurrences by source-target pair, preserves every authored byte range, and records whether each target resolves to a canonical node.

The retained graph interns IDs as `u32` indexes and keeps incoming and outgoing adjacency lists. Parsed source and syntax trees are discarded after extraction. A JSON snapshot expands the internal IDs back to strings.

Provider schema 1 has this top-level shape:

```json
{
  "schema_version": 1,
  "revision": 1,
  "nodes": [],
  "links": [],
  "diagnostics": []
}
```

Nodes are ordered by ID. Links are ordered by source and target ID. Diagnostics are ordered by path and source range. A fresh disk-backed provider starts at revision 1, so this revision is not a durable archive identifier.

### Integrity and shell operations

`zk check` prints diagnostics and exits with status 1 when any error exists. Warnings do not fail the command. The default text output includes the path, byte range when available, stable diagnostic code, and message. `zk check --format json` emits the diagnostic array used in graph snapshots.

Checks cover:

- noncanonical files or entries under `zettel/`;
- Typst syntax errors;
- missing, malformed, repeated, or misplaced metadata;
- filename and heading-label mismatches;
- dangling reference occurrences;

The query commands write JSON to standard output:

```text
zk query node <ID>
zk query links <ID>
zk query backlinks <ID>
zk query search <QUERY>
```

A node query returns metadata for one Zettel. Link and backlink queries return grouped links with every authored byte range. Missing node IDs fail the command.

Metadata search compares the query case-insensitively with IDs, projected
titles, projected abstracts, keywords, and categories. It returns every
matching node as JSON in provider ID order without an implicit cap. The
language server's workspace-symbol search uses the same matcher.

`zk remove <ID>` deletes a canonical Zettel only when it has no incoming references. A blocked removal exits with status 1, leaves the file untouched, and prints every incoming source path and byte range. A successful removal prints the deleted archive-relative path.

### Live provider sessions

The same `Provider` type supports long-lived editor sessions. `open_buffer` installs full text and a document version. `change_buffer` accepts only newer versions and calls `typst_syntax::Source::replace` for incremental reparsing. Open sources remain in memory; closed-file source and syntax trees do not.

An overlay replaces the corresponding disk node and outgoing links in one graph revision. Incoming adjacency, target resolution, and dangling diagnostics update before consumers see that revision. Opening a canonical path that does not exist on disk creates a session node.

`save_buffer` retains the overlay without reading disk. `refresh_disk` ignores open paths. `close_buffer` drops the overlay, then reloads disk or removes the session node when no disk file exists.

Every scheduled source state receives a generation. `prepare_disk_update` returns parsed work tagged with that generation, and `apply_prepared` rejects it if a newer disk or buffer state has already been scheduled. Accepted replacements increment the graph revision once. Stale document versions, stale generations, saves, and ignored disk events do not increment it.

### Language server

`zk lsp` runs a companion language server over standard input and output. It loads either the explicit `--archive PATH` root or the archive found above its working directory, then owns one live provider session.

The server advertises full-text document synchronization. Open, change, save, and close notifications map directly to the provider overlay lifecycle. When the client supports dynamic registration, the server registers `**/zettel/*.typ` for create, change, and delete events. Watched-file notifications refresh closed canonical Zettel and cannot replace open overlays.

The server prefers UTF-8 positions when the client offers them and otherwise uses UTF-16. Its adapter converts the provider's byte ranges using retained open-buffer text or one read of the closed file.

Implemented requests:

- `textDocument/completion` treats numeric text after `@` as an ID prefix and
  other text as a case-insensitive title query. It returns at most 100
  server-filtered items, marks the list incomplete for continued queries, and
  omits raw text, strings, comments, and escapes. Empty queries show the newest
  Zettel.
- In direct top-level `#category.` context, completion returns keys from the
  saved direct category dictionary in `lib/zettel.typ`. Computed or malformed
  category definitions return no candidates.
- `textDocument/hover` returns target metadata or a missing-target message.
- `textDocument/definition` opens the target title, including unsaved session nodes.
- `textDocument/references` returns incoming authored occurrences and optionally the target declaration.
- `workspace/symbol` searches IDs, titles, abstracts, keywords, and categories across live state.
- `workspace/executeCommand` supports `zk.queryNode`, `zk.links`, and `zk.backlinks`, each with one ID argument.

The server pushes diagnostics for open Zettel after every accepted source update and after watched-file changes. Resolving or creating a target republishes affected open-buffer diagnostics. Stale document versions do not alter provider state.

### Neovim adapter

The Lua plugin supports Neovim 0.12 and has no external Lua dependencies. Configure a personal fallback archive with:

```lua
require("zk").setup({
  archive = "~/zettelkasten",
})
```

The option expands `~` and environment variables, resolves relative paths against Neovim's current working directory, canonicalizes the result, and checks the canonical archive layout. The selected fallback is readable as `require("zk").archive`.

A valid fallback eagerly starts one unattached `{ "zk", "lsp" }` client. Opening one of its Zettel attaches the buffer to that existing client. The nearest `zk.toml` above the current buffer takes precedence; another local archive starts or reuses its own client when a command or Zettel needs it. Without the option, local discovery behaves as before.

`:ZkSetArchive [PATH]` reports or replaces the session fallback. A replacement becomes visible only after its server initializes. Invalid paths and failed server startup leave the previous fallback unchanged. The plugin stops an unused previous fallback client but preserves it when open Zettel still use it.

Server processes start with their archive as `cmd_cwd`. The plugin's `after/lsp/tinymist.lua` configuration places `zk.toml` before `.git` in Tinymist's root markers, so both servers discover local archive roots independently. Override `lsp_cmd` and `cli_cmd` when `zk` is not on `$PATH`. Tinymist may remain attached to the same buffer.

The plugin defines these commands:

```text
:ZkFind [query]
:ZkBacklinks
:ZkDiagnostics
:ZkCheck
:ZkNew
:ZkRemove [ID]
:ZkRefresh
:ZkSetArchive [PATH]
```

Search uses live `workspace/symbol` results and `vim.ui.select`. Backlinks and
ZK diagnostics populate quickfix. Creation, checking, and guarded removal
invoke the scriptable CLI.

`:ZkNew`, `:ZkFind` selections, and ZK definition navigation display their
target in the invocation window. They hide a modified prior buffer without
discarding its changes. If an asynchronous response arrives after that window
closes, the adapter uses the current valid window. The plugin refuses to remove
a Zettel whose loaded buffer has unsaved changes.

`ZkFind`, `ZkNew`, `ZkCheck`, `ZkDiagnostics`, and `ZkRemove ID` use the locally discovered archive or configured fallback from any buffer. Backlinks, refresh, implicit removal, and cursor language features remain specific to the current Zettel.

The default buffer mappings are `gd`, `<leader>zf`, `<leader>zb`, `<leader>zd`, and `<leader>zn`. On a ten-digit reference, `gd` requests a definition only from `zk lsp`; elsewhere it uses Neovim's ordinary LSP definition path. Mappings can be replaced or disabled in `setup`.

Resolved reference ranges receive extmarks that conceal the raw `@ID` and
insert the target title as inline virtual text. Missing targets use
`ZkMissingReference`. Refresh requests are debounced and carry a buffer change
tick, so stale responses cannot decorate newer text.

The Lua adapter exports `filter_completion_items(context, items)`. It preserves
ordinary completion items except in direct `#category.` context, where it keeps
only items from `zk lsp`. Blink users compose this helper into
`sources.transform_items`; the plugin does not depend on or configure Blink.
The Lua code recognizes narrow source contexts and converts provider byte
offsets, but it does not extract metadata or retain archive relations.

## Code entry points

- `src/main.rs` defines the command-line interface, JSON output, and process exit behavior.
- `src/archive.rs` implements initialization, root discovery, manifest validation, layout validation, timestamp-ID validation, collision-safe creation, and file removal.
- `src/extract.rs` extracts metadata, literal references, source ranges, and syntax diagnostics from one parsed Zettel.
- `src/model.rs` defines the public node, link, diagnostic, and snapshot data shapes.
- `src/provider.rs` loads files concurrently, owns mutable graph state, retains open overlays, rejects stale updates, derives integrity diagnostics, and produces snapshots.
- `src/lsp.rs` implements protocol capabilities, synchronization, position conversion, diagnostics, navigation, search, and archive commands.
- `src/templates.rs` contains the canonical templates and bundled skill registry.
- `skills/zettelkasten/SKILL.md` is the inspectable source for the bundled Zettelkasten operating skill.
- `after/lsp/tinymist.lua` makes `zk.toml` a higher-priority Tinymist workspace marker.
- `lua/zk/init.lua` configures the Neovim client, commands, mappings, pickers, quickfix presentation, CLI jobs, and extmarks.
- `doc/zk.txt` documents plugin setup and commands for `:help zk`.
- `tests/cli.rs` exercises authoring, graph output, checking, queries, and guarded removal through the executable.
- `examples/inspect_overlays.rs` drives the in-process live-provider lifecycle used by the overlay inspection script.
- `examples/lsp_probe.rs` is a framed JSON-RPC client used to inspect the server with both supported position encodings.
- `scripts/inspect_nvim.lua` drives the plugin inside headless Neovim.
- `scripts/inspect_nvim_fallback.lua` checks configured and locally discovered archive selection.

## Inspection

Run the automated checks:

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
rumdl check docs skills
```

Inspect optional skill installation and metadata search:

```sh
archive="$(mktemp -d)"
cargo run -- init --agent-skills "$archive"
cargo run -- --archive "$archive" query search "untitled"
cat "$archive/.agents/skills/zettelkasten/SKILL.md"
```

Inspect the provider with a generated archive:

```sh
scripts/inspect-graph.sh
```

The script builds `zk`, creates five canonical Zettel plus one diagnosed noncanonical file, and checks metadata extraction, malformed metadata, syntax recovery, resolved and missing links, grouped occurrences, ignored raw and commented references, and exact byte ranges. It prints the complete JSON snapshot after the assertions pass.

Preserve the generated archive and snapshot for manual inspection:

```sh
scripts/inspect-graph.sh --keep
```

The script prints the retained archive path to standard error. `KEEP_TMP=1 scripts/inspect-graph.sh` provides the same behavior.

Inspect checking, queries, and removal with another generated archive:

```sh
scripts/inspect-integrity.sh
```

This script demonstrates failing and repaired checks, node and relation queries, blocked removal with incoming locations, and successful removal. Pass `--keep` or set `KEEP_TMP=1` to retain its archive.

Inspect a live provider session:

```sh
scripts/inspect-overlays.sh
```

The script creates a disk archive and runs the Rust inspection example through overlay installation, incremental full-text changes, stale version and generation rejection, unsaved-node creation, save precedence, close reload, disk deletion, and disk restoration. It prints the final revision-seven snapshot. Pass `--keep` or set `KEEP_TMP=1` to retain the resulting archive.

Inspect the language server over its real stdio transport:

```sh
scripts/inspect-lsp.sh
```

The script launches two server sessions through the framed JSON-RPC probe from
outside the archive using explicit `--archive` selection. One negotiates UTF-8
positions and one negotiates UTF-16. Each session checks full-text
synchronization, push diagnostics, ID and title reference completion,
category completion from a modified library, hover, definitions, references,
backlinks, archive search and queries, unsaved targets, stale-version
rejection, watched-file refresh, close reload, and clean shutdown. Pass
`--keep` or set `KEEP_TMP=1` to retain the protocol reports and final archive.

Inspect the Neovim adapter with the real language server and Tinymist:

```sh
scripts/inspect-nvim.sh
```

The script first creates an archive without a Git repository and runs headless
Neovim 0.12 with both servers attached. Tinymist starts through ordinary LSP
configuration and must discover `zk.toml` as its root. This pass also checks
command registration, title extmarks and conceal ranges, diagnostics and
quickfix, context definition, search selection, backlinks, blocked and
successful removal, current-window creation, search and definition navigation
with unsaved prior buffers, completion filtering, and `zk check`.

A second headless pass starts in an unrelated buffer with a configured personal archive. It checks eager unattached startup, readable fallback state, global search and CLI commands, diagnostics, attachment to the eager client, on-demand local archive precedence, invalid switch preservation, successful switching, and preservation of clients serving open local buffers. Pass `--keep` or set `KEEP_TMP=1` to retain the archives and reports.

## Current limitations

The server loads its initial graph synchronously before accepting protocol messages. Clients without dynamic watched-file registration must arrange those notifications themselves. Diagnostics are pushed for open Zettel; archive-wide closed-file inspection remains available through `zk check`.

The built-in search presentation uses `vim.ui.select` without preview. Backlinks and diagnostics use quickfix. The plugin requests target metadata once per distinct outgoing target when it refreshes title decorations. It does not maintain a title cache across buffers.

Queries currently emit JSON only. CLI locations use UTF-8 byte ranges rather than line and column coordinates. Removal does not edit incoming references and never rewrites Zettel bodies.

The initial category dictionary contains `thoughts`, `physics`, and `coding`.
Category completion reads only the saved library, so unsaved library edits do
not affect candidates. Category and keyword policy remain deferred product
decisions. Archives may edit their user-owned `lib/zettel.typ`, but `zk` does
not migrate it yet.

Metadata search has no implicit result cap, so broad or empty CLI searches can
produce large JSON arrays. Installed agent skills are snapshots from archive
creation and do not receive automatic updates.
