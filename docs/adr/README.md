# Decisions

These accepted decisions record context, choices, and rationale. [Design](../DESIGN.md) describes their shared invariants, [System](../SYSTEM.md) describes implementation and limitations, and [contracts](../contract/README.md) define exact external behavior. [Research](../research/README.md) indexes the supporting experiments.

## Archive and authoring

- [Self-contained plain-source archives](plain-source-archive.md): canonical files beneath one root instead of a database or persistent derived index.
- [Filename identity and local timestamp IDs](filename-identity.md): permanent addresses independent of titles, including for malformed drafts.
- [Native references and an archive namespace](native-reference-namespace.md): ordinary Typst references with retained authored occurrences and grouped relationships.
- [User-declared metadata instead of fixed fields](generic-metadata.md): archive-selected attributes without engine-defined roles.
- [Template-declared metadata](template-declared-metadata.md): starter values and tracking rules together in user-owned Typst source.
- [Explicit-path asset management](explicit-asset-paths.md): opaque files, native paths, and explicit lifecycle without inferred cleanup.

## Provider and consumers

- [Source rather than evaluated metadata is authoritative](source-authority.md): malformed-buffer recovery and authored ranges without compilation.
- [Use the real Typst parser with explicit version coupling](typst-parser.md): one parser interpretation with deliberate upgrade testing.
- [Retain the graph, not closed syntax trees](eager-derived-graph.md): complete relationships without retaining expensive closed-file trees.
- [Scriptable operations instead of an engine-owned UI](scriptable-cli.md): noninteractive operations and guarded removal without automatic source edits.
- [Editor-neutral companion language server](companion-language-server.md): live archive semantics independent of editor presentation and general Typst intelligence.
- [Share versioned archive values across transports](shared-archive-values.md): common data meanings with independent interface-version discovery.
- [Opt-in operating skills instead of agent orchestration](opt-in-agent-skills.md): user-owned archive knowledge without worker coordination or a separate mutation protocol.
