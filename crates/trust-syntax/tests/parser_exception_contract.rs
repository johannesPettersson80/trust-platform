//! Specification 06 §15: CODESYS's `__TRY`, `__CATCH`, `__FINALLY` and `__ENDTRY`, and the system type
//! `__SYSTEM.ExceptionCode` in a declaration and in an expression.
mod common;
use common::*;

/// The kinds of the children of the first node of `kind` in `source`, which must parse.
fn children_of(source: &str, kind: SyntaxKind) -> Vec<SyntaxKind> {
    let parsed = parse(source);
    assert!(parsed.ok(), "{source}\n{:?}", parsed.errors());
    let node = parsed
        .syntax()
        .descendants()
        .find(|node| node.kind() == kind)
        .unwrap_or_else(|| panic!("no {kind:?} in {source}"));
    node.children().map(|child| child.kind()).collect()
}

/// A `__TRY` with both clauses is one statement: its statement, then the `__CATCH` clause with the
/// variable and its statement, then the `__FINALLY` clause with its statement. Wrong behaviour caught:
/// the words lexed as malformed names ("expected statement"), and a clause's statements parsed as the
/// try statements.
#[test]
fn a_try_statement_holds_its_statements_and_both_clauses() {
    let source = "PROGRAM P\nVAR x : INT; e : __SYSTEM.ExceptionCode; END_VAR\n\
                  __TRY\n    x := 10 / x;\n__CATCH(e)\n    x := 1;\n__FINALLY\n    x := x + 1;\n__ENDTRY;\nEND_PROGRAM";
    assert_eq!(
        children_of(source, SyntaxKind::TryStmt),
        vec![
            SyntaxKind::AssignStmt,
            SyntaxKind::CatchClause,
            SyntaxKind::FinallyClause
        ]
    );
    assert_eq!(
        children_of(source, SyntaxKind::CatchClause),
        vec![SyntaxKind::NameRef, SyntaxKind::AssignStmt]
    );
    assert_eq!(
        children_of(source, SyntaxKind::FinallyClause),
        vec![SyntaxKind::AssignStmt]
    );
}

/// Either clause alone is a `__TRY`, and neither is refused: one of them is required.
#[test]
fn a_try_statement_needs_a_catch_or_a_finally_clause() {
    for source in [
        "PROGRAM P\n__TRY ; __CATCH(e) ; __ENDTRY;\nEND_PROGRAM",
        "PROGRAM P\n__TRY ; __FINALLY ; __ENDTRY;\nEND_PROGRAM",
    ] {
        assert!(parse(source).ok(), "{source}");
    }
    let without = parse("PROGRAM P\n__TRY ; __ENDTRY;\nEND_PROGRAM");
    assert!(
        without
            .errors()
            .iter()
            .any(|error| error.to_string().contains("expected __CATCH or __FINALLY")),
        "{:?}",
        without.errors()
    );
}

/// `__SYSTEM.ExceptionCode` is a type in a declaration and the base of a value in an expression.
#[test]
fn the_system_exception_code_type_parses_in_declarations_and_expressions() {
    let source = "PROGRAM P\nVAR e : __SYSTEM.ExceptionCode; b : BOOL; END_VAR\n\
                  b := e = __SYSTEM.ExceptionCode.RTSEXCPT_DIVIDEBYZERO;\nEND_PROGRAM";
    assert_eq!(
        children_of(source, SyntaxKind::TypeRef),
        vec![SyntaxKind::SystemName]
    );
    let parsed = parse(source);
    assert!(parsed
        .syntax()
        .descendants()
        .any(|node| node.kind() == SyntaxKind::FieldExpr
            && node.text().to_string().contains("RTSEXCPT_DIVIDEBYZERO")));
}
