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
- ordinary Typst completion;
- compilation diagnostics;
- preview.

`zk lsp` handles:

- metadata contracts;
- `@ID` semantics;
- archive diagnostics;
- completion, hover, definition, and references for Zettel IDs;
- backlinks;
- archive queries.

The Neovim plugin routes context-sensitive actions. On `@ID`, definition goes to the ZK server. Elsewhere it uses normal LSP definition.

## Reference validity

Tinymist 0.15.2 cannot selectively suppress unresolved numeric-label diagnostics. `zk lsp` and `zk check` therefore own dangling-reference diagnostics.

`lib/zettel.typ` intercepts ten-digit `ref` elements so Tinymist does not report them as missing Typst labels. ZK reference validity is decided by the current in-memory archive model.

## Tinymist boundary

Tinymist is an optional integration, not part of the archive format.

- Zettel source syntax does not depend on Tinymist.
- The CLI and `zk lsp` function without it.
- No Tinymist configuration or lock file becomes canonical archive state.
- Native Tinymist completion was considered via generated synthetic labels, then dropped in favor of a cache-free in-memory model.

## Neovim plugin

The plugin is a thin editor adapter over `zk lsp`.

The language server owns a live provider session. It feeds saved-file changes and versioned open-buffer overlays into that session, then uses the resulting graph for parsing, extraction, validation, completion, navigation, backlinks, queries, and source ranges.

### Overlay lifecycle

`zk lsp` uses full-text synchronization. `didOpen` and each newer `didChange` install the complete buffer text as the source overlay; `typst-syntax` determines and incrementally reparses the changed region. Stale or repeated document versions are discarded.

An open overlay takes precedence over every disk event until `didClose`, including after `didSave`. Disk events update only closed files. Closing a buffer drops its overlay and reloads the current disk file, or removes the node if the file no longer exists. An unsaved canonical `zettel/ID.typ` buffer creates a session-only node until it is saved or closed.

Each source state has a generation number. Parse results apply only while that generation remains current. This prevents startup disk parsing, older buffer changes, and post-close work from overwriting newer state. Every accepted source replacement produces one coherent graph revision.

The provider stores half-open UTF-8 byte ranges only. For an open buffer, the LSP adapter converts ranges with the retained `typst-syntax` source. For closed files, it groups requested ranges by path, reads each small file once, and builds a temporary line index. If disk content changed after the graph generation, the provider refreshes that node before returning positions. The adapter then converts to the position encoding negotiated with Neovim.

The plugin owns:

- concealing `@ID` and displaying the target title with extmarks;
- routing `gd` and other context-sensitive actions;
- presenting backlinks, searches, and diagnostics;
- invoking explicit archive commands;
- applying LSP workspace edits.

The plugin does not parse Zettel, read an index, maintain a graph, or duplicate archive rules.

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
