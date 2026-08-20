use trust_syntax::lexer::{lex_with_text, TokenKind};

fn kinds(source: &str) -> Vec<TokenKind> {
    lex_with_text(source)
        .into_iter()
        .filter(|(token, _)| !token.kind.is_trivia())
        .map(|(token, _)| token.kind)
        .collect()
}

fn pairs(source: &str) -> Vec<(TokenKind, String)> {
    lex_with_text(source)
        .into_iter()
        .filter(|(token, _)| !token.kind.is_trivia())
        .map(|(token, text)| (token.kind, text.to_string()))
        .collect()
}

fn assert_single(source: &str, expected: TokenKind) {
    assert_eq!(kinds(source), vec![expected], "{source:?}");
}

fn assert_rejected_candidate(source: &str, forbidden: &[TokenKind]) {
    let tokens = lex_with_text(source);
    assert!(
        tokens
            .iter()
            .any(|(token, _)| token.kind == TokenKind::Error),
        "malformed candidate must emit Error: {source:?} -> {tokens:?}"
    );
    for kind in forbidden {
        assert!(
            tokens.iter().all(|(token, _)| token.kind != *kind),
            "malformed candidate must not publish {kind:?}: {source:?} -> {tokens:?}"
        );
    }
    assert_eq!(
        tokens.iter().map(|(_, text)| *text).collect::<String>(),
        source,
        "error recovery must retain every source byte"
    );
}

#[test]
fn lexical_contract_accepts_iec_identifier_forms() {
    for source in [
        "IW215", "LIM_SW_5", "LimSw5", "abcd", "ab_Cd", "_MAIN", "_12V7",
    ] {
        assert_single(source, TokenKind::Ident);
    }
}

#[test]
fn lexical_contract_keywords_are_case_insensitive() {
    for source in ["PROGRAM", "program", "PrOgRaM"] {
        assert_single(source, TokenKind::KwProgram);
    }
}

#[test]
fn lexical_contract_underscore_position_remains_identifier_significant() {
    assert_eq!(
        pairs("A_BCD AB_CD"),
        vec![
            (TokenKind::Ident, "A_BCD".to_string()),
            (TokenKind::Ident, "AB_CD".to_string())
        ]
    );
}

#[test]
fn lexical_contract_rejects_multiple_leading_underscores_as_one_candidate() {
    assert_rejected_candidate("__LIM_SW5", &[TokenKind::Ident]);
}

#[test]
fn lexical_contract_rejects_multiple_embedded_underscores_as_one_candidate() {
    assert_rejected_candidate("LIM__SW5", &[TokenKind::Ident]);
}

#[test]
fn lexical_contract_rejects_trailing_underscore_as_one_candidate() {
    assert_rejected_candidate("LIM_SW5_", &[TokenKind::Ident]);
}

#[test]
fn lexical_contract_rejects_sole_underscore_identifier() {
    assert_rejected_candidate("_", &[TokenKind::Ident]);
}

#[test]
fn lexical_contract_current_ascii_profile_rejects_unicode_identifier() {
    assert_rejected_candidate("värde", &[TokenKind::Ident]);
}

#[test]
fn lexical_contract_token_ranges_partition_exact_utf8_source() {
    let source = "value := \"Å\"; // done\n";
    let tokens = lex_with_text(source);
    let mut next = 0usize;
    for (token, text) in &tokens {
        assert_eq!(usize::from(token.range.start()), next);
        assert_eq!(
            &source[usize::from(token.range.start())..usize::from(token.range.end())],
            *text
        );
        next = usize::from(token.range.end());
    }
    assert_eq!(next, source.len());
}

#[test]
fn lexical_contract_space_tab_cr_and_lf_are_trivia() {
    let tokens = lex_with_text(" \t\r\n");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].0.kind, TokenKind::Whitespace);
    assert!(tokens[0].0.kind.is_trivia());
}

#[test]
fn lexical_contract_line_comment_stops_before_line_feed() {
    let tokens = lex_with_text("// comment\nPROGRAM");
    assert_eq!(tokens[0].0.kind, TokenKind::LineComment);
    assert_eq!(tokens[0].1, "// comment");
    assert_eq!(kinds("// comment\nPROGRAM"), vec![TokenKind::KwProgram]);
}

#[test]
fn lexical_contract_line_comment_stops_before_carriage_return() {
    let tokens = lex_with_text("// comment\rPROGRAM");
    assert_eq!(tokens[0].0.kind, TokenKind::LineComment);
    assert_eq!(tokens[0].1, "// comment");
    assert_eq!(kinds("// comment\rPROGRAM"), vec![TokenKind::KwProgram]);
}

