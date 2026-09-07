# Global archive access

Status: accepted

## Decision

Archive selection keeps local discovery and adds explicit fallback mechanisms. There is no machine-level ZK configuration file and no environment-variable fallback.

### Selection precedence

Neovim selects an archive in this order:

1. the nearest `zk.toml` above the current buffer;
2. the session fallback archive;
3. no archive, which produces the existing error.

The session fallback is initialized through native plugin setup:

```lua
require("zk").setup({
  archive = "/absolute/path/to/archive",
})
```

The plugin exposes the selected fallback as readable session state. It expands `~` and environment variables, resolves relative values against Neovim's current working directory, canonicalizes the result, and validates the archive manifest and layout before use.

An invalid setup value reports an immediate error and does not start a fallback client. An invalid later switch leaves the existing fallback and its client unchanged.

### Session switching

```text
:ZkSetArchive [PATH]
```

With a path, the command validates and selects another fallback archive for the current Neovim process. It does not rewrite the user's setup or any file. Without a path, it reports the current fallback.

Switching stops the previous fallback client and eagerly starts one for the new archive. It does not stop clients serving open buffers from other locally discovered archives. Local buffer discovery continues to take precedence over the fallback after a switch.

### Language-server lifecycle

When `archive` is configured and valid, the plugin eagerly starts one unattached `zk lsp` client rooted at that archive. Global archive commands can therefore use live provider state before any Zettel buffer opens.

Opening a Zettel from the fallback archive attaches it to the existing client. Opening a Zettel beneath another `zk.toml` starts or reuses a client for that local root. Commands issued from a local archive use its client; commands issued elsewhere use the fallback client.

If a command selects a local archive that has no running client yet, the plugin starts that client on demand without attaching the unrelated current buffer.

This changes archive selection only. Search, checking, removal, diagnostics, navigation, completion, and decoration semantics remain unchanged.

The selected root makes these existing forms usable outside an archive buffer:

- `:ZkFind`
- `:ZkNew`
- `:ZkCheck`
- `:ZkDiagnostics`
- `:ZkRemove ID`

Backlinks, refresh, removal without an ID, and language features at the cursor still require a current Zettel.

### Explicit CLI selection

The CLI accepts an explicit global option:

```text
zk --archive PATH new
zk --archive PATH check
zk --archive PATH query ...
zk --archive PATH remove ID
zk --archive PATH graph --format json
zk --archive PATH lsp
```

An explicit path always wins over current-directory discovery. Without it, the CLI retains upward `zk.toml` discovery from the current directory. Relative paths resolve against the process working directory, and the selected path must be a valid archive.

`zk init [PATH]` remains the sole initialization form and rejects `--archive`. The CLI does not persist a personal archive path.

## Rationale

The primary archive belongs in the user's Neovim configuration because Neovim is the daily interface and already owns plugin settings. A separate ZK config file would duplicate configuration without helping the editor.

Local discovery must remain first so project archives, fixtures, and copied archives continue to work without changing personal settings. An eagerly running fallback client makes live search available globally while preserving independent live state for other open archives.

The CLI option provides equivalent explicit access for scripts and shell use without introducing hidden machine state.

## Consequences

- Neovim can create, find, check, diagnose, and explicitly remove Zettel without first changing directories or opening the archive.
- The configured fallback is process-local after setup and may be switched during a session.
- A local archive can temporarily take precedence without replacing the personal fallback.
- More than one `zk lsp` process may run when buffers from different archives are open.
- Plugin client lookup must distinguish roots instead of relying only on the current buffer.
- CLI tests must cover explicit selection, local discovery, invalid paths, and the `init` conflict.
- Headless Neovim inspection must cover eager startup, global commands, local precedence, and an atomic fallback switch.

## Deferred

Automatic Git initialization and initial commits remain a separate decision.
