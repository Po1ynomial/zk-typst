use std::ops::Range;

use typst_syntax::ast::{self, Arg, Expr};
use typst_syntax::{LinkedNode, SyntaxKind, parse};

use crate::config::{MetadataContract, MetadataForm, MetadataRule};
use crate::extract::extract;
use crate::model::Severity;

/// A regular Typst source file with source-only metadata declarations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Template {
    source: String,
    pub metadata: MetadataContract,
    declarations: Vec<Range<usize>>,
    title_label: Range<usize>,
}

impl Template {
    pub fn parse(source: String) -> Result<Self, String> {
        if u32::try_from(source.len()).is_err() {
            return Err("template exceeds the 4 GiB byte-range limit".to_owned());
        }
        let root = parse(&source);
        let errors = root.errors_and_warnings().0;
        if !errors.is_empty() {
            return Err(errors
                .iter()
                .map(|error| error.message.as_str())
                .collect::<Vec<_>>()
                .join("; "));
        }
        let linked = LinkedNode::new(&root);
        let headings: Vec<_> = linked
            .children()
            .filter(|node| {
                node.cast::<ast::Heading>()
                    .is_some_and(|heading| heading.depth().get() == 1)
            })
            .collect();
        let [heading] = headings.as_slice() else {
            return Err(
                "template must contain exactly one direct level-one title heading".to_owned(),
            );
        };
        let label = heading
            .next_sibling()
            .filter(|node| node.kind() == SyntaxKind::Label)
            .ok_or("template title must be followed by a label")?;
        let title_label = label.range();
        let mut metadata = MetadataContract::default();
        let mut declarations = Vec::new();
        let mut pending = None;
        for node in linked.children() {
            if node.kind() == SyntaxKind::LineComment {
                let standalone = source[..node.offset()]
                    .rsplit('\n')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .is_empty();
                if standalone && let Some((field, kind)) = annotation(&node.full_text()) {
                    if pending.replace((field, kind)).is_some() {
                        return Err(
                            "only one metadata declaration may attach to an element".to_owned()
                        );
                    }
                    declarations.push(node.range());
                }
                continue;
            }
            if matches!(
                node.kind(),
                SyntaxKind::Space
                    | SyntaxKind::Parbreak
                    | SyntaxKind::BlockComment
                    | SyntaxKind::Hash
            ) {
                continue;
            }
            if let Some((field, kind)) = pending.take() {
                let rule = infer_rule(&node, &kind)
                    .map_err(|message| format!("metadata field `{field}`: {message}"))?;
                if metadata.0.insert(field.clone(), rule).is_some() {
                    return Err(format!("duplicate metadata field `{field}`"));
                }
            }
        }
        if pending.is_some() {
            return Err("metadata declaration has no following top-level element".to_owned());
        }
        metadata.validate()?;
        let template = Self {
            source,
            metadata,
            declarations,
            title_label,
        };
        // Validate defaults and core independently of the template's placeholder label.
        let rendered = template.render("2601010000");
        if u32::try_from(rendered.len()).is_err() {
            return Err("rendered template exceeds the 4 GiB byte-range limit".to_owned());
        }
        let extracted = extract("2601010000", "", &rendered, &template.metadata);
        let errors: Vec<_> = extracted
            .diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic.severity == Severity::Error && diagnostic.code.starts_with("metadata.")
            })
            .map(|diagnostic| diagnostic.message.as_str())
            .collect();
        if !errors.is_empty() {
            return Err(errors.join("; "));
        }
        Ok(template)
    }

    pub fn render(&self, id: &str) -> String {
        let mut edits: Vec<_> = self
            .declarations
            .iter()
            .map(|range| (range.clone(), String::new()))
            .collect();
        edits.push((self.title_label.clone(), format!("<{id}>")));
        edits.sort_by_key(|(range, _)| range.start);
        let mut result = self.source.clone();
        for (range, replacement) in edits.into_iter().rev() {
            result.replace_range(range, &replacement);
        }
        result
    }
}

/// Exact, full-comment grammar. Failure means an ordinary comment, not an error.
fn annotation(comment: &str) -> Option<(String, String)> {
    let rest = comment
        .strip_prefix("//")?
        .trim()
        .strip_prefix("@zk-field ")?
        .trim_start();
    let mut strings = serde_json::Deserializer::from_str(rest).into_iter::<String>();
    let field = strings.next()?.ok()?;
    let remaining = rest.get(strings.byte_offset()..)?;
    if !remaining.starts_with(char::is_whitespace) {
        return None;
    }
    let kind = remaining.trim().strip_prefix("kind=")?;
    if kind.is_empty()
        || !kind
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte == b'-')
    {
        return None;
    }
    Some((field, kind.to_owned()))
}

