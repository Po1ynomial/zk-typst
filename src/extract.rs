use std::collections::BTreeSet;

use typst_syntax::ast::{self, Arg, AstNode, Expr, ImportItem, Imports};
use typst_syntax::{LinkedNode, Source, SyntaxKind, SyntaxNode, parse};

use crate::model::{ByteRange, Diagnostic, MarkupValue, Severity, ZettelNode};

const REQUIRED_IMPORTS: [&str; 4] = ["zettel", "abstract", "keywords", "category"];

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

pub(crate) fn extract(id: &str, path: &str, source: &str) -> ExtractedNode {
    let root = parse(source);
    extract_root(id, path, source, &root, 1)
}

pub(crate) fn extract_source(
    id: &str,
    path: &str,
    source: &Source,
    generation: u64,
) -> ExtractedNode {
    extract_root(id, path, source.text(), source.root(), generation)
}

fn extract_root(
    id: &str,
    path: &str,
    source: &str,
    root: &SyntaxNode,
    generation: u64,
) -> ExtractedNode {
    assert!(
        u32::try_from(source.len()).is_ok(),
        "provider rejects oversized sources before extraction"
    );

    let linked_root = LinkedNode::new(root);
    let top_level: Vec<_> = linked_root.children().collect();
    let mut diagnostics = syntax_diagnostics(path, root, &linked_root);

    let imports: Vec<_> = top_level
        .iter()
        .filter(|node| is_zettel_import(node))
        .cloned()
        .collect();
    let show_rules: Vec<_> = top_level
        .iter()
        .filter(|node| is_zettel_show_rule(node))
        .cloned()
        .collect();
    let headings: Vec<_> = top_level
        .iter()
        .filter(|node| {
            node.cast::<ast::Heading>()
                .is_some_and(|heading| heading.depth().get() == 1)
        })
        .cloned()
        .collect();
    let abstracts: Vec<_> = top_level
        .iter()
        .filter(|node| is_named_call(node, "abstract"))
        .cloned()
        .collect();
    let keywords: Vec<_> = top_level
        .iter()
        .filter(|node| is_named_call(node, "keywords"))
        .cloned()
        .collect();
    let categories: Vec<_> = top_level
        .iter()
        .filter(|node| is_category_access(node))
        .cloned()
        .collect();

    require_one(
        path,
        "metadata.import",
        "required Zettel import",
        &imports,
        &mut diagnostics,
    );
    require_one(
        path,
        "metadata.show",
        "`#show: zettel` rule",
        &show_rules,
        &mut diagnostics,
    );
    require_one(
        path,
        "metadata.title",
        "level-one title heading",
        &headings,
        &mut diagnostics,
    );
    require_one(
        path,
        "metadata.abstract",
        "abstract",
        &abstracts,
        &mut diagnostics,
    );
    require_one(
        path,
        "metadata.keywords",
        "keyword list",
        &keywords,
        &mut diagnostics,
    );
    require_one(
        path,
        "metadata.category",
        "category",
        &categories,
        &mut diagnostics,
    );

    if let Some(import) = imports.first()
        && !valid_import(import)
    {
        diagnostics.push(error(
            path,
            "metadata.import",
            "the Zettel import must directly import zettel, abstract, keywords, and category",
            Some(authored_range(import)),
        ));
    }

    let title = if headings.len() == 1 {
        extract_title(path, id, &headings[0], source, &mut diagnostics)
    } else {
        None
    };

    let abstract_value = if abstracts.len() == 1 {
        extract_abstract(path, &abstracts[0], source, &mut diagnostics)
    } else {
        None
    };

    let keyword_values = if keywords.len() == 1 {
        extract_keywords(path, &keywords[0], &mut diagnostics)
    } else {
        None
    };

    let category = if categories.len() == 1 {
        extract_category(&categories[0])
    } else {
        None
    };

    check_header(
        path,
        &top_level,
        [
            &imports,
            &show_rules,
            &headings,
            &abstracts,
            &keywords,
            &categories,
        ],
        &mut diagnostics,
    );

    let mut references = Vec::new();
    collect_references(&linked_root, &mut references);

    diagnostics.sort_by(|left, right| {
        left.range
            .cmp(&right.range)
            .then_with(|| left.code.cmp(&right.code))
            .then_with(|| left.message.cmp(&right.message))
    });

    ExtractedNode {
        node: ZettelNode {
            id: id.to_owned(),
            path: path.to_owned(),
            generation,
            title,
            abstract_value,
            keywords: keyword_values,
            category,
        },
        references,
        diagnostics,
    }
}

fn is_zettel_import(node: &LinkedNode<'_>) -> bool {
    let Some(import) = node.cast::<ast::ModuleImport>() else {
        return false;
    };
    matches!(import.source(), Expr::Str(value) if value.get() == "../lib/zettel.typ")
}

