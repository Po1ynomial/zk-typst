# Design

This guide owns accepted shared responsibilities and invariants. [System](SYSTEM.md) describes their implementation; [contracts](contract/README.md) define exact external behavior. [Philosophy](PHILOSOPHY.md) explains the thinking practice these choices support.

## Archive authority

The archive is self-contained plain source and ordinary asset files. Derived metadata, links, backlinks, and diagnostics are disposable views of that source, not additional canonical state. Git is supplementary and does not define archive identity or validity.

Note identity survives changes to title, presentation, and validity. A malformed Zettel remains a node so that partial editing does not destroy navigation or conceal incoming references. The [identity decision](adr/filename-identity.md) records the trade-off; the [CLI source contract](contract/cli.md#initialization-and-source-declarations) and [node definition](contract/data.md#node) specify the concrete conventions.

## Source and presentation

Rust owns archive semantics. It parses authored Typst source without evaluating imports, bindings, or document bodies. User-owned Typst code owns presentation. Tinymist owns general syntax, bindings, imports, types, dictionary-member completion, rendering, and compilation.

The archive template supplies both starter text and declaration authority for optional metadata. Declarations define tracking rules; occurrences in Zettel supply authored data; retrieved values describe those occurrences. Bounded source matchers identify independently authored metadata occurrences rather than imposing one document skeleton. Presentation choices and unrelated prose must not determine whether tracked metadata can be retrieved.

There are no privileged additional metadata names. Fields describe archive-selected attributes, not implicit structural roles or a built-in vocabulary. The [template decision](adr/template-declared-metadata.md) and [generic metadata decision](adr/generic-metadata.md) explain why; the [CLI contract](contract/cli.md#declaration-comments) owns declaration grammar and the [data contract](contract/data.md#metadata-value) owns retrieved values.

Creation edits only the portions authorized by the source contract. Installed templates, libraries, and skills become user-owned. Later operations must not silently replace them. Missing-template creation is an explicit exception to read-only inspection, specified by the [template availability contract](contract/cli.md#template-availability-and-errors).

## Relationships and assets

Authored references establish directed relationships. Backlinks are derived incoming links, not reciprocal source edits. All authored occurrences matter for navigation and diagnostics even when several refer to the same target. The [reference decision](adr/native-reference-namespace.md) explains the namespace choice; the [link contract](contract/data.md#link) defines occurrences and grouping.

Assets are associated with note identity but remain ordinary files outside the note graph. Explicit Typst paths permit sharing across Zettel, so removing a Zettel must not infer that its assets are unused. Dependency discovery and automatic cleanup would cross the source-only boundary. The [asset decision](adr/explicit-asset-paths.md) records this policy; the [CLI asset contract](contract/cli.md#asset-management) owns lifecycle and path safety.

## Provider ownership

Each consumer owns its provider session. CLI operations see saved state; an editor session additionally owns its open source. There is no shared daemon, CLI discovery of editor sessions, or public cross-process live stream.

```text
Saved source ----------------> CLI provider ----> saved queries and snapshots
Saved source + editor source -> LSP provider ----> live editor services
```

The provider owns extraction, validation, relationships, and coherent updates. Adapters own transport and consumer-specific positions. The same archive rules apply to every source view.

The eager graph retains metadata and authored relationships; closed-file source trees are disposable. Open source takes precedence over disk. Prepared work must not overwrite a newer source state, and consumers must observe complete graph updates rather than partially repaired links or diagnostics. [Source authority](adr/source-authority.md) records the evaluation trade-off; [eager graph retention](adr/eager-derived-graph.md) records the memory trade-off supported by [prototype measurements](research/graph-index-performance.md).

Saved schema changes must preserve open source and publish a coherent re-extraction. Failed reloads retain the last valid view and visibly report the failure. The [LSP contract](contract/lsp.md#source-synchronization-and-watched-files) owns notification semantics; [System](SYSTEM.md#provider-implementation) describes generations and update methods.

## Consumers and repository boundaries

The engine owns archive semantics, its CLI, provider, language server, Typst defaults, bundled skills, and protocol tests. Editor clients own UI, editor integration, and transient presentation.

Protocol changes must land compatibly before a released client requires them. Clients are responsible for checking supported interfaces and arranging saved-file notifications.

Agent skills provide operating knowledge, not worker coordination, permissions, or live collaboration. Installed copies must remain usable independently of repository documentation. [Agent workers](adr/opt-in-agent-skills.md) records that boundary.
