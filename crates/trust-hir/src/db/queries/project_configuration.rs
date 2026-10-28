//! Find configuration-bearing sources once per revision. VAR_CONFIG checking
//! needs their original roots, not another traversal of every equipment body.
use super::salsa_backend::{parse_green, SourceInput};
use super::*;

#[salsa::tracked]
fn has_config_initializers(db: &dyn salsa::Database, input: SourceInput) -> bool {
    SyntaxNode::new_root(parse_green(db, input).clone())
        .descendants()
        .any(|node| node.kind() == SyntaxKind::ConfigInit)
}

pub(super) fn project_config_roots(
    db: &dyn salsa::Database,
    files: &[(FileId, SourceInput)],
) -> Vec<SyntaxNode> {
    files
        .iter()
        .filter_map(|(_, input)| {
            db.unwind_if_revision_cancelled();
            has_config_initializers(db, *input)
                .then(|| SyntaxNode::new_root(parse_green(db, *input).clone()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::salsa_backend::SalsaDatabase;
    use super::*;
    use salsa::Setter;

    #[test]
    fn configuration_root_selection_follows_added_moved_and_removed_bindings() {
        let mut db = SalsaDatabase::default();
        let source = SourceInput::new(&db, "PROGRAM Main END_PROGRAM".to_owned());
        let files = [(FileId(0), source)];
        assert!(project_config_roots(&db, &files).is_empty());
        let config =
            "CONFIGURATION C VAR_CONFIG Main.Value AT %IX0.0 : BOOL; END_VAR END_CONFIGURATION";
        source.set_text(&mut db).to(config.to_owned());
        let roots = project_config_roots(&db, &files);
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].text().to_string(), config);
        source.set_text(&mut db).to(format!("// moved\n{config}"));
        assert!(project_config_roots(&db, &files)[0]
            .text()
            .to_string()
            .starts_with("// moved"));
        source
            .set_text(&mut db)
            .to("PROGRAM Main END_PROGRAM".to_owned());
        assert!(project_config_roots(&db, &files).is_empty());
    }
}