fn valid_import(node: &LinkedNode<'_>) -> bool {
    let import = node.cast::<ast::ModuleImport>().expect("checked import");
    let Some(Imports::Items(items)) = import.imports() else {
        return false;
    };
    let imported: Option<Vec<_>> = items
        .iter()
        .map(|item| match item {
            ImportItem::Simple(path) => Some(path.name().as_str()),
            ImportItem::Renamed(_) => None,
        })
        .collect();
    imported.is_some_and(|names| {
        names.len() == REQUIRED_IMPORTS.len()
            && names.into_iter().collect::<BTreeSet<_>>()
                == REQUIRED_IMPORTS.into_iter().collect::<BTreeSet<_>>()
    })
}

fn is_zettel_show_rule(node: &LinkedNode<'_>) -> bool {
    let Some(show) = node.cast::<ast::ShowRule>() else {
        return false;
    };
    show.selector().is_none()
        && matches!(show.transform(), Expr::Ident(ident) if ident.as_str() == "zettel")
}

fn is_named_call(node: &LinkedNode<'_>, name: &str) -> bool {
    node.cast::<ast::FuncCall>()
        .is_some_and(|call| matches!(call.callee(), Expr::Ident(ident) if ident.as_str() == name))
}

fn is_category_access(node: &LinkedNode<'_>) -> bool {
    node.cast::<ast::FieldAccess>().is_some_and(
        |access| matches!(access.target(), Expr::Ident(ident) if ident.as_str() == "category"),
    )
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

fn extract_abstract(
    path: &str,
    call_node: &LinkedNode<'_>,
    source: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<MarkupValue> {
    let call = call_node
        .cast::<ast::FuncCall>()
        .expect("checked function call");
    let mut args = call.args().items();
    let block = match (args.next(), args.next()) {
        (Some(Arg::Pos(Expr::ContentBlock(block))), None) => block,
        _ => {
            diagnostics.push(error(
                path,
                "metadata.abstract",
                "abstract must be one direct content-block argument",
                Some(authored_range(call_node)),
            ));
            return None;
        }
    };

    let body_untyped = block.body().to_untyped();
    if contains_block_structure(body_untyped) {
        diagnostics.push(error(
            path,
            "metadata.abstract",
            "abstract cannot contain headings, lists, terms, or figures",
            Some(authored_range(call_node)),
        ));
        return None;
    }

    let body = descendants(call_node)
        .into_iter()
        .find(|node| std::ptr::eq(node.get(), body_untyped))
        .expect("linked abstract body exists");
    Some(markup_value(&body, source))
}

fn extract_keywords(
    path: &str,
    call_node: &LinkedNode<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Vec<String>> {
    let call = call_node
        .cast::<ast::FuncCall>()
        .expect("checked function call");
    let mut values = Vec::new();
    for item in call.args().items() {
        match item {
            Arg::Pos(Expr::Str(value)) => values.push(value.get().to_string()),
            _ => {
                diagnostics.push(error(
                    path,
                    "metadata.keywords",
                    "keywords must contain only positional string literals",
                    Some(authored_range(call_node)),
                ));
                return None;
            }
        }
    }
    Some(values)
}

fn extract_category(node: &LinkedNode<'_>) -> Option<String> {
    let access = node.cast::<ast::FieldAccess>()?;
    Some(access.field().as_str().to_owned())
}

fn contains_block_structure(node: &typst_syntax::SyntaxNode) -> bool {
    if matches!(
        node.kind(),
        SyntaxKind::Heading | SyntaxKind::ListItem | SyntaxKind::EnumItem | SyntaxKind::TermItem
    ) {
        return true;
    }
    if node.cast::<ast::FuncCall>().is_some_and(
        |call| matches!(call.callee(), Expr::Ident(ident) if ident.as_str() == "figure"),
    ) {
        return true;
    }
    node.children().any(contains_block_structure)
}

fn check_header(
    path: &str,
    top_level: &[LinkedNode<'_>],
    fields: [&Vec<LinkedNode<'_>>; 6],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let selected: Vec<_> = fields
        .iter()
        .filter_map(|nodes| nodes.first().cloned())
        .collect();
    for pair in selected.windows(2) {
        if pair[0].offset() > pair[1].offset() {
            diagnostics.push(error(
                path,
                "metadata.header_order",
                "metadata header fields are out of order",
                Some(authored_range(&pair[1])),
            ));
        }
    }

    let (Some(first), Some(last)) = (selected.first(), selected.last()) else {
        return;
    };
    let mut allowed: BTreeSet<_> = fields
        .iter()
        .flat_map(|nodes| nodes.iter().map(LinkedNode::offset))
        .collect();
    for heading in fields[2] {
        if let Some(label) = heading
            .next_sibling()
            .filter(|node| node.kind() == SyntaxKind::Label)
        {
            allowed.insert(label.offset());
        }
    }

    for node in top_level {
        if node.offset() < first.offset() || node.offset() >= last.range().end {
            continue;
        }
        if node.kind().is_trivia()
            || node.kind() == SyntaxKind::Hash
            || allowed.contains(&node.offset())
        {
            continue;
        }
        diagnostics.push(error(
            path,
            "metadata.header_content",
            "only direct metadata constructs are allowed in the metadata header",
            Some(range(node)),
        ));
    }
}

fn require_one(
    path: &str,
    code: &str,
    field: &str,
    nodes: &[LinkedNode<'_>],
    diagnostics: &mut Vec<Diagnostic>,
) {
    if nodes.is_empty() {
        diagnostics.push(error(path, code, format!("missing required {field}"), None));
    }
    for duplicate in nodes.iter().skip(1) {
        diagnostics.push(error(
            path,
            code,
            format!("duplicate {field}"),
            Some(authored_range(duplicate)),
        ));
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
        })
        .collect::<Vec<_>>();
    diagnostics.extend(warnings.into_iter().map(|diagnostic| Diagnostic {
        path: path.to_owned(),
        code: "syntax.warning".to_owned(),
        severity: Severity::Warning,
        message: diagnostic.message.to_string(),
        range: None,
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = r#"#import "../lib/zettel.typ": zettel, abstract, keywords, category
#show: zettel

= Path _efficiency_ $x^2$ <2603231410>

#abstract[
Repeated *traffic*, @2603220935 and #code.
]

#keywords(
  "networks",
  "optimization",
)

#category.thoughts

Body @2603220935 and @9999999999. `@1111111111`
"#;

    #[test]
    fn extracts_rich_metadata_and_literal_reference_ranges() {
        let extracted = extract("2603231410", "zettel/2603231410.typ", VALID);

        let title = extracted.node.title.unwrap();
        assert_eq!(title.source, "Path _efficiency_ $x^2$");
        assert_eq!(title.text, "Path efficiency $x^2$");
        assert_eq!(
            &VALID[title.range.start as usize..title.range.end as usize],
            title.source
        );

        let abstract_value = extracted.node.abstract_value.unwrap();
        assert_eq!(
            abstract_value.text,
            "Repeated traffic, @2603220935 and #code."
        );
        assert_eq!(
            extracted.node.keywords,
            Some(vec!["networks".to_owned(), "optimization".to_owned()])
        );
        assert_eq!(extracted.node.category.as_deref(), Some("thoughts"));
        assert!(extracted.diagnostics.is_empty());

        let authored: Vec<_> = extracted
            .references
            .iter()
            .map(|reference| &VALID[reference.range.start as usize..reference.range.end as usize])
            .collect();
        assert_eq!(authored, vec!["@2603220935", "@2603220935", "@9999999999"]);
    }

    #[test]
    fn accepts_formatter_ordered_imports() {
        let source = VALID.replacen(
            "zettel, abstract, keywords, category",
            "abstract, category, keywords, zettel",
            1,
        );
        let extracted = extract("2603231410", "zettel/2603231410.typ", &source);

        assert!(
            extracted
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.code != "metadata.import")
        );
    }

    #[test]
    fn leaves_malformed_fields_absent_and_reports_syntax_errors() {
        let source = r#"#import "../lib/zettel.typ": zettel, abstract, keywords, category
#show: zettel

= Broken <wrong>

#abstract[
- block item

#keywords("valid", computed)

#category.thoughts
"#;
        let extracted = extract("2603231410", "zettel/2603231410.typ", source);

        assert!(extracted.node.abstract_value.is_none());
        assert!(extracted.node.keywords.is_none());
        assert!(
            extracted
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "syntax.error")
        );
        assert!(
            extracted
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "metadata.id_mismatch")
        );
    }

    #[test]
    fn rejects_computed_keyword_values() {
        let source = VALID.replace(
            "  \"networks\",\n  \"optimization\",",
            "  \"networks\",\n  computed,",
        );
        let extracted = extract("2603231410", "zettel/2603231410.typ", &source);

        assert!(extracted.node.keywords.is_none());
        let diagnostic = extracted
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "metadata.keywords")
            .expect("keyword diagnostic");
        let range = diagnostic.range.expect("keyword diagnostic range");
        assert!(source[range.start as usize..range.end as usize].starts_with("#keywords("));
    }

    #[test]
    fn rejects_block_structure_inside_an_abstract() {
        let source = VALID.replace("Repeated *traffic*, @2603220935 and #code.", "- one\n- two");
        let extracted = extract("2603231410", "zettel/2603231410.typ", &source);

        assert!(extracted.node.abstract_value.is_none());
        assert!(
            extracted
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("cannot contain"))
        );
    }
}
