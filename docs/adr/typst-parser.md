# Use the real Typst parser with explicit version coupling

Implement the engine in Rust using the real Typst syntax parser rather than a separate textual parser, because incomplete-source recovery and precise authored ranges must agree across CLI and live editor consumers. Pin a supported Typst parser generation and test upgrades explicitly, since its syntax API is not a stable independent protocol.
