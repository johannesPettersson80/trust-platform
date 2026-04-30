fn hover_typed_literal(context: &IdeContext<'_>, position: TextSize) -> Option<HoverResult> {
    let offset = u32::from(position) as usize;
    let mut pending_prefix: Option<(String, TextRange, bool)> = None;

    for token in lex(&context.source) {
        let start = usize::from(token.range.start());
        let end = usize::from(token.range.end());

        if matches!(
            token.kind,
            TokenKind::TimeLiteral
                | TokenKind::DateLiteral
                | TokenKind::TimeOfDayLiteral
                | TokenKind::DateAndTimeLiteral
        ) && start <= offset
            && offset < end
        {
            let text = &context.source[start..end];
            let (prefix, _) = text.split_once('#')?;
            let doc = stdlib_docs::typed_literal_doc(prefix)?;
            let contents = format!("```st\n{}\n```\n\n{}", text, doc);
            return Some(HoverResult::new(contents).with_range(token.range));
        }

        match token.kind {
            TokenKind::TypedLiteralPrefix => {
                let text = context.source[start..end].to_string();
                let cursor_in_prefix = start <= offset && offset < end;
                pending_prefix = Some((text, token.range, cursor_in_prefix));
            }
            TokenKind::IntLiteral
            | TokenKind::RealLiteral
            | TokenKind::StringLiteral
            | TokenKind::WideStringLiteral
            | TokenKind::TimeLiteral
            | TokenKind::DateLiteral
            | TokenKind::TimeOfDayLiteral
            | TokenKind::DateAndTimeLiteral
            | TokenKind::KwTrue
            | TokenKind::KwFalse
            | TokenKind::Ident => {
                if let Some((prefix_text, prefix_range, cursor_in_prefix)) = pending_prefix.take() {
                    let cursor_in_value = start <= offset && offset < end;
                    if cursor_in_prefix || cursor_in_value {
                        let value_text = &context.source[start..end];
                        let literal_text = format!("{}{}", prefix_text, value_text);
                        let prefix = prefix_text.trim_end_matches('#');
                        let doc = stdlib_docs::typed_literal_doc(prefix)?;
                        let range = TextRange::new(prefix_range.start(), token.range.end());
                        let contents = format!("```st\n{}\n```\n\n{}", literal_text, doc);
                        return Some(HoverResult::new(contents).with_range(range));
                    }
                }
            }
            _ => {
                if !token.kind.is_trivia() {
                    pending_prefix = None;
                }
            }
        }
    }

    None
}

fn hover_standard_function(
    context: &IdeContext<'_>,
    position: TextSize,
    stdlib_filter: &StdlibFilter,
) -> Option<HoverResult> {
    let (name, range) = ident_at_offset(&context.source, position)?;
    if !stdlib_filter.allows_function(name) {
        return None;
    }
    let doc = stdlib_docs::standard_function_doc(name)?;
    let signature = signature_help(context.db, context.file_id, position)
        .and_then(|help| help.signatures.first().map(|sig| sig.label.clone()))
        .unwrap_or_else(|| name.to_string());
    let contents = format!("```st\n{signature}\n```\n\n{doc}");
    Some(HoverResult::new(contents).with_range(range))
}

