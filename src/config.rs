use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};

use serde::Deserialize;
use typst_syntax::ast;
use typst_syntax::parse;

pub const ARCHIVE_FORMAT: u32 = 2;
pub const DEFAULT_TEMPLATE_PATH: &str = "templates/zettel.typ.tpl";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MetadataForm {
    ContentCall,
    StringArgumentsCall,
    StringArrayCall,
    FieldAccess,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetadataRule {
    pub form: MetadataForm,
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(transparent)]
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
            // Argument shapes do not disambiguate declarations with the same call name.
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
#[serde(default, deny_unknown_fields)]
pub struct NewConfig {
    pub template: String,
}

impl Default for NewConfig {
    fn default() -> Self {
        Self {
            template: DEFAULT_TEMPLATE_PATH.to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub format: u32,
    #[serde(default)]
    pub metadata: MetadataContract,
    #[serde(default)]
    pub new: NewConfig,
}

impl Manifest {
    pub fn validate(&self) -> Result<(), String> {
        self.metadata.validate()?;
        let path = Path::new(&self.new.template);
        if path.as_os_str().is_empty()
            || !path
                .components()
                .all(|component| matches!(component, Component::Normal(_)))
        {
            return Err("new.template must be a relative path beneath the archive root".to_owned());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn omitted_fields_have_no_runtime_defaults() {
        for source in ["format = 2", "format = 2\n[metadata]"] {
            let manifest: Manifest = toml::from_str(source).unwrap();
            manifest.validate().unwrap();
            assert!(manifest.metadata.0.is_empty());
            assert_eq!(manifest.new.template, DEFAULT_TEMPLATE_PATH);
        }
        let manifest: Manifest = toml::from_str(
            "format = 2\n[metadata.summary]\nform = 'content-call'\nname = 'summary'",
        )
        .unwrap();
        manifest.validate().unwrap();
        assert_eq!(manifest.metadata.0.len(), 1);
        assert_eq!(manifest.metadata.0["summary"].name, "summary");
    }

    #[test]
    fn initialization_seeds_are_explicit_and_valid() {
        let manifest: Manifest = toml::from_str(crate::templates::MANIFEST).unwrap();
        manifest.validate().unwrap();
        assert_eq!(manifest.metadata.0.len(), 3);
        assert_eq!(
            manifest.metadata.0["abstract"].form,
            MetadataForm::ContentCall
        );
        assert_eq!(
            manifest.metadata.0["keywords"].form,
            MetadataForm::StringArgumentsCall
        );
        assert_eq!(
            manifest.metadata.0["category"].form,
            MetadataForm::FieldAccess
        );
    }

    #[test]
    fn arbitrary_field_names_do_not_restrict_value_forms() {
        for form in [
            "content-call",
            "string-arguments-call",
            "string-array-call",
            "field-access",
        ] {
            let source =
                format!("format = 2\n[metadata.\"custom field\"]\nform = '{form}'\nname = 'data'");
            let manifest: Manifest = toml::from_str(&source).unwrap();
            manifest.validate().unwrap();
            assert_eq!(manifest.metadata.0.len(), 1);
        }
    }

    #[test]
    fn rejects_invalid_names_and_ambiguous_selectors() {
        for name in ["", "meta.abstract", "abstract extra", "if", "abstract()"] {
            let contract = MetadataContract(BTreeMap::from([(
                "summary".to_owned(),
                MetadataRule {
                    form: MetadataForm::ContentCall,
                    name: name.to_owned(),
                },
            )]));
            assert!(contract.validate().is_err(), "accepted {name:?}");
        }
        for second_form in [
            MetadataForm::ContentCall,
            MetadataForm::StringArgumentsCall,
            MetadataForm::StringArrayCall,
        ] {
            let contract = MetadataContract(BTreeMap::from([
                (
                    "a".to_owned(),
                    MetadataRule {
                        form: MetadataForm::ContentCall,
                        name: "same".to_owned(),
                    },
                ),
                (
                    "b".to_owned(),
                    MetadataRule {
                        form: second_form,
                        name: "same".to_owned(),
                    },
                ),
            ]));
            assert!(contract.validate().is_err());
        }
        let mut contract = MetadataContract(BTreeMap::from([
            (
                "a".to_owned(),
                MetadataRule {
                    form: MetadataForm::FieldAccess,
                    name: "same".to_owned(),
                },
            ),
            (
                "b".to_owned(),
                MetadataRule {
                    form: MetadataForm::FieldAccess,
                    name: "same".to_owned(),
                },
            ),
        ]));
        assert!(contract.validate().is_err());
        contract.0.get_mut("b").unwrap().form = MetadataForm::ContentCall;
        contract.validate().unwrap();
        contract.0.insert(
            String::new(),
            MetadataRule {
                form: MetadataForm::ContentCall,
                name: "empty".to_owned(),
            },
        );
        assert!(contract.validate().is_err());
    }

    #[test]
    fn rejects_unknown_and_incomplete_configuration() {
        for source in [
            "format = 2\nextra = true",
            "format = 2\n[metadata]\nunknown = {}",
            "format = 2\n[metadata.summary]\nname = 'summary'",
            "format = 2\n[metadata.summary]\nform = 'regex'\nname = 'summary'",
            "format = 2\n[metadata.summary]\nform = 'content-call'\nname = 'summary'\nextra = true",
        ] {
            assert!(toml::from_str::<Manifest>(source).is_err());
        }
    }

    #[test]
    fn rejects_template_paths_outside_the_archive() {
        for path in [
            "",
            "/tmp/template",
            "../template",
            "templates/../../template",
        ] {
            let mut manifest: Manifest = toml::from_str("format = 2").unwrap();
            manifest.new.template = path.to_owned();
            assert!(manifest.validate().is_err(), "accepted {path:?}");
        }
    }
}
