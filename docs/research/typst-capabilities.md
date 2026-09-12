# Typst capability experiments

## Version-one outcome

These experiments map what Typst and Tinymist can do. Version one does not use evaluated Typst metadata as provider state. The accepted design keeps the live graph authoritative in Rust and reserves Typst archive computation for later compilation paths. See [Version-one architecture](../decisions/v1-architecture.md).

## Environment

- Typst CLI 0.15.1
- Tinymist 0.15.2 with embedded Typst 0.15.0
- Scratch cases: `experiment/`

## 1. Intercepting Zettel references

### Question

Can archive Typst code recognize a native ten-digit `@ID` reference, replace its rendering, and prevent the normal unresolved-reference diagnostic?

### Documentation

- `reference/model/ref.md` documents `ref.target`, transformational show rules for `ref`, and the synthesized `ref.element` field.
- `reference/styling.md` states that a transformational show rule can replace an element with arbitrary content.
- `reference/foundations/label.md` states that label names may contain digits and that `str(label)` returns the name.

### Experiment

Cases live under `experiment/01-reference-show/`. The handler was:

```typst
#show ref: it => {
  let id = str(it.target)
  if id.match(regex("^[0-9]{10}$")) != none {
    [Zettel #id]
  } else {
    it
  }
}
```

The same handler was also installed inside an imported function applied with `#show: zettel`.

Each case was compiled with both compilers:

```sh
typst compile input.typ output.pdf
tinymist compile input.typ output.pdf
```

Results:

| Case | Typst 0.15.1 | Tinymist/Typst 0.15.0 |
| --- | --- | --- |
| Unhandled `@2603231410` | Fails as an unknown label | Fails as an unknown label |
| Handled `@2603231410` | Compiles | Compiles |
| Handler installed through `#show: zettel` | Compiles | Compiles |
| Handled numeric reference plus unhandled `@missing` | Only `@missing` fails | Only `@missing` fails |

PDF text extraction confirmed that the handled reference rendered as `Zettel 2603231410`.

### Finding

Supported. A Typst library can inspect `ref.target`, classify ten-digit labels, and replace those references before normal reference realization emits an unknown-label error. Returning the original element for other labels preserves Typst's normal behavior. This also works when the rule is installed by an imported `#show: zettel` wrapper.

This proves recognition and replacement only. It does not yet prove that Typst can resolve an ID to a title, validate it against the archive, preserve a useful hyperlink, or discover backlinks.

### Design relevance

The archive library can own the syntax boundary and rendered fallback for ten-digit references. The earlier claim that these references can be neutralized for Typst and Tinymist is realistic. The ownership of title resolution, target validation, links, and graph operations remains open.

Confidence: high for the tested compiler versions.

## 2. Resolving titles and internal links

### Question

When the target heading exists in the same Typst document, can archive code render a ten-digit reference as the heading title while preserving navigation?

### Documentation

- `reference/model/ref.md` documents the synthesized `ref.element` field and warns that it can temporarily be `none` before Typst discovers the target.
- `reference/model/heading.md` documents the heading's `body` field.
- `reference/model/link.md` accepts an element location as an internal-link destination.
- `reference/introspection/location.md` lists headings as locatable elements.

### Experiment

Cases live under `experiment/02-title-link/`. For a resolved ten-digit reference, the show rule returns:

```typst
link(it.element.location(), it.element.body)
```

The cases cover references before and after the target heading, an absent target, rich title markup, and an ordinary non-Zettel reference. They were compiled with Typst 0.15.1 and Tinymist's embedded Typst 0.15.0.

Results:

- Forward and backward Zettel references compiled with both compilers.
- Both rendered `@2603231410` as the target heading body, `Path _efficiency_`.
- Typst converged when the reference appeared before the target.
- The missing-target branch rendered its fallback without a compiler diagnostic.
- The non-Zettel reference retained normal Typst numbering and linking.
- PDF inspection found link annotations whose destination was the named target `2603231410`.

### Finding

Supported within one compiled document. If the labelled heading is present, Typst can resolve an ID to rich title content and create a working internal link. The heading label remains separate from `heading.body`, so the rendered title does not include the ID.

This does not establish how an independently compiled Zettel gains access to headings from other files. Typst resolution works against elements in the current document, not the archive directory by itself.

