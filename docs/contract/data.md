# Shared data schema 2

This schema defines archive JSON values shared by the [CLI](cli.md) and [LSP archive-query commands](lsp.md#archive-query-commands). Standard LSP messages retain their standard types and are not wrapped in archive envelopes.

## Envelope

Every complete CLI archive JSON result and successful LSP archive-query result has this shape:

```json
{
  "schema_version": 2,
  "data": {}
}
```

`schema_version` is the integer 2. `data` is the command-specific value: a node, a list of nodes or links, a list of diagnostics or assets, or a graph snapshot. It is not always an object. No other envelope property is required. Consumers must reject an unsupported schema version before interpreting `data`.

An empty list result is `data: []`, not `null`. A missing queried node is an operation error, not a successful `data: null` response. Error delivery belongs to the CLI or JSON-RPC transport contract. Each response reflects one coherent provider view; separate invocations or requests are not a transaction with one another and need not observe the same source state.

All strings are Unicode. IDs and paths are strings, never JSON numbers. Offsets and revisions are nonnegative integers within the interoperable JSON integer range, at most `9007199254740991`. JSON object-property order is not significant. Ordering requirements below apply to arrays.

## Paths and byte ranges

An archive path is relative to the selected archive root, uses `/` separators, and contains no root prefix or `..` traversal. A canonical node path is `zettel/ID.typ`.

```json
{"start": 31, "end": 39}
```

A byte range is half-open: `start` is inclusive, `end` is exclusive, and `start <= end`. Both offsets address UTF-8 bytes in the source state from which the value was extracted. They are not characters, UTF-16 units, line/column positions, or offsets in rendered text. An authored empty fragment has `start == end`.

CLI values address saved source observed by that invocation. LSP archive-query values address the server's current known source, with open buffers taking precedence over disk. Ordinary LSP locations use the negotiated position encoding instead. Saved-file changes without notifications can make the server's known state stale; clients must arrange notifications as described in the LSP contract.

## Markup value

```json
{
  "source": "Summary.",
  "text": "Summary.",
  "range": {"start": 31, "end": 39}
}
```

All three properties are required. `source` is the exact inner Typst fragment, without the enclosing heading syntax or content-block brackets. `range` addresses precisely that fragment. `text` is the deterministic, trimmed search/display projection, not evaluated or rendered Typst.

The projection walks parsed markup in source order using these rules:

- Markup containers concatenate their projected children. Emphasis and strong elements recursively project only their inner markup, without their delimiters.
- Space, paragraph-break, and explicit line-break elements append one ASCII space only when output is nonempty and does not already end in an ASCII space.
- Raw elements append their parsed line contents without delimiters or language tags. Between raw lines, they apply the same conditional ASCII-space rule. Whitespace inside a raw line is preserved.
- Line and block comments encountered by this walk append nothing, including no replacement space.
- Every other element appends its exact source without recursively projecting its children. This includes literal references, equations, code expressions, headings, and lists. Comments and whitespace nested inside these retained elements therefore remain in `text`.

The final output is trimmed of leading and trailing Unicode whitespace. Whitespace inside retained source is not globally collapsed. Headings, lists, and other structures inside a content block are permitted, but the projection is not a plain-text equivalent of arbitrary evaluated Typst.

These projection examples pair exact inner `source` with `text`, using JSON escapes for newlines. They omit ranges and are not complete markup values:

```json
[
  {"source": "A  _small_\n\n*thought*", "text": "A small thought"},
  {"source": "A/* omitted */B", "text": "AB"},
  {"source": "```\na\nb\n```", "text": "a b"},
  {"source": "`raw` @2603231410 $x^2$ #code", "text": "raw @2603231410 $x^2$ #code"},
  {"source": "#box[A /* retained */ B]", "text": "#box[A /* retained */ B]"}
]
```

Changing the projection's meaning is a contract change because it changes search behavior. Strings and string-list metadata do not acquire markup projection implicitly.

## Metadata value

A non-null metadata entry has required `kind` and `value` properties:

| `kind` | `value` type | Meaning |
| --- | --- | --- |
| `markup` | Markup value | Exact authored content and its projection |
| `string` | String | A directly extracted literal member name |
| `string-list` | Array of strings | Direct literal string values in authored order |

The kinds describe values, not Typst AST matching forms. Multiple source forms may produce the same kind. String-list order, spelling, and duplicate values are preserved; values are not sorted, deduplicated, normalized, or checked against a vocabulary.

Markup ranges address inner content. Schema 2 does not require occurrence or per-item ranges for string and string-list values; diagnostics still carry authored ranges where a relevant range exists. Optional source-location properties could be added later without changing the existing meanings.

## Node

A node has these required properties:

| Property | Type | Meaning |
| --- | --- | --- |
| `id` | String | Valid ten-digit local-time `YYMMDDHHmm` filename identity |
| `path` | Archive path | `zettel/ID.typ` for this ID |
| `title` | Markup value or `null` | Recovered direct level-one title content |
| `metadata` | Object | Declared field names mapped to metadata values or `null` |

A canonical filename creates a node even when its contents are malformed. A title may be recovered despite a heading-label mismatch; identity still comes from the filename and the mismatch is diagnosed. Missing or ambiguous titles are `null`.

Metadata field names are case-sensitive, nonempty strings independent of source identifier spellings. They are not an enumeration of reserved engine fields. `abstract`, `keywords`, and `category` are optional template examples, not built-in or privileged node properties. Names inside `metadata` do not replace core node properties.

Every field declared in the template appears in each node's metadata map. A field with no occurrence in a note, a malformed occurrence, or repeated occurrences has a `null` value. Malformed or repeated occurrences additionally produce diagnostics naming that field; ordinary absence does not. An unconfigured field is not emitted. With no template metadata declarations, every node has `metadata: {}`.

An authored empty markup fragment is a non-null markup value with empty source/text and a zero-length range. An authored empty list is a non-null `string-list` with `value: []`. Internal source-generation counters are not required public node properties and must not be used as persistent note identity.

Example node-query result, assuming `summary`, `tags`, `topic`, and `review` are declared:

```json
{
  "schema_version": 2,
  "data": {
    "id": "2603231410",
    "path": "zettel/2603231410.typ",
    "title": {
      "source": "A note",
      "text": "A note",
      "range": {"start": 2, "end": 8}
    },
    "metadata": {
      "summary": {
        "kind": "markup",
        "value": {
          "source": "Summary.",
          "text": "Summary.",
          "range": {"start": 31, "end": 39}
        }
      },
      "tags": {"kind": "string-list", "value": ["one", "two"]},
      "topic": {"kind": "string", "value": "coding"},
      "review": null
    }
  }
}
```

The example's title and summary ranges correspond to the first two lines of this source:

```typst
= A note <2603231410>
#summary[Summary.]
#tag("one", "two")
#group.coding
```

## Link

```json
{
  "source": "2603231410",
  "target": "2603231411",
  "resolution": "resolved",
  "spans": [{"start": 60, "end": 71}]
}
```

All four properties are required. `source` identifies an existing node. `target` is a literal ten-ASCII-digit reference address, which may be missing or not represent a valid calendar timestamp. `resolution` is `resolved` when a matching node exists in the current view, otherwise `missing`. A malformed target note still exists and therefore resolves. Missing targets do not create synthetic nodes.

There is one logical link per ordered source/target pair. `spans` is nonempty and retains every authored occurrence, ordered by `start`, then `end`; each range covers the entire `@ID` token in the source node. Repeated occurrences are not parallel logical links. Literal references contribute links wherever they occur in source markup, regardless of metadata configuration; strings, raw text, comments, and generated references do not.

## Asset

An asset entry has these required string properties:

| Property | Meaning |
| --- | --- |
| `note_id` | Valid ten-digit note ID naming the namespace, not necessarily an existing node |
| `name` | UTF-8 path relative to that namespace, with `/` separators |
| `path` | Archive-relative `assets/ID/name` path, without a leading slash |

`name` is nonempty and contains no empty, `.` or `..` components, backslashes, or control characters. Nested paths, spaces, and Unicode are allowed. Entries describe regular files. Arrays sort lexically by `name` using UTF-8 byte order. A missing or empty namespace produces an empty array.

Example asset-list result:

```json
{
  "schema_version": 2,
  "data": [
    {
      "note_id": "2603231410",
      "name": "tiger.jpg",
      "path": "assets/2603231410/tiger.jpg"
    }
  ]
}
```

Namespace association is not a reference or exclusive-ownership claim. Assets are not nodes and do not appear as metadata or links in graph snapshots. Their bytes remain filesystem state rather than a persistent derived index. Listing is CLI-only; adding this payload does not change existing schema-2 values or introduce an LSP asset-query command.

## Diagnostic

A diagnostic has these required properties:

| Property | Type | Meaning |
| --- | --- | --- |
| `path` | Archive path | Source or archive entry concerned |
| `code` | String | Stable problem classification |
| `severity` | `error` or `warning` | Integrity severity |
| `message` | String | Human-readable explanation, not a parsing contract |
| `range` | Byte range or `null` | Relevant authored range, when available |
| `field` | String or `null` | Configured metadata field concerned, otherwise `null` |

Custom field names belong in `field`, not in diagnostic codes. The initial code vocabulary is:

| Code | Meaning |
| --- | --- |
| `archive.filename` | Noncanonical note filename |
| `archive.layout` | Noncanonical entry under `zettel/` |
| `metadata.title` | Missing or repeated direct level-one title |
| `metadata.id_label` | Title lacks its filename ID label |
| `metadata.id_mismatch` | Title label disagrees with filename identity |
| `metadata.invalid_shape` | Recognized metadata occurrence cannot be retrieved using the field's declared source form |
| `metadata.duplicate` | More than one occurrence of a configured metadata field in a Zettel |
| `reference.dangling` | Authored reference has no target node |
| `syntax.error` | Generic Typst syntax error, saved-state output only |
| `syntax.warning` | Generic Typst syntax warning, saved-state output only |

`field` is the configured key for `metadata.invalid_shape` and `metadata.duplicate`, and `null` for the other initial codes. The code vocabulary may grow additively; consumers must not assume the table is exhaustive. Existing code meanings remain stable. Exact message text is not stable.

## Graph snapshot and ordering

Graph `data` has required `revision`, `nodes`, `links`, and `diagnostics` properties:

```json
{
  "schema_version": 2,
  "data": {
    "revision": 1,
    "nodes": [],
    "links": [],
    "diagnostics": []
  }
}
```

`revision` is a positive session-local graph counter. Accepted source or extraction-rule changes advance it; stale results, retained saves, and ignored disk events do not. A new CLI view starts at revision 1. It is not a timestamp, archive ID, durable checkpoint, or comparable counter across sessions. A snapshot is a coherent provider view, not a transactional filesystem snapshot against concurrent external edits.

Node arrays sort lexically by ID. Graph links sort by source ID, then target ID. Outgoing query links sort by target ID; incoming query links sort by source ID. Diagnostic arrays sort by path, then range with `null` first and numeric `start`/`end` order, then code, field with `null` first, and message. Strings compare lexicographically by UTF-8 bytes, not locale collation. The same field-name order is used for generic hover presentation; JSON object-property order itself remains insignificant.

## Metadata search

Search examines node IDs, projected title text, and all non-null declared metadata values. It uses markup `text`, literal string values, and each string-list item separately. It does not search field names, unrecognized calls, body-only text, or evaluated values.

Matching lowercases the query and each searchable string using Unicode lowercase conversion, then checks literal substring containment. There is no locale-dependent matching, Unicode normalization, stemming, fuzzy scoring, or query language. Query whitespace is not removed. An empty query matches every node.

Every match is returned, once, in ID order without an implicit cap. The CLI and LSP workspace-symbol search use the same rule on their respective source views.
