# Metadata

## Policy

Metadata is decentralized and canonical in each Zettel. There is no centralized metadata file.

Metadata is part of the document markup. It renders nicely in the Zettel but follows a contract, so the CLI and language service can extract it from the Typst AST.

The CLI does not parse metadata with regular expressions. It parses the full Typst syntax tree.

## Fields

### Title and ID

```typst
= Path efficiency <2603231410>
```

The level-one heading holds the human-readable title and the ID label. The ID is also the filename stem.

### Abstract

```typst
#abstract[
Repeated traffic can produce efficient paths.
]
```

Rules:

- Exactly one abstract is required.
- It may be empty.
- It is a short summary of the Zettel.
- It allows inline Typst markup such as emphasis, equations, and references.
- It rejects block structures such as headings, lists, and figures.

### Keywords

```typst
#keywords(
  "networks",
  "optimization",
)
```

A list of terms. The exact policy for keywords remains unsettled. A controlled vocabulary is one candidate.

### Category

```typst
#category.thoughts
```

A high-level label drawn from a curated set, for example physics, coding, or thoughts. If a keyword policy is later established, category can be derived via statistics. Until then it uses the explicit `#category` construct.

## Provider representation

The provider represents title and abstract content without evaluating Typst:

```text
MarkupValue
  source: exact inner Typst source
  text: deterministic display and search projection
  range: source range of the inner content
```

The text projection concatenates ordinary text and spaces, recurses through emphasis and strong markup, uses raw text contents, preserves literal `@ID` references, preserves equations and code expressions as source text, and omits comments.

The provider retains this bounded source fragment rather than a full syntax tree. Keywords and category values remain ordinary strings.

## Extraction contract

Version one metadata is a restricted declarative subset of Typst. `zk` parses it with `typst-syntax` but never evaluates it.

The required constructs are direct top-level forms:

- the required import from `../lib/zettel.typ`;
- `#show: zettel`;
- one level-one heading with a literal label;
- one direct `#abstract[...]` call;
- one direct `#keywords(...)` call whose items are string literals;
- one direct `#category.name` field access.

The library import must contain exactly `zettel`, `abstract`, `keywords`, and `category`, but their order has no meaning. This allows Typst formatters to reorder the names. Renamed, repeated, missing, or additional imports are invalid.

Aliases, computed arguments, spreads, loops, and conditional metadata do not define archive metadata. They may be valid Typst, but `zk` reports the required declarative field as missing or malformed.

The metadata header appears in the order shown above, before the unrestricted body. The library defines rendering. `zk` defines archive meaning and validity. Changing how `abstract` renders cannot change what the language service extracts.

`lib/zettel.typ` may emit Typst `metadata` elements for later compilation paths, but `zk` ignores them in version one.
