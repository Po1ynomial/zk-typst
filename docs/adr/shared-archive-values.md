# Share versioned archive values across transports

Use one versioned archive-data model for CLI results and custom LSP queries rather than transport-specific payloads, so independently released consumers do not inherit incompatible interpretations of archive semantics.
Discover data and editor-protocol versions independently of executable and archive versions, because changes to source declarations or engine releases need not change retrieved values.
