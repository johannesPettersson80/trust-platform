struct SymbolRenderContext<'a> {
    source: &'a str,
    root: &'a SyntaxNode,
    range: TextRange,
}

/// Result of a hover request.
#[derive(Debug, Clone)]
pub struct HoverResult {
    /// The hover content (markdown).
    pub contents: String,
    /// The range of the hovered element.
    pub range: Option<TextRange>,
}

impl HoverResult {
    /// Creates a new hover result.
    pub fn new(contents: impl Into<String>) -> Self {
        Self {
            contents: contents.into(),
            range: None,
        }
    }

    /// Sets the range.
    #[must_use]
    pub fn with_range(mut self, range: TextRange) -> Self {
        self.range = Some(range);
        self
    }
}

/// Computes hover information at the given position.
pub fn hover(
    db: &Database,
    file_id: trust_hir::db::FileId,
    position: TextSize,
) -> Option<HoverResult> {
    hover_with_filter(db, file_id, position, &StdlibFilter::allow_all())
}

/// Computes hover information with stdlib filtering.
pub fn hover_with_filter(
    db: &Database,
    file_id: trust_hir::db::FileId,
    position: TextSize,
    stdlib_filter: &StdlibFilter,
) -> Option<HoverResult> {
    let context = IdeContext::new(db, file_id);
    if let Some(result) = hover_task_priority(&context, position) {
        return Some(result);
    }
    if let Some(result) = hover_typed_literal(&context, position) {
        return Some(result);
    }
    if has_ambiguous_reference(db, file_id, position) {
        if let Some(result) = hover_ambiguous_using(&context, position) {
            return Some(result);
        }
    }
    let target = context.resolve_target_at_position(position);
    match target {
        Some(ResolvedTarget::Symbol(symbol_id)) => {
            let symbols = &context.symbols;
            let symbol = symbols.get(symbol_id)?;
            let (symbol_source, symbol_root, symbol_range) = if let Some(origin) = symbol.origin {
                let origin_source = db.source_text(origin.file_id);
                let origin_parsed = parse(&origin_source);
                let origin_symbols = db.file_symbols(origin.file_id);
                let origin_range = origin_symbols
                    .get(origin.symbol_id)
                    .map(|sym| sym.range)
                    .unwrap_or(symbol.range);
                (origin_source, origin_parsed.syntax(), origin_range)
            } else {
                (context.source.clone(), context.root.clone(), symbol.range)
            };
            let type_name = type_name_for_id(symbols, symbol.type_id);
            let scope_id = scope_at_position(symbols, &context.root, position);
            let render = SymbolRenderContext {
                source: &symbol_source,
                root: &symbol_root,
                range: symbol_range,
            };
            let contents = format_symbol(
                symbol,
                symbols,
                &render,
                type_name.as_deref(),
                scope_id,
                stdlib_filter,
            );
            let range = context
                .root
                .token_at_offset(position)
                .right_biased()?
                .text_range();
            Some(HoverResult::new(contents).with_range(range))
        }
        Some(ResolvedTarget::Field(field)) => {
            let symbols = &context.symbols;
            let field_type = field_type(symbols, &field)?;
            let field_type_name =
                type_name_for_id(symbols, field_type).unwrap_or_else(|| "?".to_string());
            let contents = format_field(&field.name, &field_type_name);
            let range = context
                .root
                .token_at_offset(position)
                .right_biased()?
                .text_range();
            Some(HoverResult::new(contents).with_range(range))
        }
        None => hover_ambiguous_using(&context, position)
            .or_else(|| hover_standard_function(&context, position, stdlib_filter)),
    }
}

fn hover_task_priority(context: &IdeContext<'_>, position: TextSize) -> Option<HoverResult> {
    let token = context.root.token_at_offset(position).right_biased()?;
    if !token.text().eq_ignore_ascii_case("PRIORITY") {
        return None;
    }
    if !token_has_task_init_parent(&token) {
        return None;
    }

    let contents = concat!(
        "```st\nPRIORITY : UINT\n```\n",
        "0 = highest priority; larger numbers = lower priority.\n",
        "Scheduling policy is runtime-defined (preemptive or non-preemptive).\n",
        "For non-preemptive scheduling, the longest-waiting task at the highest priority runs first."
    );
    Some(HoverResult::new(contents).with_range(token.text_range()))
}

