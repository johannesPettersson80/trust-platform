use super::*;

#[test]
fn test_keywords_case_insensitive() {
    let tokens = lex("PROGRAM program Program PrOgRaM");
    let kinds: Vec<_> = tokens
        .iter()
        .filter(|(k, _)| !k.is_trivia())
        .map(|(kind, _)| *kind)
        .collect();
    assert_eq!(kinds, vec![TokenKind::KwProgram; 4]);
}

#[test]
fn test_additional_keywords() {
    let tokens = lex(
        "CHAR WCHAR LDATE ANY_DERIVED ANY_ELEMENTARY ANY_MAGNITUDE ANY_UNSIGNED \
             ANY_SIGNED ANY_DURATION ANY_CHARS ANY_CHAR EN ENO STEP END_STEP INITIAL_STEP \
             TRANSITION END_TRANSITION FROM R_EDGE F_EDGE OVERLAP overlap",
    );
    let kinds: Vec<_> = tokens
        .iter()
        .map(|(k, _)| *k)
        .filter(|k| !k.is_trivia())
        .collect();
    assert_eq!(
        kinds,
        vec![
            TokenKind::KwChar,
            TokenKind::KwWChar,
            TokenKind::KwLDate,
            TokenKind::KwAnyDerived,
            TokenKind::KwAnyElementary,
            TokenKind::KwAnyMagnitude,
            TokenKind::KwAnyUnsigned,
            TokenKind::KwAnySigned,
            TokenKind::KwAnyDuration,
            TokenKind::KwAnyChars,
            TokenKind::KwAnyChar,
            TokenKind::KwEn,
            TokenKind::KwEno,
            TokenKind::KwStep,
            TokenKind::KwEndStep,
            TokenKind::KwInitialStep,
            TokenKind::KwTransition,
            TokenKind::KwEndTransition,
            TokenKind::KwFrom,
            TokenKind::KwREdge,
            TokenKind::KwFEdge,
            TokenKind::KwOverlap,
            TokenKind::KwOverlap,
        ]
    );
}

#[test]
fn test_basic_operators() {
    let tokens = lex(":= = <> < <= > >= + - * / ** &");
    let kinds: Vec<_> = tokens
        .iter()
        .map(|(k, _)| *k)
        .filter(|k| !k.is_trivia())
        .collect();
    assert_eq!(
        kinds,
        vec![
            TokenKind::Assign,
            TokenKind::Eq,
            TokenKind::Neq,
            TokenKind::Lt,
            TokenKind::LtEq,
            TokenKind::Gt,
            TokenKind::GtEq,
            TokenKind::Plus,
            TokenKind::Minus,
            TokenKind::Star,
            TokenKind::Slash,
            TokenKind::Power,
            TokenKind::Ampersand
        ]
    );
}

#[test]
fn test_hash_prefixed_identifier_tokens() {
    let tokens = lex("#counter := #counter + 1;");
    let kinds: Vec<_> = tokens
        .iter()
        .map(|(k, _)| *k)
        .filter(|k| !k.is_trivia())
        .collect();
    assert_eq!(
        kinds,
        vec![
            TokenKind::Hash,
            TokenKind::Ident,
            TokenKind::Assign,
            TokenKind::Hash,
            TokenKind::Ident,
            TokenKind::Plus,
            TokenKind::IntLiteral,
            TokenKind::Semicolon
        ]
    );
}

#[test]
// IEC 61131-3 Ed.3 Table 5 (numeric literals)
fn test_integer_literals() {
    let tokens = lex("123 16#FF 2#1010 8#77 1_000_000");
    let kinds: Vec<_> = tokens
        .iter()
        .map(|(k, _)| *k)
        .filter(|k| !k.is_trivia())
        .collect();
    assert_eq!(kinds, vec![TokenKind::IntLiteral; 5]);
}