fn infer_rule(node: &LinkedNode<'_>, kind: &str) -> Result<MetadataRule, &'static str> {
    if !matches!(kind, "markup" | "string" | "string-list") {
        return Err("unsupported metadata kind");
    }
    if let Some(access) = node.cast::<ast::FieldAccess>()
        && kind == "string"
        && let Expr::Ident(target) = access.target()
    {
        return Ok(MetadataRule {
            form: MetadataForm::FieldAccess,
            name: target.as_str().to_owned(),
        });
    }
    if let Some(call) = node.cast::<ast::FuncCall>()
        && let Expr::Ident(name) = call.callee()
    {
        let args: Vec<_> = call.args().items().collect();
        let form = match (kind, args.as_slice()) {
            ("markup", [Arg::Pos(Expr::ContentBlock(_))]) => MetadataForm::ContentCall,
            ("string-list", [Arg::Pos(Expr::Array(_))]) => MetadataForm::StringArrayCall,
            ("string-list", args)
                if args.iter().all(|arg| matches!(arg, Arg::Pos(Expr::Str(_)))) =>
            {
                MetadataForm::StringArgumentsCall
            }
            _ => {
                return Err(
                    "element does not have a supported literal shape for its declared kind",
                );
            }
        };
        return Ok(MetadataRule {
            form,
            name: name.as_str().to_owned(),
        });
    }
    Err("declaration must attach to a supported direct top-level element")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_is_independent_and_only_declaring_comments_are_removed() {
        let source = "// ordinary café\r\n= _Title_ <new>\r\n\
                      // @zk-field \"custom field\" kind=markup\r\n\
                      // another comment\r\n#summary[]\r\n\
                      // @zk-field broken kind=markup\r\n\
                      // @zk-field \"oops\" kind=markup extra\r\n";
        let template = Template::parse(source.to_owned()).unwrap();
        assert_eq!(template.metadata.0.len(), 1);
        assert_eq!(
            template.render("2603231410"),
            source
                .replace("<new>", "<2603231410>")
                .replace("// @zk-field \"custom field\" kind=markup", "")
        );
        assert!(
            Template::parse(crate::templates::ZETTEL.to_owned())
                .unwrap()
                .metadata
                .0
                .is_empty()
        );
    }

    #[test]
    fn annotation_parsing_is_exact_and_decodes_field_names() {
        assert_eq!(
            annotation(r#"// @zk-field "custom\u0020field" kind=string-list"#),
            Some(("custom field".to_owned(), "string-list".to_owned()))
        );
        for comment in [
            "// @zk-field unquoted kind=markup",
            "// @zk-field \"name\"kind=markup",
            "// @zk-field \"name\" kind=markup extra",
            "// @zk-field \"name\" kind=markup // explanation",
            "// @zk-field \"name\"",
            "// prose @zk-field \"name\" kind=markup",
            "/* @zk-field \"name\" kind=markup */",
        ] {
            assert!(annotation(comment).is_none(), "recognized {comment}");
        }
        let source = "#let marker = \"<new>\"\n= Title <new>\n  // @zk-field \"summary\" kind=markup\n#summary[]\n";
        let template = Template::parse(source.to_owned()).unwrap();
        assert_eq!(
            template.render("2603231410"),
            "#let marker = \"<new>\"\n= Title <2603231410>\n  \n#summary[]\n"
        );
    }

    #[test]
    fn recognizes_four_shapes_without_evaluation() {
        let template = Template::parse(
            "= Title <new>\n\
            // @zk-field \"a\" kind=markup\n#summary[Rich #code]\n\
            // @zk-field \"b\" kind=string-list\n#tags()\n\
            // @zk-field \"c\" kind=string-list\n#labels(())\n\
            // @zk-field \"d\" kind=string\n#group.thoughts\n"
                .to_owned(),
        )
        .unwrap();
        assert_eq!(template.metadata.0["a"].form, MetadataForm::ContentCall);
        assert_eq!(
            template.metadata.0["b"].form,
            MetadataForm::StringArgumentsCall
        );
        assert_eq!(template.metadata.0["c"].form, MetadataForm::StringArrayCall);
        assert_eq!(template.metadata.0["d"].form, MetadataForm::FieldAccess);
    }

    #[test]
    fn annotations_in_raw_strings_nested_content_and_trailing_comments_are_not_declarations() {
        let source = "= Title <new>\n\
            `// @zk-field \"raw\" kind=markup`\n\
            #let example = \"// @zk-field \\\"string\\\" kind=markup\"\n\
            #block[\n// @zk-field \"nested\" kind=markup\n#summary[]]\n\
            #summary[] // @zk-field \"trailing\" kind=markup\n";
        let template = Template::parse(source.to_owned()).unwrap();
        assert!(template.metadata.0.is_empty());
        assert_eq!(
            template.render("2603231410"),
            source.replace("<new>", "<2603231410>")
        );
    }

    #[test]
    fn rejects_syntax_invalid_defaults_orphan_and_ambiguous_declarations() {
        for tail in [
            "#let broken = (",
            "// @zk-field \"a\" kind=markup\n",
            "// @zk-field \"a\" kind=markup\nProse\n#summary[]",
            "// @zk-field \"a\" kind=number\n#summary[]",
            "// @zk-field \"a\" kind=markup\n#module.summary[]",
            "// @zk-field \"a\" kind=string-list\n#tags((computed,))",
            "// @zk-field \"a\" kind=markup\n#summary[]\n#summary[]",
            "// @zk-field \"a\" kind=markup\n#summary[]\n// @zk-field \"b\" kind=markup\n#summary[]",
            "// @zk-field \"a\" kind=markup\n#summary[]\n// @zk-field \"a\" kind=markup\n#other[]",
            "// @zk-field \"\" kind=markup\n#summary[]",
        ] {
            assert!(
                Template::parse(format!("= Title <new>\n{tail}")).is_err(),
                "accepted {tail}"
            );
        }
        for source in ["", "= No label", "= One <new>\n= Two <other>"] {
            assert!(Template::parse(source.to_owned()).is_err());
        }
    }
}
