//! Main parser implementation.

use crate::lexer::{lex, Token, TokenKind};
use crate::parser::event::Event;
use crate::parser::sink::Sink;
use crate::parser::source::Source;
use crate::parser::{Parse, ParseError};
use crate::syntax::SyntaxKind;
use drop_bomb::DropBomb;

/// Parses source text into a syntax tree.
#[must_use]
pub fn parse(source: &str) -> Parse {
    let tokens = lex(source);
    let parser = Parser::new(&tokens, source);
    let (events, errors) = parser.parse();

    let sink = Sink::new(&tokens, source, events);
    let (green_node, mut sink_errors) = sink.finish();

    let mut all_errors = errors;
    all_errors.append(&mut sink_errors);

    Parse {
        green_node,
        errors: all_errors,
    }
}

/// The parser state.
pub(crate) struct Parser<'t, 'src> {
    pub(crate) source: Source<'t, 'src>,
    pub(crate) events: Vec<Event>,
    errors: Vec<ParseError>,
    pub(crate) expr_depth: usize,
}

pub(crate) struct Marker {
    pos: usize,
    bomb: DropBomb,
}

impl Marker {
    pub(crate) fn complete(
        mut self,
        parser: &mut Parser<'_, '_>,
        kind: SyntaxKind,
    ) -> CompletedMarker {
        self.bomb.defuse();
        match parser.events.get_mut(self.pos) {
            Some(Event::Placeholder) => {
                parser.events[self.pos] = Event::Start {
                    kind,
                    forward_parent: None,
                };
            }
            Some(Event::Start {
                kind: existing_kind,
                ..
            }) => {
                *existing_kind = kind;
            }
            _ => {}
        }
        parser.events.push(Event::Finish);
        CompletedMarker { pos: self.pos }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct CompletedMarker {
    pub(crate) pos: usize,
}

impl CompletedMarker {
    pub(crate) fn precede(self, parser: &mut Parser<'_, '_>) -> Marker {
        let new_pos = parser.events.len();
        parser.events.push(Event::Placeholder);
        set_forward_parent(&mut parser.events, self.pos, new_pos);
        Marker {
            pos: new_pos,
            bomb: DropBomb::new("uncompleted marker"),
        }
    }
}

fn set_forward_parent(events: &mut [Event], from: usize, to: usize) {
    let mut current = from;
    loop {
        match &mut events[current] {
            Event::Start {
                forward_parent: Some(fp),
                ..
            } => {
                current += *fp as usize;
            }
            Event::Start { forward_parent, .. } => {
                *forward_parent = Some((to - current) as u32);
                break;
            }
            _ => break,
        }
    }
}

impl<'t, 'src> Parser<'t, 'src> {
    fn new(tokens: &'t [Token], source: &'src str) -> Self {
        Self {
            source: Source::new(tokens, source),
            events: Vec::new(),
            errors: Vec::new(),
            expr_depth: 0,
        }
    }

    fn parse(mut self) -> (Vec<Event>, Vec<ParseError>) {
        // Start the root node
        self.start_node(SyntaxKind::SourceFile);

        // Parse top-level items
        while !self.at_end() {
            if self.at(TokenKind::KwUsing) {
                self.parse_using_directive();
            } else if self.at(TokenKind::KwProgram) || self.at(TokenKind::KwTestProgram) {
                self.parse_program();
            } else if self.at(TokenKind::KwFunction) {
                self.parse_function();
            } else if self.at(TokenKind::KwFunctionBlock) || self.at(TokenKind::KwTestFunctionBlock)
            {
                self.parse_function_block();
            } else if self.at(TokenKind::KwClass) {
                self.parse_class();
            } else if self.at(TokenKind::KwConfiguration) {
                self.parse_configuration();
            } else if self.at(TokenKind::KwInterface) {
                self.parse_interface();
            } else if self.at(TokenKind::KwType) {
                self.parse_type_decl();
            } else if self.at(TokenKind::KwNamespace) {
                self.parse_namespace();
            } else if self.current().is_trivia() {
                self.bump();
            } else {
                // Error recovery: skip unknown token
                self.error(
                    "expected PROGRAM, TEST_PROGRAM, FUNCTION, FUNCTION_BLOCK, TEST_FUNCTION_BLOCK, CLASS, CONFIGURATION, INTERFACE, TYPE, or NAMESPACE",
                );
                self.bump();
            }
        }

        self.finish_node();

        (self.events, self.errors)
    }

    // =========================================================================
    // Helper Methods
    // =========================================================================

    pub(crate) fn current(&self) -> TokenKind {
        self.source.current()
    }

    pub(crate) fn at(&self, kind: TokenKind) -> bool {
        self.source.peek_kind() == kind
    }

    pub(crate) fn at_end(&self) -> bool {
        self.source.at_end()
    }

    pub(crate) fn peek_kind_n(&self, n: usize) -> TokenKind {
        self.source.peek_kind_n(n)
    }

    pub(crate) fn bump(&mut self) {
        let kind = self.source.current();
        self.events.push(Event::token(SyntaxKind::from(kind)));
        self.source.bump();
    }

    pub(crate) fn start(&mut self) -> Marker {
        let pos = self.events.len();
        self.events.push(Event::Placeholder);
        Marker {
            pos,
            bomb: DropBomb::new("uncompleted marker"),
        }
    }

    pub(crate) fn start_node(&mut self, kind: SyntaxKind) {
        self.events.push(Event::start(kind));
    }

    pub(crate) fn finish_node(&mut self) {
        self.events.push(Event::Finish);
    }

    pub(crate) fn error(&mut self, message: &str) {
        let range = self
            .source
            .current_token()
            .map(|t| t.range)
            .unwrap_or_else(|| text_size::TextRange::empty(text_size::TextSize::from(0)));

        self.errors.push(ParseError {
            message: message.to_string(),
            range,
        });
    }

    /// Skip tokens until we find a synchronization point for error recovery.
    /// This helps the parser continue after encountering an error.
    #[allow(dead_code)]
    pub(crate) fn recover_to_sync_point(&mut self) {
        while !self.at_end() {
            // Check if current token is a sync point
            if self.is_sync_point() {
                break;
            }
            self.bump();
        }
    }

    /// Returns true if the current token is a synchronization point.
    pub(crate) fn is_sync_point(&self) -> bool {
        matches!(
            self.current(),
            // Statement terminators
            TokenKind::Semicolon
            // End of control flow
            | TokenKind::KwEndIf
            | TokenKind::KwEndFor
            | TokenKind::KwEndWhile
            | TokenKind::KwEndRepeat
            | TokenKind::KwEndCase
            // End of blocks
            | TokenKind::KwEndVar
            | TokenKind::KwEndType
            | TokenKind::KwEndStruct
            | TokenKind::KwEndUnion
            // End of POUs
            | TokenKind::KwEndProgram
            | TokenKind::KwEndTestProgram
            | TokenKind::KwEndFunction
            | TokenKind::KwEndFunctionBlock
            | TokenKind::KwEndTestFunctionBlock
            | TokenKind::KwEndClass
            | TokenKind::KwEndMethod
            | TokenKind::KwEndProperty
            | TokenKind::KwEndInterface
            | TokenKind::KwEndNamespace
            | TokenKind::KwEndConfiguration
            | TokenKind::KwEndResource
            | TokenKind::KwEndAction
            | TokenKind::KwEndGet
            | TokenKind::KwEndSet
            // Start of new constructs (recover at next item)
            | TokenKind::KwProgram
            | TokenKind::KwTestProgram
            | TokenKind::KwFunction
            | TokenKind::KwFunctionBlock
            | TokenKind::KwTestFunctionBlock
            | TokenKind::KwClass
            | TokenKind::KwMethod
            | TokenKind::KwProperty
            | TokenKind::KwInterface
            | TokenKind::KwNamespace
            | TokenKind::KwConfiguration
            | TokenKind::KwResource
            | TokenKind::KwTask
            | TokenKind::KwType
            | TokenKind::KwAction
            | TokenKind::KwVarAccess
            | TokenKind::KwVarConfig
            // Variable blocks
            | TokenKind::KwVar
            | TokenKind::KwVarInput
            | TokenKind::KwVarOutput
            | TokenKind::KwVarInOut
            | TokenKind::KwVarTemp
            | TokenKind::KwVarGlobal
            | TokenKind::KwVarExternal
        )
    }

    /// Returns true when a statement list should stop for recovery.
    pub(crate) fn at_stmt_list_end(&self) -> bool {
        self.is_sync_point() && !self.current().can_start_statement()
    }

    /// Recover at statement level - skip to next statement or block end.
    pub(crate) fn recover_statement(&mut self) {
        while !self.at_end() {
            if self.at(TokenKind::Semicolon) {
                self.bump();
                break;
            }
            if self.is_sync_point() || self.current().can_start_statement() {
                break;
            }
            self.bump();
        }
    }

    /// Consume a statement terminator, or insert it when unambiguous.
    pub(crate) fn expect_semicolon(&mut self) {
        if self.at(TokenKind::Semicolon) {
            self.bump();
            return;
        }

        if self.at_semicolon_insertion_point() {
            self.error("expected ';'");
            return;
        }

        self.error("expected ';'");
        self.recover_statement();
    }

    fn at_semicolon_insertion_point(&self) -> bool {
        if self.at_end() {
            return true;
        }

        if self.is_sync_point() || self.current().can_start_statement() {
            return true;
        }

        if matches!(
            self.current(),
            TokenKind::KwElse | TokenKind::KwElsif | TokenKind::KwUntil
        ) {
            return true;
        }

        self.current().can_start_expr() && self.source.has_case_label_ahead()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty() {
        let parse = parse("");
        assert!(parse.ok());
    }

    #[test]
    fn test_parse_simple_program() {
        let source = "PROGRAM Test END_PROGRAM";
        let parse = parse(source);
        assert!(parse.ok(), "errors: {:?}", parse.errors());
    }

    #[test]
    fn test_parse_function_block() {
        let source = r#"
FUNCTION_BLOCK FB_Motor
VAR_INPUT
    enable : BOOL;
END_VAR
END_FUNCTION_BLOCK
"#;
        let parse = parse(source);
        assert!(parse.ok(), "errors: {:?}", parse.errors());
    }

    #[test]
    fn test_parse_call_statement() {
        let source = r#"
PROGRAM Test
MyFunc(1, 2);
END_PROGRAM
"#;
        let parse = parse(source);
        assert!(parse.ok(), "errors: {:?}", parse.errors());
    }

    #[test]
    fn test_parse_typed_literal_and_deref() {
        let source = r#"
PROGRAM Test
ptr^ := INT#16#FF;
END_PROGRAM
"#;
        let parse = parse(source);
        assert!(parse.ok(), "errors: {:?}", parse.errors());
    }

    #[test]
    fn test_parse_case_enum_labels() {
        let source = r#"
PROGRAM Test
    VAR state : INT; END_VAR
    CASE state OF
        MyEnum.Starting:
            state := 1;
        MyEnum.Running:
            state := 2;
    END_CASE
END_PROGRAM
"#;
        let parse = parse(source);
        assert!(parse.ok(), "errors: {:?}", parse.errors());
    }

    #[test]
    fn test_missing_semicolon_insertion() {
        let source = r#"
PROGRAM Test
    x := 1
    y := 2;
END_PROGRAM
"#;
        let parse = parse(source);
        assert!(!parse.ok(), "expected errors for missing semicolon");
        assert!(
            parse
                .errors()
                .iter()
                .any(|error| error.message == "expected ';'"),
            "errors: {:?}",
            parse.errors()
        );
    }

    #[test]
    fn test_missing_end_case_recovery() {
        let source = r#"
PROGRAM Test
    CASE x OF
        0: y := 1;
END_PROGRAM
"#;
        let parse = parse(source);
        assert!(!parse.ok(), "expected errors for missing END_CASE");
        assert!(
            parse
                .errors()
                .iter()
                .any(|error| error.message == "expected END_CASE"),
            "errors: {:?}",
            parse.errors()
        );
        assert!(
            !parse
                .errors()
                .iter()
                .any(|error| error.message == "expected END_PROGRAM"),
            "errors: {:?}",
            parse.errors()
        );
    }
}
