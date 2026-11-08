//! The diagnostics of a source's syntax errors, as the editor shows them and every check reports
//! them: the language server's diagnostics, `trust-lsp check` and `trust-runtime check` and
//! `build` (specification 11 §6.8.1c).

use text_size::TextRange;
use trust_syntax::lexer::{lex_with_text, TokenKind};
use trust_syntax::parser::ParseError;

use super::{Diagnostic, DiagnosticCode};

/// The diagnostics of the parse errors `errors` of the source text `source`, one each: an error at
/// a token that has an identifier's shape but is no identifier of the language is E106, `invalid
/// identifier '<text>'`; an error that names what was expected is E002, a missing token; any other
/// is E001, an unexpected token.
#[must_use]
pub fn syntax_diagnostics(source: &str, errors: &[ParseError]) -> Vec<Diagnostic> {
    if errors.is_empty() {
        return Vec::new();
    }
    let invalid_identifiers: Vec<(TextRange, &str)> = lex_with_text(source)
        .into_iter()
        .filter(|(token, text)| token.kind == TokenKind::Error && has_identifier_shape(text))
        .map(|(token, text)| (token.range, text))
        .collect();
    errors
        .iter()
        .map(|error| {
            let invalid = invalid_identifiers
                .iter()
                .find(|(range, _)| *range == error.range);
            if let Some((_, text)) = invalid {
                Diagnostic::error(
                    DiagnosticCode::InvalidIdentifier,
                    error.range,
                    format!("invalid identifier '{text}'"),
                )
            } else if error.message.starts_with("expected ") {
                Diagnostic::error(DiagnosticCode::MissingToken, error.range, &error.message)
            } else {
                Diagnostic::error(DiagnosticCode::UnexpectedToken, error.range, &error.message)
            }
        })
        .collect()
}

/// Whether `text` begins with a letter or an underscore and holds only letters, digits and
/// underscores.
fn has_identifier_shape(text: &str) -> bool {
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first.is_alphabetic())
        && chars.all(|character| character == '_' || character.is_alphanumeric())
}

#[cfg(test)]
mod tests {
    use trust_syntax::parser::parse;

    use super::syntax_diagnostics;

    fn codes(source: &str) -> Vec<(&'static str, String)> {
        syntax_diagnostics(source, parse(source).errors())
            .into_iter()
            .map(|diagnostic| (diagnostic.code.code(), diagnostic.message))
            .collect()
    }

    /// The codes a parse error takes, as the editor has always shown them: `A__B`, which IEC
    /// 61131-3 §6.1.2 forbids (no multiple embedded underscores), is E106 naming the text, and a
    /// statement without its `;` is E002, whose message names what was expected; a source without
    /// errors has none. Catches an invalid identifier reported without its text, and a missing token
    /// reported as an unexpected one.
    #[test]
    fn a_parse_error_is_an_invalid_identifier_a_missing_token_or_an_unexpected_token() {
        let invalid = codes("PROGRAM Main\nVAR\n    A__B : INT;\nEND_VAR\nEND_PROGRAM\n");
        assert!(
            invalid.contains(&("E106", "invalid identifier 'A__B'".to_string())),
            "{invalid:?}"
        );
        let missing = codes("PROGRAM Main\nVAR x : INT; END_VAR\nx := 1\nEND_PROGRAM\n");
        assert!(
            missing
                .iter()
                .any(|(code, message)| *code == "E002" && message.starts_with("expected ")),
            "{missing:?}"
        );
        assert!(codes("PROGRAM Main\nEND_PROGRAM\n").is_empty());
    }
}
