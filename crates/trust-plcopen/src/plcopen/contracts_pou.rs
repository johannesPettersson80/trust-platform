#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlcopenPouType {
    Program,
    Function,
    FunctionBlock,
    Action,
}

impl PlcopenPouType {
    fn as_xml(self) -> &'static str {
        match self {
            Self::Program => "program",
            Self::Function => "function",
            Self::FunctionBlock => "functionBlock",
            Self::Action => "action",
        }
    }

    fn declaration_keyword(self) -> &'static str {
        match self {
            Self::Program => "PROGRAM",
            Self::Function => "FUNCTION",
            Self::FunctionBlock => "FUNCTION_BLOCK",
            Self::Action => "ACTION",
        }
    }

    fn end_keyword(self) -> &'static str {
        match self {
            Self::Program => "END_PROGRAM",
            Self::Function => "END_FUNCTION",
            Self::FunctionBlock => "END_FUNCTION_BLOCK",
            Self::Action => "END_ACTION",
        }
    }

    fn end_token(self) -> TokenKind {
        match self {
            Self::Program => TokenKind::KwEndProgram,
            Self::Function => TokenKind::KwEndFunction,
            Self::FunctionBlock => TokenKind::KwEndFunctionBlock,
            Self::Action => TokenKind::KwEndAction,
        }
    }

    /// The byte offset of the last end keyword of this POU kind in `source`, before which merged
    /// actions and methods go; `None` when the source has none. The lexer reads the keyword in any
    /// case, as IEC 61131-3 §6.1.3 does, and never inside a comment or a string.
    fn end_offset(self, source: &str) -> Option<usize> {
        let end = self.end_token();
        lex(source)
            .iter()
            .rev()
            .find(|token| token.kind == end)
            .map(|token| usize::from(token.range.start()))
    }

    fn from_xml(text: &str) -> Option<Self> {
        let normalized = text
            .chars()
            .filter(|ch| ch.is_ascii_alphanumeric())
            .map(|ch| ch.to_ascii_lowercase())
            .collect::<String>();
        match normalized.as_str() {
            "program" | "prg" => Some(Self::Program),
            "function" | "fc" | "fun" => Some(Self::Function),
            "functionblock" | "fb" => Some(Self::FunctionBlock),
            "action" | "act" => Some(Self::Action),
            _ => None,
        }
    }
}
