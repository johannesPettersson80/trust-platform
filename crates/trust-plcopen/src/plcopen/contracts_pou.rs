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
