# CLI contract

## Invocation and archive selection

```text
zk init [--agent-skills] [PATH]
zk [--archive PATH] new
zk [--archive PATH] remove ID
zk [--archive PATH] asset add ID SOURCE [--name RELATIVE-PATH]
zk [--archive PATH] asset list ID
zk [--archive PATH] asset remove ID RELATIVE-PATH
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
| Successful `init`, `new`, `remove`, or asset add/remove | Result path followed by a newline | Optional warnings | 0 |
| Successful query, graph, or asset list | One complete schema-2 JSON envelope | Optional warnings | 0 |
| `check --format text` | Human-readable diagnostics and totals | Operational errors or warnings | 0 without integrity errors, otherwise 1 |
| `check --format json` | One schema-2 diagnostic-list envelope if inspection completes | Operational errors or warnings | 0 without integrity errors, otherwise 1 |
| Fatal archive, configuration, template, asset, query, or I/O failure | No successful result promised | Human-readable error | 1 |
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

Canonical Zettel occupy one flat `zettel/` directory, with paths `zettel/YYMMDDHHmm.typ`. IDs contain exactly ten ASCII decimal digits and represent valid calendar timestamps. `YY` is interpreted as 2000 plus the two-digit year, so the supported century is 2000 through 2099. The filename establishes identity even when the contents are malformed. Filename identity, exactly one direct level-one title with its ID label, and literal ten-digit links are independent of the template's metadata declarations. The template's title label is a creation placeholder; any syntactically valid label is accepted. Notes must use their filename ID instead.

Saved graph discovery inspects only immediate entries under `zettel/`. Regular files with canonical filenames become nodes. Other regular files produce `archive.filename` diagnostics and no nodes. Directories, symlinks even to regular files, and special files produce `archive.layout` diagnostics and are not traversed. An unreadable canonical file or invalid UTF-8 source fails provider loading rather than producing a partial node. The archive root and `zettel/` directory may themselves resolve through symlinks during loading; this does not imply the stricter asset-command policy below.

Each note source, template source, and rendered starter must fit within 4,294,967,295 UTF-8 bytes, the current unsigned 32-bit byte-range limit. Larger templates fail template validation; larger note sources fail provider loading or the relevant live update. These are operation failures, not integrity diagnostics. Available memory may impose a lower practical limit. These source-size limits do not apply to opaque asset bytes.

Native references whose target is exactly ten ASCII decimal digits are reserved for archive links, including targets that are not valid calendar timestamps. Other native references remain ordinary Typst labels or bibliography citations. Labels unrelated to Zettel identity and bibliography keys must not use the reserved ten-digit namespace. Link extraction, grouping, and resolution are specified in the [data contract](data.md#link).

### Declaration comments

Only `templates/zettel.typ` declares additional tracked metadata. A metadata declaration defines a field's tracking rule in the template. A metadata occurrence is an authored use of that field in a Zettel; a metadata value is the retrieved data. Declarations and occurrences are distinct even when they use the same Typst element shape. A declaration is a standalone top-level line comment of this form:

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

The engine compiles these declarations into independent matchers. In notes, metadata occurrences are recognized elements that may appear in any order and position at the direct top level among prose, imports, and styles. Notes do not need declaration comments; comments inside notes never define their schema. Each configured field may have zero or one occurrence. Absence is allowed; malformed or repeated occurrences produce field-specific diagnostics and `null` values. Without annotations, every node has `metadata: {}`. Removing an annotation disables tracking without rewriting existing notes.

Abstract, keywords, and category are not built-in fields or initializer defaults. [An optional descriptive template](../../examples/templates/descriptive.typ) supplies those fields and their presentation helpers. Users may copy it into `templates/zettel.typ` or author their own. No command silently installs it.

### Template availability and errors

All archive-loading commands validate the saved template. If the file is absent, they create the minimal core template, creating its directory if needed and never overwriting a concurrent file. This is an explicit filesystem-writing exception for inspection commands and LSP startup/reload. Deleting a template resets tracking to the minimal core, leaving all note contents intact.

A present template with Typst syntax errors, invalid core structure, or invalid metadata declarations fails loading eagerly. It is never replaced or treated as absent. Read or creation failures are operational errors. Dangling symlinks are errors, not missing templates. Both an existing template's resolved path and the parent used for missing-template creation must stay beneath the archive root.

The template is an extraction dependency; imported implementations and the presentation library are not. `zk` does not evaluate imports or compile notes. Tinymist owns ordinary binding, import, and type validation.

`--agent-skills` optionally installs the bundled skills under `.agents/skills/`. Installation is best-effort: conflicts and filesystem failures warn without failing canonical archive creation. Existing same-name skills remain untouched. Installed copies are user-owned and are not subsequently validated or refreshed.

Successful initialization prints its root path. The path may be relative and reflect the supplied path spelling; consumers must not assume it is canonicalized.

## Creation and removal

`new` allocates a local-time ten-digit `YYMMDDHHmm` ID. Occupied filenames cause allocation to advance one minute at a time; crossing the century of the starting local clock fails with ID-space exhaustion. The allocator formats the clock's year as two digits and does not reject clocks outside 2000 through 2099. Such clocks are unsupported and can produce IDs interpreted as a different year. The ID is the note's permanent address, not a mutable title or a guarantee of precise creation time.

`new` re-reads the saved `templates/zettel.typ`. It replaces only the parsed core title-label range with the allocated ID and removes only successfully parsed declaration-comment ranges. All other bytes, including ordinary comments, indentation, line endings, and literal `{{id}}` text, remain unchanged. There is no general substitution language, script execution, or Typst evaluation. Imports are authored for the destination note's location.

The template must parse, have exactly one direct labelled level-one title, and contain valid defaults for its tracked fields. Validation occurs before any note is created. Existing notes are never overwritten. Success prints the archive-relative `zettel/ID.typ` path.

`remove ID` requires an existing node and refuses deletion while any incoming authored references remain, including self-references or references in malformed notes. Refusal reports every blocking occurrence's source path and UTF-8 byte range on stderr and leaves the target untouched. Success deletes only the target file and prints its archive-relative path. Removal never edits incoming references or other source files.

## Asset management

Asset namespaces use `assets/ID/`, where ID is the existing ten-digit note ID. Directories are created lazily by asset addition, not by `init` or `new`. Namespaces may contain nested directories. Files remain user-owned and may be edited directly. Other asset-root entries, such as `assets/shared/`, are outside these targeted namespace commands.

`asset add ID SOURCE [--name RELATIVE-PATH]` requires an existing regular canonical saved note file. Malformed note contents do not prevent addition; the command does not load or evaluate the note graph. SOURCE resolves relative to the process working directory, independently of archive selection, and must resolve to a regular file. Explicit source symlinks may resolve to regular files; the command copies their bytes, never the symlink.

Without `--name`, the destination uses the source basename, which must be UTF-8. An explicit name permits copying a source with a non-UTF-8 basename on filesystems that support one. Names are nonempty UTF-8 paths relative to the note namespace, using `/` separators. Absolute paths, empty components, `.`/`..` components, backslashes, and control characters are rejected. Spaces, Unicode, and nested relative paths are supported. Names are literal, with no wildcard expansion.

Addition copies only the supplied file's bytes and creates missing parent directories. It does not preserve filesystem attributes, chase helper imports, rewrite relative imports, deduplicate content, or edit the note. Existing destinations of any kind are never overwritten. Copying stages a complete file outside the ID namespaces before publishing it without clobbering. Failed copies or publication attempts leave no partial destination; empty parent directories may remain. Staging and destination must share a filesystem, otherwise publication fails explicitly.

`asset list ID` recursively returns regular files in one namespace as a schema-2 envelope whose `data` is an array of [asset entries](data.md#asset). Entries sort lexically by namespace-relative name. A missing namespace returns `data: []` and does not create asset directories. ID must be a valid timestamp but need not identify an existing note, so retained orphan namespaces remain inspectable. The result is a filesystem observation, not a transaction against concurrent external edits.

`asset remove ID RELATIVE-PATH` deletes exactly one existing regular file and prints its archive-relative path. It does not expand patterns, remove directories recursively, prune empty directories, or infer whether a file is unused. A missing file is an error. The note need not exist. Stored or namespace symlinks are neither traversed nor removed; users must manage them outside these commands. Listing fails rather than silently omitting symlinks, special files, or unsupported names.

Asset-directory components and stored files must not be symlinks, including links whose targets are inside the archive. Checks assume the archive's single-writer model, not hostile concurrent directory replacement. The archive root itself may be selected through a symlink. Abrupt termination can leave temporary staging directories outside note namespaces; automatic crash recovery is not implemented.

Successful add/remove commands print the archive-relative destination, such as `assets/2603231410/tiger.jpg`. Typst's corresponding project-root path is `/assets/2603231410/tiger.jpg`; the leading slash is added by the author or editor, not by these CLI results. There is no Typst asset resolver, injected ID binding, source insertion, or asset-specific LSP command.

`zk remove ID` leaves asset files untouched. Namespace association does not imply exclusive use: another note can reference the same path. `zk check` currently does not warn about orphan namespaces or attempt missing/unused-asset detection. Tinymist owns ordinary Typst file-loading diagnostics. Asset contents do not create graph nodes, metadata fields, or reference occurrences.

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