#[test]
// IEC 61131-3 Ed.3 Table 5 (numeric literals)
fn test_real_literals() {
    let tokens = lex("3.14 1.0E10 2.5e-3 1_000.000_1");
    let kinds: Vec<_> = tokens
        .iter()
        .map(|(k, _)| *k)
        .filter(|k| !k.is_trivia())
        .collect();
    assert_eq!(kinds, vec![TokenKind::RealLiteral; 4]);
}

#[test]
// IEC 61131-3 Ed.3 Table 3 (comments)
fn test_comments() {
    let tokens = lex("// line comment\n(* block \n comment *)");
    let kinds: Vec<_> = tokens.iter().map(|(k, _)| *k).collect();
    assert!(kinds.contains(&TokenKind::LineComment));
    assert!(kinds.contains(&TokenKind::BlockComment));
}

#[test]
fn test_direct_addresses() {
    let tokens = lex("%IX0.0 %QW10 %MD100 %IB5 %I* %Q* %M*");
    let kinds: Vec<_> = tokens
        .iter()
        .map(|(k, _)| *k)
        .filter(|k| !k.is_trivia())
        .collect();
    assert_eq!(kinds, vec![TokenKind::DirectAddress; 7]);
}

#[test]
// IEC 61131-3 Ed.3 Tables 6-7 (string literals)
fn test_strings() {
    let tokens = lex(r#"'hello' "world""#);
    let kinds: Vec<_> = tokens
        .iter()
        .map(|(k, _)| *k)
        .filter(|k| !k.is_trivia())
        .collect();
    assert_eq!(
        kinds,
        vec![TokenKind::StringLiteral, TokenKind::WideStringLiteral]
    );
}

#[test]
// IEC 61131-3 Ed.3 Tables 6-7 (string escapes)
fn test_string_escapes() {
    let tokens = lex(r#"'$N$L$$$'' "$T$R$$$"" '$0A' "$00C4""#);
    let kinds: Vec<_> = tokens
        .iter()
        .map(|(k, _)| *k)
        .filter(|k| !k.is_trivia())
        .collect();
    assert_eq!(
        kinds,
        vec![
            TokenKind::StringLiteral,
            TokenKind::WideStringLiteral,
            TokenKind::StringLiteral,
            TokenKind::WideStringLiteral
        ]
    );
}

#[test]
// IEC 61131-3 Ed.3 Tables 6-7 (string escapes)
fn test_invalid_string_escapes() {
    let tokens = lex(r#"'$Q' "$0G" "$123""#);
    let kinds: Vec<_> = tokens
        .iter()
        .map(|(k, _)| *k)
        .filter(|k| !k.is_trivia())
        .collect();
    assert!(kinds.contains(&TokenKind::Error));
    assert!(!kinds.contains(&TokenKind::StringLiteral));
    assert!(!kinds.contains(&TokenKind::WideStringLiteral));
}

#[test]
// IEC 61131-3 Ed.3 Tables 8-9 (duration/date-time literals)
fn test_time_literals() {
    let tokens = lex(
        "T#1h30m TIME#5s t#100ms LT#14.7s LTIME#5m_30s_500ms_100.1us t#12h4m34ms230us400ns T#-14ms",
    );
    let kinds: Vec<_> = tokens
        .iter()
        .map(|(k, _)| *k)
        .filter(|k| !k.is_trivia())
        .collect();
    assert_eq!(kinds, vec![TokenKind::TimeLiteral; 7]);
}

#[test]
fn test_function_block_keywords() {
    let tokens = lex("FUNCTION_BLOCK FB_Test END_FUNCTION_BLOCK");
    let kinds: Vec<_> = tokens
        .iter()
        .map(|(k, _)| *k)
        .filter(|k| !k.is_trivia())
        .collect();
    assert_eq!(
        kinds,
        vec![
            TokenKind::KwFunctionBlock,
            TokenKind::Ident,
            TokenKind::KwEndFunctionBlock
        ]
    );
}
