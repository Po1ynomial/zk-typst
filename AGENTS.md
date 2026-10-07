# Agents

## Context

Durable project knowledge is maintained under `docs/`.

- `docs/PROJECT.md`: background data of the project: current goals, no-goals, scope, constraints, success conditions.
- `docs/DESIGN.md`: current cross-slice behavioral or technical design. Create when multiple slices need a shared target.
- `docs/SYSTEM.md`: current implemented capabilities, entry points, flows, inspection commands, checks, and limitations. Create when existing documentation does not provide this guide.
- `docs/contract/`: authoritative external CLI and LSP contracts with shared data definitions. State their version and implementation status; link them from guides rather than duplicating wire schemas.
- `docs/decisions/slug.md`: durable records of settled choices that a future agent could reopen, contradict, or misunderstand.
- `docs/research/slug.md`: durable findings and evidence that are costly, external, uncertain, or likely to be reused. For example `compare-tesseract-and-cloud.md`. They should refer to the spike names but not link to exact content, else the remote Markdown links will be broken.
- `spikes/XX-slug/`: curated spike artifacts, often with one off code. They are not tracked by git. Keep the public results in the research Markdown notes.

Rules:

- Read [Project](docs/PROJECT.md) and [System](docs/SYSTEM.md) when entering the repository. Read [Design](docs/DESIGN.md), relevant decisions, and research when the task touches their subject. Inspect the code before treating a documented intention as an implemented capability.
- Create artifacts when they first have content.
- Keep current guides synchronized with accepted decisions and implemented reality.

## Verification

Do not run overly complicated checks after implementation. Make them proportionate.
In most cases, it suffices to run `just check`, which runs format checks, Clippy, tests, and Markdown lint without changing source files. Do not run the tests separately. See [System](docs/SYSTEM.md#development-tools) for tool installation and individual recipes.

## Documentation and evidence

- Keep current project guides synchronized with implemented behavior. Distinguish accepted but unimplemented requests from current capabilities.
- Record consequential settled choices under `docs/decisions/`, including status, rationale, consequences, and supporting evidence. Preserve superseded choices and link their replacements.
- Preserve reusable or costly research under `docs/research/`, including the question, findings, provenance, environment or method, confidence, limitations, and relevance.

## Handback and Git

After an implementation, summarize the change, its inspection path, verification results, and remaining uncertainty or deferred work. Propose a Conventional Commit message. Do not commit or push without explicit permission.
