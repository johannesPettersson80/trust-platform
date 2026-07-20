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

    let newline = preferred_line_ending(source);
    let formatted_index = LineIndex::new(formatted);
    let formatted_lines = (0..formatted_index.line_count())
        .filter_map(|line| formatted_index.line_text(line))
        .collect::<Vec<_>>();

    if start_line >= formatted_lines.len() || end_line >= formatted_lines.len() {
        return None;
    }

    let mut new_text = formatted_lines[start_line..=end_line].join(newline);
    if end_line + 1 < line_starts.len()
        || (ends_with_line_ending(source) && !ends_with_line_ending(&new_text))
    {
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
    let index = LineIndex::new(source);
    (0..index.line_count())
        .filter_map(|line| index.line_start(line))
        .collect()
}

fn preferred_line_ending(source: &str) -> &'static str {
    if source.contains("\r\n") {
        "\r\n"
    } else if source.contains('\r') {
        "\r"
    } else {
        "\n"
    }
}

fn ends_with_line_ending(source: &str) -> bool {
    source.ends_with(['\r', '\n'])
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