### Design relevance

This capability is outside the version-one boundary. A single Zettel lacks the archive context needed for title resolution, and full dependency resolution or aggregate compilation is deferred. Version one uses the Typst handler only to prevent unresolved-reference diagnostics. `zk lsp` resolves IDs and titles, and editor clients may display titles as decorations.

Confidence: high for the tested compiler versions and same-document targets. Relevance to version one: none beyond confirming that the deferred compilation path is possible in principle.

## 3. Proposed metadata call forms

### Question

Can `lib/zettel.typ` implement the proposed `#abstract[...]`, `#keywords(...)`, and `#category.name` forms while an imported `#show: zettel` wrapper handles numeric references throughout the resulting content?

### Documentation

- `tutorial/making-a-template.md` documents imported functions applied through an everything show rule.
- `reference/foundations/arguments.md` documents argument sinks for calls such as `#keywords("a", "b")`.
- `reference/foundations/dictionary.md` documents field access, which supports a finite category dictionary and `#category.thoughts` syntax.

### Experiment

`experiment/03-metadata-shape/` reproduces the proposed archive layout with sibling `lib/` and `zettel/` directories. It covers a rich abstract containing a numeric reference, multiple keywords, an empty abstract, an empty keyword list, valid categories, an unknown category, and a numeric reference in the body.

Results:

- Tinymist compiled both valid Zettel without unresolved-reference or import diagnostics.
- Typst compiled both valid Zettel when invoked with the archive root: `typst compile --root . zettel/ID.typ output.pdf`.
- Rich inline abstract markup rendered normally.
- Empty `#abstract[]` and `#keywords()` calls compiled.
- The imported `#show: zettel` handler intercepted numeric references inside both metadata and body content.
- Defining `category` as a dictionary made `#category.unknown` fail at the field access while accepted category fields compiled.

Running `typst compile zettel/ID.typ` without `--root` failed before evaluation because Typst treated `zettel/` as the project root and rejected `../lib/zettel.typ` as a sandbox escape. Tinymist's compile command, launched from the archive root, resolved the same import without extra options.

### Finding

Supported. Ordinary Typst functions and values can implement all three proposed call forms under the imported wrapper. A category dictionary can also enforce the names known to the library. The relative import requires the Typst project root to be the archive root, which matches an archive-aware editor session but not a bare nested-file CLI invocation.

This experiment proves that the forms evaluate and render. It does not prove that the Rust syntax-tree parser can assign their archive semantics without evaluation, or that Typst can enforce the full metadata content contract.

### Design relevance

The proposed visible metadata syntax is realistic. `lib/zettel.typ` can own rendering and can reject unknown category field names. `zk` still needs to check source-level archive contracts unless later experiments show that Typst can enforce them reliably and expose suitable diagnostics.

Confidence: high for Typst 0.15.1 and Tinymist's embedded Typst 0.15.0.

## 4. Enforcing exactly one abstract

### Question

Can the Typst library enforce the requirement that each Zettel contains exactly one `#abstract[...]` call?

### Documentation

- `reference/introspection/metadata.md` documents invisible, queryable values intended for exposing arbitrary document data.
- `reference/introspection/query.md` supports selecting metadata elements and warns that introspection causes repeated evaluation and layout.
- `reference/context.md` explains that queries require context and that the compiler attempts convergence across up to five iterations.
- `reference/foundations/assert.md` provides custom assertion diagnostics.

### Experiment

`experiment/04-abstract-count/` makes `abstract` emit an invisible marker:

```typst
#let abstract(body) = {
  metadata("zk:abstract")
  block[#emph[Abstract.] #body]
}
```

The `zettel` wrapper queries those markers and asserts that the count is one. Cases contain zero, one, or two abstract calls.

Results:

- The one-abstract case compiled with Typst and Tinymist.
- The zero- and two-abstract cases failed with the custom message `a Zettel must contain exactly one abstract` in both compilers.
- `typst eval --in ...` could retrieve the marker through `query(metadata.where(value: "zk:abstract"))`.
- The validation query converged without a warning.
- Both invalid cases point to the assertion in `lib/zettel.typ`, not to a useful Zettel source range. The message distinguishes neither missing from duplicate abstracts nor the duplicate call sites.

### Finding

