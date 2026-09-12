# Archive

## Boundary

One directory is one archive, one ID namespace, and one (possibly not connected) link graph.

The archive is self-contained and relocatable. A root configuration file identifies it. All durable notes, assets, references, and archive-specific Typst code live beneath that root. Cloning or copying the directory preserves the complete archive.

The CLI and language server locate local archives by walking upward until they
find the root configuration. `--archive PATH` selects an exact archive root
without discovery.

## Layout

```text
zk.toml
zettel/
  2603231410.typ
lib/
  zettel.typ
```

Only three paths are canonical:

- `zk.toml`
- `zettel/`
- `lib/`

All Zettel share one flat directory with no subdirectories:

```text
zettel/*.typ
```

There is no semantic grouping by topic, project, or note type. Structure notes and ordinary notes live together.

Other user directories such as assets or bibliography data are allowed. Version one assigns them no archive semantics.

## Manifest

`zk.toml` is minimal and contains only the archive format version:

```toml
format = 1
```

Paths and contracts are fixed in version one and not configurable.

## Canonical vs derived state

The Zettel source files are the archive state. Version one keeps no persistent derived cache or index. The CLI and language server rebuild the relation graph and metadata table in memory from source.

## Library boundary

`lib/zettel.typ` is durable, source-controlled, and owned by the archive.

- `zk init` supplies an initial version but never silently replaces it.
- The ZK tool defines the semantic contract for archive markup such as `abstract`, `keywords`, and `category`.
- `lib/zettel.typ` defines how those constructs render.
- Indexing never executes or interprets the library.
- Changing presentation cannot change extracted metadata.
- Adding a machine-readable metadata construct requires a schema change, not merely a new Typst function.
- Migrations may update the library only through an explicit operation.

Version one hardcodes the contracts. They can become configurable later.
