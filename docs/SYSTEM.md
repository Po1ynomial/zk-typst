# System

This guide describes implemented architecture, practical workflows, inspection paths, and limitations.
The repository builds the Rust executable `zk` and exposes its provider as a reusable library.

## Implemented capabilities

- Initialize archives and create Zettel from a user-owned Typst template.
- Extract metadata values from configured occurrences, authored links, backlinks, and diagnostics without evaluating Typst.
- Search saved metadata and emit a complete saved graph as JSON.
- Check archive integrity and guard note removal against incoming references.
- Copy, list, and explicitly remove opaque note-associated files.
- Serve reference completion, hover, navigation, backlinks, metadata search, and archive-specific diagnostics against live editor source.
- Optionally install a self-contained archive operating skill.

[CLI](contract/cli.md), [LSP](contract/lsp.md), and [shared data](contract/data.md) contracts own command and protocol specifications. [Design](DESIGN.md) owns shared invariants rather than code structure.

## Archive authoring

Initialization creates this minimal layout:

```text
zk.toml
zettel/
lib/
  zettel.typ
templates/
  zettel.typ
```

The template supplies a labelled title and imports the reference library. Neither file installs abstract, keyword, or category helpers. Both become user-owned. Initialization does not invoke Git.

A basic saved-state workflow is:

```sh
zk init ~/notes
zk --archive ~/notes new
zk --archive ~/notes query search "untitled"
zk --archive ~/notes check
```

Edit the path printed by `new` in your editor. Read the archive's template before changing metadata. [The optional descriptive example](../examples/templates/descriptive.typ) supplies additional fields and rendering helpers; copying it to the archive template enables those conventions. Existing Zettel are not rewritten.

