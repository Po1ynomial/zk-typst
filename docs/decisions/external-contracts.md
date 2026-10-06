# Extensible metadata and external contracts

Status: accepted and implemented in zk 0.2.0, archive format 2, data schema 2, and ZK protocol 2

The generic data/schema and protocol decisions remain current. The format-2 manifest declarations and initializer seed rules below are historical, superseded by [Template-declared metadata](template-schema.md), implemented in zk 0.3.0 with archive format 3.

## Decision

Maintain the external CLI and LSP contracts under [docs/contract](../contract/README.md). The two transports share one [data schema](../contract/data.md). These documents define versioned behavior and types; current implementation status remains in [System](../SYSTEM.md), and cross-slice design remains in [Design](../DESIGN.md).

Replace the fixed abstract, keywords, and category slots with a map of arbitrary user-declared metadata fields. Keep filename identity, one labelled title, and literal ten-digit references as fixed archive structures. Source matching remains bounded and direct top-level, without Typst evaluation or binding resolution.

The initializer must write the default metadata rules explicitly alongside the initial user-owned template. Runtime extraction must not restore removed fields or silently supply a field schema when no metadata definitions are present. Adding or removing fields is configuration, not a public data-schema change, and does not rewrite source, templates, or libraries.

Metadata values use `kind` and `value` with initial kinds `markup`, `string`, and `string-list`. Kinds describe output values independently of source matching forms. A configured but absent, malformed, or repeated field is `null`; malformed and repeated declarations also produce diagnostics. Unconfigured fields are not emitted. Authored empty values remain distinct from absence.

Use data schema 2 envelopes for archive JSON results over both CLI and LSP query commands. Standard LSP messages are not wrapped. ZK protocol 2 advertises `dataSchemaVersion: 2` independently of the executable and archive versions. Diagnostic codes classify problems; the custom field name is carried separately rather than embedded in the code.

Hover presents metadata generically in deterministic field-name order. Completion uses title and ID, without a privileged abstract field. No metadata name implicitly controls grouping, lifecycle, graph semantics, or editor presentation. Tinymist retains ordinary Typst language intelligence.

## Compatibility

The stable core comprises identity, paths, title, directed links, authored ranges, diagnostic severity, and session-local revision meaning. Consumers ignore unknown object properties and handle arbitrary metadata names. They preserve unfamiliar metadata kinds when forwarding values, and may omit them from presentation rather than coercing them. Unknown diagnostic codes remain actionable through severity.

Optional properties, field names, diagnostic codes, and supplementary metadata kinds can extend a supported schema when existing meanings remain unchanged. Required-property removal, incompatible value representations, changed core semantics, or changed range units requires a breaking data-schema version. Breaking editor request/response semantics requires a ZK protocol change; optional capabilities can use feature flags.

Output tolerance does not permit engines to ignore extraction rules they cannot interpret. Unsupported or ambiguous source forms must fail clearly before indexing. After deployment, breaking archive formats require explicit, reviewable migration. Indefinite historical-schema support and universal down-conversion are not promised.

Archive format 2 is not deployed yet and may be finalized in place before publication. The accepted target remains the 0.2.0 release, archive format 2, data schema 2, and ZK protocol 2. This does not establish a policy of changing deployed versions without migration.

## Rationale

The initial configurable contract changed source spellings for three predetermined fields but left their retrieval, search, and presentation hard-coded. Its runtime defaults also made deleting a declaration restore the field instead of disabling it. This was insufficient for a user-owned schema.

A generic typed map lets an archive add or remove fields without changing the node's structural contract. A bounded set of AST forms preserves precise authored ranges and malformed-source recovery without an evaluator. Shared payload definitions prevent CLI and LSP consumers from acquiring incompatible views of archive semantics.

Explicit version discovery and extension rules reduce ordinary compatibility work but do not make fundamental semantic changes free. Contract fixtures and supported-release client tests should guard deployed interfaces.

## Consequences and implementation

The development executable implements the contracts with explicit initializer rules, a generic metadata map, typed values, shared schema-2 envelopes, separate diagnostic field properties, and protocol-2 capability discovery. Shared extraction, retained values, search, creation checks, manifest reloads, and both adapters use the same semantics. Regression coverage is part of the normal test suite, including shared wire fixtures and real-transport LSP behavior.

At the time of this decision, the earlier development implementation still had three fixed metadata slots, implicit defaults, schema 1, and protocol 1. That pre-deployment interface has been replaced without a migration or downgrade layer. The independently maintained `zk.nvim` client will need to adopt the versioned contracts in its own repository; editor-specific changes do not belong here.

This decision supersedes the fixed-field, hidden-default, and schema/protocol-1 portions of [Configurable source contracts](source-contracts.md), while preserving its relaxed placement, user-owned templates, recovery, overlays, and Tinymist boundary. Historical source and interface decisions remain recorded and linked rather than deleted.

## Evidence

- The earlier versions of `src/templates.rs` and `src/config.rs` seeded a template without metadata tables and restored omitted rules from compiled defaults. This motivated explicit user ownership.
- The earlier model, search, and LSP code used dedicated abstract, keyword, and category slots. This motivated generic retrieval and presentation.
- `src/model.rs` now defines the shared schema-2 envelope, typed values, metadata map, and diagnostic field identity. CLI and LSP adapters use that envelope rather than maintaining separate result shapes.
- `tests/fixtures/schema2/` preserves node and graph wire examples, source/configuration inputs, and diagnostic shape fixtures used by the normal suites.
- `tests/cli.rs`, `tests/provider.rs`, and `tests/lsp.rs` cover generic fields, removal without restoration, exact ranges and null/empty semantics, atomic failed/successful reloads, stale updates, strict query errors, and real-transport protocol-2 behavior.
