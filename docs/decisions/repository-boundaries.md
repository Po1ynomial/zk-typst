# Repository boundaries

Status: accepted

## Decision

The Rust archive tool and Neovim plugin live in two independent Git
repositories:

```text
zk
zk.nvim
```

There is no Git submodule relationship and no umbrella repository. Local
full-stack development may use sibling checkouts and explicit paths to the
`zk` binary.

## Ownership

The `zk` repository owns:

- archive discovery, format, and migration;
- Zettel parsing and validation;
- provider and graph state;
- CLI commands and JSON output;
- the ZK language server;
- Typst templates and archive-local agent skills;
- the editor-neutral protocol contract;
- provider, CLI, and framed protocol tests.

The `zk.nvim` repository owns:

- all Lua code;
- Neovim LSP setup and client lifecycle;
- mappings, commands, completion filtering, navigation, and decorations;
- Tinymist workspace integration;
- Vim help and user-facing plugin documentation;
- headless Neovim integration tests.

After extraction, `zk` retains no Neovim-specific code, documentation, or
tests. `zk.nvim` does not parse Zettel metadata or maintain a competing archive
graph. Rust remains authoritative for archive semantics, derived relations,
diagnostics, queries, and protocol behavior. Lua owns editor workflow,
presentation, and transient interface state.

## Releases and compatibility

The repositories use independent semantic versions and release schedules.
Each `zk.nvim` release declares its minimum supported `zk` version. A plugin
change that does not require new engine behavior does not require a `zk`
release.

`zk lsp` reports an integer ZK protocol version and feature flags in its
initialization capabilities. `zk.nvim` rejects unsupported protocol versions
with a clear error and uses feature flags for optional behavior rather than
inferring capabilities from the executable version.

The CLI commands used by `zk.nvim` remain a documented stable interface.
Protocol additions land compatibly in `zk` before a released `zk.nvim` begins
to require them.

## Testing

`zk` tests its provider, CLI, and real language-server transport without
cloning or running `zk.nvim`.

`zk.nvim` owns full Neovim integration tests. Its compatibility matrix runs
against the minimum supported `zk` release and the latest release. A scheduled
test may also use `zk` main to detect upcoming incompatibilities, but it does
not gate ordinary `zk` development.

## Project docs and SDD

Each repository owns a complete local set of current project artifacts.
Plugin-specific knowledge from the current `PROJECT.md`, `DESIGN.md`,
`SYSTEM.md`, and decisions is factored into focused `zk.nvim` documents during
extraction. The plugin does not receive a wholesale copy of the mixed project
documentation.

The current `docs/.sdd/state.jsonl` remains in `zk` unchanged. It preserves the
history of work completed before the split, including earlier Neovim slices,
but accepts only `zk` work after extraction.

No SDD state is copied, filtered, synthesized, or initialized in `zk.nvim`
during extraction. The first SDD session whose working directory is
`zk.nvim` onboards from its local project artifacts and starts a fresh state.
Until then, the current working directory and SDD state remain focused on
`zk`.

Future cross-repository work is coordinated through protocol decisions,
engine commits, releases, and minimum-version declarations. The repositories
do not share slice IDs or an SDD database.

## History and extraction

The plugin repository was created from filtered `zk` history rather than a
fresh initial commit. The extraction preserves authors, timestamps, commit
messages, and plugin-owned changes for:

```text
.stylua.toml
after/
doc/
lua/
scripts/inspect-nvim.sh
scripts/inspect_nvim.lua
scripts/inspect_nvim_fallback.lua
```

Filtering necessarily creates new commit IDs. Mixed commits retain only their
plugin-owned changes, and commits with no remaining changes disappear from the
filtered history.

The extraction used `zk` commit `5891b57`, where protocol version reporting
and client validation had passed together. Its filtered plugin commit is
`5ec586a`. Commit `d169ef0` replaced the staging symlinks with an independent
plugin checkout, factored current plugin documentation, and made integration
tests consume an explicit `ZK_BIN`.

The `zk` repository keeps its complete past history and SDD log, then removes
plugin-owned files in an ordinary split commit.

## Rationale

The plugin is a separately installed, user-facing product whose interaction
code and release cadence will grow faster than the archive engine. Independent
repositories give each component a conventional package layout, focused
history, focused documentation, and independent releases.

A monorepo would preserve atomic cross-component commits and exact-revision
testing, but would keep packaging and release ownership mixed. The explicit
protocol contract and compatibility matrix provide the needed coordination
without requiring one release train.

Submodules add a third coordination layer without making changes across the
two product repositories atomic. Reproducible released pairs are already
expressed by the plugin's minimum version and test matrix.

## Consequences

- Cross-component features may require coordinated changes in two
  repositories.
- Breaking protocol changes require staged releases.
- Plugin installation no longer includes Rust source.
- Rust packaging no longer includes the Neovim runtime.
- Local integration scripts need explicit sibling checkout and binary paths.
- Protocol compatibility becomes an externally visible contract.
