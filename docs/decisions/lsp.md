# Language server

Status: accepted

## Decision

ZK intelligence runs as an editor-neutral companion language server over standard input and output:

```text
zk lsp
```

The server owns one live provider session. It receives full-text document events, applies versioned open-buffer overlays, watches saved Zettel when the client supports dynamic registration, and exposes archive behavior through standard LSP requests plus three named commands.

## Capabilities

`zk lsp` handles:

- metadata and dangling-reference diagnostics;
- reference and category completion;
- hover, definition, and references for ten-digit Zettel IDs;
- live metadata search through `workspace/symbol`;
- `zk.queryNode`, `zk.links`, and `zk.backlinks` through `workspace/executeCommand`.

Reference completion treats numeric text after `@` as an ID prefix and other text as a case-insensitive title query. It returns at most 100 items, marks the list incomplete, and replaces the temporary query with the selected ID. Empty queries return the newest Zettel first.

Direct top-level `#category.` completion reads keys from the saved direct category dictionary in `lib/zettel.typ`. It does not evaluate computed Typst.

## Protocol contract

Initialization advertises an integer ZK protocol version under `capabilities.experimental.zk`, together with feature flags. Protocol version 1 reports:

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

The ZK protocol version is independent of the executable semantic version, archive format, and provider snapshot schema. Clients reject unsupported versions and use feature flags for optional behavior.

The CLI commands used by editor clients are also a documented stable interface. A protocol addition lands compatibly before a client release requires it.

## Synchronization

The server uses full-text synchronization. `didOpen` and each newer `didChange` install the complete source, while `typst_syntax::Source::replace` incrementally reparses the changed region. Stale document versions are ignored.

`didSave` leaves the overlay authoritative. `didClose` drops it and reloads disk or removes an unsaved node. Watched-file updates affect closed canonical paths only.

Provider generations prevent delayed disk work from replacing newer state. Each accepted source replacement updates the node, outgoing links, incoming adjacency, target resolution, diagnostics, and graph revision coherently.

## Positions

The provider stores half-open UTF-8 byte ranges. The language server negotiates UTF-8 when offered and otherwise uses UTF-16.

Open files use retained source text for conversion. Closed-file requests group ranges by path and read each file once. If disk contents no longer match provider state, the node refreshes before positions are returned.

## Diagnostics

The server pushes diagnostics for open Zettel after accepted document changes and watched-file updates. Creating or resolving a target republishes affected open-buffer diagnostics. Archive-wide saved-state diagnostics remain available through `zk check`.

## Typst boundary

`zk lsp` does not compile or evaluate Typst. The archive library intercepts ten-digit references so Typst tooling can render them without treating them as ordinary unresolved labels. `zk` alone determines archive reference validity.

Other Typst language servers may receive the same document events, but they do not exchange state with `zk lsp`.

## Consequences

- Editor clients remain replaceable and independently released.
- Archive semantics stay shared between CLI and LSP through the Rust provider.
- Clients must select an archive root and arrange watched-file notifications when dynamic registration is unavailable.
- Separate client repositories need compatibility tests against supported engine releases.
- The server does not prescribe picker, quickfix, decoration, or completion-frontend presentation.
