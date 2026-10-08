# Template-declared metadata

Use a regular Typst template as both creation input and declaration authority rather than maintaining separate TOML retrieval rules, because one annotated starter element keeps its initial value and extraction shape together.
Keep metadata optional and source-only, with a minimal core when the template is absent, rather than restoring hidden predefined fields or introducing evaluation and dependency invalidation. Use parsed declaration comments instead of typed holes or a general template language so ordinary Typst tooling can read the template without preprocessing.

Missing-template creation intentionally permits inspection to write a file; invalid existing templates are never replaced.
