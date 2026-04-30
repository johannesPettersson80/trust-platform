//! LSP conversion helpers.

use std::collections::HashMap;
use tower_lsp::lsp_types::{
    OptionalVersionedTextDocumentIdentifier, Position, Range, SemanticToken, SymbolKind, TextEdit,
    Url,
};

use trust_hir::symbols::{Symbol, SymbolTable};
use trust_hir::SymbolKind as HirSymbolKind;
use trust_hir::Type;
use trust_ide::rename::RenameResult;

use crate::state::{uri_to_path, ServerState};

pub(crate) fn offset_to_position(content: &str, offset: u32) -> Position {
    let (line, col) = offset_to_line_col(content, offset);
    Position {
        line,
        character: col,
    }
}

pub(crate) fn offset_to_line_col(content: &str, offset: u32) -> (u32, u32) {
    let offset = offset as usize;
    let mut line = 0u32;
    let mut col = 0u32;

    for (i, c) in content.char_indices() {
        if i >= offset {
            break;
        }
        if c == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }

    (line, col)
}

pub(crate) fn position_to_offset(content: &str, position: Position) -> Option<u32> {
    let mut line = 0u32;
    let mut col = 0u32;

    for (i, c) in content.char_indices() {
        if line == position.line && col == position.character {
            return Some(i as u32);
        }
        if c == '\n' {
            if line == position.line {
                // Position is at end of this line
                return Some(i as u32);
            }
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }

    // Position is at end of file
    if line == position.line {
        Some(content.len() as u32)
    } else {
        None
    }
}

pub(crate) fn semantic_tokens_to_lsp(
    content: &str,
    tokens: impl IntoIterator<Item = trust_ide::SemanticToken>,
    origin_line: u32,
    origin_col: u32,
) -> Vec<SemanticToken> {
    let mut data = Vec::new();
    let mut prev_line = origin_line;
    let mut prev_start = origin_col;

    for token in tokens {
        let start = token.range.start();
        let end = token.range.end();
        let (line, col) = offset_to_line_col(content, start.into());
        let length = u32::from(end) - u32::from(start);

        let token_type = match token.token_type {
            trust_ide::SemanticTokenType::Keyword => 0,
            trust_ide::SemanticTokenType::Type => 1,
            trust_ide::SemanticTokenType::Variable => 2,
            trust_ide::SemanticTokenType::Property => 3,
            trust_ide::SemanticTokenType::Method => 4,
            trust_ide::SemanticTokenType::Function => 5,
            trust_ide::SemanticTokenType::Parameter => 6,
            trust_ide::SemanticTokenType::Number => 7,
            trust_ide::SemanticTokenType::String => 8,
            trust_ide::SemanticTokenType::Comment => 9,
            trust_ide::SemanticTokenType::Operator => 10,
            trust_ide::SemanticTokenType::EnumMember => 11,
            trust_ide::SemanticTokenType::Namespace => 12,
        };

        let mut modifiers = 0u32;
        if token.modifiers.declaration {
            modifiers |= 1 << 0;
        }
        if token.modifiers.definition {
            modifiers |= 1 << 1;
        }
        if token.modifiers.readonly {
            modifiers |= 1 << 2;
        }
        if token.modifiers.is_static {
            modifiers |= 1 << 3;
        }
        if token.modifiers.modification {
            modifiers |= 1 << 4;
        }

        let delta_line = line - prev_line;
        let delta_start = if delta_line == 0 {
            col - prev_start
        } else {
            col
        };

        data.push(SemanticToken {
            delta_line,
            delta_start,
            length,
            token_type,
            token_modifiers_bitset: modifiers,
        });

        prev_line = line;
        prev_start = col;
    }

    data
}

pub(crate) fn rename_result_to_changes(
    state: &ServerState,
    result: RenameResult,
) -> Option<HashMap<Url, Vec<TextEdit>>> {
    let mut changes = HashMap::new();

    for (file_id, edits) in result.edits {
        let target_doc = state.document_for_file_id(file_id)?;
        let lsp_edits = edits
            .into_iter()
            .map(|edit| TextEdit {
                range: Range {
                    start: offset_to_position(&target_doc.content, edit.range.start().into()),
                    end: offset_to_position(&target_doc.content, edit.range.end().into()),
                },
                new_text: edit.new_text,
            })
            .collect();
        changes.insert(target_doc.uri, lsp_edits);
    }

    Some(changes)
}

pub(crate) fn text_document_identifier_for_edit(
    state: &ServerState,
    uri: &Url,
) -> OptionalVersionedTextDocumentIdentifier {
    let version =
        state
            .get_document(uri)
            .and_then(|doc| if doc.is_open { Some(doc.version) } else { None });
    OptionalVersionedTextDocumentIdentifier {
        uri: uri.clone(),
        version,
    }
}

pub(crate) fn st_file_stem(uri: &Url) -> Option<String> {
    let path = uri_to_path(uri)?;
    let extension = path.extension().and_then(|ext| ext.to_str())?;
    if !extension.eq_ignore_ascii_case("st") {
        return None;
    }
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .map(String::from)
}

pub(crate) fn is_primary_pou_symbol_kind(kind: &HirSymbolKind) -> bool {
    matches!(
        kind,
        HirSymbolKind::Program
            | HirSymbolKind::Function { .. }
            | HirSymbolKind::FunctionBlock
            | HirSymbolKind::Class
            | HirSymbolKind::Interface
    )
}

pub(crate) fn lsp_symbol_kind(symbols: &SymbolTable, symbol: &Symbol) -> SymbolKind {
    if symbol.is_constant {
        return SymbolKind::CONSTANT;
    }
    match symbol.kind {
        trust_hir::SymbolKind::Program => SymbolKind::MODULE,
        trust_hir::SymbolKind::Configuration => SymbolKind::MODULE,
        trust_hir::SymbolKind::Resource => SymbolKind::NAMESPACE,
        trust_hir::SymbolKind::Task => SymbolKind::EVENT,
        trust_hir::SymbolKind::ProgramInstance => SymbolKind::OBJECT,
        trust_hir::SymbolKind::Namespace => SymbolKind::NAMESPACE,
        trust_hir::SymbolKind::Function { .. } => SymbolKind::FUNCTION,
        trust_hir::SymbolKind::FunctionBlock => SymbolKind::CLASS,
        trust_hir::SymbolKind::Class => SymbolKind::CLASS,
        trust_hir::SymbolKind::Method { .. } => SymbolKind::METHOD,
        trust_hir::SymbolKind::Property { .. } => SymbolKind::PROPERTY,
        trust_hir::SymbolKind::Interface => SymbolKind::INTERFACE,
        trust_hir::SymbolKind::Type => {
            let is_enum = symbols
                .type_by_id(symbol.type_id)
                .is_some_and(|ty| matches!(ty, Type::Enum { .. }))
                || symbols
                    .lookup_registered_type_name(&symbol.name)
                    .and_then(|type_id| symbols.type_by_id(type_id))
                    .is_some_and(|ty| matches!(ty, Type::Enum { .. }))
                || symbols.iter().any(|child| {
                    child.parent == Some(symbol.id)
                        && matches!(child.kind, trust_hir::SymbolKind::EnumValue { .. })
                });
            if is_enum {
                SymbolKind::ENUM
            } else {
                SymbolKind::STRUCT
            }
        }
        trust_hir::SymbolKind::EnumValue { .. } => SymbolKind::ENUM_MEMBER,
        trust_hir::SymbolKind::Variable { .. } => SymbolKind::VARIABLE,
        trust_hir::SymbolKind::Constant => SymbolKind::CONSTANT,
        trust_hir::SymbolKind::Parameter { .. } => SymbolKind::VARIABLE,
    }
}

pub(crate) fn display_symbol_name(symbols: &SymbolTable, symbol: &Symbol) -> String {
    let name = symbol.name.to_string();
    let Some(label) = iec_symbol_label(symbols, symbol) else {
        return name;
    };
    format!("{name} ({label})")
}

fn iec_symbol_label(symbols: &SymbolTable, symbol: &Symbol) -> Option<String> {
    match symbol.kind {
        trust_hir::SymbolKind::Program => Some("PROGRAM".to_string()),
        trust_hir::SymbolKind::Configuration => Some("CONFIGURATION".to_string()),
        trust_hir::SymbolKind::Resource => Some("RESOURCE".to_string()),
        trust_hir::SymbolKind::Task => Some("TASK".to_string()),
        trust_hir::SymbolKind::ProgramInstance => Some("PROGRAM".to_string()),
        trust_hir::SymbolKind::FunctionBlock => Some("FUNCTION_BLOCK".to_string()),
        trust_hir::SymbolKind::Type => match type_label(symbols, symbol) {
            Some("ENUM" | "STRUCT") => None,
            Some(label) => Some(format!("TYPE ({label})")),
            None => Some("TYPE".to_string()),
        },
        _ => None,
    }
}

fn type_label(symbols: &SymbolTable, symbol: &Symbol) -> Option<&'static str> {
    let type_id = symbols
        .type_by_id(symbol.type_id)
        .map(|_| symbol.type_id)
        .or_else(|| symbols.lookup_registered_type_name(&symbol.name));
    let ty = type_id.and_then(|id| symbols.type_by_id(id));
    match ty {
        Some(Type::Enum { .. }) => Some("ENUM"),
        Some(Type::Struct { .. }) => Some("STRUCT"),
        Some(Type::Union { .. }) => Some("UNION"),
        Some(Type::Alias { .. }) => Some("ALIAS"),
        Some(Type::Array { .. }) => Some("ARRAY"),
        Some(Type::Pointer { .. }) => Some("POINTER"),
        Some(Type::Reference { .. }) => Some("REFERENCE"),
        Some(Type::Subrange { .. }) => Some("SUBRANGE"),
        Some(Type::FunctionBlock { .. }) => Some("FUNCTION_BLOCK"),
        Some(Type::Class { .. }) => Some("CLASS"),
        Some(Type::Interface { .. }) => Some("INTERFACE"),
        Some(Type::Unknown) | None => None,
        Some(Type::Void) => Some("VOID"),
        Some(Type::Null) => Some("NULL"),
        Some(Type::Bool) => Some("BOOL"),
        Some(Type::SInt) => Some("SINT"),
        Some(Type::Int) => Some("INT"),
        Some(Type::DInt) => Some("DINT"),
        Some(Type::LInt) => Some("LINT"),
        Some(Type::USInt) => Some("USINT"),
        Some(Type::UInt) => Some("UINT"),
        Some(Type::UDInt) => Some("UDINT"),
        Some(Type::ULInt) => Some("ULINT"),
        Some(Type::Real) => Some("REAL"),
        Some(Type::LReal) => Some("LREAL"),
        Some(Type::Byte) => Some("BYTE"),
        Some(Type::Word) => Some("WORD"),
        Some(Type::DWord) => Some("DWORD"),
        Some(Type::LWord) => Some("LWORD"),
        Some(Type::Time) => Some("TIME"),
        Some(Type::LTime) => Some("LTIME"),
        Some(Type::Date) => Some("DATE"),
        Some(Type::LDate) => Some("LDATE"),
        Some(Type::Tod) => Some("TIME_OF_DAY"),
        Some(Type::LTod) => Some("LTIME_OF_DAY"),
        Some(Type::Dt) => Some("DATE_AND_TIME"),
        Some(Type::Ldt) => Some("LDATE_AND_TIME"),
        Some(Type::String { .. }) => Some("STRING"),
        Some(Type::WString { .. }) => Some("WSTRING"),
        Some(Type::Char) => Some("CHAR"),
        Some(Type::WChar) => Some("WCHAR"),
        Some(Type::Any) => Some("ANY"),
        Some(Type::AnyDerived) => Some("ANY_DERIVED"),
        Some(Type::AnyElementary) => Some("ANY_ELEMENTARY"),
        Some(Type::AnyMagnitude) => Some("ANY_MAGNITUDE"),
        Some(Type::AnyInt) => Some("ANY_INT"),
        Some(Type::AnyUnsigned) => Some("ANY_UNSIGNED"),
        Some(Type::AnySigned) => Some("ANY_SIGNED"),
        Some(Type::AnyReal) => Some("ANY_REAL"),
        Some(Type::AnyNum) => Some("ANY_NUM"),
        Some(Type::AnyDuration) => Some("ANY_DURATION"),
        Some(Type::AnyBit) => Some("ANY_BIT"),
        Some(Type::AnyChars) => Some("ANY_CHARS"),
        Some(Type::AnyString) => Some("ANY_STRING"),
        Some(Type::AnyChar) => Some("ANY_CHAR"),
        Some(Type::AnyDate) => Some("ANY_DATE"),
    }
}

pub(crate) fn symbol_container_name(symbols: &SymbolTable, symbol: &Symbol) -> Option<String> {
    symbol
        .parent
        .and_then(|parent_id| symbols.get(parent_id))
        .map(|parent| parent.name.to_string())
}
