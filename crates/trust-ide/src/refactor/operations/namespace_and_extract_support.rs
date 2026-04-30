fn apply_move_in_file(
    db: &Database,
    file_id: FileId,
    old_path: &[SmolStr],
    new_path: &[SmolStr],
    result: &mut RenameResult,
) {
    let source = db.source_text(file_id);
    let parsed = parse(&source);
    let root = parsed.syntax();
    let symbols = db.file_symbols_with_project(file_id);

    for scope in symbols.scopes() {
        for using in &scope.using_directives {
            if !path_eq_ignore_ascii_case(&using.path, old_path) {
                continue;
            }
            let new_text = join_namespace_path(new_path);
            result.add_edit(
                file_id,
                TextEdit {
                    range: using.range,
                    new_text,
                },
            );
        }
    }

    for node in root
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::Namespace)
    {
        let name_node = node
            .children()
            .find(|child| matches!(child.kind(), SyntaxKind::Name | SyntaxKind::QualifiedName));
        let Some(name_node) = name_node else {
            continue;
        };
        let Some(parts) = qualified_name_parts_from_node(&name_node) else {
            continue;
        };
        if !path_starts_with_ignore_ascii_case(&parts, old_path) {
            continue;
        }
        let mut updated = new_path.to_vec();
        updated.extend_from_slice(&parts[old_path.len()..]);
        let new_text = join_namespace_path(&updated);
        result.add_edit(
            file_id,
            TextEdit {
                range: node_token_range(&name_node),
                new_text,
            },
        );
    }

    for node in root
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::QualifiedName)
    {
        if node
            .ancestors()
            .skip(1)
            .any(|ancestor| ancestor.kind() == SyntaxKind::UsingDirective)
        {
            continue;
        }
        if node
            .parent()
            .map(|parent| parent.kind() == SyntaxKind::Namespace)
            .unwrap_or(false)
        {
            continue;
        }

        let parts = qualified_name_parts(&node);
        if !path_starts_with_ignore_ascii_case(&parts, old_path) {
            continue;
        }

        let mut updated = new_path.to_vec();
        updated.extend_from_slice(&parts[old_path.len()..]);
        let new_text = join_namespace_path(&updated);
        result.add_edit(
            file_id,
            TextEdit {
                range: node_token_range(&node),
                new_text,
            },
        );
    }

    for node in root
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::FieldExpr)
    {
        let Some(parts) = qualified_name_from_field_expr(&node) else {
            continue;
        };
        if !path_starts_with_ignore_ascii_case(&parts, old_path) {
            continue;
        }
        if symbols.resolve_qualified(&parts).is_none() {
            continue;
        }
        let mut updated = new_path.to_vec();
        updated.extend_from_slice(&parts[old_path.len()..]);
        let new_text = join_namespace_path(&updated);
        result.add_edit(
            file_id,
            TextEdit {
                range: node_token_range(&node),
                new_text,
            },
        );
    }
}

#[derive(Debug, Clone)]
struct ImplementedMembers {
    methods: FxHashSet<SmolStr>,
    properties: FxHashSet<SmolStr>,
}

#[derive(Debug, Clone)]
struct InterfaceStub {
    name_key: SmolStr,
    kind: InterfaceStubKind,
}

#[derive(Debug, Clone)]
enum InterfaceStubKind {
    Method(MethodStub),
    Property(PropertyStub),
}

#[derive(Debug, Clone)]
struct MethodStub {
    name: SmolStr,
    return_type: Option<String>,
    var_blocks: Vec<String>,
}

#[derive(Debug, Clone)]
struct PropertyStub {
    name: SmolStr,
    type_name: Option<String>,
    has_get: bool,
    has_set: bool,
}

fn collect_missing_interface_stubs(
    db: &Database,
    symbols: &SymbolTable,
    interfaces: &[Vec<SmolStr>],
    implemented: &ImplementedMembers,
    fallback_file_id: FileId,
) -> Vec<InterfaceStub> {
    let mut stubs = Vec::new();
    let mut seen = FxHashSet::default();

    for parts in interfaces {
        if parts.is_empty() {
            continue;
        }
        let interface_id = symbols
            .resolve_qualified(parts)
            .or_else(|| symbols.resolve_global_or_qualified_name(&join_namespace_path(parts)));
        let Some(interface_id) = interface_id else {
            continue;
        };
        collect_interface_stubs(
            db,
            symbols,
            interface_id,
            fallback_file_id,
            implemented,
            &mut seen,
            &mut stubs,
        );
    }

    stubs
}

