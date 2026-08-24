use super::*;

#[test]
fn test_test_pou_keywords() {
    let tokens = lex(
        "TEST_PROGRAM Test END_TEST_PROGRAM TEST_FUNCTION_BLOCK FB_Test END_TEST_FUNCTION_BLOCK",
    );
    let kinds: Vec<_> = tokens
        .iter()
        .map(|(k, _)| *k)
        .filter(|k| !k.is_trivia())
        .collect();
    assert_eq!(
        kinds,
        vec![
            TokenKind::KwTestProgram,
            TokenKind::Ident,
            TokenKind::KwEndTestProgram,
            TokenKind::KwTestFunctionBlock,
            TokenKind::Ident,
            TokenKind::KwEndTestFunctionBlock
        ]
    );
}

#[test]
fn test_class_and_configuration_keywords() {
    let tokens = lex(
            "CLASS C END_CLASS CONFIGURATION Conf END_CONFIGURATION RESOURCE Res END_RESOURCE ON READ_WRITE READ_ONLY USING",
        );
    let kinds: Vec<_> = tokens
        .iter()
        .map(|(k, _)| *k)
        .filter(|k| !k.is_trivia())
        .collect();
    assert_eq!(
        kinds,
        vec![
            TokenKind::KwClass,
            TokenKind::Ident,
            TokenKind::KwEndClass,
            TokenKind::KwConfiguration,
            TokenKind::Ident,
            TokenKind::KwEndConfiguration,
            TokenKind::KwResource,
            TokenKind::Ident,
            TokenKind::KwEndResource,
            TokenKind::KwOn,
            TokenKind::KwReadWrite,
            TokenKind::KwReadOnly,
            TokenKind::KwUsing
        ]
    );
}

#[test]
fn test_var_keywords() {
    let tokens = lex("VAR VAR_INPUT VAR_OUTPUT VAR_IN_OUT VAR_TEMP END_VAR");
    let kinds: Vec<_> = tokens
        .iter()
        .map(|(k, _)| *k)
        .filter(|k| !k.is_trivia())
        .collect();
    assert_eq!(
        kinds,
        vec![
            TokenKind::KwVar,
            TokenKind::KwVarInput,
            TokenKind::KwVarOutput,
            TokenKind::KwVarInOut,
            TokenKind::KwVarTemp,
            TokenKind::KwEndVar
        ]
    );
}

#[test]
fn test_pragmas() {
    // IEC 61131-3 Table 4 examples
    let tokens = lex("{VERSION 2.0} {AUTHOR JHC} {x:= 256, y:= 384}");
    let pragma_texts: Vec<_> = tokens
        .iter()
        .filter(|(kind, _)| *kind == TokenKind::Pragma)
        .map(|(_, text)| *text)
        .collect();
    assert_eq!(
        pragma_texts,
        vec!["{VERSION 2.0}", "{AUTHOR JHC}", "{x:= 256, y:= 384}"]
    );
    // Pragmas are trivia, so filtering them should leave nothing
    let non_trivia: Vec<_> = tokens.iter().filter(|(k, _)| !k.is_trivia()).collect();
    assert!(non_trivia.is_empty());
}

#[test]
fn test_pragma_with_code() {
    let tokens = lex("{VERSION 2.0} x := 42;");
    let pragma_texts: Vec<_> = tokens
        .iter()
        .filter(|(kind, _)| *kind == TokenKind::Pragma)
        .map(|(_, text)| *text)
        .collect();
    assert_eq!(pragma_texts, vec!["{VERSION 2.0}"]);
    let kinds: Vec<_> = tokens
        .iter()
        .map(|(k, _)| *k)
        .filter(|k| !k.is_trivia())
        .collect();
    // Pragma is trivia, only x, :=, 42, ; remain
    assert_eq!(
        kinds,
        vec![
            TokenKind::Ident,
            TokenKind::Assign,
            TokenKind::IntLiteral,
            TokenKind::Semicolon
        ]
    );
}

#[test]
fn test_pragma_content_preserved() {
    let tokens = lex("{AUTHOR JHC}");
    let pragma_tokens: Vec<_> = tokens
        .iter()
        .filter(|(k, _)| *k == TokenKind::Pragma)
        .collect();
    assert_eq!(pragma_tokens.len(), 1);
    assert_eq!(pragma_tokens[0].1, "{AUTHOR JHC}");
}