/// Formats a type for hover display.
pub fn format_type(ty: &Type) -> String {
    match ty {
        Type::Bool => "BOOL".to_string(),
        Type::SInt => "SINT".to_string(),
        Type::Int => "INT".to_string(),
        Type::DInt => "DINT".to_string(),
        Type::LInt => "LINT".to_string(),
        Type::USInt => "USINT".to_string(),
        Type::UInt => "UINT".to_string(),
        Type::UDInt => "UDINT".to_string(),
        Type::ULInt => "ULINT".to_string(),
        Type::Real => "REAL".to_string(),
        Type::LReal => "LREAL".to_string(),
        Type::Byte => "BYTE".to_string(),
        Type::Word => "WORD".to_string(),
        Type::DWord => "DWORD".to_string(),
        Type::LWord => "LWORD".to_string(),
        Type::Time => "TIME".to_string(),
        Type::LTime => "LTIME".to_string(),
        Type::Date => "DATE".to_string(),
        Type::LDate => "LDATE".to_string(),
        Type::Tod => "TIME_OF_DAY".to_string(),
        Type::LTod => "LTIME_OF_DAY".to_string(),
        Type::Dt => "DATE_AND_TIME".to_string(),
        Type::Ldt => "LDATE_AND_TIME".to_string(),
        Type::Char => "CHAR".to_string(),
        Type::WChar => "WCHAR".to_string(),
        Type::String { max_len } => {
            if let Some(len) = max_len {
                format!("STRING[{}]", len)
            } else {
                "STRING".to_string()
            }
        }
        Type::WString { max_len } => {
            if let Some(len) = max_len {
                format!("WSTRING[{}]", len)
            } else {
                "WSTRING".to_string()
            }
        }
        Type::Array { dimensions, .. } => {
            let dims: Vec<String> = dimensions
                .iter()
                .map(ArrayDimensionExt::display_bounds)
                .collect();
            format!("ARRAY[{}] OF ...", dims.join(", "))
        }
        Type::Struct { name, .. } => format!("STRUCT {}", name),
        Type::Union { name, .. } => format!("UNION {}", name),
        Type::Enum { name, .. } => name.to_string(),
        Type::Pointer { .. } => "POINTER TO ...".to_string(),
        Type::Reference { .. } => "REF_TO ...".to_string(),
        Type::Subrange { base, lower, upper } => {
            let base_name = TypeId::builtin_name(*base).unwrap_or("?");
            format!("{}({}..{})", base_name, lower, upper)
        }
        Type::FunctionBlock { name } => name.to_string(),
        Type::Class { name } => name.to_string(),
        Type::Interface { name } => name.to_string(),
        Type::Alias { name, .. } => name.to_string(),
        Type::Any => "ANY".to_string(),
        Type::AnyDerived => "ANY_DERIVED".to_string(),
        Type::AnyElementary => "ANY_ELEMENTARY".to_string(),
        Type::AnyMagnitude => "ANY_MAGNITUDE".to_string(),
        Type::AnyInt => "ANY_INT".to_string(),
        Type::AnyUnsigned => "ANY_UNSIGNED".to_string(),
        Type::AnySigned => "ANY_SIGNED".to_string(),
        Type::AnyReal => "ANY_REAL".to_string(),
        Type::AnyNum => "ANY_NUM".to_string(),
        Type::AnyDuration => "ANY_DURATION".to_string(),
        Type::AnyBit => "ANY_BIT".to_string(),
        Type::AnyChars => "ANY_CHARS".to_string(),
        Type::AnyString => "ANY_STRING".to_string(),
        Type::AnyChar => "ANY_CHAR".to_string(),
        Type::AnyDate => "ANY_DATE".to_string(),
        Type::Unknown => "?".to_string(),
        Type::Void => "VOID".to_string(),
        Type::Null => "NULL".to_string(),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum VarSectionKind {
    Input,
    InputConstant,
    Output,
    OutputConstant,
    InOut,
    InOutConstant,
    Var,
    VarTemp,
    VarTempConstant,
    VarStat,
    VarStatConstant,
    VarGlobal,
    VarExternal,
    VarAccess,
    Constant,
}

impl VarSectionKind {
    fn header(self) -> &'static str {
        match self {
            VarSectionKind::Input => "VAR_INPUT",
            VarSectionKind::InputConstant => "VAR_INPUT CONSTANT",
            VarSectionKind::Output => "VAR_OUTPUT",
            VarSectionKind::OutputConstant => "VAR_OUTPUT CONSTANT",
            VarSectionKind::InOut => "VAR_IN_OUT",
            VarSectionKind::InOutConstant => "VAR_IN_OUT CONSTANT",
            VarSectionKind::Var => "VAR",
            VarSectionKind::VarTemp => "VAR_TEMP",
            VarSectionKind::VarTempConstant => "VAR_TEMP CONSTANT",
            VarSectionKind::VarStat => "VAR_STAT",
            VarSectionKind::VarStatConstant => "VAR_STAT CONSTANT",
            VarSectionKind::VarGlobal => "VAR_GLOBAL",
            VarSectionKind::VarExternal => "VAR_EXTERNAL",
            VarSectionKind::VarAccess => "VAR_ACCESS",
            VarSectionKind::Constant => "VAR CONSTANT",
        }
    }
}

fn format_function_block(
    symbol: &Symbol,
    symbols: &SymbolTable,
    root: &SyntaxNode,
    source: &str,
    symbol_range: TextRange,
    header_prefix: &str,
) -> String {
    let mut lines = Vec::new();
    lines.push(format!("FUNCTION_BLOCK {}{}", header_prefix, symbol.name));
    for line in inheritance_lines(symbols, root, symbol, symbol_range) {
        lines.push(line);
    }

    let mut sections: std::collections::HashMap<VarSectionKind, Vec<(u32, String)>> =
        std::collections::HashMap::new();

    let filter = SymbolFilter::new(symbols);
    for member in filter.members_of_owner(symbol.id) {
        let section = match member.kind {
            SymbolKind::Parameter { direction } => match direction {
                trust_hir::symbols::ParamDirection::In if member.is_constant => {
                    VarSectionKind::InputConstant
                }
                trust_hir::symbols::ParamDirection::In => VarSectionKind::Input,
                trust_hir::symbols::ParamDirection::Out if member.is_constant => {
                    VarSectionKind::OutputConstant
                }
                trust_hir::symbols::ParamDirection::Out => VarSectionKind::Output,
                trust_hir::symbols::ParamDirection::InOut if member.is_constant => {
                    VarSectionKind::InOutConstant
                }
                trust_hir::symbols::ParamDirection::InOut => VarSectionKind::InOut,
            },
            SymbolKind::Variable { qualifier } => match qualifier {
                VarQualifier::Local => VarSectionKind::Var,
                VarQualifier::Temp if member.is_constant => VarSectionKind::VarTempConstant,
                VarQualifier::Temp => VarSectionKind::VarTemp,
                VarQualifier::Static if member.is_constant => VarSectionKind::VarStatConstant,
                VarQualifier::Static => VarSectionKind::VarStat,
                VarQualifier::Global => VarSectionKind::VarGlobal,
                VarQualifier::External => VarSectionKind::VarExternal,
                VarQualifier::Access => VarSectionKind::VarAccess,
                VarQualifier::Input if member.is_constant => VarSectionKind::InputConstant,
                VarQualifier::Input => VarSectionKind::Input,
                VarQualifier::Output if member.is_constant => VarSectionKind::OutputConstant,
                VarQualifier::Output => VarSectionKind::Output,
                VarQualifier::InOut if member.is_constant => VarSectionKind::InOutConstant,
                VarQualifier::InOut => VarSectionKind::InOut,
            },
            SymbolKind::Constant => VarSectionKind::Constant,
            _ => continue,
        };

        let info = var_decl_info_for_name(root, source, member.name.as_str());
        let declared_type = info
            .declared_type
            .clone()
            .or_else(|| declared_member_type_from_text(source, member.name.as_str()));
        let type_name = type_name_for_id(symbols, member.type_id)
            .filter(|name| name != "?")
            .or(declared_type)
            .unwrap_or_else(|| "?".to_string());
        let mut line = format!("    {} : {}", member.name, type_name);
        if let Some(initializer) = info.initializer {
            line.push_str(&format!(" := {}", initializer));
        }
        line.push(';');
        sections
            .entry(section)
            .or_default()
            .push((u32::from(member.range.start()), line));
    }

    let section_order = [
        VarSectionKind::Input,
        VarSectionKind::InputConstant,
        VarSectionKind::Output,
        VarSectionKind::OutputConstant,
        VarSectionKind::InOut,
        VarSectionKind::InOutConstant,
        VarSectionKind::Var,
        VarSectionKind::VarTemp,
        VarSectionKind::VarTempConstant,
        VarSectionKind::VarStat,
        VarSectionKind::VarStatConstant,
        VarSectionKind::VarGlobal,
        VarSectionKind::VarExternal,
        VarSectionKind::VarAccess,
        VarSectionKind::Constant,
    ];

    for section in section_order {
        let Some(mut entries) = sections.remove(&section) else {
            continue;
        };
        entries.sort_by_key(|(start, _)| *start);
        lines.push(section.header().to_string());
        for (_, line) in entries {
            lines.push(line);
        }
        lines.push("END_VAR".to_string());
    }

    lines.push("END_FUNCTION_BLOCK".to_string());
    lines.join("\n")
}

fn declared_member_type_from_text(text: &str, member_name: &str) -> Option<String> {
    for raw_line in text.lines() {
        let line = raw_line.split("//").next().unwrap_or("").trim();
        if line.is_empty() || !line.contains(':') {
            continue;
        }
        let (left, right) = line.split_once(':')?;
        let names = left
            .split(',')
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .collect::<Vec<_>>();
        if !names
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(member_name))
        {
            continue;
        }
        let rhs = right.trim();
        let rhs = rhs
            .split_once(":=")
            .map_or(rhs, |(before, _)| before)
            .trim();
        let rhs = rhs.split_once(';').map_or(rhs, |(before, _)| before).trim();
        if !rhs.is_empty() {
            return Some(rhs.to_string());
        }
    }
    None
}

