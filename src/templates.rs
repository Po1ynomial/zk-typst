pub const MANIFEST: &str = "format = 1\n";

pub struct AgentSkill {
    pub name: &'static str,
    pub source: &'static str,
}

pub const AGENT_SKILLS: &[AgentSkill] = &[AgentSkill {
    name: "zettelkasten",
    source: include_str!("../skills/zettelkasten/SKILL.md"),
}];

pub const LIBRARY: &str = r#"#let is-zettel-id(target) = {
  str(target).match(regex("^[0-9]{10}$")) != none
}

#let render-reference(it) = {
  if is-zettel-id(it.target) {
    [@#str(it.target)]
  } else {
    it
  }
}

#let zettel(body) = {
  show ref: render-reference
  body
}

#let abstract(body) = block(
  inset: (left: 1em),
  stroke: (left: 0.5pt),
)[
  #emph[Abstract.] #body
]

#let keywords(..items) = {
  let values = items.pos()
  block[
    #emph[Keywords.] #values.join[, ]
  ]
}

#let category = (
  thoughts: [Thoughts],
  physics: [Physics],
  coding: [Coding],
)
"#;

pub fn zettel(id: &str) -> String {
    format!(
        r#"#import "../lib/zettel.typ": zettel, abstract, keywords, category
#show: zettel

= Untitled <{id}>

#abstract[]

#keywords()

#category.thoughts

"#
    )
}
