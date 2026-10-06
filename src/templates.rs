pub const MANIFEST: &str = "format = 3\n";

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

"#;

pub const ZETTEL: &str = r#"#import "../lib/zettel.typ": zettel
#show: zettel

= Untitled <new>
"#;

/// An optional user-owned starting point, never a runtime default.
pub const DESCRIPTIVE_TEMPLATE: &str = include_str!("../examples/templates/descriptive.typ");