Supported, but with weak diagnostics. An `abstract` function can emit an invisible semantic marker, and the document wrapper can enforce its global cardinality through introspection. The marker is also available to Typst's command-line query mechanism.

This duplicates a check that `zk lsp` can perform from source with precise ranges. It also adds a contextual query to every Tinymist evaluation and only runs when Typst evaluates the document far enough to apply the wrapper.

### Design relevance

Typst can enforce this invariant, but capability alone does not make it the best owner. The choice is between defense in depth through Typst and one precise implementation in `zk`. Performance and editor behavior of several such validation queries remain untested.

Confidence: high for correctness in the tested cases; low on editor cost and diagnostic usability at archive scale.

## 5. Deriving one authoritative metadata object

### Question

Can `lib/zettel.typ` derive one aggregate metadata object from the visible heading, abstract, keywords, and category constructs without `zk` supplying those values?

### Experiment

`experiment/06-aggregate-metadata/` defines the visible metadata helpers so that each emits an invisible marker alongside its presentation. The `#show: zettel` wrapper queries:

- the level-one heading for its label and body;
- the abstract marker for rich content;
- the keywords marker for an array of strings;
- the category marker for its name.

After cardinality checks, the wrapper emits:

```typst
metadata((
  kind: "zettel",
  schema: 1,
  id: str(title.label),
  title: title.body,
  abstract: abstracts.first(),
  keywords: keyword-lists.first(),
  category: categories.first(),
))
```

The test title contains emphasis and an equation. The abstract contains strong emphasis, a numeric reference, and an equation.

### Results

- Typst 0.15.1 and Tinymist's embedded Typst 0.15.0 compiled the Zettel.
- Querying for the aggregate returned exactly one metadata value.
- The ID was derived from the level-one heading label.
- Keywords serialized as a string array and category as a string.
- Title and abstract serialized as structured Typst content trees. Emphasis, strong emphasis, equations, spaces, text, and the unresolved numeric reference remained distinct elements.
- The numeric-reference show rule prevented unresolved-label diagnostics without removing the reference from the metadata value.
- Metadata query results exposed layout locations and page positions. `typst eval` did not serialize source paths or source ranges for the metadata elements or nested values.

### Finding

Supported. Typst can own node-metadata assembly and emit one authoritative object from the visible Zettel markup. `zk` does not need to reimplement the field-combination semantics or derive the ID from source syntax for a valid evaluated document.

The metadata object can preserve rich Typst content rather than reducing title and abstract to source fragments. The serialized content format is structured but tied to Typst's value model. Source mapping and behavior for malformed or incomplete buffers remain unresolved.

### Design relevance

The provider pipeline can be Typst evaluation first for node metadata, followed by source-level link extraction and range handling where Typst query results are insufficient. This reverses the earlier assumption that the Rust syntax-tree parser owns metadata semantics.

Confidence: high for valid documents on the tested versions.

## 6. Central Typst archive value versus provider merge

### Question

Can Typst produce one archive-wide metadata value containing all nodes and grouped links, and where does this differ from `zk` merging independently evaluated node values?

### Documentation

- `reference/scripting.md` states that `include` accepts a path expression and returns the included file's content.
- `reference/foundations/path.md` states that Typst cannot enumerate a directory or check path existence. Those capabilities may be added to the path type in the future.
- `reference/foundations/sys.md` states that external inputs are strings. Structured input must be decoded, for example as JSON.
- `reference/foundations/selector.md` documents `within(here())` for queries local to a context expression.

### Central experiment

`experiment/07-central-vs-merge/` contains a synthetic archive root. `zk.toml` marks the project root. The root receives a JSON file list, then dynamically includes each path:

```typst
#let files = json(bytes(sys.inputs.files))
#for file in files {
  include path(file)
}
```

Each Zettel wrapper uses `selector(...).within(here())` to derive a node metadata value from only its own body. The archive root queries those node values and emits one aggregate value.

With two valid Zettel, Typst produced one archive object containing:

- both rich node metadata values;
- one grouped link for repeated references from A to B;
- resolved and missing target status;
- the reverse B-to-A link.

A second root accepts source-derived reference occurrences from `zk`. Typst grouped equal source-target pairs, retained all supplied byte spans, and computed target resolution against the evaluated node IDs. This produced the accepted logical link shape in one central Typst value.

