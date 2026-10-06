use typst_syntax::ast::{self, Arg, ArrayItem, AstNode, Expr};
use typst_syntax::{LinkedNode, Source, SyntaxKind, SyntaxNode, parse};

use crate::config::{MetadataContract, MetadataForm, MetadataRule};
use crate::model::{
    ByteRange, Diagnostic, MarkupValue, MetadataValue, Severity, ZettelNode, compare_diagnostics,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReferenceOccurrence {
    pub target: String,
    pub range: ByteRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExtractedNode {
    pub node: ZettelNode,
    pub references: Vec<ReferenceOccurrence>,
    pub diagnostics: Vec<Diagnostic>,
}

pub(crate) fn extract(
    id: &str,
    path: &str,
    source: &str,
    contract: &MetadataContract,
) -> ExtractedNode {
    let root = parse(source);
    extract_root(id, path, source, &root, 1, contract)
}

pub(crate) fn extract_source(
    id: &str,
    path: &str,
    source: &Source,
    generation: u64,
    contract: &MetadataContract,
) -> ExtractedNode {
    extract_root(id, path, source.text(), source.root(), generation, contract)
}

fn extract_root(
    id: &str,
    path: &str,
    source: &str,
    root: &SyntaxNode,
    generation: u64,
    contract: &MetadataContract,
) -> ExtractedNode {
    assert!(
        u32::try_from(source.len()).is_ok(),
        "provider rejects oversized sources before extraction"
    );
    let linked_root = LinkedNode::new(root);
    let top_level: Vec<_> = linked_root.children().collect();
    let mut diagnostics = syntax_diagnostics(path, root, &linked_root);
    let headings: Vec<_> = top_level
        .iter()
        .filter(|node| {
            node.cast::<ast::Heading>()
                .is_some_and(|heading| heading.depth().get() == 1)
        })
        .cloned()
        .collect();
    if headings.is_empty() {
        diagnostics.push(error(
            path,
            "metadata.title",
            "missing required level-one title heading",
            None,
        ));
    }
    for duplicate in headings.iter().skip(1) {
        diagnostics.push(error(
            path,
            "metadata.title",
            "duplicate level-one title heading",
            Some(authored_range(duplicate)),
        ));
    }
    let title = if headings.len() == 1 {
        extract_title(path, id, &headings[0], source, &mut diagnostics)
    } else {
        None
    };

    let metadata = contract
        .0
        .iter()
        .map(|(field, rule)| {
            let declarations: Vec<_> = top_level
                .iter()
                .filter(|node| matches_rule(node, rule))
                .collect();
            let value = match declarations.as_slice() {
                [] => None,
                [node] => match extract_value(node, rule.form, source) {
                    Ok(value) => Some(value),
                    Err(message) => {
                        let mut diagnostic = error(
                            path,
                            "metadata.invalid_shape",
                            message,
                            Some(authored_range(node)),
                        );
                        diagnostic.field = Some(field.clone());
                        diagnostics.push(diagnostic);
                        None
                    }
                },
                _ => {
                    for duplicate in declarations.iter().skip(1) {
                        let mut diagnostic = error(
                            path,
                            "metadata.duplicate",
                            format!("duplicate metadata field `{field}`"),
                            Some(authored_range(duplicate)),
                        );
                        diagnostic.field = Some(field.clone());
                        diagnostics.push(diagnostic);
                    }
                    None
                }
            };
            (field.clone(), value)
        })
        .collect();

    let mut references = Vec::new();
    collect_references(&linked_root, &mut references);
    diagnostics.sort_by(compare_diagnostics);
    ExtractedNode {
        node: ZettelNode {
            id: id.to_owned(),
            path: path.to_owned(),
            generation,
            title,
            metadata,
        },
        references,
        diagnostics,
    }
}

fn matches_rule(node: &LinkedNode<'_>, rule: &MetadataRule) -> bool {
    match rule.form {
        MetadataForm::ContentCall
        | MetadataForm::StringArgumentsCall
        | MetadataForm::StringArrayCall => node.cast::<ast::FuncCall>().is_some_and(
            |call| matches!(call.callee(), Expr::Ident(ident) if ident.as_str() == rule.name),
        ),
        MetadataForm::FieldAccess => node.cast::<ast::FieldAccess>().is_some_and(
            |access| matches!(access.target(), Expr::Ident(ident) if ident.as_str() == rule.name),
        ),
    }
}

fn extract_title(
    path: &str,
    id: &str,
    heading_node: &LinkedNode<'_>,
    source: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<MarkupValue> {
    let body = heading_node
        .children()
        .find(|child| child.kind() == SyntaxKind::Markup)
        .expect("headings contain markup");
    let label = heading_node
        .next_sibling()
        .filter(|sibling| sibling.kind() == SyntaxKind::Label);
    match label {
        Some(node) => {
            let label = node
                .cast::<ast::Label>()
                .expect("label syntax kind")
                .get()
                .to_owned();
            if label != id {
                diagnostics.push(error(
                    path,
                    "metadata.id_mismatch",
                    format!("heading label `{label}` does not match filename ID `{id}`"),
                    Some(range(&node)),
                ));
            }
        }
        None => diagnostics.push(error(
            path,
            "metadata.id_label",
            "the title heading must be followed by its Zettel ID label",
            Some(authored_range(heading_node)),
        )),
    }
    Some(markup_value(&body, source))
}

fn extract_value(
    node: &LinkedNode<'_>,
    form: MetadataForm,
    source: &str,
) -> Result<MetadataValue, &'static str> {
    if !node.get().errors_and_warnings().0.is_empty() {
        return Err("metadata declaration is incomplete or malformed");
    }
    if form == MetadataForm::FieldAccess {
        let access = node
            .cast::<ast::FieldAccess>()
            .expect("matched field access");
        return Ok(MetadataValue::String(access.field().as_str().to_owned()));
    }
    let call = node.cast::<ast::FuncCall>().expect("matched function call");
    match form {
        MetadataForm::ContentCall => {
            let mut args = call.args().items();
            let block = match (args.next(), args.next()) {
                (Some(Arg::Pos(Expr::ContentBlock(block))), None) => block,
                _ => return Err("metadata must contain one literal content-block argument"),
            };
            let body_untyped = block.body().to_untyped();
            let body = descendants(node)
                .into_iter()
                .find(|child| std::ptr::eq(child.get(), body_untyped))
                .expect("linked content body exists");
            Ok(MetadataValue::Markup(markup_value(&body, source)))
        }
        MetadataForm::StringArgumentsCall => call
            .args()
            .items()
            .map(|item| match item {
                Arg::Pos(Expr::Str(value)) => Some(value.get().to_string()),
                _ => None,
            })
            .collect::<Option<Vec<_>>>()
            .map(MetadataValue::StringList)
            .ok_or("metadata must contain only positional string literals"),
        MetadataForm::StringArrayCall => {
            let mut args = call.args().items();
            let values = match (args.next(), args.next()) {
                (Some(Arg::Pos(Expr::Array(array))), None) => array
                    .items()
                    .map(|item| match item {
                        ArrayItem::Pos(Expr::Str(value)) => Some(value.get().to_string()),
                        _ => None,
                    })
                    .collect::<Option<Vec<_>>>(),
                _ => None,
            };
            values
                .map(MetadataValue::StringList)
                .ok_or("metadata must contain one literal string-array argument")
        }
        MetadataForm::FieldAccess => unreachable!("field access handled above"),
    }
}

fn collect_references(node: &LinkedNode<'_>, output: &mut Vec<ReferenceOccurrence>) {
    if let Some(reference) = node.cast::<ast::Ref>() {
        let target = reference.target();
        if target.len() == 10 && target.bytes().all(|byte| byte.is_ascii_digit()) {
            output.push(ReferenceOccurrence {
                target: target.to_owned(),
                range: range(node),
            });
        }
    }
    for child in node.children() {
        collect_references(&child, output);
    }
}

fn syntax_diagnostics(
    path: &str,
    root: &typst_syntax::SyntaxNode,
    linked_root: &LinkedNode<'_>,
) -> Vec<Diagnostic> {
    let (errors, warnings) = root.errors_and_warnings();
    let error_ranges: Vec<_> = if errors.is_empty() {
        Vec::new()
    } else {
        descendants(linked_root)
            .into_iter()
            .filter(|node| node.kind() == SyntaxKind::Error)
            .map(|node| range(&node))
            .collect()
    };
    let mut diagnostics = errors
        .into_iter()
        .enumerate()
        .map(|(index, diagnostic)| Diagnostic {
            path: path.to_owned(),
            code: "syntax.error".to_owned(),
            severity: Severity::Error,
            message: diagnostic.message.to_string(),
            range: error_ranges.get(index).copied(),
            field: None,
        })
        .collect::<Vec<_>>();
    diagnostics.extend(warnings.into_iter().map(|diagnostic| Diagnostic {
        path: path.to_owned(),
        code: "syntax.warning".to_owned(),
        severity: Severity::Warning,
        message: diagnostic.message.to_string(),
        range: None,
        field: None,
    }));
    diagnostics
}

fn markup_value(node: &LinkedNode<'_>, source: &str) -> MarkupValue {
    let range = range(node);
    MarkupValue {
        source: source[range.start as usize..range.end as usize].to_owned(),
        text: project_markup(node),
        range,
    }
}

fn project_markup(node: &LinkedNode<'_>) -> String {
    let mut output = String::new();
    project_node(node, &mut output);
    output.trim().to_owned()
}

fn project_node(node: &LinkedNode<'_>, output: &mut String) {
    match node.kind() {
        SyntaxKind::Markup => {
            for child in node.children() {
                project_node(&child, output);
            }
        }
        SyntaxKind::Strong | SyntaxKind::Emph => {
            if let Some(body) = node
                .children()
                .find(|child| child.kind() == SyntaxKind::Markup)
            {
                project_node(&body, output);
            }
        }
        SyntaxKind::Raw => {
            let raw = node.cast::<ast::Raw>().expect("raw node");
            for (index, line) in raw.lines().enumerate() {
                if index > 0 {
                    push_space(output);
                }
                output.push_str(line.get());
            }
        }
        SyntaxKind::Space | SyntaxKind::Parbreak | SyntaxKind::Linebreak => push_space(output),
        SyntaxKind::LineComment | SyntaxKind::BlockComment => {}
        _ => output.push_str(&node.full_text()),
    }
}

fn push_space(output: &mut String) {
    if !output.is_empty() && !output.ends_with(' ') {
        output.push(' ');
    }
}

fn descendants<'a>(node: &LinkedNode<'a>) -> Vec<LinkedNode<'a>> {
    let mut output = vec![node.clone()];
    for child in node.children() {
        output.extend(descendants(&child));
    }
    output
}

