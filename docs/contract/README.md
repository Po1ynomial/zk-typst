# External contracts

These documents define the external interfaces owned by `zk`:

- [CLI](cli.md): invocation, archive selection, note/asset lifecycle, streams, exit statuses, and command results.
- [LSP JSON contract](lsp.md): standard LSP behavior, ZK capabilities, archive-query commands, and diagnostic data.
- [Shared data schema](data.md): the JSON values used by both interfaces. This is a shared definition, not a third transport.

## Status

These contracts are implemented in the 0.3.0 development executable. [System](../SYSTEM.md) describes flows and limitations; [Design](../DESIGN.md) describes the shared design. Asset policy is recorded in [Explicit-path asset management](../decisions/asset-management.md). The metadata rationale is recorded in [Template-declared metadata](../decisions/template-schema.md) and [Extensible metadata and external contracts](../decisions/external-contracts.md).

| Contract | Implemented version | Scope |
| --- | --- | --- |
| Executable | 0.3.0 | Development release before deployment |
| Archive format | 3 | A regular Typst template declares tracked fields through comments |
| Public data schema | 2 | Shared envelopes, generic typed metadata, and additive CLI asset-list entries |
| ZK editor protocol | 2 | Independent `dataSchemaVersion: 2` discovery |

Archive format 3 intentionally replaces the undeployed format 2. There is no automatic migration or schema-1 downgrade. Public data schema 2 and ZK protocol 2 remain unchanged because the source declaration authority changed, not the retrieved values or request semantics.

## Authority and scope

These contracts are authoritative for the implemented external interfaces. Historical decisions remain under `docs/decisions/`; duplicated wire definitions in design guides should be replaced by links here. Runtime limitations and any future implementation gaps belong in `docs/SYSTEM.md`.

The CLI and LSP use the same archive semantics and data definitions through the provider. They differ in source visibility: CLI queries read saved state, while an LSP process owns its open-buffer overlays. Neither interface compiles or evaluates Typst, rewrites user-owned presentation code, or manages an editor.

A compatible implementation must satisfy the specified behavior. Examples illustrate it; JSON object-property order, whitespace, human-facing messages, and presentation formatting are not contracts unless explicitly stated.

## Compatibility rules

The executable version, archive format, data schema, and ZK protocol are independent. Clients must check the versions relevant to the interface they consume rather than infer them from another version.

Within a supported data schema or ZK protocol:

- Consumers must ignore unknown object properties and feature flags.
- Consumers must handle arbitrary metadata field names. A name does not imply an engine-defined role.
- Consumers must preserve unfamiliar metadata values when forwarding data, and may omit them from presentation if their `kind` is unsupported. They must not coerce an unfamiliar kind into a known one.
- Unknown diagnostic codes must still be handled according to their severity. Message text is not a machine-readable error identifier.
- New optional properties, metadata field names, diagnostic codes, or supplementary metadata kinds are additive when existing meanings remain unchanged.
- Removing required properties, changing existing value representations or meanings, changing core identity or link semantics, or changing range units requires a breaking data-schema version.
- Breaking ZK request, response, or capability semantics requires a ZK protocol version change. Adding an optional capability may instead use a feature flag.

Output tolerance is not permission to guess extraction semantics. An engine must reject unsupported or ambiguous source-matching rules before indexing under them. New supported forms may extend the bounded matching vocabulary without changing existing forms or output kinds; an archive using such a form requires an engine that supports it.

After deployment, breaking archive-format changes require explicit, reviewable migration. Supporting every historical schema indefinitely or automatically down-converting arbitrary values is not promised. Compatibility should be verified with contract fixtures and client tests against the releases a client claims to support. Schema-2 fixtures are maintained in `tests/fixtures/schema2/`. The normal CLI, provider, and real-transport LSP suites cover version-2 output, generic retrieval, removal and reload of fields, diagnostic data, request errors, and lifecycle behavior.