fn collect_interface_stubs(
    db: &Database,
    symbols: &SymbolTable,
    interface_id: SymbolId,
    fallback_file_id: FileId,
    implemented: &ImplementedMembers,
    seen: &mut FxHashSet<SmolStr>,
    out: &mut Vec<InterfaceStub>,
) {
    let mut stack = vec![interface_id];
    let mut visited = FxHashSet::default();

    while let Some(current) = stack.pop() {
        if !visited.insert(current) {
            continue;
        }

        let Some(interface_symbol) = symbols.get(current) else {
            continue;
        };
        if !matches!(interface_symbol.kind, SymbolKind::Interface) {
            continue;
        }

        let interface_file_id = interface_symbol
            .origin
            .map(|origin| origin.file_id)
            .unwrap_or(fallback_file_id);
        let interface_source = db.source_text(interface_file_id);
        let interface_root = parse(&interface_source).syntax();
        if let Some(interface_node) =
            find_interface_node_for_symbol(&interface_root, interface_symbol.range)
        {
            for child in interface_node.children() {
                match child.kind() {
                    SyntaxKind::Method => {
                        let Some(stub) =
                            method_stub_from_interface(&interface_source, &child, implemented)
                        else {
                            continue;
                        };
                        if seen.insert(stub.name_key.clone()) {
                            out.push(stub);
                        }
                    }
                    SyntaxKind::Property => {
                        let Some(stub) =
                            property_stub_from_interface(&interface_source, &child, implemented)
                        else {
                            continue;
                        };
                        if seen.insert(stub.name_key.clone()) {
                            out.push(stub);
                        }
                    }
                    _ => {}
                }
            }
        }

        if let Some(base) = symbols.extends_reference(current) {
            let base_name = base.name().display();
            if let Some(base_id) = symbols.resolve_oop_reference_for_owner(current, base_name.as_str()) {
                stack.push(base_id);
            }
        }
    }
}

fn method_stub_from_interface(
    source: &str,
    node: &SyntaxNode,
    implemented: &ImplementedMembers,
) -> Option<InterfaceStub> {
    let name_node = node
        .children()
        .find(|child| child.kind() == SyntaxKind::Name)?;
    let name = name_from_name_node(&name_node)?;
    let key = normalize_member_name(name.as_str());
    if implemented.methods.contains(&key) {
        return None;
    }

    let return_type = node
        .children()
        .find(|child| child.kind() == SyntaxKind::TypeRef)
        .map(|child| text_for_range(source, child.text_range()));
    let var_blocks = node
        .children()
        .filter(|child| child.kind() == SyntaxKind::VarBlock)
        .map(|block| text_for_range(source, block.text_range()))
        .filter(|block| !block.is_empty())
        .collect::<Vec<_>>();

    Some(InterfaceStub {
        name_key: key,
        kind: InterfaceStubKind::Method(MethodStub {
            name,
            return_type,
            var_blocks,
        }),
    })
}

fn property_stub_from_interface(
    source: &str,
    node: &SyntaxNode,
    implemented: &ImplementedMembers,
) -> Option<InterfaceStub> {
    let name_node = node
        .children()
        .find(|child| child.kind() == SyntaxKind::Name)?;
    let name = name_from_name_node(&name_node)?;
    let key = normalize_member_name(name.as_str());
    if implemented.properties.contains(&key) {
        return None;
    }

    let type_name = node
        .children()
        .find(|child| child.kind() == SyntaxKind::TypeRef)
        .map(|child| text_for_range(source, child.text_range()));
    let has_get = node
        .children()
        .any(|child| child.kind() == SyntaxKind::PropertyGet);
    let has_set = node
        .children()
        .any(|child| child.kind() == SyntaxKind::PropertySet);

    Some(InterfaceStub {
        name_key: key,
        kind: InterfaceStubKind::Property(PropertyStub {
            name,
            type_name,
            has_get,
            has_set,
        }),
    })
}