#[test]
fn lexical_contract_line_comment_stops_before_form_feed() {
    let tokens = lex_with_text("// comment\u{000c}PROGRAM");
    assert_eq!(tokens[0].0.kind, TokenKind::LineComment);
    assert_eq!(tokens[0].1, "// comment");
    assert_eq!(
        kinds("// comment\u{000c}PROGRAM"),
        vec![TokenKind::KwProgram]
    );
}

#[test]
fn lexical_contract_pascal_comments_nest_with_matching_pairs() {
    let tokens = lex_with_text("(* outer (* inner *) outer *)");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].0.kind, TokenKind::BlockComment);
    assert_eq!(tokens[0].1, "(* outer (* inner *) outer *)");
}

#[test]
fn lexical_contract_c_comments_nest_with_matching_pairs() {
    let tokens = lex_with_text("/* outer /* inner */ outer */");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].0.kind, TokenKind::BlockComment);
    assert_eq!(tokens[0].1, "/* outer /* inner */ outer */");
}

#[test]
fn lexical_contract_alternative_delimiters_are_text_inside_other_comment_family() {
    let tokens = lex_with_text("(* /* text */ *) /* (* text *) */");
    assert_eq!(
        tokens
            .iter()
            .filter(|(token, _)| token.kind == TokenKind::BlockComment)
            .count(),
        2
    );
    assert!(tokens
        .iter()
        .all(|(token, _)| token.kind != TokenKind::Error));
}

#[test]
fn lexical_contract_unterminated_pascal_comment_is_error() {
    assert_rejected_candidate("(* unterminated", &[TokenKind::BlockComment]);
}

#[test]
fn lexical_contract_unterminated_c_comment_is_error() {
    assert_rejected_candidate("/* unterminated", &[TokenKind::BlockComment]);
}

#[test]
fn lexical_contract_comment_markers_inside_strings_are_not_comments() {
    assert_eq!(
        kinds("'(* text *)' \"/* text */\""),
        vec![TokenKind::StringLiteral, TokenKind::WideStringLiteral]
    );
}

#[test]
fn lexical_contract_balanced_pragma_is_lossless_trivia() {
    let tokens = lex_with_text("{VERSION 2.0}");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].0.kind, TokenKind::Pragma);
    assert_eq!(tokens[0].1, "{VERSION 2.0}");
    assert!(tokens[0].0.kind.is_trivia());
}

#[test]
fn lexical_contract_braces_inside_string_are_not_pragma() {
    assert_single("'{VERSION 2.0}'", TokenKind::StringLiteral);
}

#[test]
fn lexical_contract_unterminated_pragma_is_error() {
    assert_rejected_candidate("{VERSION 2.0", &[TokenKind::Pragma]);
}

#[test]
fn lexical_contract_accepts_decimal_digit_separators() {
    for source in ["0", "123_4", "9_876_543"] {
        assert_single(source, TokenKind::IntLiteral);
    }
}

#[test]
fn lexical_contract_accepts_binary_octal_and_hex_digits() {
    for source in ["2#1111_0000", "8#377", "16#Af_F0"] {
        assert_single(source, TokenKind::IntLiteral);
    }
}

#[test]
fn lexical_contract_accepts_real_fraction_and_exponent_forms() {
    for source in ["0.0", "3.14159_26", "1.34E-12", "1.0e+6"] {
        assert_single(source, TokenKind::RealLiteral);
    }
}

#[test]
fn lexical_contract_typed_literal_keeps_prefix_and_signed_payload_separate() {
    assert_eq!(
        pairs("INT#-123 BOOL#TRUE"),
        vec![
            (TokenKind::TypedLiteralPrefix, "INT#".to_string()),
            (TokenKind::Minus, "-".to_string()),
            (TokenKind::IntLiteral, "123".to_string()),
            (TokenKind::TypedLiteralPrefix, "BOOL#".to_string()),
            (TokenKind::KwTrue, "TRUE".to_string())
        ]
    );
}

#[test]
fn lexical_contract_rejects_leading_numeric_separator() {
    assert_rejected_candidate("_123", &[TokenKind::IntLiteral]);
}

#[test]
fn lexical_contract_rejects_trailing_numeric_separator() {
    assert_rejected_candidate("123_", &[TokenKind::IntLiteral]);
}

#[test]
fn lexical_contract_rejects_repeated_numeric_separator() {
    assert_rejected_candidate("12__34", &[TokenKind::IntLiteral]);
}

#[test]
fn lexical_contract_rejects_digit_outside_binary_base() {
    assert_rejected_candidate("2#102", &[TokenKind::IntLiteral]);
}

#[test]
fn lexical_contract_rejects_digit_outside_octal_base() {
    assert_rejected_candidate("8#8", &[TokenKind::IntLiteral]);
}

#[test]
fn lexical_contract_rejects_digit_outside_hexadecimal_base() {
    assert_rejected_candidate("16#FG", &[TokenKind::IntLiteral]);
}

