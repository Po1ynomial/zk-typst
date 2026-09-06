# Zettel

## Identity

Each Zettel is one file whose filename is its ID:

```text
zettel/YYMMDDHHmm.typ
```

The ID uses local time with minute resolution. Example:

```text
zettel/2603231410.typ
```

Rules:

- The filename is exactly the ID. Titles change, IDs do not.
- On collision, the allocator tries the following minute until it finds a free filename.
- Only one machine writes the archive, so local collision detection is sufficient.
- The format has a century limit, which is acceptable. Migration to a longer format is possible later.

The filename establishes node existence independently of whether the file currently satisfies the content contract. A Zettel with a malformed title or abstract still exists. A dangling reference means no matching file exists.

## Shape

Every Zettel is ordinary top-level Typst markup:

```typst
#import "../lib/zettel.typ": zettel, abstract, keywords, category
#show: zettel

= Path efficiency <2603231410>

#abstract[
Repeated traffic can produce efficient paths.
]

#keywords(
  "networks",
  "optimization",
)

#category.thoughts

The body begins here.

This relates to @2603220935 because ...
```

The contract:

- The explicit import is required.
- `#show: zettel` installs archive presentation.
- Exactly one level-one heading identifies the Zettel.
- The heading label equals the filename stem.
- The body is unrestricted Typst markup after the metadata.
- Lower-level headings remain available inside the body.

No wrapper, no module export, no `#let body = [...]`, no closing bracket at the end.

## Node types

Version one has one node type: Zettel. All Zettel are structurally identical.

There is no `structure`, `hub`, or `atomic` type metadata. A structure note is a Zettel whose content arranges links. A hub emerges from its graph position.

Structural roles may be added later if actual usage finds them useful.
