# Explicit-path asset management

Status: accepted and implemented in zk 0.3.0, archive format 3, data schema 2, and ZK protocol 2

## Decision

Manage opaque files beneath `assets/ID/`, using the note's existing ten-digit ID. Create directories lazily on ingestion, not on initialization or note creation. Permit nested relative paths so auxiliary files can retain their layout.

Provide `zk asset add`, `zk asset list`, and `zk asset remove`. Addition requires an existing regular canonical saved note file, copies exactly one regular source file, defaults to its basename, and supports an explicit relative destination name. Refuse clobbering and traversal. Commands do not traverse symlinks in asset namespaces; an explicitly supplied source symlink may resolve to a regular file whose bytes are copied. Files remain user-owned and editable through ordinary tools.

Addition preserves bytes, not filesystem attributes. It does not chase dependencies, copy directories recursively, rewrite imports, insert note source, deduplicate content, or evaluate Typst. Stage complete bytes outside ID namespaces and publish without overwriting. Copy or publication failure must not leave a partial destination. Staging and destination must share a filesystem. Empty parent directories may remain; abrupt termination can leave staging directories. Crash recovery and protection against hostile concurrent directory replacement are not provided.

Listing recursively returns regular files in UTF-8 lexical namespace-relative name order through the existing schema-2 envelope. Missing namespaces return an empty list without creating asset directories. Unsupported entries or names fail listing rather than being silently omitted. Removal deletes exactly one named regular file, without wildcards, recursive deletion, or unused-file inference.

Listing and removal work after the associated note disappears. Note removal never automatically deletes assets because ordinary paths can be shared across notes. Namespace association is not exclusive ownership. Orphan-namespace warnings and asset-specific live queries remain deferred.

Authors use explicit native Typst paths, such as `/assets/2603231410/tiger.jpg`. Do not install a contextual resolver, operation-specific loader wrappers, generated ID binding, or special path syntax. Add/remove print archive-relative paths without the leading slash; editors may insert the corresponding escaped Typst string. Editor presentation remains outside this repository.

## Rationale

A shared directory alone leaves naming and organization to the user. Per-note namespaces make placement predictable and permit familiar local filenames without making asset files into graph nodes. Safe copying and structured enumeration are sufficient to support an editor picker or path insertion without owning the UI.

The [asset-resolution research](../research/asset-resolution.md) establishes that label-derived paths require Typst context. Transparent use with native loaders would need an early generated binding or custom machinery. Explicit paths are less clever, preserve ordinary Typst semantics, and remain stable because IDs are stable. The user selected this compromise rather than extending note creation or installing wrappers.

Asset paths may be computed or embedded in imported code. Syntax-only scanning cannot prove that a file is unused, and resolving arbitrary dependencies would cross the project's evaluator boundary. Explicit removal and retention are therefore safer than automatic cleanup.

## Compatibility and boundaries

Archive format 3 already permits auxiliary directories; `assets/` is optional and adds no manifest setting or extraction requirement. Asset entries are an additive CLI payload under data schema 2. Existing nodes, metadata, links, graph snapshots, and ZK protocol-2 capabilities remain unchanged. No asset query is advertised through LSP.

Asset commands validate the archive template but use saved filename identity without loading the note graph. Malformed selected note contents and unrelated unreadable notes do not prevent file management. Asset contents do not create nodes, metadata, or authored reference occurrences. Tinymist owns ordinary Typst loading diagnostics.

The authoritative invocation, path safety, streams, and lifecycle rules live in the [CLI contract](../contract/cli.md#asset-management). The asset entry shape lives in the [shared data contract](../contract/data.md#asset). This decision records ownership and rationale rather than a competing wire definition.

## Evidence

- `src/assets.rs` implements shared Archive methods for copying, listing, removal, relative-name validation, and symlink-free traversal.
- `src/main.rs` exposes the three asset commands with existing archive selection and schema-2 serialization.
- `tests/assets.rs` covers opaque bytes, nested paths, no clobbering, exact removal, retained orphan namespaces, relocation, deterministic ordering, invalid IDs/names/sources, source links, and destination/note symlink rejection.
- The failed-copy unit seam verifies that interrupted reads and publication collisions do not leave partial assets or staging files.
- `tests/fixtures/schema2/assets.json` preserves the additive listing payload. Asset ingestion tests compare graph snapshots before and after copying to verify unchanged graph semantics.