#[test]
fn lexical_contract_rejects_sign_embedded_after_based_prefix() {
    assert_rejected_candidate("2#-1", &[TokenKind::IntLiteral]);
}

#[test]
fn lexical_contract_rejects_exponent_without_digits() {
    assert_rejected_candidate("1.0E+", &[TokenKind::RealLiteral]);
}

#[test]
fn lexical_contract_accepts_all_narrow_string_escape_families() {
    for source in ["'$$'", "'$''", "'$L$N$P$R$T'", "'$0A'"] {
        assert_single(source, TokenKind::StringLiteral);
    }
}

#[test]
fn lexical_contract_accepts_all_wide_string_escape_families() {
    for source in [r#""$$""#, r#""$"""#, r#""$L$N$P$R$T""#, r#""$00C4""#] {
        assert_single(source, TokenKind::WideStringLiteral);
    }
}

#[test]
fn lexical_contract_rejects_unknown_narrow_string_escape() {
    assert_rejected_candidate("'$Q'", &[TokenKind::StringLiteral]);
}

#[test]
fn lexical_contract_rejects_quote_escape_in_wrong_string_family() {
    assert_rejected_candidate("'$\"'", &[TokenKind::StringLiteral]);
}

#[test]
fn lexical_contract_rejects_short_narrow_hex_escape() {
    assert_rejected_candidate("'$A'", &[TokenKind::StringLiteral]);
}

#[test]
fn lexical_contract_rejects_short_wide_hex_escape() {
    assert_rejected_candidate(r#""$0A""#, &[TokenKind::WideStringLiteral]);
}

#[test]
fn lexical_contract_rejects_unterminated_narrow_string() {
    assert_rejected_candidate("'unterminated", &[TokenKind::StringLiteral]);
}

#[test]
fn lexical_contract_rejects_unterminated_wide_string() {
    assert_rejected_candidate("\"unterminated", &[TokenKind::WideStringLiteral]);
}

#[test]
fn lexical_contract_accepts_duration_prefix_aliases() {
    for source in ["T#1s", "TIME#1s", "LT#1s", "LTIME#1s"] {
        assert_single(source, TokenKind::TimeLiteral);
    }
}

#[test]
fn lexical_contract_duration_units_and_prefix_are_case_insensitive() {
    for source in ["t#25H_15M", "Time#5d_14h_12m_18s_3.5ms"] {
        assert_single(source, TokenKind::TimeLiteral);
    }
}

#[test]
fn lexical_contract_duration_fraction_is_allowed_on_least_significant_unit() {
    assert_single("LTIME#5m_30s_500ms_100.1us", TokenKind::TimeLiteral);
}

#[test]
fn lexical_contract_rejects_duration_fraction_before_less_significant_unit() {
    assert_rejected_candidate("T#1.5h_30m", &[TokenKind::TimeLiteral]);
}

#[test]
fn lexical_contract_rejects_duration_exponent() {
    assert_rejected_candidate("T#1e3s", &[TokenKind::TimeLiteral]);
}

#[test]
fn lexical_contract_accepts_date_prefix_aliases() {
    for source in [
        "D#1984-06-25",
        "DATE#1984-06-25",
        "LD#1984-06-25",
        "LDATE#1984-06-25",
    ] {
        assert_single(source, TokenKind::DateLiteral);
    }
}

#[test]
fn lexical_contract_accepts_time_of_day_prefix_aliases() {
    for source in [
        "TOD#15:36:55.36",
        "TIME_OF_DAY#15:36:55.36",
        "LTOD#15:36:55.360_227_400",
        "LTIME_OF_DAY#15:36:55.360_227_400",
    ] {
        assert_single(source, TokenKind::TimeOfDayLiteral);
    }
}

#[test]
fn lexical_contract_accepts_date_and_time_prefix_aliases() {
    for source in [
        "DT#1984-06-25-15:36:55.36",
        "DATE_AND_TIME#1984-06-25-15:36:55.36",
        "LDT#1984-06-25-15:36:55.360_227_400",
        "LDATE_AND_TIME#1984-06-25-15:36:55.360_227_400",
    ] {
        assert_single(source, TokenKind::DateAndTimeLiteral);
    }
}

#[test]
fn lexical_contract_rejects_date_with_wrong_field_width() {
    assert_rejected_candidate("D#1984-6-25", &[TokenKind::DateLiteral]);
}

#[test]
fn lexical_contract_retains_shape_valid_invalid_calendar_date_as_date_token() {
    assert_single("D#2023-02-29", TokenKind::DateLiteral);
}

#[test]
fn lexical_contract_retains_shape_valid_invalid_clock_as_tod_token() {
    assert_single("TOD#24:00:00", TokenKind::TimeOfDayLiteral);
}
