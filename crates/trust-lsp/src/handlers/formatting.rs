//! Document formatting handler.

use tower_lsp::lsp_types::{
    DocumentFormattingParams, DocumentOnTypeFormattingParams, DocumentRangeFormattingParams,
    Position, Range, TextEdit, Url,
};

use serde_json::Value;
use trust_lsp::document_text::LineIndex;
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

    let newline = preferred_line_ending(source);
    let mut result = output_lines.join(newline);
    if ends_with_line_ending(source) && !ends_with_line_ending(&result) {
        result.push_str(newline);
    }
    result
}

include!("formatting/block_and_line_formatting.rs");
include!("formatting/alignment_wrap_and_tests.rs");
include!("formatting/tests.rs");
