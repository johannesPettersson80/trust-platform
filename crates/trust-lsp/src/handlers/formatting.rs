//! Document formatting handler.

use tower_lsp::lsp_types::{
    DocumentFormattingParams, DocumentOnTypeFormattingParams, DocumentRangeFormattingParams,
    Position, Range, TextEdit, Url,
};

use serde_json::Value;
use trust_syntax::{lex, Token, TokenKind};

use crate::state::ServerState;

use super::config::{bool_with_aliases, lsp_section, string_with_aliases, value_with_aliases};
use super::lsp_utils::offset_to_position;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum KeywordCase {
    Preserve,
    Upper,
    Lower,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SpacingStyle {
    Spaced,
    Compact,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EndKeywordStyle {
    Aligned,
    Indented,
}

#[derive(Clone, Debug)]
struct FormatConfig {
    indent_width: usize,
    insert_spaces: bool,
    keyword_case: KeywordCase,
    align_var_decl_colons: bool,
    align_assignments: bool,
    max_line_length: Option<usize>,
    spacing_style: SpacingStyle,
    end_keyword_style: EndKeywordStyle,
}

fn format_config(
    state: &ServerState,
    uri: &Url,
    options: &tower_lsp::lsp_types::FormattingOptions,
) -> FormatConfig {
    let mut config = FormatConfig {
        indent_width: options.tab_size as usize,
        insert_spaces: options.insert_spaces,
        keyword_case: KeywordCase::Preserve,
        align_var_decl_colons: true,
        align_assignments: true,
        max_line_length: None,
        spacing_style: SpacingStyle::Spaced,
        end_keyword_style: EndKeywordStyle::Aligned,
    };

    if let Some(workspace_config) = state.workspace_config_for_uri(uri) {
        let overrides = format_profile_overrides(workspace_config.vendor_profile.as_deref());
        apply_format_overrides(&mut config, overrides);
    }

    let value = state.config();
    let format = lsp_section(&value)
        .and_then(|section| value_with_aliases(section, &["format", "formatting"]));

    if let Some(format) = format {
        if let Some(width) =
            value_with_aliases(format, &["indentWidth", "indent_width"]).and_then(Value::as_u64)
        {
            config.indent_width = width.max(1) as usize;
        }
        if let Some(insert) = bool_with_aliases(format, &["insertSpaces", "insert_spaces"]) {
            config.insert_spaces = insert;
        }
        if let Some(case) = string_with_aliases(format, &["keywordCase", "keyword_case"]) {
            config.keyword_case = match case.to_ascii_lowercase().as_str() {
                "upper" => KeywordCase::Upper,
                "lower" => KeywordCase::Lower,
                _ => KeywordCase::Preserve,
            };
        }
        if let Some(align) = bool_with_aliases(format, &["alignVarDecls", "align_var_decls"]) {
            config.align_var_decl_colons = align;
        }
        if let Some(align) = bool_with_aliases(format, &["alignAssignments", "align_assignments"]) {
            config.align_assignments = align;
        }
        if let Some(max) = value_with_aliases(format, &["maxLineLength", "max_line_length"])
            .and_then(Value::as_u64)
        {
            if max > 0 {
                config.max_line_length = Some(max as usize);
            }
        }
        if let Some(style) = string_with_aliases(format, &["spacingStyle", "spacing_style"]) {
            config.spacing_style = match style.to_ascii_lowercase().as_str() {
                "compact" | "tight" => SpacingStyle::Compact,
                _ => SpacingStyle::Spaced,
            };
        }
        if let Some(style) = string_with_aliases(format, &["endKeywordStyle", "end_keyword_style"])
        {
            config.end_keyword_style = match style.to_ascii_lowercase().as_str() {
                "indented" | "indent" => EndKeywordStyle::Indented,
                _ => EndKeywordStyle::Aligned,
            };
        }
    }

    config
}

#[derive(Default)]
struct FormatOverrides {
    indent_width: Option<usize>,
    insert_spaces: Option<bool>,
    keyword_case: Option<KeywordCase>,
    align_var_decl_colons: Option<bool>,
    align_assignments: Option<bool>,
    max_line_length: Option<usize>,
    spacing_style: Option<SpacingStyle>,
    end_keyword_style: Option<EndKeywordStyle>,
}

fn apply_format_overrides(config: &mut FormatConfig, overrides: FormatOverrides) {
    if let Some(width) = overrides.indent_width {
        config.indent_width = width.max(1);
    }
    if let Some(insert) = overrides.insert_spaces {
        config.insert_spaces = insert;
    }
    if let Some(case) = overrides.keyword_case {
        config.keyword_case = case;
    }
    if let Some(align) = overrides.align_var_decl_colons {
        config.align_var_decl_colons = align;
    }
    if let Some(align) = overrides.align_assignments {
        config.align_assignments = align;
    }
    if let Some(max) = overrides.max_line_length {
        if max > 0 {
            config.max_line_length = Some(max);
        }
    }
    if let Some(style) = overrides.spacing_style {
        config.spacing_style = style;
    }
    if let Some(style) = overrides.end_keyword_style {
        config.end_keyword_style = style;
    }
}

fn format_profile_overrides(profile: Option<&str>) -> FormatOverrides {
    let Some(profile) = profile else {
        return FormatOverrides::default();
    };
    let profile = profile.trim().to_ascii_lowercase();
    match profile.as_str() {
        "codesys" | "beckhoff" | "twincat" | "mitsubishi" | "gxworks3" => FormatOverrides {
            indent_width: Some(4),
            insert_spaces: Some(true),
            keyword_case: Some(KeywordCase::Upper),
            align_var_decl_colons: Some(true),
            align_assignments: Some(true),
            max_line_length: Some(120),
            spacing_style: Some(SpacingStyle::Spaced),
            end_keyword_style: Some(EndKeywordStyle::Aligned),
        },
        "siemens" => FormatOverrides {
            indent_width: Some(2),
            insert_spaces: Some(true),
            keyword_case: Some(KeywordCase::Upper),
            align_var_decl_colons: Some(true),
            align_assignments: Some(true),
            max_line_length: Some(120),
            spacing_style: Some(SpacingStyle::Compact),
            end_keyword_style: Some(EndKeywordStyle::Aligned),
        },
        _ => FormatOverrides::default(),
    }
}

pub fn formatting(state: &ServerState, params: DocumentFormattingParams) -> Option<Vec<TextEdit>> {
    let uri = &params.text_document.uri;
    let doc = state.get_document(uri)?;
    let config = format_config(state, uri, &params.options);
    let formatted = format_document(&doc.content, &config);
    if formatted == doc.content {
        return Some(Vec::new());
    }

    let end = offset_to_position(&doc.content, doc.content.len() as u32);
    Some(vec![TextEdit {
        range: Range {
            start: Position::new(0, 0),
            end,
        },
        new_text: formatted,
    }])
}

pub fn range_formatting(
    state: &ServerState,
    params: DocumentRangeFormattingParams,
) -> Option<Vec<TextEdit>> {
    let uri = &params.text_document.uri;
    let doc = state.get_document(uri)?;
    let config = format_config(state, uri, &params.options);
    let formatted = format_document(&doc.content, &config);
    if formatted == doc.content {
        return Some(Vec::new());
    }

    let line_starts = line_starts(&doc.content);
    let start_line = params.range.start.line as usize;
    let mut end_line = params.range.end.line as usize;
    if params.range.end.character == 0 && end_line > start_line {
        end_line = end_line.saturating_sub(1);
    }
    if start_line >= line_starts.len() || end_line >= line_starts.len() || start_line > end_line {
        return Some(Vec::new());
    }

    let (start_line, end_line) = expand_range_to_block(&doc.content, start_line, end_line);
    let edit = format_lines_edit(&doc.content, &formatted, start_line, end_line)?;
    Some(vec![edit])
}

pub fn on_type_formatting(
    state: &ServerState,
    params: DocumentOnTypeFormattingParams,
) -> Option<Vec<TextEdit>> {
    let uri = &params.text_document_position.text_document.uri;
    let doc = state.get_document(uri)?;

    let config = format_config(state, uri, &params.options);
    let formatted = format_document(&doc.content, &config);
    if formatted == doc.content {
        return Some(Vec::new());
    }

    let line_starts = line_starts(&doc.content);
    let line = params.text_document_position.position.line as usize;
    if line >= line_starts.len() {
        return Some(Vec::new());
    }

    let edit = format_lines_edit(&doc.content, &formatted, line, line)?;
    Some(vec![edit])
}

fn format_document(source: &str, config: &FormatConfig) -> String {
    let tokens = lex(source);
    let line_starts = line_starts(source);
    let line_count = line_starts.len();
    let mut line_tokens: Vec<Vec<Token>> = vec![Vec::new(); line_count];
    let mut line_in_block_comment = vec![false; line_count];
    let mut line_has_line_comment = vec![false; line_count];
    let mut line_has_pragma = vec![false; line_count];
    let mut line_has_string_literal = vec![false; line_count];

    for token in tokens {
        if token.kind == TokenKind::BlockComment {
            let start_line = line_index(&line_starts, usize::from(token.range.start()));
            let end_offset = usize::from(token.range.end()).saturating_sub(1);
            let end_line = line_index(&line_starts, end_offset);
            for idx in start_line..=end_line {
                if idx < line_in_block_comment.len() {
                    line_in_block_comment[idx] = true;
                }
            }
            continue;
        }
        if token.kind == TokenKind::LineComment {
            let line_idx = line_index(&line_starts, usize::from(token.range.start()));
            if let Some(line) = line_has_line_comment.get_mut(line_idx) {
                *line = true;
            }
            continue;
        }
        if token.kind == TokenKind::Pragma {
            let line_idx = line_index(&line_starts, usize::from(token.range.start()));
            if let Some(line) = line_has_pragma.get_mut(line_idx) {
                *line = true;
            }
            continue;
        }
        if matches!(
            token.kind,
            TokenKind::StringLiteral | TokenKind::WideStringLiteral
        ) {
            let line_idx = line_index(&line_starts, usize::from(token.range.start()));
            if let Some(line) = line_has_string_literal.get_mut(line_idx) {
                *line = true;
            }
        }
        if token.kind.is_trivia() {
            continue;
        }
        let start = usize::from(token.range.start());
        let line_idx = line_index(&line_starts, start);
        if let Some(line) = line_tokens.get_mut(line_idx) {
            line.push(token);
        }
    }

    let indent_unit = if config.insert_spaces {
        " ".repeat(config.indent_width.max(1))
    } else {
        "\t".to_string()
    };

    let mut indent_level: i32 = 0;
    let mut output_lines = Vec::with_capacity(line_count);
    let mut line_in_var_block = vec![false; line_count];
    let mut line_colon_index: Vec<Option<usize>> = vec![None; line_count];
    let mut in_var_block = false;

    for i in 0..line_count {
        let line_start = line_starts[i];
        let line_end = if i + 1 < line_count {
            line_starts[i + 1].saturating_sub(1)
        } else {
            source.len()
        };

        let line_text = &source[line_start..line_end];
        let line_text = line_text.strip_suffix('\r').unwrap_or(line_text);
        let tokens = &line_tokens[i];
        let has_var_start = tokens.iter().any(|token| token.kind.is_var_keyword());
        let has_var_end = tokens.iter().any(|token| token.kind == TokenKind::KwEndVar);
        line_in_var_block[i] = in_var_block && !has_var_end;

        if line_in_block_comment[i] {
            output_lines.push(line_text.to_string());
            if has_var_start {
                in_var_block = true;
            }
            if has_var_end {
                in_var_block = false;
            }
            continue;
        }

        let trimmed = line_text.trim();
        if trimmed.is_empty() {
            output_lines.push(String::new());
            if has_var_start {
                in_var_block = true;
            }
            if has_var_end {
                in_var_block = false;
            }
            continue;
        }

        let mut current_indent = indent_level;
        let mut dedent_after = false;
        if let Some(first) = tokens.first() {
            if is_dedent_token(first.kind) {
                let should_dedent = match config.end_keyword_style {
                    EndKeywordStyle::Aligned => true,
                    EndKeywordStyle::Indented => !is_end_keyword(first.kind),
                };
                if should_dedent {
                    current_indent = (current_indent - 1).max(0);
                } else {
                    dedent_after = true;
                }
            }
        }

        let indent_prefix = indent_unit.repeat(current_indent as usize);
        let formatted_line = if line_has_line_comment[i] || line_has_pragma[i] {
            format!("{}{}", indent_prefix, trimmed)
        } else {
            let content =
                format_line_tokens(tokens, source, config.keyword_case, config.spacing_style);
            format!("{}{}", indent_prefix, content)
        };
        if line_in_var_block[i] && !line_has_line_comment[i] && !line_has_pragma[i] {
            line_colon_index[i] = find_type_colon(&formatted_line);
        }

        output_lines.push(formatted_line);

        if line_has_indent_start(tokens) {
            indent_level = current_indent + 1;
        } else {
            indent_level = current_indent;
        }
        if dedent_after {
            indent_level = indent_level.saturating_sub(1);
        }
        if has_var_start {
            in_var_block = true;
        }
        if has_var_end {
            in_var_block = false;
        }
    }

    if config.align_var_decl_colons {
        align_var_block_colons(&mut output_lines, &line_in_var_block, &line_colon_index);
    }
    let line_masks = LineFormatMasks {
        in_var_block: &line_in_var_block,
        in_block_comment: &line_in_block_comment,
        has_line_comment: &line_has_line_comment,
        has_pragma: &line_has_pragma,
        has_string_literal: &line_has_string_literal,
    };
    if config.align_assignments {
        align_assignment_ops(&mut output_lines, &line_masks);
    }

    let output_lines = if let Some(max) = config.max_line_length {
        wrap_long_lines(&output_lines, &line_masks, &indent_unit, max)
    } else {
        output_lines
    };

    let newline = if source.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let mut result = output_lines.join(newline);
    if source.ends_with('\n') && !result.ends_with('\n') {
        result.push_str(newline);
    }
    result
}

fn format_lines_edit(
    source: &str,
    formatted: &str,
    start_line: usize,
    end_line: usize,
) -> Option<TextEdit> {
    let line_starts = line_starts(source);
    let start_offset = *line_starts.get(start_line)?;
    let end_offset = if end_line + 1 < line_starts.len() {
        line_starts[end_line + 1]
    } else {
        source.len()
    };

    let newline = if source.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let mut formatted_lines: Vec<&str> = formatted.split('\n').collect();
    for line in &mut formatted_lines {
        if let Some(stripped) = line.strip_suffix('\r') {
            *line = stripped;
        }
    }

    if start_line >= formatted_lines.len() || end_line >= formatted_lines.len() {
        return None;
    }

    let mut new_text = formatted_lines[start_line..=end_line].join(newline);
    if end_line + 1 < line_starts.len() || (source.ends_with('\n') && !new_text.ends_with('\n')) {
        new_text.push_str(newline);
    }

    Some(TextEdit {
        range: Range {
            start: offset_to_position(source, start_offset as u32),
            end: offset_to_position(source, end_offset as u32),
        },
        new_text,
    })
}

fn line_starts(source: &str) -> Vec<usize> {
    let mut starts = vec![0];
    for (idx, ch) in source.char_indices() {
        if ch == '\n' {
            starts.push(idx + 1);
        }
    }
    starts
}

fn line_index(line_starts: &[usize], offset: usize) -> usize {
    match line_starts.binary_search(&offset) {
        Ok(idx) => idx,
        Err(idx) => idx.saturating_sub(1),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BlockKind {
    Program,
    Function,
    FunctionBlock,
    Class,
    Method,
    Property,
    Interface,
    Namespace,
    Action,
    VarBlock,
    Type,
    Struct,
    Union,
    If,
    Case,
    For,
    While,
    Repeat,
    Get,
    Set,
    Step,
    Transition,
    Configuration,
    Resource,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BlockSpan {
    start_line: usize,
    end_line: usize,
    kind: BlockKind,
}

fn expand_range_to_block(source: &str, start_line: usize, end_line: usize) -> (usize, usize) {
    let spans = block_spans(source);
    let mut best: Option<BlockSpan> = None;
    for span in spans {
        if span.start_line <= start_line && span.end_line >= end_line {
            let span_len = span.end_line.saturating_sub(span.start_line);
            let best_len = best.map(|current| current.end_line.saturating_sub(current.start_line));
            if best_len.is_none_or(|len| span_len < len) {
                best = Some(span);
            }
        }
    }
    if let Some(span) = best {
        (span.start_line, span.end_line)
    } else {
        (start_line, end_line)
    }
}

fn block_spans(source: &str) -> Vec<BlockSpan> {
    let tokens = lex(source);
    let line_starts = line_starts(source);
    let mut spans = Vec::new();
    let mut stack: Vec<(BlockKind, usize)> = Vec::new();

    for token in tokens {
        if token.kind.is_trivia() {
            continue;
        }
        let line = line_index(&line_starts, usize::from(token.range.start()));
        if let Some(kind) = block_start_kind(token.kind) {
            stack.push((kind, line));
            continue;
        }
        let Some(kind) = block_end_kind(token.kind) else {
            continue;
        };
        if let Some(pos) = stack.iter().rposition(|(open_kind, _)| *open_kind == kind) {
            let (open_kind, start_line) = stack.remove(pos);
            spans.push(BlockSpan {
                start_line,
                end_line: line,
                kind: open_kind,
            });
        }
    }

    spans
}

fn block_start_kind(kind: TokenKind) -> Option<BlockKind> {
    match kind {
        TokenKind::KwProgram => Some(BlockKind::Program),
        TokenKind::KwFunction => Some(BlockKind::Function),
        TokenKind::KwFunctionBlock => Some(BlockKind::FunctionBlock),
        TokenKind::KwClass => Some(BlockKind::Class),
        TokenKind::KwMethod => Some(BlockKind::Method),
        TokenKind::KwProperty => Some(BlockKind::Property),
        TokenKind::KwInterface => Some(BlockKind::Interface),
        TokenKind::KwNamespace => Some(BlockKind::Namespace),
        TokenKind::KwAction => Some(BlockKind::Action),
        TokenKind::KwVar
        | TokenKind::KwVarInput
        | TokenKind::KwVarOutput
        | TokenKind::KwVarInOut
        | TokenKind::KwVarTemp
        | TokenKind::KwVarGlobal
        | TokenKind::KwVarExternal
        | TokenKind::KwVarAccess
        | TokenKind::KwVarConfig
        | TokenKind::KwVarStat => Some(BlockKind::VarBlock),
        TokenKind::KwType => Some(BlockKind::Type),
        TokenKind::KwStruct => Some(BlockKind::Struct),
        TokenKind::KwUnion => Some(BlockKind::Union),
        TokenKind::KwIf => Some(BlockKind::If),
        TokenKind::KwCase => Some(BlockKind::Case),
        TokenKind::KwFor => Some(BlockKind::For),
        TokenKind::KwWhile => Some(BlockKind::While),
        TokenKind::KwRepeat => Some(BlockKind::Repeat),
        TokenKind::KwGet => Some(BlockKind::Get),
        TokenKind::KwSet => Some(BlockKind::Set),
        TokenKind::KwStep => Some(BlockKind::Step),
        TokenKind::KwTransition => Some(BlockKind::Transition),
        TokenKind::KwConfiguration => Some(BlockKind::Configuration),
        TokenKind::KwResource => Some(BlockKind::Resource),
        _ => None,
    }
}

fn block_end_kind(kind: TokenKind) -> Option<BlockKind> {
    match kind {
        TokenKind::KwEndProgram => Some(BlockKind::Program),
        TokenKind::KwEndFunction => Some(BlockKind::Function),
        TokenKind::KwEndFunctionBlock => Some(BlockKind::FunctionBlock),
        TokenKind::KwEndClass => Some(BlockKind::Class),
        TokenKind::KwEndMethod => Some(BlockKind::Method),
        TokenKind::KwEndProperty => Some(BlockKind::Property),
        TokenKind::KwEndInterface => Some(BlockKind::Interface),
        TokenKind::KwEndNamespace => Some(BlockKind::Namespace),
        TokenKind::KwEndAction => Some(BlockKind::Action),
        TokenKind::KwEndVar => Some(BlockKind::VarBlock),
        TokenKind::KwEndType => Some(BlockKind::Type),
        TokenKind::KwEndStruct => Some(BlockKind::Struct),
        TokenKind::KwEndUnion => Some(BlockKind::Union),
        TokenKind::KwEndIf => Some(BlockKind::If),
        TokenKind::KwEndCase => Some(BlockKind::Case),
        TokenKind::KwEndFor => Some(BlockKind::For),
        TokenKind::KwEndWhile => Some(BlockKind::While),
        TokenKind::KwEndRepeat => Some(BlockKind::Repeat),
        TokenKind::KwEndGet => Some(BlockKind::Get),
        TokenKind::KwEndSet => Some(BlockKind::Set),
        TokenKind::KwEndStep => Some(BlockKind::Step),
        TokenKind::KwEndTransition => Some(BlockKind::Transition),
        TokenKind::KwEndConfiguration => Some(BlockKind::Configuration),
        TokenKind::KwEndResource => Some(BlockKind::Resource),
        _ => None,
    }
}

fn is_dedent_token(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::KwEndProgram
            | TokenKind::KwEndFunction
            | TokenKind::KwEndFunctionBlock
            | TokenKind::KwEndMethod
            | TokenKind::KwEndProperty
            | TokenKind::KwEndInterface
            | TokenKind::KwEndNamespace
            | TokenKind::KwEndAction
            | TokenKind::KwEndVar
            | TokenKind::KwEndType
            | TokenKind::KwEndStruct
            | TokenKind::KwEndUnion
            | TokenKind::KwEndIf
            | TokenKind::KwEndCase
            | TokenKind::KwEndFor
            | TokenKind::KwEndWhile
            | TokenKind::KwEndRepeat
            | TokenKind::KwEndGet
            | TokenKind::KwEndSet
            | TokenKind::KwElse
            | TokenKind::KwElsif
            | TokenKind::KwUntil
    )
}

fn is_end_keyword(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::KwEndProgram
            | TokenKind::KwEndFunction
            | TokenKind::KwEndFunctionBlock
            | TokenKind::KwEndMethod
            | TokenKind::KwEndProperty
            | TokenKind::KwEndInterface
            | TokenKind::KwEndNamespace
            | TokenKind::KwEndAction
            | TokenKind::KwEndVar
            | TokenKind::KwEndType
            | TokenKind::KwEndStruct
            | TokenKind::KwEndUnion
            | TokenKind::KwEndIf
            | TokenKind::KwEndCase
            | TokenKind::KwEndFor
            | TokenKind::KwEndWhile
            | TokenKind::KwEndRepeat
            | TokenKind::KwEndGet
            | TokenKind::KwEndSet
    )
}

fn line_has_indent_start(tokens: &[Token]) -> bool {
    tokens.iter().any(|token| {
        let kind = token.kind;
        matches!(
            kind,
            TokenKind::KwProgram
                | TokenKind::KwFunction
                | TokenKind::KwFunctionBlock
                | TokenKind::KwMethod
                | TokenKind::KwProperty
                | TokenKind::KwInterface
                | TokenKind::KwNamespace
                | TokenKind::KwAction
                | TokenKind::KwVar
                | TokenKind::KwVarInput
                | TokenKind::KwVarOutput
                | TokenKind::KwVarInOut
                | TokenKind::KwVarTemp
                | TokenKind::KwVarGlobal
                | TokenKind::KwVarExternal
                | TokenKind::KwVarAccess
                | TokenKind::KwVarConfig
                | TokenKind::KwVarStat
                | TokenKind::KwType
                | TokenKind::KwStruct
                | TokenKind::KwUnion
                | TokenKind::KwIf
                | TokenKind::KwCase
                | TokenKind::KwFor
                | TokenKind::KwWhile
                | TokenKind::KwRepeat
                | TokenKind::KwGet
                | TokenKind::KwSet
                | TokenKind::KwElse
                | TokenKind::KwElsif
        )
    })
}

fn format_line_tokens(
    tokens: &[Token],
    source: &str,
    keyword_case: KeywordCase,
    spacing_style: SpacingStyle,
) -> String {
    let mut out = String::new();
    let mut prev_kind: Option<TokenKind> = None;

    for token in tokens {
        let kind = token.kind;
        if let Some(prev) = prev_kind {
            if !should_glue(prev, kind, spacing_style) {
                out.push(' ');
            }
        }
        let start = usize::from(token.range.start());
        let end = usize::from(token.range.end());
        let text = &source[start..end];
        if keyword_case == KeywordCase::Preserve || !kind.is_keyword() {
            out.push_str(text);
        } else if keyword_case == KeywordCase::Upper {
            out.push_str(&text.to_ascii_uppercase());
        } else {
            out.push_str(&text.to_ascii_lowercase());
        }
        prev_kind = Some(kind);
    }

    out
}

fn should_glue(prev: TokenKind, current: TokenKind, spacing_style: SpacingStyle) -> bool {
    if spacing_style == SpacingStyle::Compact
        && (is_symbolic_operator(prev) || is_symbolic_operator(current))
    {
        return true;
    }

    if spacing_style == SpacingStyle::Compact
        && matches!(
            prev,
            TokenKind::Comma | TokenKind::Semicolon | TokenKind::Colon
        )
    {
        return true;
    }

    if matches!(
        prev,
        TokenKind::LParen
            | TokenKind::LBracket
            | TokenKind::Dot
            | TokenKind::DotDot
            | TokenKind::Hash
            | TokenKind::Caret
            | TokenKind::At
            | TokenKind::TypedLiteralPrefix
    ) {
        return true;
    }

    if matches!(
        current,
        TokenKind::RParen
            | TokenKind::RBracket
            | TokenKind::Comma
            | TokenKind::Semicolon
            | TokenKind::Dot
            | TokenKind::DotDot
            | TokenKind::Hash
            | TokenKind::Caret
            | TokenKind::At
            | TokenKind::TypedLiteralPrefix
            | TokenKind::Colon
    ) {
        return true;
    }

    if matches!(current, TokenKind::LParen | TokenKind::LBracket) && prev == TokenKind::Ident {
        return true;
    }

    false
}

fn is_symbolic_operator(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Assign
            | TokenKind::Arrow
            | TokenKind::RefAssign
            | TokenKind::Eq
            | TokenKind::Neq
            | TokenKind::Lt
            | TokenKind::LtEq
            | TokenKind::Gt
            | TokenKind::GtEq
            | TokenKind::Plus
            | TokenKind::Minus
            | TokenKind::Star
            | TokenKind::Slash
            | TokenKind::Power
            | TokenKind::Ampersand
    )
}

fn find_type_colon(line: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b':' {
            if i + 1 < bytes.len() && bytes[i + 1] == b'=' {
                i += 2;
                continue;
            }
            return Some(i);
        }
        i += 1;
    }
    None
}

fn align_var_block_colons(
    lines: &mut [String],
    line_in_var_block: &[bool],
    line_colon_index: &[Option<usize>],
) {
    let mut i = 0usize;
    while i < lines.len() {
        if !line_in_var_block[i] {
            i += 1;
            continue;
        }

        while i < lines.len() && line_in_var_block[i] && line_colon_index[i].is_none() {
            i += 1;
        }
        if i >= lines.len() || !line_in_var_block[i] {
            continue;
        }

        let start = i;
        let mut max_colon = 0usize;
        while i < lines.len() && line_in_var_block[i] {
            let Some(colon_idx) = line_colon_index[i] else {
                break;
            };
            max_colon = max_colon.max(colon_idx);
            i += 1;
        }

        if max_colon == 0 {
            continue;
        }

        for idx in start..i {
            let Some(colon_idx) = line_colon_index[idx] else {
                continue;
            };
            if colon_idx >= max_colon {
                continue;
            }
            let pad = max_colon - colon_idx;
            let line = &lines[idx];
            if colon_idx > line.len() {
                continue;
            }
            let mut updated = String::with_capacity(line.len() + pad);
            updated.push_str(&line[..colon_idx]);
            updated.extend(std::iter::repeat_n(' ', pad));
            updated.push_str(&line[colon_idx..]);
            lines[idx] = updated;
        }
    }
}

struct LineFormatMasks<'a> {
    in_var_block: &'a [bool],
    in_block_comment: &'a [bool],
    has_line_comment: &'a [bool],
    has_pragma: &'a [bool],
    has_string_literal: &'a [bool],
}

