---
name: zettelkasten
description: Work with this Typst Zettelkasten archive. Use when searching, creating, linking, checking, or removing Zettel.
compatibility: Requires zk with archive format 3, public data schema 2, and the note, query, check, graph, and asset CLI commands described here.
---

# Zettelkasten

## What you are doing

A Zettelkasten is the owner's personal thinking environment. Develop one principal thought per Zettel, write it in your own words, and preserve the context needed to revisit it. Atomicity is about independently addressable ideas, not a word-count rule. Quotations may support the thought but do not replace processing it.

A note ID is a stable address, independent of its title. A reference is one authored occurrence pointing to that address; a link is the directed relationship represented by one or more occurrences. Explain in prose why the target matters. Backlinks expose incoming relationships, not reciprocal links.

Search provides entry points. Follow connections and revise earlier thoughts when needed. Structure notes arrange and explain relationships as ordinary Zettel, without a special node type. Do not add links merely to increase connectivity or invent a vocabulary policy.

Typst source keeps prose, notation, and user-owned presentation together. The engine supplies retrieval and integrity checks, not intellectual judgment or rendering. Preserve the owner's terminology and perspective rather than producing a generic encyclopedia. Passing `zk check` does not establish writing quality.

This workflow adapts the principles in [Introduction to the Zettelkasten Method](https://zettelkasten.de/introduction/) to this archive's Typst format and reference rules.

## Working method

- Search before creating a Zettel.
- Read promising notes and inspect their outgoing links and backlinks.
- Decide whether to revise an existing thought or create a separately addressable one.
- Keep one principal idea in each new Zettel and develop it in your own words.
- Connect it where the relationship can be stated explicitly in prose.
- Do not invent category or keyword policy. Follow the archive's existing usage.
- Check archive integrity after making changes.

## Query the archive

Run commands inside the archive or pass `--archive <PATH>`.

This skill expects archive format 3 and JSON data schema 2. Read `zk.toml` before operating on an existing archive, and reject unsupported `schema_version` values before interpreting JSON results. The executable's version alone does not establish interface compatibility. Installed skills are user-owned and are never refreshed automatically; review an installed copy when engine or archive requirements change.

Every archive-loading command, including queries, checks, asset commands, and LSP startup, validates the saved template. If it is missing, loading creates a minimal core and disables tracking of additional fields without rewriting notes. Inspection can therefore write a file. A present invalid template fails explicitly and is never replaced. Inspect template availability before commands when preserving existing tracking matters.

Search metadata with a specific term:

```sh
zk query search "path efficiency"
```

Search matches IDs, titles, and all textual metadata declared in `templates/zettel.typ` without regard to case. It does not search field names or arbitrary body text. It returns every matching node and has no result limit. Archive JSON results use `{schema_version: 2, data: ...}`; nodes have a generic `metadata` map with typed `kind`/`value` entries rather than fixed abstract, keyword, or category properties. Broad or empty searches can produce large JSON output. Narrow the term or filter the result before putting it into agent context:

```sh
zk query search "path" | jq '.data | map({id, title: .title.text})'
```

Inspect a known Zettel and its relations:

```sh
zk query node <ID>
zk query links <ID>
zk query backlinks <ID>
```

Use `zk graph --format json` only when the task needs the complete saved graph. These CLI queries read saved state.

## Create and remove Zettel

Create through `zk` so it allocates the timestamp ID:

```sh
zk new
```

The command reads `templates/zettel.typ`, replaces its parsed title label with the allocated ID, strips metadata declaration comments, and prints the new archive-relative path. All other bytes are preserved. Edit the new note through the available editor workflow. Do not manually choose or change its ID or rewrite the user-owned template or library unless the task calls for it. A missing template is created with the minimal core. A present invalid template raises an error; do not bypass it by creating notes manually.

Remove through the guarded command:

```sh
zk remove <ID>
```

Removal fails while incoming references exist. Do not bypass that check by deleting a Zettel directly.

## Attach files

Use the asset commands when a note needs an image, local helper, or Typst fragment:

```sh
zk asset add <ID> <SOURCE>
zk asset add <ID> <SOURCE> --name fragments/example.typ
zk asset list <ID>
zk asset remove <ID> <RELATIVE-PATH>
```

Addition requires an existing saved canonical note and copies one regular file without overwriting. It defaults to the source basename; `--name` chooses a safe relative name, including nested paths. Add/remove print `assets/ID/name`. Listing returns a schema-2 array of `note_id`, `name`, and archive-relative `path` values. Source paths resolve from the shell's working directory. No file contents or imports are rewritten, and helper dependencies must be supplied separately.

Insert ordinary Typst project-root paths into the note, with a leading slash:

```typst
#figure(image("/assets/2603231410/tiger.jpg"), caption: [A tiger])
#import "/assets/2603231410/helper.typ": draw
#include "/assets/2603231410/fragments/example.typ"
```

There is no special asset resolver or implicit ID binding. Asset contents do not create archive links or metadata. Keep meaningful `@ID` references in the note itself. Note removal leaves its assets intact; listing and explicit asset removal still work afterwards. Removal does not prove a file is unused, so inspect known uses before deleting a shared asset. Stored namespace symlinks and recursive directory deletion are unsupported. An explicitly supplied source symlink to a regular file copies the target bytes.

## Source contract

A Zettel lives at `zettel/YYMMDDHHmm.typ`. Its filename is its identity. `zk.toml` contains only `format = 3`. Read `templates/zettel.typ` before editing metadata: it defines both the starter note and tracked fields. Initialization supplies only this minimal core:

```typst
#import "../lib/zettel.typ": zettel
#show: zettel

= Untitled <new>
```

A metadata field is an archive-selected attribute. A declaration defines its tracking rule in the template; an occurrence is an authored use in a Zettel; a value is the retrieved data. Additional metadata is optional and user-owned. In the template, a standalone comment such as `// @zk-field "summary" kind=markup` above `#summary[]` declares tracking of that direct call as the field `summary`. Other supported shapes are positional string arguments, one literal string array, and direct field access. The comment declares the field name and kind; the element supplies its selector and starter value.

`zk new` removes successfully parsed declaration comments. Existing notes need no annotations, and comments within notes never define tracking. Removing a template declaration disables extraction without rewriting notes. Abstract, keywords, and category are optional conventions, not built-in fields. Use whatever the archive's current template declares, and do not invent presentation helpers or vocabulary policy.

Exactly one direct level-one title heading is required, and its label must equal the filename stem. Each configured metadata field may have zero or one occurrence in a note. Unconfigured fields are not emitted. Configured fields absent from a note have `null` values; malformed or repeated occurrences also yield `null` with a diagnostic naming the field. Authored empty values remain distinct. Metadata occurrences may appear in any order and anywhere at the direct top level, among prose, imports, and styling. Imports and show rules are user presentation choices. Markup metadata and the rest of the note may contain unrestricted Typst.

A literal content-block call supplies `markup`; positional string arguments or one literal string-array argument, such as `#tags(("one", "two"))`, supply `string-list`; direct field access supplies `string`. The starter element must match its declared kind. Field names have no implicit engine role. Do not substitute computed values, spreads, aliases, imported declarations, or nested calls for the configured direct source forms. `zk` does not evaluate Typst to retrieve metadata.

A literal ten-digit `@ID` creates a directed Zettel link. Generated references, strings, raw blocks, and comments do not create links. `zk lsp` handles archive relations and metadata; Tinymist handles ordinary Typst language features, compilation, and dictionary-member completion.

After changes, run:

```sh
zk check
```

Resolve reported metadata, identity, and dangling-reference errors before finishing.
