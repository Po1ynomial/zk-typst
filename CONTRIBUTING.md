# Contributing

`zk` is a single-maintainer engine with external consumers: archive users, editor clients, and agent workers operating installed skills. This guide explains how to report problems, request changes, and contribute code so that requests can be verified quickly.

## Before filing

Check the [compatibility table](docs/contract/README.md) first. Executable, archive format, data schema, and editor-protocol versions are independent, and reports that name them concretely avoid a round trip. The [system guide](docs/SYSTEM.md) lists implemented capabilities and known limitations, and the [project scope](docs/PROJECT.md) records accepted non-goals such as publishing, aggregate compilation, evaluated metadata, agent orchestration, and a custom query language.

## Reporting

Use the issue templates when they fit. Blank issues remain open as an escape valve; every new issue receives the `needs-triage` label automatically and moves through `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, or `wontfix`.

For bug reports, include `zk --version`, the affected interface, and a reproduction that works from an empty directory. Behavior is verified against the [CLI](docs/contract/cli.md), [LSP](docs/contract/lsp.md), and [data](docs/contract/data.md) contracts: a report contradicting a contract is a bug, while behavior the contracts permit is usually a `wontfix` or an enhancement request.

Enhancement requests should describe the problem and use case before the proposed mechanism. A request that conflicts with a non-goal is still welcome, but it must explain what changed or why the constraint should be revisited.

Usage questions that reveal documentation gaps are treated as documentation improvements rather than defects.

## Code changes

Open an issue or discuss before investing in a large diff; pull requests are triaged like issues with attached code. Describe the change with a summary view and evidence, and assess merge danger using the pull-request template. Keep changes contract-compatible, and land editor-protocol changes before a released client depends on them. Run `just check` before pushing, and update contracts, guides, and ADRs in the same change when behavior or accepted decisions move. Rust and Markdown checks do not rewrite files; run `cargo fmt` explicitly when formatting is intended.

## Triage notes

Triage comments on issues and pull requests are generated with AI assistance and marked with a disclaimer. Label meanings:

| Label | Meaning |
| --- | --- |
| `needs-triage` | Received; awaiting maintainer evaluation |
| `needs-info` | Waiting on the reporter for reproduction or detail |
| `ready-for-agent` | Specified with acceptance criteria; agent-ready |
| `ready-for-human` | Specified; requires human judgment or access |
| `wontfix` | Rejected or already implemented; see the closing comment |
| `bug` / `enhancement` | Category labels paired with one state label |