impl<'a> LineFormatMasks<'a> {
    fn in_var_block(&self, idx: usize) -> bool {
        self.in_var_block.get(idx).copied().unwrap_or(false)
    }

    fn skip_alignment(&self, idx: usize) -> bool {
        self.in_block_comment.get(idx).copied().unwrap_or(false)
            || self.has_line_comment.get(idx).copied().unwrap_or(false)
            || self.has_pragma.get(idx).copied().unwrap_or(false)
            || self.has_string_literal.get(idx).copied().unwrap_or(false)
    }

    fn skip_wrapping(&self, idx: usize) -> bool {
        self.skip_alignment(idx)
    }
}

fn align_assignment_ops(lines: &mut [String], masks: &LineFormatMasks<'_>) {
    let mut i = 0usize;
    while i < lines.len() {
        if masks.skip_alignment(i) {
            i += 1;
            continue;
        }
        let indent = leading_whitespace(&lines[i]).to_string();
        let Some(mut max_op) = find_assignment_op(&lines[i]) else {
            i += 1;
            continue;
        };

        let start = i;
        i += 1;
        while i < lines.len() {
            let line = &lines[i];
            if masks.skip_alignment(i) {
                break;
            }
            if leading_whitespace(line) != indent {
                break;
            }
            let Some(op_idx) = find_assignment_op(line) else {
                break;
            };
            max_op = max_op.max(op_idx);
            i += 1;
        }

        for line in lines.iter_mut().take(i).skip(start) {
            if let Some(op_idx) = find_assignment_op(line) {
                if op_idx < max_op {
                    let padding = " ".repeat(max_op - op_idx);
                    line.insert_str(op_idx, &padding);
                }
            }
        }
    }
}

