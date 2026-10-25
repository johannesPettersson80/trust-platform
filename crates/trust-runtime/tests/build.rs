use trust_hir::TypeId;
use trust_runtime::harness::{CompileSession, SourceFile};
use trust_syntax::SyntaxKind;

#[test]
fn workspace_builds() {
    assert_eq!(TypeId::BOOL, TypeId::BOOL);
    assert_eq!(SyntaxKind::Ident, SyntaxKind::Ident);
    // A project needs a PROGRAM (specification 18 §2.2); one without statements is admitted.
    let declarations = CompileSession::from_sources(vec![SourceFile::with_path(
        "main.st",
        "PROGRAM Main END_PROGRAM",
    )])
    .declarations()
    .expect("admit a program without statements");
    assert_eq!(declarations.metadata().programs().len(), 1);
}
