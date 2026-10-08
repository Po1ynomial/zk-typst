# External contracts

These documents define the external interfaces owned by `zk`:

- [CLI](cli.md): invocation, archive selection, note/asset lifecycle, streams, exit statuses, and command results.
- [LSP JSON contract](lsp.md): standard LSP behavior, ZK capabilities, archive-query commands, and diagnostic data.
- [Shared data schema](data.md): the JSON values used by both interfaces. This is a shared definition, not a third transport.

## Versions and compatibility

These are the currently implemented versions, not interchangeable release numbers:

| Version | Current value | Discovery and authoritative definition |
| --- | --- | --- |
| Executable | Package version in [Cargo.toml](../../Cargo.toml) | `zk --version` or LSP `serverInfo`; [CLI discovery](cli.md#invocation-and-archive-selection) |
| Archive format | 3 | Saved `zk.toml`; [source contract](cli.md#initialization-and-source-declarations) |
| Public data schema | 2 | JSON `schema_version` or LSP `dataSchemaVersion`; [envelope](data.md#envelope) |
| ZK editor protocol | 2 | LSP `capabilities.experimental.zk.protocolVersion`; [initialization](lsp.md#initialization-and-version-discovery) |

Only the current archive format is accepted; no migration command or older-format compatibility layer is implemented. Consumers must check data and editor-protocol versions independently of the executable version. The linked contracts define rejection behavior and feature discovery.

## Authority and scope

These contracts are authoritative for the implemented external interfaces.

The CLI and LSP use the same archive semantics and data definitions through the provider. They differ in source visibility: CLI queries read saved state, while an LSP process owns its open-buffer overlays. Neither interface compiles or evaluates Typst, rewrites user-owned presentation code, or manages an editor.

A compatible implementation must satisfy the specified behavior. Examples illustrate it; JSON object-property order, whitespace, human-facing messages, and presentation formatting are not contracts unless explicitly stated.