fn find_assignment_op(line: &str) -> Option<usize> {
    let assign = line.find(":=");
    let arrow = line.find("=>");
    match (assign, arrow) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

fn leading_whitespace(line: &str) -> &str {
    let end = line
        .chars()
        .take_while(|c| c.is_whitespace())
        .map(|c| c.len_utf8())
        .sum();
    &line[..end]
}

fn wrap_long_lines(
    lines: &[String],
    masks: &LineFormatMasks<'_>,
    indent_unit: &str,
    max_len: usize,
) -> Vec<String> {
    let mut output = Vec::new();
    for (idx, line) in lines.iter().enumerate() {
        if masks.in_var_block(idx) {
            output.push(line.clone());
            continue;
        }
        if masks.skip_wrapping(idx) {
            output.push(line.clone());
            continue;
        }
        if line.len() <= max_len || !line.contains(',') {
            output.push(line.clone());
            continue;
        }
        let indent = leading_whitespace(line);
        let continuation = format!("{indent}{indent_unit}");
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() <= 1 {
            output.push(line.clone());
            continue;
        }
        let mut current = parts[0].trim_end().to_string();
        for part in parts.iter().skip(1) {
            output.push(format!("{current},"));
            current = format!("{continuation}{}", part.trim_start());
        }
        output.push(current);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::{format_document, EndKeywordStyle, FormatConfig, KeywordCase, SpacingStyle};

    #[test]
    fn format_document_normalizes_spacing() {
        let source = "PROGRAM Test\nVAR\nx:=1+2; y :=3; \nEND_VAR\nx := y+1;\nEND_PROGRAM\n";
        let config = FormatConfig {
            indent_width: 4,
            insert_spaces: true,
            keyword_case: KeywordCase::Preserve,
            align_var_decl_colons: true,
            align_assignments: true,
            max_line_length: None,
            spacing_style: SpacingStyle::Spaced,
            end_keyword_style: EndKeywordStyle::Aligned,
        };
        let formatted = format_document(source, &config);
        assert!(formatted.contains("x := 1 + 2;"));
        assert!(formatted.contains("y := 3;"));
        assert!(formatted.contains("x := y + 1;"));
    }

    #[test]
    fn format_document_aligns_var_colons() {
        let source =
            "PROGRAM Test\nVAR\n    a: INT;\n    longer_name: REAL;\nEND_VAR\nEND_PROGRAM\n";
        let config = FormatConfig {
            indent_width: 4,
            insert_spaces: true,
            keyword_case: KeywordCase::Preserve,
            align_var_decl_colons: true,
            align_assignments: true,
            max_line_length: None,
            spacing_style: SpacingStyle::Spaced,
            end_keyword_style: EndKeywordStyle::Aligned,
        };
        let formatted = format_document(source, &config);
        let mut lines = formatted.lines();
        let _program = lines.next().unwrap();
        let _var = lines.next().unwrap();
        let a_line = lines.next().unwrap();
        let longer_line = lines.next().unwrap();
        assert!(a_line.contains("a"));
        assert!(longer_line.contains("longer_name"));
        let a_colon = a_line.find(':').unwrap();
        let longer_colon = longer_line.find(':').unwrap();
        assert_eq!(a_colon, longer_colon);
    }

    #[test]
    fn format_document_respects_var_alignment_groups() {
        let source = "PROGRAM Test\nVAR\n    short: INT;\n    // separator\n    much_longer_name: REAL;\nEND_VAR\nEND_PROGRAM\n";
        let config = FormatConfig {
            indent_width: 4,
            insert_spaces: true,
            keyword_case: KeywordCase::Preserve,
            align_var_decl_colons: true,
            align_assignments: true,
            max_line_length: None,
            spacing_style: SpacingStyle::Spaced,
            end_keyword_style: EndKeywordStyle::Aligned,
        };
        let formatted = format_document(source, &config);
        println!("{formatted}");
        let lines: Vec<&str> = formatted.lines().collect();
        let short_line = lines.iter().find(|line| line.contains("short")).unwrap();
        let long_line = lines
            .iter()
            .find(|line| line.contains("much_longer_name"))
            .unwrap();
        let short_colon = short_line.find(':').unwrap();
        let long_colon = long_line.find(':').unwrap();
        assert_ne!(short_colon, long_colon);
    }

    #[test]
    fn format_document_compact_spacing() {
        let source = "PROGRAM Test\nVAR\nx:INT;\nEND_VAR\nx:=1+2;\nEND_PROGRAM\n";
        let config = FormatConfig {
            indent_width: 4,
            insert_spaces: true,
            keyword_case: KeywordCase::Preserve,
            align_var_decl_colons: true,
            align_assignments: true,
            max_line_length: None,
            spacing_style: SpacingStyle::Compact,
            end_keyword_style: EndKeywordStyle::Aligned,
        };
        let formatted = format_document(source, &config);
        assert!(formatted.contains("x:INT;"));
        assert!(formatted.contains("x:=1+2;"));
    }

    #[test]
    fn format_document_indented_end_keywords() {
        let source = "PROGRAM Test\nIF x THEN\nx:=1;\nEND_IF\nEND_PROGRAM\n";
        let config = FormatConfig {
            indent_width: 2,
            insert_spaces: true,
            keyword_case: KeywordCase::Preserve,
            align_var_decl_colons: true,
            align_assignments: true,
            max_line_length: None,
            spacing_style: SpacingStyle::Spaced,
            end_keyword_style: EndKeywordStyle::Indented,
        };
        let formatted = format_document(source, &config);
        let lines: Vec<&str> = formatted.lines().collect();
        let end_if = lines.iter().find(|line| line.contains("END_IF")).unwrap();
        let end_program = lines
            .iter()
            .find(|line| line.contains("END_PROGRAM"))
            .unwrap();
        assert!(end_if.starts_with("    END_IF"));
        assert_eq!(*end_program, "  END_PROGRAM");
    }

    #[test]
    fn format_document_preserves_mixed_pragma_lines() {
        let source =
            "PROGRAM Test\nVAR\n    x: INT;\nEND_VAR\n    x:=1  {PRAGMA}  y:=2;\nEND_PROGRAM\n";
        let config = FormatConfig {
            indent_width: 4,
            insert_spaces: true,
            keyword_case: KeywordCase::Preserve,
            align_var_decl_colons: true,
            align_assignments: true,
            max_line_length: None,
            spacing_style: SpacingStyle::Spaced,
            end_keyword_style: EndKeywordStyle::Aligned,
        };
        let formatted = format_document(source, &config);
        assert!(formatted.contains("    x:=1  {PRAGMA}  y:=2;"));
    }

    #[test]
    fn format_document_skips_wrapping_string_literal_lines() {
        let source = "PROGRAM Test\nVAR\n    msg : STRING;\n    value : INT;\n    longer_name : INT;\nEND_VAR\n    msg := 'a,b,c,d,e,f';\n    value := 1;\n    longer_name := 2;\nEND_PROGRAM\n";
        let config = FormatConfig {
            indent_width: 4,
            insert_spaces: true,
            keyword_case: KeywordCase::Preserve,
            align_var_decl_colons: true,
            align_assignments: true,
            max_line_length: Some(20),
            spacing_style: SpacingStyle::Spaced,
            end_keyword_style: EndKeywordStyle::Aligned,
        };
        let formatted = format_document(source, &config);
        assert!(formatted.contains("msg := 'a,b,c,d,e,f';"));
        let lines: Vec<&str> = formatted.lines().collect();
        let value_line = lines
            .iter()
            .find(|line| line.contains("value") && line.contains(":="))
            .unwrap();
        let longer_line = lines
            .iter()
            .find(|line| line.contains("longer_name") && line.contains(":="))
            .unwrap();
        assert_eq!(
            value_line.find(":=").unwrap(),
            longer_line.find(":=").unwrap()
        );
    }
}
