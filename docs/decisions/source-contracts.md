# Configurable source contracts and user-owned templates

Status: implemented development contract, partially superseded by an accepted target

This document records the initial 0.2.0/archive-format-2 implementation. Its fixed metadata slots, hidden runtime defaults, and schema-1/protocol-1 interface choices are superseded by [Extensible metadata and external contracts](external-contracts.md) and the authoritative [external contracts](../contract/README.md). That replacement is implemented in zk 0.2.0 with data schema 2 and ZK protocol 2. The fixed-field/default and version-1 payload descriptions below remain historical. Relaxed placement, bounded source parsing, user-owned templates, overlay precedence, and the Tinymist boundary remain in force.

Archive format 3 subsequently moves declaration authority into a regular Typst template. See [Template-declared metadata](template-schema.md) for the current source contract and missing-template policy.

## Decision

Archive metadata is defined by a small configurable set of direct top-level Typst source forms. `zk.toml` owns the extraction contract. The shared provider applies it to CLI saved state and LSP open-buffer overlays without evaluating Typst.

Imports, show rules, styling, intervening prose, and declaration order do not determine archive validity. A note requires exactly one direct level-one title heading followed by its filename ID label. Abstract, keywords, and category are optional, with at most one declaration each. Present declarations must have the configured shape. Duplicates produce diagnostics and leave the field absent instead of selecting a winner.

An abstract accepts unrestricted Typst content inside its literal content block. Empty and omitted abstract or keywords have the same effect on ordinary search, but remain distinct in the provider. An omitted field is `null`; an authored empty field retains its value and, where applicable, its exact source range.

## Configuration

A minimal manifest is:

```toml
format = 2
```

Omitted settings use these defaults:

```toml
[metadata.abstract]
form = "content-call"
name = "abstract"

[metadata.keywords]
form = "string-arguments-call"
name = "keywords"

[metadata.category]
form = "field-access"
name = "category"

[new]
template = "templates/zettel.typ.tpl"
```

Each specified metadata rule must contain both `form` and `name`. Names must be direct Typst identifiers, not qualified paths or expressions. Rules identify source spellings without resolving bindings or aliases. A function definition, conditional, loop, imported declaration, or nested content block does not declare note metadata. Unrecognized source constructs remain ordinary Typst.

Supported forms are deliberately bounded:

| Field | Form | Example |
| --- | --- | --- |
| abstract | `content-call` | `#abstract[Content]` |
| keywords | `string-arguments-call` | `#keywords("one", "two")` |
| keywords | `string-array-call` | `#tags(("one", "two"))` with `name = "tags"` |
| category | `field-access` | `#category.coding` |

Content calls take exactly one literal content-block argument. Keyword calls accept literal strings only, without computed values, spreads, named arguments, or extra arguments to the array form. Category field access extracts the literal member name without checking dictionary membership. Abstract and keyword rules cannot share the same call name. Unknown settings, invalid names, unsupported field/form combinations, and ambiguous call rules fail manifest loading rather than falling back.

Changing source spellings does not change semantic field names or provider schema 1. The title and ten-digit reference contracts remain fixed.

Saved manifest changes in an LSP session re-extract all closed notes and retained open sources in one graph revision. Open text and document versions survive; prepared results under the previous rules become stale. Invalid configuration or a failed source read leaves the previous rules and graph intact and is reported through an LSP log message. Unsaved manifest text is not an extraction-rule overlay.

## Note creation

`zk init` installs an editable `templates/zettel.typ.tpl` alongside the initial Typst library and records its path in the manifest. Both files become user-owned. The template path may be changed to another relative path beneath the archive root; escaping paths and symlink targets outside the archive are rejected.

`zk new` reads the configured UTF-8 template, allocates a collision-free timestamp ID, replaces every literal `{{id}}` marker, and writes a new canonical note without overwriting an existing file. All other bytes, including line endings, remain unchanged. The template must contain the ID marker, and its rendered title and recognized metadata must satisfy the archive contract before a file is created.

There is no general template language, script execution, evaluation, or silent fallback when a template is missing or invalid. Imports are authored for the destination Zettel's location, not the template's storage directory. Creation checks archive metadata, not compilation, import resolution, or the author's intention behind unrecognized calls. Changing matcher names therefore requires reviewing the template as well; it does not authorize rewriting it or the presentation library.

## Complementary language servers

`zk lsp` owns note identity, configured metadata extraction, literal archive references, backlinks, archive navigation, and archive search. Tinymist owns general Typst syntax, bindings, imports, type checking, dictionary-member completion, rendering, and compilation.

The provider still parses full source for recovery and exact ranges. Generic syntax diagnostics remain available in saved graph snapshots and `zk check`, but `zk lsp` does not publish them. A malformed recognized metadata declaration remains an archive diagnostic. The engine no longer requires a particular import or show rule and no longer restricts an abstract to inline content.

Dictionary-member category completion is removed from `zk lsp`; protocol version 1 now advertises `categoryCompletion = false`. Reference completion triggers only on `@`. Ordinary library buffers are ignored by the archive session, and requests outside canonical Zettel paths return no archive completion, hover, definition, or references.

## Rationale and consequences

A matcher describes how to read metadata but cannot infer imports, styles, preambles, or default prose. A user-owned template defines what to write; the manifest defines what to recognize. These are separate responsibilities with a consistency check at creation.

Bounded AST matching preserves exact authored ranges, malformed-source recovery, and deterministic extraction without depending on compilation. It also avoids trying to duplicate Tinymist's evaluator or language intelligence. Supporting arbitrary computed or nested metadata would be a separate semantic decision.

This is an intentional breaking change before any deployed persistent archives. The executable is 0.2.0 and accepts archive format 2 only. There is no format-1 migration command or compatibility layer. Graph schema 1 and editor protocol 1 remain unchanged because their shapes are unchanged and protocol feature flags already permit omitting category completion.

This decision replaces the rigid source/header and template policies in [Archive](archive.md), [Zettel](zettel.md), [Metadata](metadata.md), [CLI](cli.md), [Language server](lsp.md), [Implementation](implementation.md), and [Version-one architecture](v1-architecture.md). Their previous rationale remains recorded as historical decisions.

## Evidence

- [Typst capability spikes](../research/typst-capabilities.md) describe evaluation alternatives and their diagnostic, recovery, and source-range limitations.
- `tests/cli.rs` covers relaxed metadata, optional fields, configured extraction, exact template bytes, creation failures, and configuration errors.
- `tests/provider.rs` covers atomic manifest reloads, disk and overlay extraction, stale updates, document versions, and failed-reload preservation.
- `tests/lsp.rs` covers custom contracts and manifest notifications over real stdio, archive-only diagnostics, and complementary behavior with ordinary library requests.
