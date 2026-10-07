# Asset resolution from a Zettel title label

Status: reproduced research, not an accepted asset policy or implemented `zk` feature

## Question

Can a note load files from `assets/<ID>/` by basename alone, deriving its ID from the unique title label rather than requiring the user to repeat it?

The desired spelling uses native Typst operations without explicit context:

```typst
#figure(
  image(asset("tiger.jpg")),
  caption: [A tiger],
)
```

The same resolver should work with `import` and `include`. Identity must remain in the existing direct labelled title; replacing title syntax, injecting external compiler inputs, and substituting custom image loaders are outside this question.

## Findings

The ID is available through the title label. The limitation is evaluation timing, not missing identity.

- A plain resolver can query the title and return a normal `path` when the caller provides Typst context. Native image loading, imports, and includes then work. Imported definitions remain local to that context block.
- A contextual callback can supply the resolver and hide context behind image/include wrappers or a module-use callback. This is feasible, but adds custom authoring APIs.
- One explicit `context` around the whole body also works. An everything show rule that wraps its resulting content in context is not equivalent.
- With the ID only in the title and native operations unchanged, the tested supported mechanisms cannot provide a transparent context-free resolver. Show rules cannot retroactively provide context for argument evaluation or defer native image-source loading.
- An early-bound literal ID makes the exact desired spelling work without context, including top-level imports. The fixture simulates a future generated binding; the current engine does not generate it.
- Literal project-root paths work without helpers and provide the simplest baseline.

These conclusions do not select between user-managed assets, a shared directory, or managed per-note namespaces.

## Environment and provenance

The curated artifacts are under [spikes/08-asset-resolution](../../spikes/08-asset-resolution/README.md). They were promoted from temporary session experiments and re-run from their repository location.

| Component | Recorded value |
| --- | --- |
| Host | macOS, Darwin, arm64 |
| Typst CLI | `typst 0.15.1 (unknown commit)` |
| Tinymist | `v0.15.8`, commit `32f908199ee17ea295512bbc27166e890c438175` |
| Tinymist embedded Typst | 0.15.1, fork commit `59b5999da8e74e74583069408d2564fc1f9bc973` |
| Tinymist build timestamp | `2026-09-08T10:13:40.300092000Z` |
| Reference/source checkout inspected | Upstream `47f1d1f93a43a85147f87e7df301284cb2dff192`, workspace version 0.15.1 |
| Compiler project root | Explicit `--root` set to the spike directory |

The inspected upstream checkout explains the observed behavior; it is not asserted to be the exact source revision of either compiler binary. Local versioned documentation was used rather than an unversioned web summary.

These are compiler and metadata-query probes. Tinymist was exercised through its compile command, not its editor transport. There is no `zk` runtime or wire-contract change in this spike.

## Method

Run from the repository root:

```sh
spikes/08-asset-resolution/run.sh
```

The runner executes 16 case compilations per compiler: nine must succeed without warnings and seven must fail with the expected diagnostic classification. In the recorded run, all 32 outcomes matched and no successful case emitted convergence warnings.

It also performs nine namespace checks with Typst's `eval --in` command. Each successful case must produce the expected imported-helper marker, included-fragment marker, and image-source path. For the first note these include:

```text
["helper from first note"]
["first fragment"]
["path(\"/assets/2603231410/diagram.svg\")"]
```

The second note selects `2603231411` and emits the corresponding second-note values. Both directories contain `diagram.svg`, `helper.typ`, and `fragment.typ`. The first fragment contains ordinary unlabelled and non-numeric-labelled headings, testing that they do not confuse note-ID selection.

The image-path assertion accepts both literal-string and `path` representations. Failure-message checks protect the interpretation of these experiments; their exact wording is compiler-version-specific, not an external `zk` contract.

## Successful cases

All cases below compile with both recorded compilers.

| Case under `cases/` | What it establishes |
| --- | --- |
| `2603231410.typ` | A contextual callback loads an image, imports module definitions, and includes content from the title's namespace |
| `2603231411.typ` | The same callback selects another namespace even when used before the title |
| `convenience.typ` | Operation-specific wrappers can hide context; module exports are consumed through a callback |
| `plain-assets.typ` | A plain `asset(name)` returns a normal path when used in an explicit context block |
| `plain-before-title.typ` | The plain resolver also works before the title and selects the second namespace |
| `plain-inline.typ` | Inline image/include contexts and an import block work with altered heading offsets and included ordinary headings |
| `figure-body-context.typ` | One outer body context supports the desired figure expression and native imports/includes without per-operation wrappers |
| `figure-bound-id.typ` | A literal namespace bound before evaluation supports ordinary figure syntax and top-level imports/includes without context |
| `literal-paths.typ` | Native explicit paths work with no resolver, query, or context |

### Plain resolver

The prototype separates ID lookup from path construction:

```typst
#let note-id() = {
  let titles = query(heading.where(depth: 1)).filter(it => {
    it.has("label") and
      str(it.label).match(regex("^[0-9]{10}$")) != none
  })
  assert(titles.len() == 1, message: "expected one Zettel title label")
  str(titles.first().label)
}

#let asset(name) = path("/assets/" + note-id() + "/" + name)
```

