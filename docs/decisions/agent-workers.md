# Agent workers

Status: accepted. The standard-template creation description below is superseded by user-owned archive templates in [Configurable source contracts](source-contracts.md). The raw search-result array below records schema 1; the implemented [CLI contract](../contract/cli.md) instead specifies schema-2 envelopes.

## Decision

Agent workers may use `zk` for archive queries and whole-file lifecycle
operations. `zk` does not coordinate workers or act as the primary
collaboration channel. External editor tooling may coordinate agents with live
buffers.

### Archive-local skills

Ordinary archive initialization remains agent-neutral. The boolean
`--agent-skills` option installs the complete bundled skill set:

```text
zk init --agent-skills [PATH]
```

Skills live under:

```text
<archive>/.agents/skills/<name>/SKILL.md
```

The initial set contains `zettelkasten`. Its skill explains the source
contract, the available `zk` commands, and a short writing method:

- search before creating a Zettel;
- keep one principal idea in each Zettel;
- inspect related notes and backlinks;
- explain links in prose;
- preserve the fixed metadata and reference forms;
- run `zk check` after changes.

The skill does not prescribe a category or keyword policy.
Its methodological background adapts
[Introduction to the Zettelkasten Method](https://zettelkasten.de/introduction/)
to this archive's Typst source and reference conventions.

Bundled skill templates are plain Markdown files in the source repository.
Rust includes those files in the executable rather than embedding their text
in string literals.

Skill installation is best-effort. If a bundled skill's destination already
exists, `zk init` leaves it untouched and prints a warning. Other skill
installations continue. A skill installation failure does not fail or roll
back canonical archive initialization.

Installed skills are user-owned immediately. Later `zk` commands do not
validate, update, remove, or otherwise manage `.agents/`. Other additional
files and external state, including Git state, also remain untouched after
their explicit creation or setup.

### Metadata search

The CLI adds:

```text
zk query search <QUERY>
```

Search compares the query case-insensitively against Zettel IDs, projected
titles, projected abstracts, keywords, and categories. Matching uses
deterministic substring containment rather than typo-tolerant scoring. The CLI
and LSP share this matching rule.

The command returns every matching `ZettelNode` as one JSON array in provider
ID order. It has no implicit result cap and no separate compact result schema.
The `zettelkasten` skill tells workers to use specific terms, inspect or filter
results with tools such as `jq`, and avoid broad or empty searches unless the
task requires them.

### Whole-file lifecycle

`zk new` remains an ID allocator that writes the standard complete Zettel
template and prints its path. `zk remove` remains a guarded whole-file removal
that refuses deletion while incoming references exist.

Agents use the same operations. `zk` does not add initial-content input,
field-level updates, source replacement, patch application, or an
agent-specific mutation protocol.

## Rationale

Archive-local skills give workers the method and command knowledge they need
without affecting unrelated agent sessions. A skill directory allows more
focused skills to be added later, unlike one archive-wide instruction file.
Opt-in installation keeps ordinary archives independent of agent tooling.

Direct metadata search is useful to humans, scripts, and agents, so it belongs
in the general CLI rather than an agent integration. Reusing the LSP matcher
avoids two meanings of archive search. Complete deterministic results preserve
ordinary shell composability.

Live collaboration already has an external editor path. Adding shared
sessions, write coordination, or structured edits to `zk` would duplicate that
path and enlarge the archive manager without improving its core data model.

## Consequences

- A user must opt in when initializing an archive to receive bundled skills.
- Skills travel with their archive and can diverge after installation.
- A newer `zk` does not silently refresh an older installed skill.
- Existing same-name skills survive repeated or conflicting setup attempts.
- Broad metadata searches may emit large JSON arrays.
- Workers can discover notes without loading and filtering the complete graph.
- Worker orchestration, permissions, review, and live-buffer coordination
  remain outside `zk`.
