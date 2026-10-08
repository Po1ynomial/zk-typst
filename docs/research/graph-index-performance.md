# Graph index performance

Status: recorded prototype benchmark; its recommendation was adopted by the [eager graph retention decision](../adr/eager-derived-graph.md). These are not measurements of the current executable.

## Question

At an upper bound of 50,000 Zettel, 8 KiB per file, and 20 references per Zettel, does retaining the full directed graph and source ranges cause enough startup or memory cost to justify lazy graph indexing?

## Environment

Measured on 2026-09-04:

- Apple M4, arm64
- 16 GiB RAM
- macOS on APFS
- Rust 1.94.0
- `typst-syntax` 0.15.1
- eight parser threads for the reported startup measurements

The benchmark and raw output are under `spikes/05-graph-bench/`.

## Models compared

The benchmark parses every file with `typst-syntax` and retains progressively more data:

- `metadata`: node ID, path, title, abstract, keywords, and category; syntax trees are discarded.
- `edges`: metadata plus an occurrence arena and incoming/outgoing adjacency indices. Occurrences contain interned `u32` source and target node indices.
- `spans`: the edge model plus `u32` start and end byte offsets for every occurrence.
- `asts`: the span graph plus every `typst_syntax::Source`, including source text, line index, and syntax tree.

The graph uses one occurrence arena. Incoming and outgoing adjacency lists store occurrence indices rather than duplicate edge records.

Two source profiles were generated:

- `plain`: long prose runs with little markup.
- `rich`: repeated emphasis, raw text, equations, lists, function calls, content blocks, and quotes. This produces a much denser syntax tree.

Each archive has a flat directory and all references resolve. Timings are medians of three fresh-process runs unless stated otherwise. Peak RSS comes from `/usr/bin/time -l`.

## Results

### Scaling with plain bodies

| Zettel | Model | Startup | Peak RSS |
| ---: | --- | ---: | ---: |
| 1,000 | metadata | 9.7 ms | 4.1 MiB |
| 1,000 | spans | 9.8 ms | 5.2 MiB |
| 1,000 | retained ASTs | 10.3 ms | 27.8 MiB |
| 10,000 | metadata | 86.1 ms | 13.1 MiB |
| 10,000 | spans | 91.1 ms | 22.8 MiB |
| 10,000 | retained ASTs | 104.7 ms | 253.2 MiB |
| 50,000 | metadata | 641.9 ms | 48.1 MiB |
| 50,000 | spans | 662.5 ms | 95.3 MiB |
| 50,000 | retained ASTs | 676.4 ms | 1,249 MiB |

Additional interleaved 50,000-file runs put metadata startup at 608 to 646 ms, excluding one 687 ms run. The span model was 718 to 728 ms in two runs, 915 ms in one outlier. This variance does not change the memory result or the size of graph construction itself.

### Higher syntax-tree density

| Zettel | Model | Startup | Peak RSS |
| ---: | --- | ---: | ---: |
| 10,000 | metadata | 284.1 ms | 14.7 MiB |
| 10,000 | edges | 305.1 ms | 22.9 MiB |
| 10,000 | spans | 304.6 ms | 24.4 MiB |
| 10,000 | retained ASTs | 332.9 ms | 2,322 MiB |
| 50,000 | metadata | 1,564.6 ms | 49.0 MiB |
| 50,000 | edges | 1,617.3 ms | 88.2 MiB |
| 50,000 | spans | 1,635.4 ms | 96.0 MiB |

The 50,000-file rich fixture contains one million reference occurrences. Building the span arena and both adjacency tables took about 20 ms. Compared with metadata-only loading, extracting and retaining the complete graph added about 71 ms and 47 MiB at this scale.

Adding byte ranges to the edge-only model cost about 18 ms at startup and 8 MiB of peak RSS. A representation that retains both byte offsets and complete LSP line and UTF-16 coordinates would add an estimated 16 MiB for one million occurrences. It would still stay near 112 MiB total in this benchmark.

The rich retained-AST model reached 2.3 GiB at only 10,000 files. The 50,000-file rich retained-AST case was not run because linear scaling would approach the machine's available memory.

### Live updates and lookups

On one 8 KiB open buffer with 20 references, an incremental `Source::edit` followed by reference extraction measured:

- mean: 6.9 microseconds
- p50: 6.8 microseconds
- p95: 7.0 microseconds

Replacing one node's 20 outgoing links in a mutable 50,000-node graph, including backlink removal and insertion, measured:

- mean: 0.34 microseconds
- p50: 0.33 microseconds
- p95: 0.38 microseconds

One million incoming-adjacency length lookups took about 0.8 ms. This uniform synthetic graph is favorable to cache locality, so that figure should only establish that direct adjacency lookup is not a concern.

## Interpretation

Lazy graph indexing does not address the main startup cost. Global metadata search already requires reading and parsing every file. Extracting references during those parses added little work, and constructing the graph took about 20 ms at the 50,000-file stress case.

The full graph with one million source ranges used less than 100 MiB. The source ranges themselves accounted for about 8 MiB beyond an edge-only graph. This is too small to justify an incomplete graph, repeated archive scans, or a persistent cache in version one.

Retained syntax trees are different. Their memory depends heavily on markup density and becomes expensive quickly. Closed-file source text and syntax trees should be discarded after extraction. Open-buffer syntax trees can be loaded on demand and updated incrementally.

## Recommendation from the spike

Use an eager global graph with lazy source hydration:

- parse all Zettel at startup;
- retain all node metadata;
- intern Zettel IDs to compact integer indices;
- retain every literal reference occurrence and its compact source range;
- maintain incoming and outgoing adjacency indices;
- discard source text and syntax trees for closed files;
- retain incrementally updated `typst_syntax::Source` values only for open buffers.

Do not add lazy graph indexing or a persistent cache for version one. Revisit that choice only if measurements on a real archive differ materially from this spike.

## Limitations

- The fixture was synthetic and used fixed-size files with regular metadata.
- The files were generated immediately before testing, so OS file caches were likely warm. This does not represent worst-case cold storage.
- Metadata values were retained realistically, but metadata extraction itself was simplified. References and headings came from the real Typst AST.
- The benchmark did not include malformed source, filesystem watching, JSON serialization, LSP protocol overhead, or Tinymist running alongside `zk`.
- The graph used compact integer indices. Copying ten-digit strings and paths into every edge would consume more memory and is not recommended.
