# LSP JSON contract

Status: implemented in zk 0.3.0 with ZK protocol 2 and data schema 2. See the [version matrix](README.md#status).

This document defines the editor-neutral companion server launched by `zk lsp`. It uses [LSP 3.17](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/) over JSON-RPC 2.0. Standard requests and notifications retain their LSP shapes. Only ZK archive-query results use the shared archive-data envelope.

## Transport, archive, and lifecycle

The transport is standard input/output with LSP `Content-Length` framing. stdout must contain protocol messages only, never CLI path output, progress prose, or logs. Startup failure is a process error on stderr, not an initialization response from a functioning session. Runtime operational errors may be reported through `window/logMessage` or a JSON-RPC error response. Failed saved-schema reloads also send `window/showMessage` with error severity.

One process owns one archive and one live provider session. `--archive PATH` or working-directory discovery selects the archive before initialization. `rootUri` and `workspaceFolders` do not switch it. There is no shared daemon, multi-archive session, CLI session discovery, or live cross-process change stream.

The client sends `initialize`, waits for its result, then sends `initialized` before ordinary operations. Standard `shutdown` and `exit` semantics apply: shutdown returns `null`; exit after shutdown has process status 0, while exit without shutdown has process status 1. Standard framing, cancellation, request IDs, and lifecycle errors follow LSP/JSON-RPC rather than a separate ZK protocol.

Document URIs must be local `file:` URIs. Archive document services apply only to canonical `zettel/ID.typ` paths beneath the selected root. Ordinary library and asset buffers do not create graph nodes or archive diagnostics. Completion, hover, definition, and references outside canonical notes return `null` rather than taking over general Typst requests. Invalid URIs or positions may produce invalid-parameter errors.

## Initialization and version discovery

The server reports its executable version in standard `serverInfo`. ZK clients must separately check the required integer versions in `capabilities.experimental.zk`:

```json
{
  "protocolVersion": 2,
  "dataSchemaVersion": 2,
  "features": {
    "archiveQueries": true,
    "categoryCompletion": false,
    "referenceCompletion": true,
    "referenceTitleDecorations": true
  }
}
```

This object is the value of `capabilities.experimental.zk`, not the whole initialization response. A client must reject unsupported ZK protocol or data schema versions before interpreting custom archive results. Missing required version properties do not imply version 2. The server does not negotiate or downgrade to protocol 1.

Clients must ignore unknown capability properties and feature flags. Missing feature flags mean false. Standard LSP capabilities remain authoritative for whether a standard method is supported; an optional ZK feature must not be inferred from the executable version.

| Feature | Meaning when true |
| --- | --- |
| `archiveQueries` | The three ZK execute commands below are supported and advertised |
| `referenceCompletion` | Archive ID/title completion is supported through standard completion |
| `referenceTitleDecorations` | Queries expose authored reference spans and core target titles sufficient for client-owned title decorations |
| `categoryCompletion` | Legacy optional flag; false in this target because dictionary-member completion belongs to Tinymist |

Reference-title decoration data requires archive queries. There is no separate decoration request, pushed decoration stream, or prescribed editor presentation.

The baseline server advertises full document synchronization with open/close events and save notifications, standard hover, definition, references, and workspace symbols. Reference completion advertises only `@` as a trigger. `executeCommandProvider.commands` contains `zk.queryNode`, `zk.links`, and `zk.backlinks` when archive queries are enabled.

The server selects UTF-8 positions when offered through `general.positionEncodings`; otherwise it uses UTF-16. It reports the selected encoding in `capabilities.positionEncoding`. Standard LSP positions are zero-based and use that encoding. Archive-query values and diagnostic byte-range data always remain UTF-8 byte offsets, regardless of LSP position encoding.

## Source synchronization and watched files

| Notification | Required source-state effect |
| --- | --- |
| `textDocument/didOpen` | Install full source text and document version as the authoritative overlay |
| `textDocument/didChange` | Accept a newer version and full replacement text; ignore stale or equal versions |
| `textDocument/didSave` | Keep the overlay authoritative; do not reload disk over it |
| `textDocument/didClose` | Drop the overlay and reload saved source, or remove the session node if the file no longer exists |

Only full replacements are supported; clients must not send ranged incremental changes. If multiple full replacements occur in one change notification, the last text is authoritative. A canonical unsaved document absent from disk creates a session-only node. Disk updates and delayed results must not replace open overlays. A failed source read or close is logged and must not discard the retained source state.

When the client advertises dynamic watched-file registration, the server registers create/change/delete events for `**/zettel/*.typ`, `**/zk.toml`, and `**/templates/zettel.typ` through `client/registerCapability`. Registration IDs are opaque. Clients without this capability must arrange equivalent `workspace/didChangeWatchedFiles` notifications themselves.

Only events for the selected archive root matter. Note events refresh closed notes and cannot overwrite overlays. Saved manifest and schema-template events reload the schema; `textDocument/didSave` for either file also reloads it. A successful rule change re-extracts all closed notes and retained open sources in one coherent graph revision, preserves document versions, and invalidates older prepared results. Changes to starter values, prose, or formatting that leave compiled rules unchanged do not change the graph revision.

A missing template is created with the minimal core, as defined by the [CLI contract](cli.md#template-availability-and-errors). Invalid templates, invalid manifests, and failed reads preserve the last valid rules and graph. Failed reloads send both an error log message and a visible error message; the server continues serving its retained view rather than pretending the reload succeeded. Invalid templates at startup fail the process before an LSP session begins.

Unsaved manifest and template text are not schema overlays. Ordinary template-buffer language intelligence belongs to Tinymist; only saved files change tracking. Library dictionaries do not determine metadata or completion. Clients cannot assume a saved-file edit is visible before a corresponding notification has been processed.

## Archive-query commands

These requests use standard `workspace/executeCommand`. Each command takes exactly one string argument identifying an existing node:

```json
{
  "jsonrpc": "2.0",
  "id": 7,
  "method": "workspace/executeCommand",
  "params": {
    "command": "zk.queryNode",
    "arguments": ["2603231410"]
  }
}
```

| Command | Successful envelope `data` |
| --- | --- |
| `zk.queryNode` | Node |
| `zk.links` | Outgoing link array |
| `zk.backlinks` | Incoming link array |

The JSON-RPC `result` contains the [schema-2 envelope](data.md#envelope), not an unwrapped node or array. Shapes, null semantics, ordering, and byte ranges are identical to the corresponding CLI query results. Source visibility differs: these commands use the live session, including unsaved and session-only nodes. Empty relations return `data: []`. Missing queried nodes are errors, not successful null values.

No required request argument selects a schema version; the server advertises the one it implements. There are no mutation, graph-streaming, whole-archive snapshot, or custom search commands in this protocol. Live search is available through `workspace/symbol`; saved full snapshots are available through the CLI.

| Failure | JSON-RPC error code |
| --- | --- |
| Malformed parameters, wrong argument count/type, missing node, or invalid document position | `-32602`, invalid params |
| Unsupported method or ZK execute command | `-32601`, method not found |
| Operational read or serialization failure during a request | `-32603`, internal error |

An unknown command is rejected independently of node existence. Error messages are human-facing and not stable identifiers. Error responses do not contain a successful archive-data envelope. Notification failures use log messages because JSON-RPC notifications have no response.

## Standard archive services

### Completion

`textDocument/completion` returns `null` outside an archive-reference query context. It ignores raw text, strings, comments, and escaped reference text. Within a query after `@`, it returns a standard `CompletionList` with `isIncomplete: true` and at most 100 items.

An all-ASCII-digit query matches an ID prefix. Other nonempty queries match projected title text by case-insensitive substring containment. Empty queries show the largest IDs first. Title-prefix matches rank before other title matches, with descending ID order within each tier. Numeric queries also use descending ID order. Matching uses the shared lowercase behavior without evaluating titles.

Each item inserts the selected ten-digit ID through `textEdit`, replacing the temporary query but leaving `@` intact. Items use reference kind and a title/ID label; clients must not parse the label to recover identity. Missing titles use a human-facing fallback. No metadata field name is privileged for completion documentation; schema-2 completion requires only the core title and ID. There is no `completionItem/resolve` contract.

### Hover and navigation

`textDocument/hover` over a literal ten-digit archive reference returns a standard Markdown hover containing target title and ID, followed by non-null declared metadata in deterministic field-name order. Markup uses its text projection; strings and lists are displayed as values without evaluation or vocabulary inference. Formatting is presentation, not a machine-readable data interface; clients needing structured metadata must use `zk.queryNode`. Arbitrary source text must not be evaluated as Typst during presentation.

A missing target returns a missing-target hover. Hover's range covers the authored reference token. No archive reference at the requested position returns `null`.

`textDocument/definition` returns one `Location` at the target's title-content range, or a zero-length document-start range when no title is recoverable. A missing target or a position without an archive reference returns `null`. Unsaved session-only target nodes are navigable.

`textDocument/references` returns incoming authored occurrences as standard `Location` values, grouped in source-ID order and then source-range order. When `includeDeclaration` is true and the target node exists, its title location is prepended, with the same document-start fallback. A dangling target can still have incoming reference locations. A position without an archive reference returns `null`.

The server uses open source for open-document positions. Closed-document positions depend on its known saved state and client file notifications. It does not promise a filesystem transaction across independently edited closed files.

### Workspace symbols

`workspace/symbol` applies the [shared metadata search rule](data.md#metadata-search) to the live view. It returns standard `SymbolInformation` entries in ID order without an implicit result cap. Entries use object kind, a title/ID name, and the core title location with the document-start fallback. No configurable metadata field is automatically treated as a container or grouping role. Names are presentation; consumers must not parse them as structured archive metadata.

## Diagnostic notifications

The server publishes standard `textDocument/publishDiagnostics` for open canonical notes after accepted source updates, relevant disk changes, or schema reloads. Target creation/removal may change diagnostics in other open notes. Closed-file archive-wide inspection remains a CLI operation.

Each publication replaces the previous diagnostics for its URI and carries the currently accepted open-document version. Closing a document clears its published diagnostics without a version. The same document version can receive different diagnostics after another note or the schema changes; clients must not assume equal versions imply identical diagnostics.

The standard diagnostic properties are:

- `source`: `zk`;
- `code`: a stable string from the [shared diagnostic contract](data.md#diagnostic), without a custom field name embedded in it;
- `severity`: LSP error 1 or warning 2;
- `message`: human-facing text;
- `range`: the authored range converted to negotiated LSP positions, or a zero-length document-start range when no source range exists;
- `data`: the following ZK source-information object.

```json
{
  "schema_version": 2,
  "path": "zettel/2603231410.typ",
  "field": "tags",
  "byte_range": {"start": 41, "end": 58}
}
```

All four properties of this `data` object are required. `field` and `byte_range` may be `null` with the same meanings as shared diagnostics. The data object is not a query-result envelope and contains no nested `data` property. Its byte range stays UTF-8 even when standard diagnostic positions use UTF-16. Consumers must ignore additional properties and reject or leave uninterpreted an unsupported data schema version.

Generic `syntax.error` and `syntax.warning` diagnostics are not published by `zk lsp`. Tinymist owns general Typst syntax, bindings, imports, types, dictionary-member completion, rendering, and compilation. `zk` still reports missing titles, identity disagreement, malformed or duplicate configured declarations, and dangling archive references. Parsing full source internally does not grant the archive server responsibility for general Typst language intelligence.
