# Git lifecycle

Status: accepted

## Decision

`zk init` treats Git as supplementary archive setup.

After creating the fixed archive layout, it attempts Git setup when a `git`
executable is available in `PATH`:

1. run `git init` in the archive root;
2. stage only `zk.toml` and `lib/zettel.typ`;
3. commit only those two paths with
   `chore: initialize zettelkasten archive`.

The path-limited commit must not include or unstage unrelated index entries in
an existing repository.

`zk` does not inspect parent repositories or special-case an existing
repository at the archive root. It does not select an initial branch name,
change Git identity or signing configuration, bypass hooks, add ignore rules,
or manage later commits. Git's configuration controls those behaviors.

The empty `zettel/` directory is not represented in the commit. `zk init` does
not create `.gitkeep`, `.gitignore`, or another placeholder. The user owns any
policy for preserving that directory in clones.

If `git` is absent from `PATH`, initialization succeeds without a warning and
without Git state. If any invoked Git command cannot start or exits
unsuccessfully, initialization still succeeds and prints a warning to standard
error. It leaves the archive and resulting Git state intact rather than
rolling back files or index changes.

Successful initialization keeps standard output limited to the archive path,
whether Git setup runs or not.

## Rationale

The archive remains useful without Git, so a missing identity, a hook failure,
or another local Git problem should not block archive creation. Attempting
`git init` unconditionally when Git is available gives a new archive useful
history without adding repository-discovery policy.

Restricting both staging and committing to the two durable generated files
avoids capturing unrelated files or staged work when the target directory
already contains a repository. Leaving branch names, parent repositories, and
empty-directory conventions to the user keeps `zk` out of general repository
management.

## Consequences

- A typical new archive starts with one commit containing `zk.toml` and
  `lib/zettel.typ`.
- An archive created without Git has the same canonical source layout.
- Initialization may succeed with no commit when local Git configuration or
  hooks reject it.
- A failed Git step may leave a repository or generated files staged for the
  user to inspect and commit.
- Initial branch names vary with the user's Git configuration.
- Cloning the initial commit alone does not create `zettel/`; the user must
  choose an empty-directory convention if that matters.
- Initializing inside a parent repository may create a nested repository.