fn token_has_task_init_parent(token: &SyntaxToken) -> bool {
    let Some(parent) = token.parent() else {
        return false;
    };
    parent
        .ancestors()
        .any(|node| node.kind() == SyntaxKind::TaskInit)
}

fn hover_ambiguous_using(context: &IdeContext<'_>, position: TextSize) -> Option<HoverResult> {
    let (name, range) = ident_at_offset(&context.source, position)?;
    let scope_id = scope_at_position(&context.symbols, &context.root, position);
    let candidates = collect_using_candidates(&context.symbols, scope_id, name);
    if candidates.len() <= 1 {
        return None;
    }

    let mut candidate_names = candidates
        .iter()
        .map(|parts| join_namespace_path(parts))
        .collect::<Vec<_>>();
    candidate_names.sort();
    candidate_names.dedup();

    let mut using_paths = candidates
        .iter()
        .filter_map(|parts| {
            if parts.len() > 1 {
                Some(join_namespace_path(&parts[..parts.len() - 1]))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    using_paths.sort();
    using_paths.dedup();

    let mut contents = format!("```st\n{name}\n```\n\nAmbiguous reference to `{name}`.");
    let mut sections = Vec::new();
    sections.push(format!("Candidates:\n- {}", candidate_names.join("\n- ")));
    if !using_paths.is_empty() {
        sections.push(format!("USING:\n- {}", using_paths.join("\n- ")));
    }
    if !sections.is_empty() {
        contents.push_str("\n\n---\n\n");
        contents.push_str(&sections.join("\n\n"));
    }

    Some(HoverResult::new(contents).with_range(range))
}

fn has_ambiguous_reference(
    db: &Database,
    file_id: trust_hir::db::FileId,
    position: TextSize,
) -> bool {
    let diagnostics = db.diagnostics(file_id);
    diagnostics.iter().any(|diag| {
        diag.code == DiagnosticCode::CannotResolve
            && diag.message.contains("ambiguous reference to")
            && diag.range.contains(position)
    })
}

fn collect_using_candidates(
    symbols: &SymbolTable,
    scope_id: ScopeId,
    name: &str,
) -> Vec<Vec<SmolStr>> {
    let mut candidates = Vec::new();
    let mut current = Some(scope_id);
    while let Some(scope_id) = current {
        let Some(scope) = symbols.get_scope(scope_id) else {
            break;
        };
        if scope.lookup_local(name).is_some() {
            break;
        }
        for using in &scope.using_directives {
            let mut parts = using.path.clone();
            parts.push(SmolStr::new(name));
            let Some(symbol_id) = symbols.resolve_qualified(&parts) else {
                continue;
            };
            if let Some(symbol) = symbols.get(symbol_id) {
                if matches!(symbol.kind, SymbolKind::Namespace) {
                    continue;
                }
            }
            candidates.push(parts);
        }
        current = scope.parent;
    }

    let mut seen = std::collections::HashSet::new();
    let mut unique = Vec::new();
    for parts in candidates {
        let key = parts
            .iter()
            .map(|part| part.to_ascii_uppercase())
            .collect::<Vec<_>>()
            .join(".");
        if seen.insert(key) {
            unique.push(parts);
        }
    }
    unique
}

fn join_namespace_path(parts: &[SmolStr]) -> String {
    let mut out = String::new();
    for (idx, part) in parts.iter().enumerate() {
        if idx > 0 {
            out.push('.');
        }
        out.push_str(part.as_str());
    }
    out
}

/// Formats a symbol for hover display.
fn format_symbol(
    symbol: &Symbol,
    symbols: &SymbolTable,
    render: &SymbolRenderContext<'_>,
    type_name: Option<&str>,
    scope_id: trust_hir::symbols::ScopeId,
    stdlib_filter: &StdlibFilter,
) -> String {
    let mut result = String::new();

    // Add code block with signature
    result.push_str("```st\n");
    let visibility = visibility_prefix(symbol.visibility);
    let modifiers = modifiers_prefix(symbol.modifiers);
    let header_prefix = format_symbol_prefix(visibility, modifiers.as_deref());

    match &symbol.kind {
        SymbolKind::Variable { qualifier } => {
            let info = var_decl_info_for_symbol(render.root, render.source, render.range);
            let resolved_type = type_name
                .filter(|name| *name != "?")
                .map(str::to_string)
                .or(info.declared_type.clone())
                .unwrap_or_else(|| "?".to_string());
            let mut qual = match qualifier {
                VarQualifier::Input => "VAR_INPUT",
                VarQualifier::Output => "VAR_OUTPUT",
                VarQualifier::InOut => "VAR_IN_OUT",
                VarQualifier::Local => "VAR",
                VarQualifier::Temp => "VAR_TEMP",
                VarQualifier::Global => "VAR_GLOBAL",
                VarQualifier::External => "VAR_EXTERNAL",
                VarQualifier::Access => "VAR_ACCESS",
                VarQualifier::Static => "VAR_STAT",
            }
            .to_string();
            if symbol_is_constant(symbol) {
                qual.push_str(" CONSTANT");
            }
            if let Some(retention) = info.retention {
                qual.push(' ');
                qual.push_str(retention);
            }
            result.push_str(&format!("({}) {} : {}", qual, symbol.name, resolved_type));
            if let Some(initializer) = info.initializer {
                result.push_str(&format!(" := {}", initializer));
            }
        }
        SymbolKind::Constant => {
            let info = var_decl_info_for_symbol(render.root, render.source, render.range);
            let resolved_type = type_name
                .filter(|name| *name != "?")
                .map(str::to_string)
                .or(info.declared_type.clone())
                .unwrap_or_else(|| "?".to_string());
            result.push_str(&format!(
                "(CONSTANT) {}{} : {}",
                header_prefix, symbol.name, resolved_type
            ));
            if let Some(initializer) = info.initializer {
                result.push_str(&format!(" := {}", initializer));
            }
        }
        SymbolKind::Function { .. } => {
            result.push_str(&format!(
                "FUNCTION {}{} : {}",
                header_prefix,
                symbol.name,
                type_name.unwrap_or("?")
            ));
        }
        SymbolKind::FunctionBlock => {
            result.push_str(&format_function_block(
                symbol,
                symbols,
                render.root,
                render.source,
                render.range,
                &header_prefix,
            ));
        }
        SymbolKind::Class => {
            let mut header = format!("CLASS {}{}", header_prefix, symbol.name);
            for line in inheritance_lines(symbols, render.root, symbol, render.range) {
                header.push('\n');
                header.push_str(&line);
            }
            result.push_str(&header);
        }
        SymbolKind::Method { return_type, .. } => {
            if return_type.is_some() {
                result.push_str(&format!(
                    "METHOD {}{} : {}",
                    header_prefix,
                    symbol.name,
                    type_name.unwrap_or("?")
                ));
            } else {
                result.push_str(&format!("METHOD {}{}", header_prefix, symbol.name));
            }
        }
        SymbolKind::Property {
            has_get, has_set, ..
        } => {
            let access = match (has_get, has_set) {
                (true, true) => "GET/SET",
                (true, false) => "GET",
                (false, true) => "SET",
                (false, false) => "",
            };
            result.push_str(&format!(
                "PROPERTY {}{} : {} [{}]",
                header_prefix,
                symbol.name,
                type_name.unwrap_or("?"),
                access
            ));
        }
        SymbolKind::Interface => {
            let mut header = format!("INTERFACE {}{}", header_prefix, symbol.name);
            for line in inheritance_lines(symbols, render.root, symbol, render.range) {
                header.push('\n');
                header.push_str(&line);
            }
            result.push_str(&header);
        }
        SymbolKind::Namespace => {
            result.push_str(&format!("NAMESPACE {}{}", header_prefix, symbol.name));
        }
        SymbolKind::Program => {
            result.push_str(&format!("PROGRAM {}{}", header_prefix, symbol.name));
        }
        SymbolKind::Configuration => {
            result.push_str(&format!("CONFIGURATION {}{}", header_prefix, symbol.name));
        }
        SymbolKind::Resource => {
            let resource_type = resource_type_for_symbol(render.root, render.source, render.range);
            if let Some(resource_type) = resource_type {
                result.push_str(&format!(
                    "RESOURCE {}{} ON {}",
                    header_prefix, symbol.name, resource_type
                ));
            } else {
                result.push_str(&format!("RESOURCE {}{}", header_prefix, symbol.name));
            }
        }
        SymbolKind::Task => {
            let task_init =
                task_init_for_symbol(render.root, render.source, render.range).unwrap_or_default();
            if task_init.is_empty() {
                result.push_str(&format!("TASK {}{}", header_prefix, symbol.name));
            } else {
                result.push_str(&format!(
                    "TASK {}{} ({})",
                    header_prefix, symbol.name, task_init
                ));
            }
        }
        SymbolKind::ProgramInstance => {
            let (type_name, task_name, retain) =
                program_config_details(render.root, render.source, render.range);
            let mut header = format!(
                "PROGRAM {}{} : {}",
                header_prefix,
                symbol.name,
                type_name.unwrap_or_else(|| "?".to_string())
            );
            if let Some(task) = task_name {
                header.push_str(&format!(" WITH {task}"));
            }
            if let Some(retain) = retain {
                header.push_str(&format!(" [{retain}]"));
            }
            result.push_str(&header);
        }
        SymbolKind::Type => {
            result.push_str(&format_type_definition(symbols, symbol));
        }
        SymbolKind::EnumValue { value } => {
            result.push_str(&format!("{}{} := {}", header_prefix, symbol.name, value));
        }
        SymbolKind::Parameter { direction } => {
            let mut dir = match direction {
                trust_hir::symbols::ParamDirection::In => "IN",
                trust_hir::symbols::ParamDirection::Out => "OUT",
                trust_hir::symbols::ParamDirection::InOut => "IN_OUT",
            }
            .to_string();
            if symbol_is_constant(symbol) {
                dir.push_str(" CONSTANT");
            }
            result.push_str(&format!(
                "({}) {}{} : {}",
                dir,
                header_prefix,
                symbol.name,
                type_name.unwrap_or("?")
            ));
        }
    }

    result.push_str("\n```");

    let mut sections = Vec::new();
    if let Some(doc) = &symbol.doc {
        sections.push(doc.to_string());
    } else if stdlib_filter.allows_function_block(symbol.name.as_str()) {
        if let Some(std_doc) = stdlib_docs::standard_fb_doc(symbol.name.as_str()) {
            sections.push(std_doc.to_string());
        }
    }

    let ns_parts = namespace_path_for_symbol(symbols, symbol);
    if !ns_parts.is_empty() {
        let namespace = ns_parts
            .iter()
            .map(|part| part.as_str())
            .collect::<Vec<_>>()
            .join(".");
        sections.push(format!("Namespace: {namespace}"));
    }
    if let Some(using_path) =
        using_path_for_symbol(symbols, scope_id, symbol.name.as_str(), symbol.id)
    {
        let using = using_path
            .iter()
            .map(|part| part.as_str())
            .collect::<Vec<_>>()
            .join(".");
        sections.push(format!("USING {using}"));
    }

    if !sections.is_empty() {
        result.push_str("\n\n---\n\n");
        result.push_str(&sections.join("\n\n"));
    }

    result
}

fn format_symbol_prefix(visibility: Option<&str>, modifiers: Option<&str>) -> String {
    let mut parts = Vec::new();
    if let Some(vis) = visibility {
        parts.push(vis);
    }
    if let Some(mods) = modifiers {
        parts.push(mods);
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!("{} ", parts.join(" "))
    }
}

fn visibility_prefix(visibility: Visibility) -> Option<&'static str> {
    match visibility {
        Visibility::Public => None,
        Visibility::Private => Some("PRIVATE"),
        Visibility::Protected => Some("PROTECTED"),
        Visibility::Internal => Some("INTERNAL"),
    }
}

fn modifiers_prefix(modifiers: SymbolModifiers) -> Option<String> {
    let mut parts = Vec::new();
    if modifiers.is_final {
        parts.push("FINAL");
    }
    if modifiers.is_abstract {
        parts.push("ABSTRACT");
    }
    if modifiers.is_override {
        parts.push("OVERRIDE");
    }
    (!parts.is_empty()).then_some(parts.join(" "))
}
