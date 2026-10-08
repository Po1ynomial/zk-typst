# Research

These notes preserve questions, recorded methods, findings, and limits. They refer to untracked spike artifacts without requiring those artifacts in a clone. Experiments do not define current archive behavior; [contracts](../contract/README.md) do. [Decisions](../adr/README.md) record which alternatives were accepted.

| Note | Question and findings | Current relevance and limits |
| --- | --- | --- |
| [Typst capability spikes](typst-capabilities.md) | Reference interception, title resolution, evaluated metadata, aggregate compilation, and Tinymist runtime inputs and hover | Supports the source-authority boundary and reference presentation. Fixed-field conventions are historical experiments. The temporary Tinymist hover client and logs are not retained with the named spikes. |
| [Graph index performance](graph-index-performance.md) | Whether full graph and source-range retention justify lazy indexing at 50,000 Zettel | Supports eager graph retention and discarding closed syntax trees. Synthetic prototype measurements do not establish executable performance. |
| [Asset resolution from a Zettel title label](asset-resolution.md) | Whether native file loaders can derive a note namespace without explicit context or repeated identity | Supports explicit native paths rather than installed resolver helpers. Compiler probes do not establish editor behavior or asset-management safety. |

Executable benchmarking and acceptance thresholds for startup time, memory, and live-update latency remain deferred. The [project expectations](../PROJECT.md#quality-expectations) are goals, not results of the prototype benchmark.