Typst also represented a syntactically valid Zettel with a missing abstract as a partial node with `valid: false`, `abstract: none`, and a semantic error list. Query-dependent validation had to return diagnostics as data. Asserting on an initially empty local query aborted before introspection could converge.

### Independent experiment

The same valid Zettel produced equivalent node metadata when evaluated one file at a time. A file with an unclosed content block failed its own evaluation while the other files remained queryable.

When that malformed file was included by the central root, the complete archive evaluation failed and returned no archive metadata value. Typst has no exception mechanism around `include` that would let the root skip one failed file.

### Capability boundary

Typst can:

- dynamically include paths supplied by `zk`;
- isolate introspection to each included Zettel body;
- construct rich node metadata;
- preserve semantically incomplete but evaluable nodes;
- group reference occurrences supplied by `zk`;
- compute resolved and missing link status;
- emit one centralized archive metadata value.

Typst cannot:

- discover the archive root or enumerate `zettel/`;
- derive reliable filename identity unless `zk` supplies paths or IDs;
- expose authored byte or LSP source ranges through ordinary metadata queries;
- recover an archive value when any included file has a syntax or evaluation error.

`zk` must therefore provide the project root, ordered file manifest, current source overlays, and literal reference ranges. Large manifests and occurrence tables should be exposed through the embedded Typst world's virtual files rather than command-line `sys.inputs` strings.

### Comparison

The central approach lets Typst own global grouping, resolution, and archive-level derived metadata. It also shares one failure domain across all included files and evaluates the included document bodies.

Independent evaluation isolates failures and supports partial archive updates naturally. It requires `zk` to merge node values and compute global link status outside Typst.

### Design relevance

An archive-wide Typst value is feasible for valid inputs, but version one does not use it as provider state. The accepted v1 boundary keeps the live graph authoritative in Rust and limits archive metadata to direct declarative source forms. Central Typst metadata remains relevant to later compilation, reporting, and publication paths.

This direction avoids aggregate compilation in the editing loop, preserves malformed-buffer recovery and exact ranges, and requires no live data channel between `zk lsp` and Tinymist.

Confidence: high for the tested capabilities and failure behavior on Typst 0.15.1.

## 7. Tinymist runtime inputs and sampled hover values

### Questions

Does Tinymist compile unsaved Neovim buffer text, can runtime configuration change `sys.inputs`, and can hover display current evaluated values?

### Documentation and source

Tinymist 0.15.2 documentation lists `--input` entries in `tinymist.typstExtraArgs`. Its LSP implementation accepts `workspace/didChangeConfiguration`, includes user inputs in primary compiler options, and reloads projects when those options change.

Tinymist's hover implementation traces non-literal expressions through the active Typst world and labels the result `Sampled Values`.

Local source used for inspection:

```text
/Users/polynomial/projects/local-docs/experiment/tinymist/source
```

### LSP probe

A temporary JSON-RPC client started `tinymist lsp`, initialized a workspace, opened a Typst document, changed configuration, sent full-text buffer changes, and requested `textDocument/hover`.

For injected input, the document decoded `sys.inputs.archive` and inserted the result into a metadata element. Hover first returned:

```typst
(revision: 1, title: "first")
```

After `workspace/didChangeConfiguration` replaced the input, hover returned:

```typst
(revision: 2, title: "second")
```

A second document defined a contextual `wiki()` function that queried node metadata. After an unsaved `didChange` changed the node ID from `1` to `2`, hover on the call expression changed from:

```typst
((kind: "node", id: "1"),)
```

to:

```typst
((kind: "node", id: "2"),)
```

### Finding

Tinymist evaluates unsaved buffer overlays and hover can show current sampled values. Runtime configuration can replace `sys.inputs`.

Changing `typstExtraArgs` is not a cheap streaming channel. The probe logs showed Tinymist recreating project state and its shared font resolver after each input change. Inputs are strings, large values need encoding, and a separate `zk lsp` process cannot send configuration directly to Tinymist without editor mediation.

### Design relevance

These capabilities are useful for later Typst-native archive inspection. Version one does not inject the live Rust graph into Tinymist or use sampled hover as a provider API.

Confidence: high for Tinymist 0.15.2.
