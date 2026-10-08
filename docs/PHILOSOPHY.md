# Philosophy

This project is a personal thinking environment built from connected Zettel written in Typst. Its purpose is to make thoughts available for later questioning, revision, and reuse, rather than to maximize the amount of material stored. [Project](PROJECT.md) defines scope; the [glossary](../GLOSSARY.md) defines the terms used here.

## Thinking through writing

Writing a Zettel is a way of processing an idea. State the claim, explain its reasoning, or formulate the question in your own words. A quotation can support that work, but collecting quotations does not replace it. Distinguish what a source says from your interpretation so that a later reader can examine both.

A useful Zettel should remain understandable after the immediate reading session or task has passed. Record the assumptions and context needed to revisit the thought. The archive need not read like a finished publication, but a fragment whose meaning depends entirely on memory is a poor basis for future thinking.

The owner supplies the perspective and judgment. Agents may help search, draft, connect, or check the archive, but should preserve the owner's terminology and questions rather than convert it into a generic encyclopedia. The amount of agent-written text is not a measure of progress.

## Atomicity and stable addresses

A Zettel develops one principal thought that can be addressed independently. Atomicity is a guide to intellectual boundaries, not a page or word limit. A long explanation may belong together; two short claims may deserve separate addresses if they can be questioned or connected independently.

Consider a note arguing that repeated traffic creates efficient paths, then explaining a particular simulation algorithm. If the algorithm is useful independently of that claim, make it a separate Zettel and explain the relationship. Splitting solely to shorten the original note would miss the point.

An address should survive changes in title, wording, and interpretation. This lets a later thought refer back to an earlier one without depending on its current classification. Stable addresses do not make the contents immutable. Revising an idea and inspecting the thoughts that rely on it are normal archive work.

## Connections carry meaning

A reference records where to look. Link context records why to look there. Explain whether the target supports, contradicts, qualifies, exemplifies, or develops the current thought. A connection discovered while writing can matter more than the initial reason for creating the Zettel.

For example, a note about route formation might refer to a note about local feedback because it supplies a mechanism. A later note might challenge whether that mechanism produces a global optimum. These are different relationships even if all three share a keyword such as "optimization".

Do not add references merely to increase connectivity. Backlinks expose later uses of an idea, but they neither manufacture reciprocal relationships nor judge whether the uses are sound. An isolated Zettel can be a legitimate starting point while its connections are still being worked out.

## Retrieval and developing structure

Search provides entry points. Follow links and backlinks from a promising Zettel, compare contexts, and return to earlier claims when new work changes their significance. The intended result is an encounter with relevant prior thinking, not just a list of matching files.

Metadata helps find these entry points. The project does not prescribe a universal category hierarchy or keyword vocabulary. Follow the archive's existing usage, and develop conventions when they solve an actual retrieval problem. Classification should not decide in advance every context in which an idea may later become useful.

Structure notes make larger arguments and routes through the archive explicit. A structure note may arrange a sequence of claims, compare competing explanations, or provide entry points into a topic. It is an ordinary Zettel about relationships, not a separate structural file type. Several arrangements can coexist without moving the underlying Zettel into exclusive folders.

## Typst as the source medium

Plain Typst source is a project commitment. It keeps prose together with mathematical notation, figures, citations, scripting, and user-owned presentation. A thought that needs an equation or diagram can express it in the same source medium as its explanation.

Source and assets remain files that the owner can inspect, edit, copy, and version with ordinary tools. The archive should remain useful over many years without depending on a particular editor's private data store. Presentation helpers may change without changing the identity of a thought or the references authored to it.

The engine handles archive identity, retrieval, relationships, and integrity. Typst tools handle rendering and general language intelligence. The project's current scope does not include compiling or publishing the archive.

## Discipline and enforcement

The writing principles ask for judgment that a parser cannot supply:

| Writing discipline | Engine responsibility |
| --- | --- |
| Develop one principal thought | Provide an addressable Zettel, without judging atomicity |
| Write in your own words | Preserve source, without assessing originality or understanding |
| Explain connections | Track references and backlinks, without judging intellectual relevance |
| Revisit and improve prior thinking | Support retrieval and integrity checks, without autonomously reorganizing the archive |
| Develop useful structure | Keep structure notes available as ordinary Zettel, without assigning special node types |

An archive can pass every integrity check and still contain weak thinking. Technical validity and intellectual quality are different concerns.

## Sources and adaptation

[Sascha Fast's Introduction to the Zettelkasten Method](https://zettelkasten.de/introduction/) describes atomicity, personal writing, meaningful connections, stable addresses, and structure notes. [Niklas Luhmann's Communication with Zettelkastens, in the translation published at Zettelkasten.de](https://zettelkasten.de/communications-with-zettelkastens/) describes the value of revisiting a network of notes that can bring unexpected connections into a current inquiry.

This project adapts those ideas rather than reproducing Luhmann's paper arrangement. Typst source, the engine's identity convention, configurable metadata, and the division between engine and editor are project choices.
