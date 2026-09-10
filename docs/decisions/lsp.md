# Language server and editor integration

## Architecture

ZK intelligence runs as a companion language server, `zk lsp`, separate from Tinymist.

```text
Neovim
  |-- Tinymist      Typst language intelligence
  |-- zk lsp        ZK semantics
```

There is no Tinymist fork and no protocol proxy.

## Capability split

Tinymist handles:

- Typst syntax;
- formatting;
- ordinary Typst completion outside ZK-owned contexts;
- compilation diagnostics;
- preview.

`zk lsp` handles:

- metadata contracts;
- `@ID` semantics;
- archive diagnostics;
- searchable completion, hover, definition, and references for Zettel IDs;
- category-key completion for the direct metadata form;
- backlinks;
- archive queries.

The Neovim plugin routes context-sensitive actions. On `@ID`, definition goes to the ZK server. Elsewhere it uses normal LSP definition.

Live archive search uses `workspace/symbol`. Targeted adapter queries use the `zk.queryNode`, `zk.links`, and `zk.backlinks` execute commands with one Zettel ID argument.

## Reference validity

Tinymist 0.15.2 cannot selectively suppress unresolved numeric-label diagnostics. `zk lsp` and `zk check` therefore own dangling-reference diagnostics.

`lib/zettel.typ` intercepts ten-digit `ref` elements so Tinymist does not report them as missing Typst labels. ZK reference validity is decided by the current in-memory archive model.

## Tinymist boundary

Tinymist is an optional integration, not part of the archive format.

- Zettel source syntax does not depend on Tinymist.
- The CLI and `zk lsp` function without it.
- No Tinymist configuration or lock file becomes canonical archive state.
- Native Tinymist completion was considered via generated synthetic labels, then dropped in favor of a cache-free in-memory model.

The Neovim plugin contributes an `after/lsp/tinymist.lua` override that places `zk.toml` before `.git` in Tinymist's root markers. Tinymist and `zk lsp` therefore discover the same archive root without requiring the archive to be a Git repository.

## Neovim plugin

The plugin is a thin editor adapter over `zk lsp`.

`require("zk").setup({ archive = PATH })` configures a personal fallback archive. Local `zk.toml` discovery from the current buffer takes precedence. A valid fallback starts one eager client so archive-level commands work from unrelated buffers. `:ZkSetArchive [PATH]` reports or replaces that fallback for the current Neovim process without changing persistent configuration.

Switching the fallback stops its previous client but leaves clients for open local archives running. Invalid paths leave the existing fallback untouched. See [Global archive access](global-archive-access.md).

The language server owns a live provider session. It feeds saved-file changes and versioned open-buffer overlays into that session, then uses the resulting graph for parsing, extraction, validation, completion, navigation, backlinks, queries, and source ranges.

### Document opening

Every ZK-controlled document open uses the window where the action began.
This includes `:ZkNew`, a `:ZkFind` selection, and ZK definition navigation.
When that window's buffer has unsaved changes, Neovim hides the buffer without
discarding its changes before displaying the target. These actions do not open
a split or force a write.

Asynchronous command, picker, and language-server responses retain the
invocation window. If that window closes before the response arrives, the
adapter uses the current valid window.

### Reference completion

Text after `@` acts as a temporary completion query. Numeric text retains ID
prefix matching. Other text searches titles only, case-insensitively.
Accepting an item replaces the complete query after `@` with the target's
ten-digit ID, while the menu displays the title and ID.

The server returns at most 100 candidates and marks the list incomplete so the
client requests updated results as the query changes. An empty query returns
the 100 newest Zettel. ID matches rank before title matches. Title prefixes
rank before title substrings, with newer IDs breaking ties.

This matching and ranking policy is provisional. The implementation must keep
it easy to inspect and adjust after use with a large archive.

### Category completion

In the direct top-level `#category.<query>` form, `zk lsp` completes only
category keys. It reads those keys from the saved direct top-level
`#let category = (...)` dictionary in `lib/zettel.typ` without evaluating
Typst. A computed or malformed category definition yields no ZK category
candidates.

LSP has no way for one server to suppress another server's completion response.
The Neovim plugin therefore exports a completion-item filter for frontends to
compose into their configuration. In category context the filter retains only
items from `zk lsp`; elsewhere it leaves all completion items unchanged.

The supported Blink integration composes this helper into
`sources.transform_items`. The plugin neither requires Blink nor mutates its
configuration. Other completion frontends may apply the same rule.

Reading category keys for completion does not make the library authoritative
for stored metadata and does not settle category validation or controlled
vocabulary policy.

### Overlay lifecycle

`zk lsp` uses full-text synchronization. `didOpen` and each newer `didChange` install the complete buffer text as the source overlay; `typst-syntax` determines and incrementally reparses the changed region. Stale or repeated document versions are discarded.

An open overlay takes precedence over every disk event until `didClose`, including after `didSave`. Disk events update only closed files. Closing a buffer drops its overlay and reloads the current disk file, or removes the node if the file no longer exists. An unsaved canonical `zettel/ID.typ` buffer creates a session-only node until it is saved or closed.

Each source state has a generation number. Parse results apply only while that generation remains current. This prevents startup disk parsing, older buffer changes, and post-close work from overwriting newer state. Every accepted source replacement produces one coherent graph revision.

The provider stores half-open UTF-8 byte ranges only. For an open buffer, the LSP adapter converts ranges with the retained `typst-syntax` source. For closed files, it groups requested ranges by path, reads each small file once, and builds a temporary line index. If disk content changed after the graph generation, the provider refreshes that node before returning positions. The adapter then converts to the position encoding negotiated with Neovim.

The plugin owns:

- concealing `@ID` and displaying the target title with extmarks;
- routing `gd` and other context-sensitive actions;
- filtering completion items in ZK-owned contexts when the configured
  completion frontend uses its helper;
- presenting backlinks, searches, and diagnostics;
- invoking explicit archive commands;
- applying LSP workspace edits.

The plugin recognizes narrow source contexts for routing but does not extract
metadata, read an index, maintain a graph, or duplicate archive data. Its
default presentation uses `vim.ui.select` for search, quickfix for backlinks
and diagnostics, and inline extmarks for target titles. These choices remain
replaceable by user configuration or picker integrations.

## Version-one editor scope

The Neovim integration supports the daily Zettel loop:

- create a Zettel from the standard template;
- find and open a Zettel by ID, title, or metadata;
- complete `@ID` references;
- conceal `@ID` as the target title;
- follow a reference under the cursor;
- show incoming references with source locations;
- expose metadata and dangling-link diagnostics;
- refresh decorations when buffers change.

Excluded in version one:

- graph visualization;
- compilation and publishing;
- transclusion;
- structure-note tooling;
- interactive metadata forms;
- agent-assisted organization;
- custom query languages.

Search presentation may use quickfix, `vim.ui.select`, or an installed picker. That is an implementation choice, not archive semantics.