fn inheritance_lines(
    symbols: &SymbolTable,
    root: &SyntaxNode,
    symbol: &Symbol,
    symbol_range: TextRange,
) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(base) = symbols.extends_reference(symbol.id) {
        lines.push(format!("EXTENDS {}", base.name().display()));
    }
    if matches!(symbol.kind, SymbolKind::FunctionBlock | SymbolKind::Class) {
        let implements = implements_names_for_symbol(root, symbol, symbol_range);
        if !implements.is_empty() {
            lines.push(format!("IMPLEMENTS {}", implements.join(", ")));
        }
    }
    lines
}

fn implements_names_for_symbol(
    root: &SyntaxNode,
    symbol: &Symbol,
    symbol_range: TextRange,
) -> Vec<String> {
    let kind = match symbol.kind {
        SymbolKind::FunctionBlock => SyntaxKind::FunctionBlock,
        SymbolKind::Class => SyntaxKind::Class,
        _ => return Vec::new(),
    };
    let Some(node) = find_named_node(root, symbol_range, kind) else {
        return Vec::new();
    };
    let Some(clause) = node
        .children()
        .find(|child| child.kind() == SyntaxKind::ImplementsClause)
    else {
        return Vec::new();
    };
    qualified_names_in_clause(&clause)
}

fn resource_type_for_symbol(
    root: &SyntaxNode,
    _source: &str,
    symbol_range: TextRange,
) -> Option<String> {
    let node = find_named_node(root, symbol_range, SyntaxKind::Resource)?;
    let mut saw_on = false;
    for element in node.children_with_tokens() {
        if let Some(token) = element.as_token() {
            if token.kind() == SyntaxKind::KwOn {
                saw_on = true;
                continue;
            }
        }
        if saw_on {
            if let Some(child) = element
                .as_node()
                .filter(|node| matches!(node.kind(), SyntaxKind::QualifiedName | SyntaxKind::Name))
            {
                if let Some(name) = qualified_name_text(child) {
                    return Some(name);
                }
            }
        }
    }
    None
}

