# Version-one architecture

Status: accepted

## Decision

Version one uses a Rust-authoritative live archive provider over a restricted Typst source contract.

The Rust provider parses source with `typst-syntax` but does not evaluate Typst. It owns identity, metadata extraction and validation, open-buffer overlays, literal links and ranges, graph construction, diagnostics, queries, CLI behavior, and ZK language-server behavior.

Typst remains the canonical source and presentation language. `lib/zettel.typ` renders archive constructs and intercepts ten-digit references for Typst and Tinymist. Typst-side archive metadata and business logic are non-authoritative in version one.

The provider eagerly retains a compact complete graph and discards closed-file source trees. Live bundled consumers use an internal Rust API. External consumers use versioned JSON snapshots. There is no shared daemon or public live stream.

## Rationale

The daily workflow requires current metadata, links, backlinks, and diagnostics for unsaved and malformed buffers. Rust can update one source node independently, preserve exact authored ranges, and serve LSP requests without depending on aggregate Typst compilation.

Experiments proved that Typst can construct rich node metadata and a centralized archive value, and Tinymist can show updated sampled values from unsaved buffers. Those paths still lack authored source ranges, share failures across aggregate compilation, and provide no direct live channel from Tinymist to `zk lsp`.

A compact eager Rust graph measured about 96 MiB at 50,000 synthetic Zettel with one million reference occurrences. Graph construction took about 20 ms. Retaining syntax trees, not retaining links or ranges, caused the material memory cost.

Restricting metadata to direct top-level forms removes ambiguity between source syntax and runtime evaluation. The unrestricted body still uses ordinary Typst.

## Consequences

- Valid metadata must use the fixed direct syntax declared by archive format 1.
- Equivalent computed Typst does not define archive metadata.
- Rust and the presentation library share a documented source contract and require compatibility tests.
- Rich title and abstract values preserve exact source plus deterministic non-evaluated text.
- Literal references alone define links.
- Open buffers override disk state in their provider session.
- Closed source and syntax trees are loaded only when needed.
- Typst-native archive computation remains possible later but is not in the editing path.
- Separate processes cannot observe the editor's unsaved graph in version one.

## Supporting research

- [Typst capability experiments](../research/typst-capabilities.md)
- [Graph index performance](../research/graph-index-performance.md)

## Related decisions

- [Archive](archive.md)
- [Zettel](zettel.md)
- [Metadata](metadata.md)
- [References and graph](references.md)
- [CLI](cli.md)
- [Language server and editor](lsp.md)
- [Implementation and migration](implementation.md)
