---
name: zettelkasten
description: Work with this Typst Zettelkasten archive. Use when searching, creating, linking, checking, or removing Zettel.
compatibility: Requires the zk command-line tool.
---

# Zettelkasten

## What you are doing

A Zettelkasten is a personal tool for thinking and writing. Its purpose is to build a web of thoughts whose parts can be revisited, connected, and reused over time. Connection matters more than collecting a large number of notes.

A Zettel is the smallest addressable unit in that web. In this archive, each Zettel should develop one principal idea, such as a claim, argument, concept, observation, or question. Atomicity is a guide rather than a word-count rule. Split a note when its ideas need to be addressed or connected independently.

Write in your own words. Do not preserve information merely because it looks useful. Explain what it means, why it matters, or how it bears on the question at hand. Quotations and source material can support a Zettel, but they do not replace this processing.

Each Zettel has a stable timestamp ID. The ID is its address and does not change when its title changes. References to that address make the archive a navigable hypertext rather than a folder of independent documents.

A link records a relationship between thoughts. The prose around a link should state why the target is relevant. A bare link leaves that relationship for a future reader to guess. Look for meaningful connections when adding a Zettel, but do not manufacture links that have no useful explanation.

Search provides entry points into the web. Follow links and backlinks from a promising entry instead of treating search results as the whole organization. Structure can grow from the bottom up. An ordinary Zettel may act as a structure note by arranging links and explaining how their ideas relate; it requires no special file type.

This is the human owner's personal thinking environment, not a generic encyclopedia. Preserve the owner's terminology, perspective, and existing writing practice. Use the current task as the reason for working in the archive while leaving useful ideas available for later work.

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

There is no special asset resolver or implicit ID binding. Asset contents do not create archive links or metadata. Keep meaningful `@ID` references in the note itself. Note removal leaves its assets intact; listing and explicit asset removal still work afterwards. Removal does not prove a file is unused, so inspect known uses before deleting a shared asset. Symlinks and recursive directory deletion are unsupported by the asset commands.

## Source contract

A Zettel lives at `zettel/YYMMDDHHmm.typ`. Its filename is its identity. `zk.toml` contains only `format = 3`. Read `templates/zettel.typ` before editing metadata: it defines both the starter note and tracked fields. Initialization supplies only this minimal core:

```typst
#import "../lib/zettel.typ": zettel
#show: zettel

= Title <YYMMDDHHmm>
```

Additional metadata is optional and user-owned. In the template, a standalone comment such as `// @zk-field "summary" kind=markup` above `#summary[]` tracks that direct call as the field `summary`. Other supported shapes are positional string arguments, one literal string array, and direct field access. The comment declares the field name and kind; the element supplies its selector and starter value.

`zk new` removes successfully parsed declaration comments. Existing notes need no annotations, and comments within notes never define tracking. Removing a template declaration disables extraction without rewriting notes. Abstract, keywords, and category are optional conventions, not built-in fields. Use whatever the archive's current template declares, and do not invent presentation helpers or vocabulary policy.

Exactly one direct level-one title heading is required, and its label must equal the filename stem. Each configured metadata field may be omitted, but may be declared at most once. Unconfigured fields are not emitted. Configured fields absent from a note are `null`; malformed or repeated declarations are also `null` with a diagnostic naming the field. Authored empty values remain distinct. They may appear in any order and anywhere at the direct top level, among prose, imports, and styling. Imports and show rules are user presentation choices. Markup metadata and the rest of the note may contain unrestricted Typst.

`content-call` retrieves one literal content block as markup; `string-arguments-call` retrieves positional string literals as a string list; `field-access` retrieves a literal member name as a string. The array-call shape retrieves one literal string-array argument, such as `#tags(("one", "two"))`. Any configured field can use any supported form. Field names have no implicit engine role. Do not substitute computed values, spreads, aliases, imported declarations, or nested calls for the configured direct source forms. `zk` does not evaluate Typst to retrieve metadata.

A literal ten-digit `@ID` creates a directed Zettel link. Generated references, strings, raw blocks, and comments do not create links. `zk lsp` handles archive relations and metadata; Tinymist handles ordinary Typst language features, compilation, and dictionary-member completion.

After changes, run:

```sh
zk check
```

Resolve reported metadata, identity, and dangling-reference errors before finishing.
