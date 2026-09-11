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

Search matches IDs, titles, abstracts, keywords, and categories without regard to case. It returns every complete matching node and has no result limit. Broad or empty searches can produce large JSON output. Narrow the term or filter the result before putting it into agent context:

```sh
zk query search "path" | jq 'map({id, title: .title.text, category})'
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

The command writes the standard template and prints its archive-relative path. Edit that file through the available editor workflow. Do not manually choose or change its ID.

Remove through the guarded command:

```sh
zk remove <ID>
```

Removal fails while incoming references exist. Do not bypass that check by deleting a Zettel directly.

## Source contract

A Zettel lives at `zettel/YYMMDDHHmm.typ`. Its filename is its identity. Keep the direct top-level header in this order:

```typst
#import "../lib/zettel.typ": zettel, abstract, keywords, category
#show: zettel

= Title <YYMMDDHHmm>

#abstract[
Short summary.
]

#keywords(
  "term",
)

#category.thoughts
```

The heading label must equal the filename stem. The body after the category is unrestricted Typst. A literal ten-digit `@ID` creates a directed Zettel link. Generated references, strings, raw blocks, and comments do not create links.

After changes, run:

```sh
zk check
```

Resolve reported metadata, identity, and dangling-reference errors before finishing.
