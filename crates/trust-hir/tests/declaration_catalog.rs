use trust_hir::db::{Database, FileId, SemanticDatabase, SourceDatabase};
use trust_hir::semantic::{DeclarationKind, SemanticRole};
use trust_hir::TypeId;

#[test]
fn declaration_catalog_exposes_qualified_declarations_roles_and_sources() {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(
        file,
        r#"
NAMESPACE CellA
INTERFACE IProbe
    METHOD Read : INT
    END_METHOD
END_INTERFACE

FUNCTION_BLOCK Probe IMPLEMENTS IProbe
VAR PUBLIC
    value : INT;
END_VAR
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    instance : Probe;
END_VAR
END_PROGRAM
END_NAMESPACE
"#
        .to_string(),
    );

    let analysis = db.analyze(file);
    let catalog = &analysis.declaration_catalog;

    let program = catalog
        .find_qualified("CellA.Main")
        .expect("catalog should include namespaced PROGRAM");
    assert_eq!(program.kind(), DeclarationKind::Program);
    assert_eq!(program.role(), SemanticRole::ScopeOwner);
    assert_eq!(program.source().file_id(), file);
    assert_eq!(program.qualified_name().display(), "CellA.Main");

    let fb = catalog
        .find_qualified("CellA.Probe")
        .expect("catalog should include namespaced FUNCTION_BLOCK");
    assert_eq!(fb.kind(), DeclarationKind::FunctionBlock);
    assert_eq!(fb.role(), SemanticRole::Type);
    assert_eq!(fb.source().file_id(), file);

    let member = catalog
        .find_qualified("CellA.Probe.value")
        .expect("catalog should include owned FB member");
    assert_eq!(member.kind(), DeclarationKind::Variable);
    assert_eq!(member.role(), SemanticRole::Value);
    assert_eq!(member.owner_symbol_id(), Some(fb.symbol_id()));
    assert!(member.owner_scope_id().is_some());

    let references = catalog.references();
    assert!(
        references.iter().any(|reference| {
            let referenced = reference.name().display().to_ascii_uppercase();
            reference.owner_symbol_id() == fb.symbol_id()
                && referenced.ends_with("IPROBE")
                && reference.outcome().is_resolved()
        }),
        "catalog should retain the resolved IMPLEMENTS reference for Probe"
    );
}

#[test]
fn declaration_catalog_marks_project_imports_and_translated_type_identity() {
    let mut db = Database::new();
    let library = FileId(0);
    let application = FileId(1);
    db.set_source_text(
        library,
        r#"
NAMESPACE Lib
FUNCTION_BLOCK Probe
VAR PUBLIC
    value : INT;
END_VAR
END_FUNCTION_BLOCK
END_NAMESPACE
"#
        .to_string(),
    );
    db.set_source_text(
        application,
        r#"
PROGRAM Main
VAR
    probe : Lib.Probe;
END_VAR
END_PROGRAM
"#
        .to_string(),
    );

    let analysis = db.analyze(application);
    let catalog = &analysis.declaration_catalog;

    let imported = catalog
        .find_qualified("Lib.Probe")
        .expect("catalog should include imported function block");
    assert!(imported.is_imported());
    assert_eq!(imported.source().file_id(), library);
    assert_ne!(imported.type_id(), TypeId::UNKNOWN);

    let local = catalog
        .find_qualified("Main")
        .expect("catalog should include local program");
    assert!(!local.is_imported());
    assert_eq!(local.source().file_id(), application);
}
