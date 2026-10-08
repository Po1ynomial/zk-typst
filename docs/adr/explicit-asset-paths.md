# Explicit-path asset management

Associate opaque files with note IDs and use ordinary explicit Typst paths rather than a contextual resolver, generated ID binding, or loader wrappers, because [asset-resolution research](../research/asset-resolution.md) showed that transparent title-derived paths require additional authoring or creation machinery.
Retain assets when a Zettel is removed, because other Zettel can share those paths and source-only scanning cannot prove a file is unused.
Provide explicit safe copying, listing, and removal without dependency discovery or automatic cleanup, leaving assets user-owned and outside graph semantics.
