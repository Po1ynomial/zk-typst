use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use typst_syntax::{ast, parse};

pub const ARCHIVE_FORMAT: u32 = 3;
pub const DEFAULT_TEMPLATE_PATH: &str = "templates/zettel.typ";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetadataForm {
    ContentCall,
    StringArgumentsCall,
    StringArrayCall,
    FieldAccess,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataRule {
    pub form: MetadataForm,
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MetadataContract(pub BTreeMap<String, MetadataRule>);

impl MetadataContract {
    pub fn validate(&self) -> Result<(), String> {
        let mut selectors = BTreeSet::new();
        for (field, rule) in &self.0 {
            if field.is_empty() {
                return Err("metadata field names must not be empty".to_owned());
            }
            let root = parse(&format!("#{}", rule.name));
            if !root.errors_and_warnings().0.is_empty()
                || !root.children().any(|node| {
                    node.cast::<ast::Ident>()
                        .is_some_and(|ident| ident.as_str() == rule.name)
                })
            {
                return Err(format!(
                    "metadata field `{field}` must select one direct Typst identifier"
                ));
            }
            let selector = (rule.form == MetadataForm::FieldAccess, rule.name.as_str());
            if !selectors.insert(selector) {
                return Err(format!(
                    "metadata field `{field}` overlaps another source rule"
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub format: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_only_declares_the_archive_format() {
        let manifest: Manifest = toml::from_str(crate::templates::MANIFEST).unwrap();
        assert_eq!(manifest.format, ARCHIVE_FORMAT);
        for source in [
            "format = 3\nextra = true",
            "format = 3\n[metadata.summary]\nform = 'content-call'\nname = 'summary'",
            "format = 3\n[new]\ntemplate = 'custom.typ'",
        ] {
            assert!(toml::from_str::<Manifest>(source).is_err());
        }
    }
}
