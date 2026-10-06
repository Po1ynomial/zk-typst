# CLI contract

Status: implemented in zk 0.2.0. See the [version matrix](README.md#status) and [System](../SYSTEM.md) for flows and limitations.

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

`init` creates the root manifest, flat `zettel/` directory, initial `lib/zettel.typ`, and initial `templates/zettel.typ.tpl`. It must not overwrite an existing manifest, note directory, library file, or template file. Installed source files immediately become user-owned. The generated manifest explicitly contains:

```toml
format = 2

[new]
template = "templates/zettel.typ.tpl"

[metadata.abstract]
form = "content-call"
name = "abstract"

[metadata.keywords]
form = "string-arguments-call"
name = "keywords"

[metadata.category]
form = "field-access"
name = "category"
```

These metadata definitions are initialization seeds only. Runtime extraction must not restore a deleted definition. An omitted or empty metadata table means no extra metadata extraction. Adding, removing, or renaming a field changes its retrieved entries without rewriting existing notes, templates, or libraries.

`metadata` is a map of user-selected, nonempty field names. Each declaration has required `form` and `name` properties. `name` is one direct Typst identifier; it is independent of the map key. Any declared field can use one of these forms:

| Form | Source example | Output kind |
| --- | --- | --- |
| `content-call` | `#summary[Content]` | `markup` |
| `string-arguments-call` | `#tag("one", "two")` | `string-list` |
| `string-array-call` | `#tag(("one", "two"))` | `string-list` |
| `field-access` | `#group.coding` | `string` |

The example names must be supplied as the rule's `name`. Calls require direct literal arguments of the shown shape. The array form accepts exactly one literal array argument. Content calls accept exactly one literal content block with unrestricted Typst inside it. Lists reject computed items, spreads, named arguments, and extra arguments to the array form.

Only direct top-level declarations count. They may appear in any order and position among imports, styles, show rules, and prose. Matching identifies source spellings without resolving imports, bindings, aliases, qualified calls, nested declarations, conditionals, or generated values. Unrecognized constructs remain ordinary Typst. Rules selecting the same call name are ambiguous even if their argument forms differ; repeated field-access rules for the same target are also ambiguous. Invalid or unsupported rules must fail configuration loading rather than falling back.

Each declared field may occur zero or one times in a note. Absence is allowed. Duplicates or malformed recognized declarations produce field-specific diagnostics and a `null` value. One direct level-one title with its filename ID label remains required independently of configurable metadata. Missing or malformed contents never erase a canonical filename's node identity.

Only `zk.toml` and `zettel/` are needed for archive inspection. An imported implementation or a particular presentation library is not an extraction requirement. The initial library is supplied for rendering convenience; Tinymist owns import, binding, and type validation.

`--agent-skills` optionally installs the bundled skills under `.agents/skills/`. Installation is best-effort: conflicts and filesystem failures warn without failing canonical archive creation. Existing same-name skills remain untouched. Installed copies are user-owned and are not subsequently validated or refreshed. Initialization does not invoke Git, stage files, or create commits. Supplementary Git initialization is a separate accepted but unimplemented request.

Successful initialization prints its root path. The path may be relative and reflect the supplied path spelling; consumers must not assume it is canonicalized.

## Creation and removal

`new` allocates a local-time ten-digit `YYMMDDHHmm` ID. Occupied filenames cause allocation to advance one minute at a time within the supported century. The ID is the note's permanent address, not a mutable title or a guarantee of precise creation time.

`new.template` selects a relative UTF-8 template file beneath the archive root. Its default path, when this setting is omitted, is `templates/zettel.typ.tpl`; there is no fallback to compiled-in template contents. Resolved symlink targets must also stay beneath the root.

The template must contain a literal `{{id}}` marker. Creation replaces every occurrence with the allocated ID and preserves all other bytes, including line endings. There are no other substitutions, template expressions, scripts, or evaluation. Imports must be authored for the destination Zettel's location, not the template directory.

The rendered title and recognized metadata must satisfy the source contract before a new file is created. This is not compilation or general Typst validation, and it cannot infer intent behind unrecognized calls. Missing templates, missing markers, or invalid rendered metadata fail without creating a note. Existing files are never overwritten. Success prints the archive-relative `zettel/ID.typ` path.

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

Search uses the shared [metadata matching rule](data.md#metadata-search). It searches IDs, titles, and all textual declared fields, not just default metadata names. Broad or empty searches can produce large results. Node, link, range, diagnostic, and ordering definitions are shared across all commands.

Graph and query commands do not fail merely because the inspected archive contains integrity diagnostics; they return available data. Check fails on error-severity diagnostics but not warnings. Fatal loading, unsupported configuration, or I/O failures fail the operation. Checking includes generic Typst syntax diagnostics as a standalone saved-state check, but does not compile or evaluate Typst.

## Language-server process

`zk lsp` uses the selected archive and owns a private live provider session. stdout is reserved for LSP framing; startup errors appear on stderr and exit 1 before a successful session. Standard initialization, requests, notifications, shutdown, and exit are specified in the [LSP contract](lsp.md). Normal shutdown followed by exit succeeds; exit without shutdown follows the LSP specification's failing process-exit behavior.