fn collect_implementation_members(symbols: &SymbolTable, owner_id: SymbolId) -> ImplementedMembers {
    let mut methods = FxHashSet::default();
    let mut properties = FxHashSet::default();
    let mut visited = FxHashSet::default();
    let mut current = Some(owner_id);

    while let Some(symbol_id) = current {
        if !visited.insert(symbol_id) {
            break;
        }

        for sym in symbols.iter() {
            if sym.parent != Some(symbol_id) {
                continue;
            }
            match sym.kind {
                SymbolKind::Method { .. } => {
                    if sym.modifiers.is_abstract {
                        continue;
                    }
                    methods.insert(normalize_member_name(sym.name.as_str()));
                }
                SymbolKind::Property { .. } => {
                    properties.insert(normalize_member_name(sym.name.as_str()));
                }
                _ => {}
            }
        }

        current = symbols.extends_reference(symbol_id).and_then(|base| {
            let base_name = base.name().display();
            symbols.resolve_oop_reference_for_owner(symbol_id, base_name.as_str())
        });
    }

    ImplementedMembers {
        methods,
        properties,
    }
}

fn find_interface_node_for_symbol(root: &SyntaxNode, name_range: TextRange) -> Option<SyntaxNode> {
    root.descendants()
        .filter(|node| node.kind() == SyntaxKind::Interface)
        .find(|interface_node| {
            interface_node
                .children()
                .filter(|node| node.kind() == SyntaxKind::Name)
                .filter_map(|node| ident_token_in_name(&node))
                .any(|ident| ident.text_range() == name_range)
        })
}

fn implements_clause_names(node: &SyntaxNode) -> Vec<Vec<SmolStr>> {
    let mut names = Vec::new();
    for child in node.children() {
        if !matches!(child.kind(), SyntaxKind::Name | SyntaxKind::QualifiedName) {
            continue;
        }
        if let Some(parts) = qualified_name_parts_from_node(&child) {
            names.push(parts);
        }
    }
    names
}

fn owner_end_token_offset(node: &SyntaxNode) -> Option<usize> {
    let end_kind = match node.kind() {
        SyntaxKind::Class => SyntaxKind::KwEndClass,
        SyntaxKind::FunctionBlock => SyntaxKind::KwEndFunctionBlock,
        _ => return None,
    };
    let token = node
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| token.kind() == end_kind)?;
    Some(usize::from(token.text_range().start()))
}

fn member_indent_for_owner(source: &str, owner_node: &SyntaxNode) -> String {
    if let Some(member) = owner_node
        .children()
        .find(|child| matches!(child.kind(), SyntaxKind::Method | SyntaxKind::Property))
    {
        return line_indent_at_offset(source, member.text_range().start());
    }

    let owner_indent = line_indent_at_offset(source, owner_node.text_range().start());
    let indent_unit = if owner_indent.contains('\t') {
        "\t"
    } else {
        "    "
    };
    format!("{owner_indent}{indent_unit}")
}

fn build_stub_insert_text(
    source: &str,
    insert_offset: usize,
    stubs: &[InterfaceStub],
    member_indent: &str,
) -> String {
    let indent_unit = if member_indent.contains('\t') {
        "\t"
    } else {
        "    "
    };
    let child_indent = format!("{member_indent}{indent_unit}");

    let mut chunks = Vec::new();
    for stub in stubs {
        let text = match &stub.kind {
            InterfaceStubKind::Method(method) => {
                build_method_stub(method, member_indent, &child_indent)
            }
            InterfaceStubKind::Property(property) => {
                build_property_stub(property, member_indent, &child_indent)
            }
        };
        chunks.push(text);
    }

    let mut insert = String::new();
    if insert_offset > 0 {
        let prev = source.as_bytes()[insert_offset - 1];
        if prev != b'\n' && prev != b'\r' {
            insert.push('\n');
        }
    }

    insert.push_str(&chunks.join("\n\n"));
    if !insert.ends_with('\n') {
        insert.push('\n');
    }
    insert
}

fn build_insert_text(source: &str, insert_offset: usize, block: &str) -> String {
    let mut insert = String::new();
    if insert_offset > 0 {
        let prev = source.as_bytes()[insert_offset - 1];
        if prev != b'\n' && prev != b'\r' {
            insert.push('\n');
        }
    }
    insert.push('\n');
    insert.push_str(block);
    if !insert.ends_with('\n') {
        insert.push('\n');
    }
    insert
}

