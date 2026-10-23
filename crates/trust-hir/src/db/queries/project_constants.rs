//! Revision-tracked constant locations. Collect each file once, then resolve its
//! initializer nodes from the original syntax tree without rescanning other POUs.
use super::collector::{ConstantExpressions, SymbolCollector};
use super::salsa_backend::{parse_green, SourceInput};
use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
struct ConstantLocation {
    scope: Option<SmolStr>,
    name: SmolStr,
    range: TextRange,
}

#[salsa::tracked(returns(ref))]
fn file_constant_locations(db: &dyn salsa::Database, input: SourceInput) -> Vec<ConstantLocation> {
    let root = SyntaxNode::new_root(parse_green(db, input).clone());
    let mut entries: Vec<_> = SymbolCollector::constant_expressions(&root)
        .into_iter()
        .map(|((scope, name), expr)| ConstantLocation {
            scope,
            name,
            range: expr.text_range(),
        })
        .collect();
    entries.sort_by(|a, b| (&a.scope, &a.name).cmp(&(&b.scope, &b.name)));
    entries
}

pub(super) fn project_constant_expressions(
    db: &dyn salsa::Database,
    files: &[(FileId, SourceInput)],
) -> ConstantExpressions {
    let mut expressions = ConstantExpressions::default();
    // Inputs are already in source-ID order, preserving first-declaration wins.
    for (_, input) in files {
        db.unwind_if_revision_cancelled();
        let entries = file_constant_locations(db, *input);
        if entries.is_empty() {
            continue;
        }
        let root = SyntaxNode::new_root(parse_green(db, *input).clone());
        for entry in entries {
            let element = root.covering_element(entry.range);
            let node = match element {
                rowan::NodeOrToken::Node(node) => Some(node),
                rowan::NodeOrToken::Token(token) => token.parent(),
            };
            let expression = node
                .and_then(|node| {
                    node.ancestors()
                        .filter(|node| {
                            node.text_range() == entry.range
                                && crate::db::diagnostics::is_expression_kind(node.kind())
                        })
                        .last()
                })
                .expect("constant range belongs to its source revision");
            expressions
                .entry((entry.scope.clone(), entry.name.clone()))
                .or_insert(expression);
        }
    }
    expressions
}

#[cfg(test)]
mod tests {
    use super::super::salsa_backend::SalsaDatabase;
    use super::*;
    use salsa::Setter;

    #[test]
    fn cached_constants_preserve_nodes_order_and_source_revision() {
        let mut db = SalsaDatabase::default();
        let first = SourceInput::new(
            &db,
            "NAMESPACE Plant VAR_GLOBAL CONSTANT N : INT := 4 + 1; END_VAR END_NAMESPACE"
                .to_owned(),
            TemporalStorage::default(),
        );
        let second = SourceInput::new(
            &db,
            "NAMESPACE Plant VAR_GLOBAL CONSTANT N : INT := 9; END_VAR END_NAMESPACE".to_owned(),
            TemporalStorage::default(),
        );
        let files = [(FileId(0), first), (FileId(1), second)];
        let key = (Some(SmolStr::new("PLANT")), SmolStr::new("N"));
        let before = project_constant_expressions(&db, &files);
        assert_eq!(before[&key].text().to_string().trim(), "4 + 1");
        let snapshot = |values: &ConstantExpressions| {
            values
                .iter()
                .map(|(key, node)| (key.clone(), (node.text_range(), node.text().to_string())))
                .collect::<std::collections::BTreeMap<_, _>>()
        };
        assert_eq!(
            snapshot(&before),
            snapshot(&SymbolCollector::constant_expressions(
                &SyntaxNode::new_root(parse_green(&db, first).clone())
            ))
        );
        assert_eq!(
            snapshot(&project_constant_expressions(&db, &files)),
            snapshot(&before)
        );
        // Equal-width value edits leave the location prelude unchanged, but the
        // initializer must still come from the new parse revision.
        let original_locations = file_constant_locations(&db, first).clone();
        first.set_text(&mut db).to(
            "NAMESPACE Plant VAR_GLOBAL CONSTANT N : INT := 6 + 2; END_VAR END_NAMESPACE"
                .to_owned(),
        );
        assert_eq!(file_constant_locations(&db, first), &original_locations);
        assert_eq!(
            project_constant_expressions(&db, &files)[&key]
                .text()
                .to_string()
                .trim(),
            "6 + 2"
        );
        first.set_text(&mut db).to(
            "// moved declaration\nNAMESPACE Plant VAR_GLOBAL CONSTANT N : INT := 7; END_VAR END_NAMESPACE".to_owned());
        let after = project_constant_expressions(&db, &files);
        assert_eq!(after[&key].text().to_string().trim(), "7");
        assert_ne!(before[&key].text_range(), after[&key].text_range());
        first
            .set_text(&mut db)
            .to("PROGRAM Main END_PROGRAM".to_owned());
        assert_eq!(
            project_constant_expressions(&db, &files)[&key]
                .text()
                .to_string()
                .trim(),
            "9"
        );
    }
}