`note-id()` requires context but does not create a context expression. The caller supplies it:

```typst
#context {
  import asset("helper.typ"): describe
  image(asset("diagram.svg"))
  include asset("fragment.typ")
  describe()
}
```

`depth: 1` follows the authored heading depth rather than a level altered by offset styling. Numeric-label filtering excludes the ordinary headings in the included fragment.

The plain uniqueness assertion succeeds for valid titles, including forward-title cases. The earlier guarded callback remains as an exploratory alternative; its empty-query guard is not universally necessary. That callback skips an empty result and therefore does not itself diagnose a permanently missing title. The plain resolver's missing/ambiguous-title cases do diagnose it. Neither prototype is a proposed production validation policy.

### Early-bound namespace

This fixture uses:

```typst
#let asset(name) = path("/assets/2603231410/" + name)
```

It supplies the ID before argument evaluation, so no query is needed. A future creation feature could generate such a binding without manual ID entry. That would duplicate the ID once in source and extend the current creation contract, which replaces only the title label and strips schema-declaration comments. The spike does not implement that extension.

## Failing cases

Both compilers fail all seven cases with these diagnostic excerpts.

| Case under `cases/` | Expected failure |
| --- | --- |
| `plain-no-context.typ` | `can only be used when context is known` |
| `plain-no-title.typ` | `assertion failed: expected one Zettel title label` |
| `plain-ambiguous-title.typ` | The same assertion rejects two numeric-labelled titles |
| `top-level-import.typ` | `expected path, module, function, or type, found content` |
| `figure-global-show-context.typ` | The title query still lacks context during argument evaluation |
| `figure-show-context.typ` | Figure-level show context is also too late for the query |
| `image-show-deferred-path.typ` | Native image construction reports the missing `assets/__self__/diagram.svg` before a show rule can resolve it |

The contextual-value import case distinguishes two mechanisms. Importing a normal path inside context works. Returning contextual content from a resolver and handing it to a top-level native import does not turn that content into a path.

## Why show rules cannot make this transparent

The inspected implementation and reference material explain the negative results:

1. [Function-call evaluation](https://github.com/typst/typst/blob/47f1d1f93a43a85147f87e7df301284cb2dff192/crates/typst-eval/src/call.rs#L24) evaluates the callee and arguments before invoking the function. `asset(...)` therefore runs before native `image` or `figure` can do anything with it.
2. [Context semantics](https://typst.app/docs/reference/context/) make contextual results opaque content. Everything depending on a contextual value must happen inside context. Wrapping already evaluated content with an everything show rule cannot establish context for earlier expressions.
3. [Native image-source construction](https://github.com/typst/typst/blob/47f1d1f93a43a85147f87e7df301284cb2dff192/crates/typst-library/src/visualize/image/mod.rs#L100) calls `source.load(engine.world)` while parsing constructor arguments. Image decoding may happen later, but file loading already occurred. An image show rule cannot repair a nonexistent placeholder path.
4. [Native module import](https://github.com/typst/typst/blob/47f1d1f93a43a85147f87e7df301284cb2dff192/crates/typst-eval/src/import.rs#L17) evaluates its source expression and requires a supported import source such as a string, rooted path, or module. Contextual content is not one of those sources.
5. [Path semantics](https://typst.app/docs/reference/foundations/path/) define `/assets/...` relative to the project root and allow a resolved `path` to retain its meaning across helper boundaries. They do not provide a current-note-ID lookup.

## Confidence and limits

Confidence is high for the recorded successful forms and failure mechanisms on these compiler versions. The context-free conclusion is bounded to title-only identity, ordinary built-in loaders, and normal compilation without external ID injection or source rewriting. It is not a claim about every possible wrapper, plugin, compiler modification, or future Typst version.

The prototype queries the evaluated document, not the Rust source AST. Exactly one direct source title does not guarantee exactly one numeric-labelled heading after arbitrary includes or generated content. Multiple complete notes in one compilation need a separate scoping design. This research does not establish reliable filename identity or validate calendar timestamps through Typst.

The image fixture is SVG. Raster codecs, unsaved editor overlays, completions, asset lifecycle commands, symlink policy, transitively imported helper dependencies, and archive-scale performance were not tested. The prototypes do not validate basenames or enforce per-note containment; they are path-resolution probes, not asset-management implementations.

## Project relevance

Per-note storage and names-only Typst access are separate choices. The former does not require a Typst evaluator in `zk`; the latter requires either explicit Typst context or an ID supplied before evaluation.

Literal paths preserve ordinary Typst authoring and can be inserted by an editor without manual ID entry. An early-bound generated resolver is also ordinary Typst, but needs an explicit creation decision. Callback helpers and a body context are demonstrated alternatives, not accepted archive defaults. No asset command, helper installation, cleanup policy, dependency graph, or asset-specific LSP capability has been approved or implemented here.