Archive loading validates the saved template. If it is missing, loading creates the minimal core, including during inspection and language-server startup or reload. An invalid template fails explicitly instead. This filesystem-writing behavior is important when inspecting old or incomplete archives. Exact loading and creation rules live in the [CLI source contract](contract/cli.md#initialization-and-source-declarations).

Opt-in skill installation copies [the bundled skill](../skills/zettelkasten/SKILL.md) into the archive. Conflicts and installation failures warn without failing canonical archive creation. Installed copies remain user-owned and are not refreshed by subsequent commands. Ordinary initialization does not install skills.

## Queries and integrity

Use metadata search to find entry points, then inspect a selected Zettel's outgoing links and backlinks. Search does not inspect arbitrary body text; the [shared matching rule](contract/data.md#metadata-search) defines its inputs and behavior. Broad searches may produce large results.

Queries and graph output return available data even when notes contain integrity diagnostics. A check distinguishes those diagnostics from fatal loading errors. Guarded removal reports blocking authored locations instead of unlinking or rewriting other Zettel. Exact results and failure behavior belong in the [CLI inspection contract](contract/cli.md#inspection-and-json-results).

Saved-state queries do not contact a running editor session. Use the editor's LSP session when unsaved source matters.

## Asset workflow

Copy a supporting file, then insert its ordinary Typst path in the Zettel:

```sh
zk asset add <ID> diagram.svg
zk asset list <ID>
```

```typst
#image("/assets/2603231410/diagram.svg")
```

The copy operation does not insert source, rewrite imports, or collect helper dependencies. The printed archive-relative path needs a leading slash when used as a Typst project-root path. Supply dependencies separately.

Asset commands validate the saved template through `Archive::open` in `src/archive.rs`. `src/assets.rs` validates IDs and filesystem paths, and addition checks saved filename identity without loading the note graph. It stages bytes outside note namespaces before no-clobber publication. Assets remain editable files, do not enter the graph, and survive note removal. Listing and explicit removal also work for orphan namespaces. Path safety, supported inputs, and result shapes are specified by the [asset contract](contract/cli.md#asset-management) and [asset data definition](contract/data.md#asset).

## Provider implementation

`Provider::load` reopens the archive, discovers canonical note paths, parses them concurrently with the pinned `typst-syntax` dependency, extracts metadata and references, and builds adjacency and diagnostics. Initial loading is synchronous for graph-consuming commands and for the language server.

The retained representation interns IDs to compact integer indexes and groups outgoing occurrences with byte ranges. It keeps incoming adjacency and all node metadata, but discards source text and syntax trees for closed files. Open buffers retain `typst_syntax::Source` values. The [graph benchmark](research/graph-index-performance.md) measured a prototype, not this executable. Executable measurements at the 50,000-Zettel stress case and acceptance thresholds for startup time, memory, and live-update latency remain deferred.

The provider's update methods implement live source ownership:

- `open_buffer` installs full source and a document version.
- `change_buffer` accepts newer versions and reparses through `Source::replace`.
- `save_buffer` retains the overlay; `refresh_disk` ignores open paths.
- `close_buffer` drops the overlay and reloads disk, or removes an unsaved node.

Generations reject delayed results prepared for superseded source. Applying an update replaces the node's outgoing links, repairs incoming adjacency and resolution, and rebuilds affected diagnostics before publishing a revision.

`reload_schema` validates the saved archive and prepares extraction for closed notes and open source before changing live state. On success it invalidates older prepared work and retains open document versions. Failed reads or invalid rules leave the previous view intact. Starter-text changes that preserve compiled rules do not alter graph semantics. External synchronization requirements are defined in the [LSP contract](contract/lsp.md#source-synchronization-and-watched-files).

## Language-server implementation

`src/lsp.rs` adapts standard-input/output JSON-RPC to one provider session. It translates document events into provider updates and handles saved note, manifest, and template notifications. Failed schema reloads are logged and visibly reported while the session continues serving its last valid view.

Archive services apply to canonical notes, not ordinary library or asset buffers. The server publishes archive-specific diagnostics for open Zettel; generic Typst syntax diagnostics remain in saved graph output and checks. Tinymist supplies general Typst intelligence and dictionary-member completion.

For positions, the adapter uses retained text for open source and reads saved text for each closed-file location conversion. Protocol discovery, request names, encoding negotiation, errors, and diagnostic payloads are specified in the [LSP contract](contract/lsp.md).

## Editor setup

Use an LSP client capable of attaching both `zk` and Tinymist to Typst notes. The client starts separate standard-input/output processes:

```sh
zk --archive /absolute/path/to/notes lsp
tinymist lsp
```

These are client launch commands, not interactive shell services. Alternatively, launch `zk lsp` with its working directory inside the archive to use discovery. Selecting an archive for `zk` does not configure Tinymist, and LSP workspace parameters do not switch `zk` to another archive.

Set the client workspace to the archive root and configure Tinymist's Typst project root to that same directory, using its `tinymist.rootPath` setting through the client's configuration mechanism. The default relative library import and project-root asset paths depend on this choice. A workspace directory and a compiler root are separate settings; do not assume the client sets both. Tinymist owns [its compiler configuration](https://myriad-dreamin.github.io/tinymist/feature/compiler-settings.html).

The client must synchronize open note buffers and deliver saved-file notifications for notes, the manifest, and the template. Enable dynamic watched-file registration when supported, or arrange equivalent notifications in the client. The [LSP contract](contract/lsp.md#source-synchronization-and-watched-files) owns exact events and source precedence. Verify compatibility through [version discovery](contract/README.md#versions-and-compatibility), not executable-version assumptions.

Use archive completion and navigation from `zk`, and general Typst intelligence from Tinymist. The client owns request routing and how results from both servers are presented.

## Code entry points

- `src/main.rs`: CLI arguments, JSON output, and exit behavior.
- `src/archive.rs`: initialization, discovery, validation, creation, skills, and note removal.
- `src/assets.rs`: asset copying, recursive listing, exact removal, and path validation.
- `src/config.rs`: manifest and bounded metadata matcher types.
- `src/template.rs`: comment declarations, starter validation, and exact creation edits.
- `src/extract.rs`: configured metadata, references, authored ranges, and syntax diagnostics.
- `src/model.rs`: public values, envelopes, and diagnostic ordering.
- `src/provider.rs`: retained graph, open source, stale-work rejection, and snapshots.
- `src/lsp.rs`: transport adapter, synchronization, navigation, queries, and diagnostics.
- `src/templates.rs`: initialization defaults and bundled skill registry.
- `skills/zettelkasten/SKILL.md`: bundled operating instructions included in the executable.
- `tests/cli.rs`: authoring, search, checks, guarded removal, skills, and JSON fixtures.
- `tests/assets.rs`: opaque copying, no-clobber behavior, orphan cleanup, relocation, and filesystem boundaries.
- `tests/provider.rs`: overlay lifecycle, schema reloads, and coherent graph updates; provider unit tests cover individual transitions and stale work.
- `tests/lsp.rs`: real framed transport, positions, capabilities, requests, reloads, diagnostics, and process lifecycle.
- `tests/fixtures/schema2/`: shared CLI/LSP values and source inputs. Asset fixtures are CLI-only.

## Development tools

Development checks use Rust 1.94.0 with Rustfmt and Clippy, `just` 1.58.0, and `rumdl` 0.2.78. `rust-toolchain.toml` pins the development toolchain, not the executable's minimum supported Rust version.

With Rustup installed, set up tools from the repository root:

```sh
rustup show active-toolchain
cargo install --locked just --version 1.58.0
cargo install --locked rumdl --version 0.2.78
```

Rustup installs the configured toolchain and components when first used. Recipes use a POSIX shell and support development on Linux and macOS.

## Inspection

Run the same source-preserving checks used by CI:

```sh
just check
```

This runs format checks, Clippy, tests, and Markdown lint. `just` without arguments also runs `check`. Individual recipes are `just fmt-check`, `just lint`, `just test`, and `just docs`; the test recipe accepts Cargo arguments for focused investigation. Each check captures output once, prints a short success line or complete failure output, preserves the exit status, and removes temporary logs.

The underlying commands are:

```sh
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked
rumdl check README.md GLOSSARY.md docs skills
```

Checks do not rewrite Rust or Markdown. Cargo verification uses `--locked` to preserve dependency resolution. Run `cargo fmt` explicitly when formatting changes are intended. Package inspection is available through `cargo package --list --allow-dirty`.

To inspect generated skills and metadata search in a disposable archive:

```sh
archive="$(mktemp -d)"
cargo run -- init --agent-skills "$archive"
cargo run -- --archive "$archive" new
cargo run -- --archive "$archive" query search "untitled"
```

These commands create files in the disposable archive. Normal integration tests use isolated temporary archives. LSP tests send framed messages over the real transport with request and exit timeouts and reap the server on failures.

## Continuous integration

[CI](../.github/workflows/ci.yml) runs `just check` on Ubuntu 24.04 for pull requests, pushes to `main`, and manual dispatch. It uses the repository toolchain and the same pinned `just` and `rumdl` versions. Tools come from prebuilt releases with checksum verification; actions are pinned to commit SHAs.

The job uses read-only permissions, does not persist checkout credentials, caches dependencies, restricts cache writes to `main`, cancels superseded runs, and has a 20-minute timeout. It does not publish releases or run a macOS or Windows matrix. A separate [triage workflow](../.github/workflows/issue-triage.yml) applies the `needs-triage` label to opened and reopened issues with write access scoped to issues. Tool updates must keep workflow pins, the toolchain file, and setup instructions synchronized.

Issue and pull-request templates under `.github/` route reports toward the contracts and scope. [CONTRIBUTING.md](../CONTRIBUTING.md) explains the reporting rules and triage labels.

## Current limitations

- The initial graph loads before the server serves requests; background readiness is not implemented.
- Clients must arrange saved-file notifications when dynamic watched-file registration is unavailable. Closed-file location conversions do not group reads or refresh graph state, so missing notifications can leave graph ranges stale relative to saved text.
- Diagnostics are pushed for open Zettel only. Archive-wide saved inspection remains a CLI operation.
- Only the current archive format is accepted. There is no migration command or older-format compatibility layer.
- Source matching does not follow bindings, imports, aliases, computed values, or nested metadata. Supported direct forms are enumerated in the [declaration contract](contract/cli.md#declaration-comments).
- There is no asset-specific LSP service, orphan warning, unused-file detector, recursive directory importer, or automatic cleanup.
- Asset staging and destination must share a filesystem. Abrupt termination can leave staging directories; crash recovery and hostile concurrent directory replacement are not managed.
- The descriptive template's category dictionary is presentation code. Tinymist owns its completion, and archive vocabulary policy remains deferred.