fn authored_range(node: &LinkedNode<'_>) -> ByteRange {
    if let Some(previous) = node.prev_sibling_with_trivia()
        && previous.kind() == SyntaxKind::Hash
        && previous.range().end == node.offset()
    {
        return ByteRange::from_usize(previous.offset(), node.range().end)
            .expect("source size checked");
    }
    range(node)
}

fn range(node: &LinkedNode<'_>) -> ByteRange {
    ByteRange::from_usize(node.offset(), node.range().end).expect("source size checked")
}

fn error(
    path: &str,
    code: impl Into<String>,
    message: impl Into<String>,
    range: Option<ByteRange>,
) -> Diagnostic {
    Diagnostic {
        path: path.to_owned(),
        code: code.into(),
        severity: Severity::Error,
        message: message.into(),
        range,
        field: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Manifest;

    fn contract(source: &str) -> MetadataContract {
        let manifest: Manifest = toml::from_str(source).unwrap();
        manifest.validate().unwrap();
        manifest.metadata
    }

    fn extract(id: &str, source: &str) -> ExtractedNode {
        super::extract(
            id,
            &format!("zettel/{id}.typ"),
            source,
            &contract(crate::templates::MANIFEST),
        )
    }

    const VALID: &str = r#"#import "../lib/zettel.typ": zettel, abstract, keywords, category
#show: zettel

= Path _efficiency_ $x^2$ <2603231410>

#abstract[
Repeated *traffic*, @2603220935 and #code.
]

#keywords("networks", "optimization")
#category.thoughts

Body @2603220935 and @9999999999. `@1111111111`
"#;

    #[test]
    fn extracts_rich_metadata_and_literal_reference_ranges() {
        let extracted = extract("2603231410", VALID);
        let title = extracted.node.title.as_ref().unwrap();
        assert_eq!(title.source, "Path _efficiency_ $x^2$");
        assert_eq!(title.text, "Path efficiency $x^2$");
        assert_eq!(
            &VALID[title.range.start as usize..title.range.end as usize],
            title.source
        );
        let abstract_value = extracted.node.metadata["abstract"]
            .as_ref()
            .unwrap()
            .as_markup()
            .unwrap();
        assert_eq!(
            abstract_value.text,
            "Repeated traffic, @2603220935 and #code."
        );
        assert_eq!(
            &VALID[abstract_value.range.start as usize..abstract_value.range.end as usize],
            abstract_value.source
        );
        assert_eq!(
            extracted.node.metadata["keywords"],
            Some(MetadataValue::StringList(vec![
                "networks".to_owned(),
                "optimization".to_owned()
            ]))
        );
        assert_eq!(
            extracted.node.metadata["category"],
            Some(MetadataValue::String("thoughts".to_owned()))
        );
        assert!(extracted.diagnostics.is_empty());
        let authored: Vec<_> = extracted
            .references
            .iter()
            .map(|reference| &VALID[reference.range.start as usize..reference.range.end as usize])
            .collect();
        assert_eq!(authored, vec!["@2603220935", "@2603220935", "@9999999999"]);
    }

    #[test]
    fn metadata_is_independent_of_order_and_presentation() {
        let source = r#"#import "styles.typ": preamble, extra
#show: preamble
#keywords("one")
Some prose before the title.
#category.coding
#set text(size: 11pt)
#abstract[= Rich summary

- Item
#figure[Content]]
#import "../lib/zettel.typ": *
= Flexible <2603231410>
More prose.
"#;
        let extracted = extract("2603231410", source);
        assert!(extracted.diagnostics.is_empty());
        assert_eq!(extracted.node.title.unwrap().text, "Flexible");
        assert!(
            extracted.node.metadata["abstract"]
                .as_ref()
                .unwrap()
                .as_markup()
                .unwrap()
                .source
                .contains("#figure")
        );
        let reordered = VALID.replace(
            "zettel, abstract, keywords, category",
            "abstract, category, keywords, zettel, extra",
        );
        assert!(extract("2603231410", &reordered).diagnostics.is_empty());
    }

    #[test]
    fn omitted_empty_and_unconfigured_fields_are_distinct() {
        let minimal = "= Minimal <2603231410>\n";
        let omitted = extract("2603231410", minimal);
        assert!(omitted.diagnostics.is_empty());
        assert_eq!(omitted.node.metadata.len(), 3);
        assert!(omitted.node.metadata.values().all(Option::is_none));
        let source = format!("{minimal}#abstract[]\n#keywords()\n");
        let empty = extract("2603231410", &source);
        let abstract_value = empty.node.metadata["abstract"]
            .as_ref()
            .unwrap()
            .as_markup()
            .unwrap();
        assert_eq!(abstract_value.source, "");
        assert_eq!(abstract_value.range.start, abstract_value.range.end);
        assert_eq!(
            abstract_value.range.start as usize,
            source.find("[]").unwrap() + 1
        );
        assert_eq!(
            empty.node.metadata["keywords"],
            Some(MetadataValue::StringList(vec![]))
        );
        let unconfigured = super::extract(
            "2603231410",
            "zettel/2603231410.typ",
            VALID,
            &MetadataContract::default(),
        );
        assert!(unconfigured.node.metadata.is_empty());
        assert!(unconfigured.diagnostics.is_empty());
        assert_eq!(unconfigured.references.len(), 3);
    }

    #[test]
    fn arbitrary_fields_and_forms_preserve_types_and_core_identity() {
        let rules = contract(
            r#"format = 2
[metadata.summary]
form = "content-call"
name = "summary"
[metadata.tags]
form = "string-array-call"
name = "tags"
[metadata.topic]
form = "field-access"
name = "group"
[metadata.title]
form = "string-arguments-call"
name = "custom-title"
[metadata.review]
form = "content-call"
name = "review"
"#,
        );
        let source = "= Core title <2603231410>\n#summary[café *content*]\n#tags((\"two\", \"one\", \"two\"))\n#group.coding\n#custom-title(\"auxiliary\")\n#abstract[Not metadata]\n";
        let extracted = super::extract("2603231410", "zettel/2603231410.typ", source, &rules);
        assert!(extracted.diagnostics.is_empty());
        assert_eq!(extracted.node.title.as_ref().unwrap().text, "Core title");
        assert_eq!(extracted.node.metadata.len(), 5);
        assert!(!extracted.node.metadata.contains_key("abstract"));
        assert!(extracted.node.metadata["review"].is_none());
        let value = extracted.node.metadata["summary"]
            .as_ref()
            .unwrap()
            .as_markup()
            .unwrap();
        assert_eq!(value.text, "café content");
        assert_eq!(
            &source[value.range.start as usize..value.range.end as usize],
            value.source
        );
        assert_eq!(
            extracted.node.metadata["tags"],
            Some(MetadataValue::StringList(vec![
                "two".to_owned(),
                "one".to_owned(),
                "two".to_owned()
            ]))
        );
        assert_eq!(
            extracted.node.metadata["topic"],
            Some(MetadataValue::String("coding".to_owned()))
        );
        assert_eq!(
            extracted.node.metadata["title"],
            Some(MetadataValue::StringList(vec!["auxiliary".to_owned()]))
        );
    }

    #[test]
    fn duplicates_are_errors_without_a_winner_and_use_stable_codes() {
        let source = "= Title <2603231410>\n#abstract[a]\n#abstract[b]\n#keywords()\n#keywords(\"x\")\n#category.a\n#category.b\n";
        let extracted = extract("2603231410", source);
        assert!(extracted.node.metadata.values().all(Option::is_none));
        assert_eq!(extracted.diagnostics.len(), 3);
        for field in ["abstract", "keywords", "category"] {
            let diagnostic = extracted
                .diagnostics
                .iter()
                .find(|value| value.field.as_deref() == Some(field))
                .unwrap();
            assert_eq!(diagnostic.code, "metadata.duplicate");
            assert!(diagnostic.range.is_some());
        }
    }

    #[test]
    fn malformed_declarations_have_null_values_and_authored_field_diagnostics() {
        for call in [
            "#abstract(computed)",
            "#abstract[one][two]",
            "#abstract()",
            "#keywords(\"valid\", computed)",
            "#keywords(..values)",
            "#keywords(named: \"value\")",
        ] {
            let field = if call.starts_with("#abstract") {
                "abstract"
            } else {
                "keywords"
            };
            let source = format!("= Title <2603231410>\n{call}\n");
            let extracted = extract("2603231410", &source);
            assert!(extracted.node.metadata[field].is_none(), "accepted {call}");
            let diagnostic = extracted
                .diagnostics
                .iter()
                .find(|value| value.code == "metadata.invalid_shape")
                .unwrap();
            assert_eq!(diagnostic.field.as_deref(), Some(field));
            let range = diagnostic.range.unwrap();
            assert_eq!(&source[range.start as usize..range.end as usize], call);
        }
    }

    #[test]
    fn array_forms_require_one_literal_string_array() {
        let rules =
            contract("format = 2\n[metadata.tags]\nform = 'string-array-call'\nname = 'tags'");
        for call in [
            "#tags((computed,))",
            "#tags((..values))",
            "#tags(\"one\", \"two\")",
            "#tags(values)",
            "#tags((\"one\",), extra: 1)",
        ] {
            let source = format!("= Title <2603231410>\n{call}\n");
            let extracted = super::extract("2603231410", "zettel/2603231410.typ", &source, &rules);
            assert!(extracted.node.metadata["tags"].is_none(), "accepted {call}");
            assert!(
                extracted
                    .diagnostics
                    .iter()
                    .any(|value| value.code == "metadata.invalid_shape"
                        && value.field.as_deref() == Some("tags"))
            );
        }
        let empty = super::extract(
            "2603231410",
            "zettel/2603231410.typ",
            "= Title <2603231410>\n#tags(())",
            &rules,
        );
        assert!(empty.diagnostics.is_empty());
        assert_eq!(
            empty.node.metadata["tags"],
            Some(MetadataValue::StringList(vec![]))
        );
    }

    #[test]
    fn nested_generated_raw_and_commented_constructs_do_not_declare_metadata() {
        let source = r#"= Title <2603231410>
#let example() = [#abstract[Unused] #keywords("hidden") #category.hidden]
#block[#abstract[Nested]]
#if true { keywords("conditional") }
`#abstract[Raw]`
// #category.comment
"#;
        let extracted = extract("2603231410", source);
        assert!(extracted.diagnostics.is_empty());
        assert!(extracted.node.metadata.values().all(Option::is_none));
    }

    #[test]
    fn malformed_source_retains_identity_and_recoverable_title() {
        let source = "= Broken <wrong>\n#abstract[Unclosed\n#keywords(computed)\n";
        let extracted = extract("2603231410", source);
        assert_eq!(extracted.node.id, "2603231410");
        assert_eq!(extracted.node.title.unwrap().text, "Broken");
        assert!(extracted.node.metadata["abstract"].is_none());
        for code in [
            "syntax.error",
            "metadata.id_mismatch",
            "metadata.invalid_shape",
        ] {
            assert!(
                extracted.diagnostics.iter().any(|value| value.code == code),
                "missing {code}"
            );
        }
    }

    #[test]
    fn incomplete_category_access_does_not_invent_a_member_name() {
        let extracted = extract("2603231410", "= Title <2603231410>\n#category.");
        assert!(extracted.node.metadata["category"].is_none());
    }

    #[test]
    fn required_title_diagnostics_do_not_claim_a_custom_field() {
        for source in [
            "#abstract[]",
            "= One <2603231410>\n= Two <2603231410>",
            "= Missing label",
        ] {
            let extracted = extract("2603231410", source);
            assert!(!extracted.diagnostics.is_empty());
            assert!(
                extracted
                    .diagnostics
                    .iter()
                    .all(|value| value.field.is_none())
            );
        }
    }
}