fn build_method_extract_text(name: &str, indent: &str, params: &str, body: &str) -> String {
    let mut lines = Vec::new();
    lines.push(format!("{indent}METHOD {name}"));
    if !params.is_empty() {
        lines.push(params.to_string());
    }
    if !body.trim().is_empty() {
        lines.push(body.to_string());
    }
    lines.push(format!("{indent}END_METHOD"));
    lines.join("\n")
}

fn build_property_extract_text(
    name: &str,
    type_name: &str,
    expr: &str,
    indent: &str,
    body_indent: &str,
) -> String {
    let mut lines = Vec::new();
    lines.push(format!("{indent}PROPERTY {name} : {type_name}"));
    lines.push(format!("{indent}GET"));
    lines.push(format!("{body_indent}{name} := {expr};"));
    lines.push(format!("{indent}END_GET"));
    lines.push(format!("{indent}END_PROPERTY"));
    lines.join("\n")
}

fn build_function_extract_text(
    name: &str,
    return_type: &str,
    params: &str,
    body: &str,
    body_indent: &str,
    result_expr: Option<&str>,
) -> String {
    let mut lines = Vec::new();
    lines.push(format!("FUNCTION {name} : {return_type}"));
    if !params.is_empty() {
        lines.push(params.to_string());
    }
    if !body.trim().is_empty() {
        lines.push(body.to_string());
    }
    let result = result_expr.unwrap_or("TRUE");
    lines.push(format!("{body_indent}{name} := {result};"));
    lines.push("END_FUNCTION".to_string());
    lines.join("\n")
}

fn build_var_output_block(indent: &str, indent_unit: &str, name: &str, type_name: &str) -> String {
    let child_indent = format!("{indent}{indent_unit}");
    let mut lines = Vec::new();
    lines.push(format!("{indent}VAR_OUTPUT"));
    lines.push(format!("{child_indent}{name} : {type_name};"));
    lines.push(format!("{indent}END_VAR"));
    lines.join("\n")
}

fn build_var_block(indent: &str, indent_unit: &str, name: &str, type_name: &str) -> String {
    let child_indent = format!("{indent}{indent_unit}");
    let mut lines = Vec::new();
    lines.push(format!("{indent}VAR"));
    lines.push(format!("{child_indent}{name} : {type_name};"));
    lines.push(format!("{indent}END_VAR"));
    lines.join("\n")
}

fn build_method_stub(stub: &MethodStub, indent: &str, child_indent: &str) -> String {
    let mut lines = Vec::new();
    let mut header = format!("{indent}METHOD PUBLIC {}", stub.name);
    if let Some(return_type) = &stub.return_type {
        header.push_str(&format!(" : {}", return_type));
    }
    lines.push(header);

    for block in &stub.var_blocks {
        let block = reindent_block(block, indent);
        if !block.is_empty() {
            lines.push(block);
        }
    }

    lines.push(format!("{child_indent}// TODO: implement"));
    lines.push(format!("{indent}END_METHOD"));
    lines.join("\n")
}

fn build_property_stub(stub: &PropertyStub, indent: &str, child_indent: &str) -> String {
    let mut lines = Vec::new();
    let type_suffix = stub
        .type_name
        .as_ref()
        .map(|ty| format!(" : {}", ty))
        .unwrap_or_default();
    lines.push(format!(
        "{indent}PROPERTY PUBLIC {}{}",
        stub.name, type_suffix
    ));
    if stub.has_get {
        lines.push(format!("{indent}GET"));
        lines.push(format!("{child_indent}// TODO: implement"));
        lines.push(format!("{indent}END_GET"));
    }
    if stub.has_set {
        lines.push(format!("{indent}SET"));
        lines.push(format!("{child_indent}// TODO: implement"));
        lines.push(format!("{indent}END_SET"));
    }
    lines.push(format!("{indent}END_PROPERTY"));
    lines.join("\n")
}

fn indent_unit_for(indent: &str) -> &str {
    utilities::indent_unit_for(indent)
}

fn reindent_block(block: &str, indent: &str) -> String {
    utilities::reindent_block(block, indent)
}

fn line_indent_at_offset(source: &str, offset: TextSize) -> String {
    utilities::line_indent_at_offset(source, offset)
}

