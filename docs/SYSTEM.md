# System

## Implemented capabilities

The repository builds one Rust executable named `zk`.

`zk init [PATH]` creates the version-one archive layout without overwriting an existing manifest or canonical path:

```text
zk.toml
zettel/
lib/
  zettel.typ
```

The default path is the current directory. The generated manifest declares `format = 1`. The bundled Typst library renders the fixed metadata forms and intercepts ten-digit references.

`zk new` walks upward from the current directory to find `zk.toml`, validates archive format 1 and the canonical layout, then creates a Zettel under `zettel/`. The filename uses the current local minute in `YYMMDDHHmm.typ` form. If that ID exists, allocation advances by one minute until a free name is available. The command prints the new path relative to the archive root.

The generated Zettel has the required import, show rule, labelled level-one heading, abstract, keyword list, and category before an empty body.

## Code entry points

- `src/main.rs` defines the command-line interface and process exit behavior.
- `src/archive.rs` implements initialization, root discovery, manifest validation, layout validation, and collision-safe Zettel creation.
- `src/templates.rs` contains the canonical manifest, Typst library, and Zettel templates.
- `tests/cli.rs` exercises archive initialization and nested-directory authoring through the executable.

## Inspection

Run the automated checks:

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Inspect the complete authoring flow in a temporary directory:

```sh
tmp="$(mktemp -d)"
cargo build
zk_bin="$PWD/target/debug/zk"
"$zk_bin" init "$tmp/archive"
created="$(cd "$tmp/archive" && "$zk_bin" new)"
typst compile --root "$tmp/archive" "$tmp/archive/$created" "$tmp/zettel.pdf"
```

## Current limitations

Only `init` and `new` are implemented. The executable does not yet parse Zettel metadata or references, build a graph, check integrity, answer queries, remove notes, emit JSON snapshots, or run a language server. The Neovim adapter is also not implemented.

The initial category dictionary contains `thoughts`, `physics`, and `coding`. Category and keyword policy remain deferred product decisions. Archives may edit their user-owned `lib/zettel.typ`, but `zk` does not migrate it yet.
