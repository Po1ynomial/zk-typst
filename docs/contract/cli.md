# CLI contract

Status: implemented in zk 0.3.0. See the [version matrix](README.md#status) and [System](../SYSTEM.md) for flows and limitations.

## Invocation and archive selection

```text
zk init [--agent-skills] [PATH]
zk [--archive PATH] new
zk [--archive PATH] remove ID
zk [--archive PATH] check [--format text|json]
zk [--archive PATH] query node ID
zk [--archive PATH] query links ID
zk [--archive PATH] query backlinks ID
zk [--archive PATH] query search QUERY
zk [--archive PATH] graph --format json
zk [--archive PATH] lsp
zk --help
zk --version
```

`--archive PATH` is a global option and may also follow a subcommand. It selects an existing exact archive root and takes precedence over discovery. Relative CLI paths resolve against the process working directory. Without this option, existing-archive commands walk upward to the nearest `zk.toml`; an invalid nearest manifest is an error, not a reason to fall back to another archive.

`init` defaults `PATH` to the current directory and rejects `--archive`. There is no persistent fallback archive setting. LSP workspace parameters do not override the archive selected when the server process starts.

`QUERY` is one shell argument. Multiword and empty queries must be quoted. All commands are noninteractive; no operation launches an editor or asks for confirmation.

`--version` prints `zk VERSION` followed by a newline, where `VERSION` is the executable semantic version. This is not archive-format, public-data-schema, or ZK-protocol discovery. Help text is human-facing and has no stable layout.

## Streams and exit status

| Operation | stdout | stderr | Exit status |
| --- | --- | --- | --- |
| Successful `init`, `new`, or `remove` | Result path followed by a newline | Optional warnings | 0 |
| Successful query or graph | One complete schema-2 JSON envelope | Optional warnings | 0 |
| `check --format text` | Human-readable diagnostics and totals | Operational errors or warnings | 0 without integrity errors, otherwise 1 |
| `check --format json` | One schema-2 diagnostic-list envelope if inspection completes | Operational errors or warnings | 0 without integrity errors, otherwise 1 |
| Fatal archive, configuration, template, query, or I/O failure | No successful result promised | Human-readable error | 1 |
| Removal blocked by incoming references | No successful result path | Blocking source paths and byte ranges | 1 |
| Argument-parser usage error | No successful result | Usage/error text | 2 |
| Help or executable version | Requested text | No diagnostic required | 0 |
| `lsp` | Framed JSON-RPC only | Startup/process errors | Defined by the LSP lifecycle |

The invalid combination `--archive` with `init` is application validation and exits 1. No error string, warning wording, help layout, text-diagnostic formatting, or JSON whitespace is a machine-readable contract. Consumers must use exit status and structured results instead of matching prose.

JSON output is one envelope, not JSON Lines or a stream of progress records. Its `data` shape is defined below and in [data schema 2](data.md). A trailing newline is permitted. Logs must not be mixed into successful JSON stdout. Consumers must reject incomplete JSON, including partial output from a failed write. A completed integrity check may intentionally return a valid diagnostic envelope with exit status 1; a fatal inspection failure need not return that envelope.

## Initialization and source declarations

`init` creates `zk.toml`, the flat `zettel/` directory, `lib/zettel.typ`, and `templates/zettel.typ`. It must not overwrite existing paths. Installed files immediately become user-owned. The manifest contains only:

```toml
format = 3
```

Unknown manifest properties, including earlier metadata tables and template-path settings, are rejected. The template path is fixed. The initialized template is ordinary Typst with no extra metadata declarations:

```typst
#import "../lib/zettel.typ": zettel
#show: zettel

= Untitled <new>
```

Filename identity, exactly one direct level-one title with its ID label, and literal ten-digit links are independent of the template's metadata declarations. The template's title label is a creation placeholder; any syntactically valid label is accepted. Notes must use their filename ID instead.

### Declaration comments

Only `templates/zettel.typ` declares additional tracked metadata. A declaration is a standalone top-level line comment of this form:

```typst
// @zk-field "summary" kind=markup
#summary[]
```

After the comment delimiter and optional whitespace, the marker is exactly `@zk-field` followed by a space, a JSON-quoted field name, whitespace, and `kind=KIND`. The remaining comment must contain only the kind and optional whitespace. JSON string escapes are decoded. Field names are case-sensitive, nonempty strings with no engine-defined roles. The kind token contains lowercase ASCII letters and hyphens; supported kinds are `markup`, `string`, and `string-list`.

Recognition uses parsed comment nodes, not source-text searches. Strings, raw blocks, nested comments, trailing comments on an element's line, and block comments cannot declare fields. Comments that do not fully parse as declarations are ordinary Typst comments, silently preserved. A syntactically valid declaration with an empty name, unsupported kind, unsupported element, or conflicting selector is a template error.

A declaration attaches to the next direct top-level element. Whitespace and ordinary comments may intervene; prose or other elements may not be skipped. Only one declaration may attach to an element. An unattached declaration is an error. The element supplies the selector, starter value, and one of four inferred retrieval shapes:

| Template element | Declared kind | Inferred retrieval |
| --- | --- | --- |
| `#summary[Content]` | `markup` | One literal content-block argument |
| `#tag("one", "two")` or `#tag()` | `string-list` | Positional string literals |
| `#tag(("one", "two"))` or `#tag(())` | `string-list` | One literal string-array argument |
| `#group.coding` | `string` | Literal member name of a direct field access |

The callee or field-access target must be a direct Typst identifier. Markup may contain arbitrary Typst, retained as source rather than evaluated. Lists reject computed values, spreads, named arguments, and extra array-form arguments. Qualified calls, binding resolution, literal dictionaries, and evaluated values are unsupported. Every starter value must satisfy its inferred rule. Duplicate field names, overlapping call selectors even with different argument shapes, and overlapping field-access selectors are errors.

The engine compiles these declarations into independent matchers. In notes, recognized elements may appear in any order and position at the direct top level among prose, imports, and styles. Notes do not need declaration comments; comments inside notes never define their schema. Each declared field may occur zero or one times. Absence is allowed; malformed or repeated recognized elements produce field-specific diagnostics and `null`. Without annotations, every node has `metadata: {}`. Removing an annotation disables tracking without rewriting existing notes.

Abstract, keywords, and category are not built-in fields or initializer defaults. [An optional descriptive template](../../examples/templates/descriptive.typ) supplies those fields and their presentation helpers. Users may copy it into `templates/zettel.typ` or author their own. No command silently installs it.

### Template availability and errors

All archive-loading commands validate the saved template. If the file is absent, they create the minimal core template, creating its directory if needed and never overwriting a concurrent file. This is an explicit filesystem-writing exception for inspection commands and LSP startup/reload. Deleting a template resets tracking to the minimal core, leaving all note contents intact.

A present template with Typst syntax errors, invalid core structure, or invalid metadata declarations fails loading eagerly. It is never replaced or treated as absent. Read or creation failures are operational errors. Dangling symlinks are errors, not missing templates. Both an existing template's resolved path and the parent used for missing-template creation must stay beneath the archive root.

The template is an extraction dependency; imported implementations and the presentation library are not. `zk` does not evaluate imports or compile notes. Tinymist owns ordinary binding, import, and type validation.

`--agent-skills` optionally installs the bundled skills under `.agents/skills/`. Installation is best-effort: conflicts and filesystem failures warn without failing canonical archive creation. Existing same-name skills remain untouched. Installed copies are user-owned and are not subsequently validated or refreshed. Initialization does not invoke Git, stage files, or create commits. Supplementary Git initialization is a separate accepted but unimplemented request.

Successful initialization prints its root path. The path may be relative and reflect the supplied path spelling; consumers must not assume it is canonicalized.

## Creation and removal

`new` allocates a local-time ten-digit `YYMMDDHHmm` ID. Occupied filenames cause allocation to advance one minute at a time within the supported century. The ID is the note's permanent address, not a mutable title or a guarantee of precise creation time.

`new` re-reads the saved `templates/zettel.typ`. It replaces only the parsed core title-label range with the allocated ID and removes only successfully parsed declaration-comment ranges. All other bytes, including ordinary comments, indentation, line endings, and literal `{{id}}` text, remain unchanged. There is no general substitution language, script execution, or Typst evaluation. Imports are authored for the destination note's location.

The template must parse, have exactly one direct labelled level-one title, and contain valid defaults for its tracked fields. Validation occurs before any note is created. Existing notes are never overwritten. Success prints the archive-relative `zettel/ID.typ` path.

`remove ID` requires an existing node and refuses deletion while any incoming authored references remain, including self-references or references in malformed notes. Refusal reports every blocking occurrence's source path and UTF-8 byte range on stderr and leaves the target untouched. Success deletes only the target file and prints its archive-relative path. Removal never edits incoming references or other source files.

## Inspection and JSON results

All inspection commands use saved state. They do not discover, contact, or share unsaved state with a running LSP process. Each invocation constructs its own provider view. A graph is a complete coherent provider observation, not a filesystem transaction against concurrent writers.

| Command | Envelope `data` | Additional behavior |
| --- | --- | --- |
| `query node ID` | Node | Missing node is an operation error |
| `query links ID` | Array of links sourced by ID | Node must exist; no links returns `[]` |
| `query backlinks ID` | Array of links targeting ID | Node must exist; no backlinks returns `[]` |
| `query search QUERY` | Array of matching nodes | Every match in ID order; no implicit limit |
| `graph --format json` | Graph snapshot | Includes partial nodes, unresolved links, and diagnostics |
| `check --format json` | Array of diagnostics | Integrity errors set exit status 1 |

The graph format option is required and only `json` is supported. Check defaults to `text`. Queries are JSON-only and have no format option. Missing query targets produce errors rather than successful `null` results. A dangling link target remains visible through graph output even though no target node can be queried.

Search uses the shared [metadata matching rule](data.md#metadata-search). It searches IDs, titles, and all textual declared fields, without privileged metadata names. Broad or empty searches can produce large results. Node, link, range, diagnostic, and ordering definitions are shared across all commands.

Graph and query commands do not fail merely because the inspected archive contains integrity diagnostics; they return available data. Check fails on error-severity diagnostics but not warnings. Fatal loading, unsupported configuration, or I/O failures fail the operation. Checking includes generic Typst syntax diagnostics as a standalone saved-state check, but does not compile or evaluate Typst.

## Language-server process

`zk lsp` uses the selected archive and owns a private live provider session. stdout is reserved for LSP framing; startup errors appear on stderr and exit 1 before a successful session. Standard initialization, requests, notifications, shutdown, and exit are specified in the [LSP contract](lsp.md). Normal shutdown followed by exit succeeds; exit without shutdown follows the LSP specification's failing process-exit behavior.
