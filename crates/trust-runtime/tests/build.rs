use trust_hir::TypeId;
use trust_runtime::RuntimeBuilder;
use trust_syntax::SyntaxKind;

#[test]
fn workspace_builds() {
    assert_eq!(TypeId::BOOL, TypeId::BOOL);
    assert_eq!(SyntaxKind::Ident, SyntaxKind::Ident);
    let _runtime = RuntimeBuilder::new()
        .build()
        .expect("admit empty application");
}
