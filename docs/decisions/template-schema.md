# Template-declared metadata

Status: accepted and implemented in zk 0.3.0, archive format 3, data schema 2, and ZK protocol 2

## Decision

Use the regular Typst file `templates/zettel.typ` as both the note-creation template and declaration authority for additional metadata. `zk.toml` contains only `format = 3`. The template path is fixed rather than separately configurable.

Filename identity, exactly one direct labelled level-one title, and literal ten-digit references remain engine-defined. This core does not depend on extra metadata declarations. Creation replaces the parsed title label with the allocated ID; no special placeholder syntax is needed.

Standalone top-level line comments declare fields. The comment names the field and output kind; its following element supplies the direct selector and literal shape. `zk new` strips successfully parsed declaring comments and preserves all other bytes apart from the core label replacement. Unparseable annotation-like comments are ordinary Typst, silently retained. A successfully parsed declaration with invalid or ambiguous semantics is an error. Notes do not need annotations, and annotations within notes never define their schema.

Support the existing four source shapes initially: one literal content block, positional string literals, one literal string array, and direct field access. Names are direct identifiers. Comments do not authorize evaluation, binding resolution, qualified calls, named-argument captures, dictionary captures, or arbitrary AST patterns. Rich Typst inside markup remains exact authored source, not an evaluated value.

There are no built-in abstract, keywords, or category fields. Initialization supplies only the core template and reference library. The optional descriptive example supplies these familiar fields and their helpers without becoming a runtime default.

All archive loading validates the saved template. If it is missing, create the minimal core without overwriting anything, including during inspection and LSP loading. A present invalid template fails eagerly and is never replaced. Read/creation failures and dangling or escaping symlinks are errors. Deleting the template resets tracking, not source content.

Saved schema changes atomically re-extract disk notes and open overlays. Invalid live reloads retain the last valid graph and rules and report both an error log and a visible LSP message. Saved-template notifications and saves trigger reloads; unsaved schema text is not an overlay. Changes to defaults or presentation that preserve compiled rules do not change graph semantics or revision.

## Rationale

The previous format separated the starter template from TOML retrieval rules. Users had to keep the two descriptions synchronized. An annotated example element supplies its starter value and retrieval shape once, while remaining ordinary Typst for tooling and presentation.

Comments are preferable to typed holes for the initial forms because they require no augmented-source preprocessing or separate default-value language. They are less direct for several captures within one element; that expansion is deferred until a concrete need justifies explicit selectors.

Metadata exists to find and connect atomic notes. A general evaluated database would add dependency invalidation, compilation failure modes, and different rich-value and source-range semantics. Source-only extraction keeps the editing loop useful for malformed notes and preserves authored ranges. The existing [Typst capability research](../research/typst-capabilities.md) demonstrates evaluation alternatives and their limitations.

## Compatibility and consequences

Archive format 3 distinguishes the new declaration authority and required source input from undeployed format 2. The executable accepts format 3 only, without a migration or downgrade layer. Data schema 2 and ZK protocol 2 remain unchanged because retrieved values, source ranges, query shapes, and identity/link semantics remain unchanged. Future breaking changes to deployed archives require explicit migration.

Inspection is no longer strictly read-only: creating a missing core template is an intentional exception. Imported rendering helpers are not extraction dependencies. The initializer never automatically installs the optional descriptive template. Installed templates and libraries remain user-owned.

This supersedes the manifest-declaration and initialization-seed portions of [Extensible metadata and external contracts](external-contracts.md), while preserving its generic metadata map, typed values, null/empty semantics, diagnostics, search, shared envelopes, and independent version discovery. The authoritative grammar and behaviors live in the [CLI contract](../contract/cli.md); transport reload/error behavior lives in the [LSP contract](../contract/lsp.md).

## Evidence

- `src/template.rs` parses comment nodes, compiles bounded selectors, validates defaults, and renders exact range edits.
- `src/archive.rs` validates templates on loading and safely creates missing core templates.
- Template unit tests cover the four shapes, ordinary comment preservation, nested/raw/string exclusions, invalid defaults, orphan declarations, and ambiguity.
- CLI tests cover minimal initialization, exact CRLF/Unicode rendering, schema-2 fixtures, missing-template creation, and eager errors across inspection, creation, and LSP startup.
- Provider and real-transport LSP tests cover atomic schema changes, stale results, retained overlays, deletion/reset, unchanged-default revisions, visible failures, and reloads through both saves and watched-file events.
