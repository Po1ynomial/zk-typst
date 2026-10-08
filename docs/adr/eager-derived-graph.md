# Retain the graph, not closed syntax trees

Keep an eager derived graph rather than lazy graph indexing or a persistent cache, but discard closed-file syntax trees, because [prototype measurements](../research/graph-index-performance.md) found graph retention inexpensive while dense retained syntax trees caused substantial memory cost. Metadata search already requires reading every Zettel, so lazy edge discovery would sacrifice complete relationships without addressing the main startup work.