fn build_formal_args(params: &[ExtractParam]) -> String {
    if params.is_empty() {
        return String::new();
    }
    let args = params
        .iter()
        .map(|param| format!("{} := {}", param.name, param.name))
        .collect::<Vec<_>>()
        .join(", ");
    format!("({args})")
}

fn build_param_blocks(params: &[ExtractParam], indent: &str, indent_unit: &str) -> String {
    let mut blocks = Vec::new();

    let inputs: Vec<_> = params
        .iter()
        .filter(|param| param.direction == ExtractParamDirection::Input)
        .collect();
    if !inputs.is_empty() {
        blocks.push(build_param_block("VAR_INPUT", &inputs, indent, indent_unit));
    }

    let in_outs: Vec<_> = params
        .iter()
        .filter(|param| param.direction == ExtractParamDirection::InOut)
        .collect();
    if !in_outs.is_empty() {
        blocks.push(build_param_block(
            "VAR_IN_OUT",
            &in_outs,
            indent,
            indent_unit,
        ));
    }

    blocks.join("\n")
}

fn build_param_block(
    label: &str,
    params: &[&ExtractParam],
    indent: &str,
    indent_unit: &str,
) -> String {
    let child_indent = format!("{indent}{indent_unit}");
    let mut lines = Vec::new();
    lines.push(format!("{indent}{label}"));
    for param in params {
        lines.push(format!(
            "{child_indent}{} : {};",
            param.name, param.type_name
        ));
    }
    lines.push(format!("{indent}END_VAR"));
    lines.join("\n")
}

fn build_call_expression(name: &str, args: &str) -> String {
    let args = if args.is_empty() { "()" } else { args };
    format!("{name}{args}")
}

fn call_replace_text(
    source: &str,
    range: TextRange,
    indent: &str,
    name: &str,
    args: &str,
) -> String {
    let args = if args.is_empty() { "()" } else { args };
    let mut text = format!("{indent}{name}{args};");
    if let Some(suffix) = source.get(usize::from(range.end())..) {
        if suffix.starts_with('\n') || suffix.starts_with("\r\n") {
            text.push('\n');
        }
    }
    text
}

fn collect_extract_params<F>(
    db: &Database,
    file_id: FileId,
    source: &str,
    root: &SyntaxNode,
    selection: TextRange,
    capture: F,
) -> Vec<ExtractParam>
where
    F: Fn(&trust_hir::symbols::Symbol) -> bool,
{
    let symbols = db.file_symbols_with_project(file_id);
    let declared = declared_symbols_in_range(&symbols, selection);
    let mut params: FxHashMap<SymbolId, ExtractParam> = FxHashMap::default();

    for name_ref in root
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::NameRef)
    {
        let range = node_token_range(&name_ref);
        if !range_contains(selection, range) {
            continue;
        }
        if is_call_name(&name_ref) {
            continue;
        }
        let target = resolve_target_at_position_with_context(
            db,
            file_id,
            range.start(),
            source,
            root,
            &symbols,
        );
        let Some(ResolvedTarget::Symbol(symbol_id)) = target else {
            continue;
        };
        if declared.contains(&symbol_id) {
            continue;
        }
        let Some(symbol) = symbols.get(symbol_id) else {
            continue;
        };
        if !capture(symbol) {
            continue;
        }
        if !matches!(
            symbol.kind,
            SymbolKind::Variable { .. } | SymbolKind::Parameter { .. } | SymbolKind::Constant
        ) {
            continue;
        }
        let Some(type_name) = symbols.type_name(symbol.type_id) else {
            continue;
        };

        let entry = params.entry(symbol_id).or_insert(ExtractParam {
            name: symbol.name.clone(),
            type_name,
            direction: ExtractParamDirection::Input,
            first_pos: range.start(),
        });
        if range.start() < entry.first_pos {
            entry.first_pos = range.start();
        }
        if is_write_context(&name_ref) {
            entry.direction = ExtractParamDirection::InOut;
        }
    }

    let mut params: Vec<_> = params.into_values().collect();
    params.sort_by_key(|param| param.first_pos);
    params
}

fn declared_symbols_in_range(symbols: &SymbolTable, range: TextRange) -> FxHashSet<SymbolId> {
    symbols
        .iter()
        .filter(|symbol| range_contains(range, symbol.range))
        .map(|symbol| symbol.id)
        .collect()
}