fn task_init_for_symbol(
    root: &SyntaxNode,
    source: &str,
    symbol_range: TextRange,
) -> Option<String> {
    let node = find_named_node(root, symbol_range, SyntaxKind::TaskConfig)?;
    let init = node
        .children()
        .find(|child| child.kind() == SyntaxKind::TaskInit)?;
    let mut parts = Vec::new();
    let elements: Vec<SyntaxElement> = init.children_with_tokens().collect();
    let mut idx = 0;
    while idx < elements.len() {
        let Some(name_node) = elements[idx]
            .as_node()
            .filter(|node| node.kind() == SyntaxKind::Name)
        else {
            idx += 1;
            continue;
        };
        let Some(assign) = elements
            .get(idx + 1)
            .and_then(|element| element.as_token())
            .filter(|token| token.kind() == SyntaxKind::Assign)
        else {
            idx += 1;
            continue;
        };
        let _ = assign;
        let Some(name) = qualified_name_text(name_node) else {
            idx += 1;
            continue;
        };
        let mut expr_range = None;
        let mut j = idx + 2;
        while j < elements.len() {
            if let Some(node) = elements[j].as_node() {
                expr_range = Some(node.text_range());
                break;
            }
            if let Some(token) = elements[j].as_token() {
                if matches!(token.kind(), SyntaxKind::Comma | SyntaxKind::RParen) {
                    break;
                }
            }
            j += 1;
        }
        if let Some(range) = expr_range {
            if let Some(expr_text) = slice_source(source, range) {
                parts.push(format!("{name} := {expr_text}"));
            }
        }
        idx = j;
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(", "))
    }
}

fn program_config_details(
    root: &SyntaxNode,
    source: &str,
    symbol_range: TextRange,
) -> (Option<String>, Option<String>, Option<String>) {
    let Some(node) = find_named_node(root, symbol_range, SyntaxKind::ProgramConfig) else {
        return (None, None, None);
    };

    let mut retain = None;
    let mut task = None;
    let mut type_name = None;
    let mut saw_colon = false;
    let mut saw_with = false;

    for element in node.children_with_tokens() {
        if let Some(token) = element.as_token() {
            match token.kind() {
                SyntaxKind::KwRetain => retain = Some("RETAIN".to_string()),
                SyntaxKind::KwNonRetain => retain = Some("NON_RETAIN".to_string()),
                SyntaxKind::KwWith => {
                    saw_with = true;
                }
                SyntaxKind::Colon => {
                    saw_colon = true;
                }
                _ => {}
            }
        }
        if let Some(child) = element.as_node() {
            if saw_with && task.is_none() && child.kind() == SyntaxKind::Name {
                task = qualified_name_text(child);
                saw_with = false;
            } else if saw_colon
                && type_name.is_none()
                && matches!(
                    child.kind(),
                    SyntaxKind::QualifiedName | SyntaxKind::TypeRef | SyntaxKind::Name
                )
            {
                type_name = qualified_name_text(child)
                    .or_else(|| slice_source(source, child.text_range()).map(|s| s.to_string()));
                saw_colon = false;
            }
        }
    }

    (type_name, task, retain)
}

fn qualified_name_text(node: &SyntaxNode) -> Option<String> {
    let target = match node.kind() {
        SyntaxKind::QualifiedName | SyntaxKind::Name => node.clone(),
        SyntaxKind::TypeRef => node
            .children()
            .find(|child| matches!(child.kind(), SyntaxKind::QualifiedName | SyntaxKind::Name))?,
        _ => return None,
    };
    let mut parts = Vec::new();
    for child in target
        .children()
        .filter(|child| child.kind() == SyntaxKind::Name)
    {
        if let Some(ident) = ident_token_in_name(&child) {
            parts.push(ident.text().to_string());
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("."))
    }
}

fn slice_source(source: &str, range: TextRange) -> Option<&str> {
    let start: usize = range.start().into();
    let end: usize = range.end().into();
    source.get(start..end)
}
