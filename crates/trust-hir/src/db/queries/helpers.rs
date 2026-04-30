use super::*;

pub(in crate::db) fn name_from_node(node: &SyntaxNode) -> Option<(SmolStr, TextRange)> {
    let token = node
        .children()
        .find(|n| n.kind() == SyntaxKind::Name)
        .and_then(|name_node| first_ident_token(&name_node))
        .or_else(|| first_ident_token(node))?;
    Some((SmolStr::new(token.text()), token.text_range()))
}

pub(in crate::db) fn qualified_name_parts(
    node: &SyntaxNode,
) -> Option<(Vec<(SmolStr, TextRange)>, TextRange)> {
    let name_node = if matches!(node.kind(), SyntaxKind::Name | SyntaxKind::QualifiedName) {
        node.clone()
    } else {
        node.children()
            .find(|n| matches!(n.kind(), SyntaxKind::Name | SyntaxKind::QualifiedName))?
    };

    match name_node.kind() {
        SyntaxKind::Name => {
            let (name, range) = name_from_node(&name_node)?;
            Some((vec![(name, range)], range))
        }
        SyntaxKind::QualifiedName => {
            let mut parts = Vec::new();
            for child in name_node
                .children()
                .filter(|n| n.kind() == SyntaxKind::Name)
            {
                if let Some((name, range)) = name_from_node(&child) {
                    parts.push((name, range));
                }
            }
            if parts.is_empty() {
                None
            } else {
                let range = qualified_name_token_range(&name_node)?;
                Some((parts, range))
            }
        }
        _ => None,
    }
}

pub(in crate::db) fn qualified_name_token_range(node: &SyntaxNode) -> Option<TextRange> {
    let mut first = None;
    let mut last = None;
    for token in node
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
    {
        if token.kind() != SyntaxKind::Ident {
            continue;
        }
        if first.is_none() {
            first = Some(token.clone());
        }
        last = Some(token);
    }
    match (first, last) {
        (Some(first), Some(last)) => Some(TextRange::new(
            first.text_range().start(),
            last.text_range().end(),
        )),
        _ => None,
    }
}

pub(in crate::db) fn qualified_name_string(parts: &[SmolStr]) -> SmolStr {
    if parts.is_empty() {
        return SmolStr::new("");
    }
    let mut buf = String::new();
    for (idx, part) in parts.iter().enumerate() {
        if idx > 0 {
            buf.push('.');
        }
        buf.push_str(part.as_str());
    }
    SmolStr::new(buf)
}

pub(in crate::db) fn qualify_name(namespace: &[SmolStr], name: &SmolStr) -> SmolStr {
    if namespace.is_empty() {
        return name.clone();
    }
    let mut buf = String::new();
    for (idx, part) in namespace.iter().enumerate() {
        if idx > 0 {
            buf.push('.');
        }
        buf.push_str(part.as_str());
    }
    buf.push('.');
    buf.push_str(name.as_str());
    SmolStr::new(buf)
}

pub(in crate::db) fn explicit_visibility_from_node(node: &SyntaxNode) -> Option<Visibility> {
    for element in node.children_with_tokens() {
        let Some(token) = element.into_token() else {
            continue;
        };
        let visibility = match token.kind() {
            SyntaxKind::KwPublic => Visibility::Public,
            SyntaxKind::KwPrivate => Visibility::Private,
            SyntaxKind::KwProtected => Visibility::Protected,
            SyntaxKind::KwInternal => Visibility::Internal,
            _ => continue,
        };
        return Some(visibility);
    }
    None
}

pub(in crate::db) fn visibility_from_node(node: &SyntaxNode) -> Visibility {
    explicit_visibility_from_node(node).unwrap_or(Visibility::Public)
}

pub(in crate::db) fn modifiers_from_node(node: &SyntaxNode) -> SymbolModifiers {
    let mut modifiers = SymbolModifiers::default();
    for element in node.children_with_tokens() {
        let Some(token) = element.into_token() else {
            continue;
        };
        match token.kind() {
            SyntaxKind::KwFinal => modifiers.is_final = true,
            SyntaxKind::KwAbstract => modifiers.is_abstract = true,
            SyntaxKind::KwOverride => modifiers.is_override = true,
            _ => {}
        }
    }
    modifiers
}

pub(in crate::db) fn implements_clause_names(node: &SyntaxNode) -> Vec<(Vec<SmolStr>, TextRange)> {
    let mut names = Vec::new();
    for child in node.children() {
        if !matches!(child.kind(), SyntaxKind::Name | SyntaxKind::QualifiedName) {
            continue;
        }
        if let Some((parts, range)) = qualified_name_parts(&child) {
            let segments: Vec<SmolStr> = parts.into_iter().map(|(name, _)| name).collect();
            names.push((segments, range));
        }
    }
    names
}

pub(in crate::db) fn normalize_member_name(name: &str) -> SmolStr {
    SmolStr::new(name.to_ascii_lowercase())
}

pub(in crate::db) fn first_ident_token(node: &SyntaxNode) -> Option<SyntaxToken> {
    node.descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| {
            matches!(
                token.kind(),
                SyntaxKind::Ident
                    | SyntaxKind::KwEn
                    | SyntaxKind::KwEno
                    | SyntaxKind::KwGet
                    | SyntaxKind::KwSet
            )
        })
}

pub(in crate::db) fn type_path_from_type_ref(
    node: &SyntaxNode,
) -> Option<(Vec<(SmolStr, TextRange)>, TextRange)> {
    if let Some(array_node) = node.children().find(|n| n.kind() == SyntaxKind::ArrayType) {
        if let Some(inner) = array_node
            .children()
            .find(|n| n.kind() == SyntaxKind::TypeRef)
        {
            return type_path_from_type_ref(&inner);
        }
    }

    if let Some(pointer_node) = node
        .children()
        .find(|n| n.kind() == SyntaxKind::PointerType)
    {
        if let Some(inner) = pointer_node
            .children()
            .find(|n| n.kind() == SyntaxKind::TypeRef)
        {
            return type_path_from_type_ref(&inner);
        }
    }

    if let Some(reference_node) = node
        .children()
        .find(|n| n.kind() == SyntaxKind::ReferenceType)
    {
        if let Some(inner) = reference_node
            .children()
            .find(|n| n.kind() == SyntaxKind::TypeRef)
        {
            return type_path_from_type_ref(&inner);
        }
    }

    if let Some(string_node) = node.children().find(|n| n.kind() == SyntaxKind::StringType) {
        for token in string_node
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
        {
            if let Some(name) = builtin_type_name_from_syntax(token.kind()) {
                let part = SmolStr::new(name);
                return Some((vec![(part, token.text_range())], token.text_range()));
            }
        }
    }

    if let Some((parts, range)) = qualified_name_parts(node) {
        return Some((parts, range));
    }

    for token in node
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
    {
        if token.kind() == SyntaxKind::Ident {
            let part = SmolStr::new(token.text());
            return Some((vec![(part, token.text_range())], token.text_range()));
        }
        if let Some(name) = builtin_type_name_from_syntax(token.kind()) {
            let part = SmolStr::new(name);
            return Some((vec![(part, token.text_range())], token.text_range()));
        }
    }
    None
}

pub(in crate::db) fn builtin_type_name_from_syntax(kind: SyntaxKind) -> Option<&'static str> {
    match kind {
        SyntaxKind::KwBool => Some("BOOL"),
        SyntaxKind::KwSInt => Some("SINT"),
        SyntaxKind::KwInt => Some("INT"),
        SyntaxKind::KwDInt => Some("DINT"),
        SyntaxKind::KwLInt => Some("LINT"),
        SyntaxKind::KwUSInt => Some("USINT"),
        SyntaxKind::KwUInt => Some("UINT"),
        SyntaxKind::KwUDInt => Some("UDINT"),
        SyntaxKind::KwULInt => Some("ULINT"),
        SyntaxKind::KwReal => Some("REAL"),
        SyntaxKind::KwLReal => Some("LREAL"),
        SyntaxKind::KwByte => Some("BYTE"),
        SyntaxKind::KwWord => Some("WORD"),
        SyntaxKind::KwDWord => Some("DWORD"),
        SyntaxKind::KwLWord => Some("LWORD"),
        SyntaxKind::KwTime => Some("TIME"),
        SyntaxKind::KwLTime => Some("LTIME"),
        SyntaxKind::KwDate => Some("DATE"),
        SyntaxKind::KwLDate => Some("LDATE"),
        SyntaxKind::KwTimeOfDay => Some("TIME_OF_DAY"),
        SyntaxKind::KwLTimeOfDay => Some("LTIME_OF_DAY"),
        SyntaxKind::KwDateAndTime => Some("DATE_AND_TIME"),
        SyntaxKind::KwLDateAndTime => Some("LDATE_AND_TIME"),
        SyntaxKind::KwString => Some("STRING"),
        SyntaxKind::KwWString => Some("WSTRING"),
        SyntaxKind::KwChar => Some("CHAR"),
        SyntaxKind::KwWChar => Some("WCHAR"),
        SyntaxKind::KwAny => Some("ANY"),
        SyntaxKind::KwAnyDerived => Some("ANY_DERIVED"),
        SyntaxKind::KwAnyElementary => Some("ANY_ELEMENTARY"),
        SyntaxKind::KwAnyMagnitude => Some("ANY_MAGNITUDE"),
        SyntaxKind::KwAnyInt => Some("ANY_INT"),
        SyntaxKind::KwAnyUnsigned => Some("ANY_UNSIGNED"),
        SyntaxKind::KwAnySigned => Some("ANY_SIGNED"),
        SyntaxKind::KwAnyReal => Some("ANY_REAL"),
        SyntaxKind::KwAnyNum => Some("ANY_NUM"),
        SyntaxKind::KwAnyDuration => Some("ANY_DURATION"),
        SyntaxKind::KwAnyBit => Some("ANY_BIT"),
        SyntaxKind::KwAnyChars => Some("ANY_CHARS"),
        SyntaxKind::KwAnyString => Some("ANY_STRING"),
        SyntaxKind::KwAnyChar => Some("ANY_CHAR"),
        SyntaxKind::KwAnyDate => Some("ANY_DATE"),
        _ => None,
    }
}

pub(in crate::db) fn var_qualifier_from_block(node: &SyntaxNode) -> VarQualifier {
    for token in node
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
    {
        match token.kind() {
            SyntaxKind::KwVarInput => return VarQualifier::Input,
            SyntaxKind::KwVarOutput => return VarQualifier::Output,
            SyntaxKind::KwVarInOut => return VarQualifier::InOut,
            SyntaxKind::KwVarTemp => return VarQualifier::Temp,
            SyntaxKind::KwVarGlobal => return VarQualifier::Global,
            SyntaxKind::KwVarExternal => return VarQualifier::External,
            SyntaxKind::KwVarStat => return VarQualifier::Static,
            SyntaxKind::KwVar | SyntaxKind::KwVarAccess | SyntaxKind::KwVarConfig => {
                return VarQualifier::Local
            }
            _ => {}
        }
    }
    VarQualifier::Local
}

pub(in crate::db) fn var_block_is_constant(node: &SyntaxNode) -> bool {
    node.descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .any(|token| token.kind() == SyntaxKind::KwConstant)
}

#[derive(Default, Clone, Copy)]
pub(in crate::db) struct VarBlockModifiers {
    pub(in crate::db) constant: bool,
    pub(in crate::db) retain: Option<TextRange>,
    pub(in crate::db) non_retain: Option<TextRange>,
    pub(in crate::db) persistent: Option<TextRange>,
}

pub(in crate::db) fn var_block_modifiers(node: &SyntaxNode) -> VarBlockModifiers {
    let mut modifiers = VarBlockModifiers::default();
    for token in node
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
    {
        match token.kind() {
            SyntaxKind::KwConstant => modifiers.constant = true,
            SyntaxKind::KwRetain => modifiers.retain = Some(token.text_range()),
            SyntaxKind::KwNonRetain => modifiers.non_retain = Some(token.text_range()),
            SyntaxKind::KwPersistent => modifiers.persistent = Some(token.text_range()),
            _ => {}
        }
    }
    modifiers
}

pub(in crate::db) fn retention_modifier_range(modifiers: &VarBlockModifiers) -> Option<TextRange> {
    modifiers
        .retain
        .or(modifiers.non_retain)
        .or(modifiers.persistent)
}

pub(in crate::db) fn direct_address_has_wildcard(address: &str) -> bool {
    address.contains('*')
}

pub(in crate::db) fn var_decl_direct_address(node: &SyntaxNode) -> Option<SmolStr> {
    let mut saw_at = false;
    for token in node
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
    {
        if token.kind() == SyntaxKind::KwAt {
            saw_at = true;
            continue;
        }
        if saw_at {
            if token.kind() == SyntaxKind::DirectAddress {
                return Some(SmolStr::new(token.text()));
            }
            if token.kind().is_trivia() {
                continue;
            }
            break;
        }
    }
    None
}

pub(in crate::db) fn config_init_direct_address(node: &SyntaxNode) -> Option<SmolStr> {
    let mut saw_at = false;
    for token in node
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
    {
        if token.kind() == SyntaxKind::KwAt {
            saw_at = true;
            continue;
        }
        if saw_at {
            if token.kind() == SyntaxKind::DirectAddress {
                return Some(SmolStr::new(token.text()));
            }
            if token.kind().is_trivia() {
                continue;
            }
            break;
        }
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::db) enum AccessMode {
    ReadOnly,
    ReadWrite,
}

pub(in crate::db) fn access_decl_mode(node: &SyntaxNode) -> AccessMode {
    for token in node
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
    {
        match token.kind() {
            SyntaxKind::KwReadOnly => return AccessMode::ReadOnly,
            SyntaxKind::KwReadWrite => return AccessMode::ReadWrite,
            SyntaxKind::Ident => {
                let text = token.text().to_ascii_uppercase();
                if text == "READ_ONLY" {
                    return AccessMode::ReadOnly;
                }
                if text == "READ_WRITE" {
                    return AccessMode::ReadWrite;
                }
            }
            _ => {}
        }
    }
    AccessMode::ReadWrite
}

pub(in crate::db) fn config_init_has_initializer(node: &SyntaxNode) -> bool {
    node.children_with_tokens()
        .filter_map(|e| e.into_token())
        .any(|token| token.kind() == SyntaxKind::Assign)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::db) enum AccessPathSegment {
    Field(SmolStr),
    Index(usize),
}

#[derive(Debug, Clone)]
pub(in crate::db) struct ParsedAccessPath {
    pub root: SmolStr,
    pub segments: Vec<AccessPathSegment>,
}

pub(in crate::db) fn parse_access_path(node: &SyntaxNode) -> Option<ParsedAccessPath> {
    let mut root: Option<SmolStr> = None;
    let mut segments = Vec::new();
    let mut in_index = false;
    let mut index_count = 0usize;
    let mut expect_member = false;

    for element in node.children_with_tokens() {
        if let Some(token) = element.as_token() {
            match token.kind() {
                SyntaxKind::DirectAddress => {
                    return None;
                }
                SyntaxKind::IntLiteral if expect_member => {
                    return None;
                }
                SyntaxKind::Dot => {
                    expect_member = true;
                }
                SyntaxKind::LBracket => {
                    in_index = true;
                    index_count = 1;
                }
                SyntaxKind::Comma if in_index => {
                    index_count += 1;
                }
                SyntaxKind::RBracket if in_index => {
                    segments.push(AccessPathSegment::Index(index_count));
                    in_index = false;
                }
                _ => {}
            }
            continue;
        }

        let Some(child) = element.as_node() else {
            continue;
        };
        if child.kind() != SyntaxKind::Name {
            continue;
        }
        let (name, _) = name_from_node(child)?;
        if root.is_none() {
            root = Some(name);
        } else {
            segments.push(AccessPathSegment::Field(name));
        }
        expect_member = false;
    }

    root.map(|root| ParsedAccessPath { root, segments })
}

#[derive(Debug, Clone, Copy)]
pub(in crate::db) struct AccessTarget {
    pub(in crate::db) symbol_id: SymbolId,
    pub(in crate::db) leaf_type: TypeId,
}

pub(in crate::db) fn program_config_instance_and_type(
    node: &SyntaxNode,
) -> Option<(SmolStr, Vec<SmolStr>)> {
    let mut instance: Option<SmolStr> = None;
    let mut type_parts: Option<Vec<SmolStr>> = None;
    let mut expect_instance = false;
    let mut expect_type = false;

    for element in node.children_with_tokens() {
        if let Some(token) = element.as_token() {
            match token.kind() {
                SyntaxKind::KwProgram => {
                    expect_instance = true;
                }
                SyntaxKind::Colon => {
                    expect_type = true;
                }
                _ => {}
            }
            continue;
        }

        let Some(child) = element.as_node() else {
            continue;
        };
        match child.kind() {
            SyntaxKind::Name => {
                if expect_instance && instance.is_none() {
                    if let Some((name, _)) = name_from_node(child) {
                        instance = Some(name);
                        expect_instance = false;
                    }
                    continue;
                }
                if expect_type && type_parts.is_none() {
                    if let Some((parts, _)) = qualified_name_parts(child) {
                        type_parts = Some(parts.into_iter().map(|(name, _)| name).collect());
                        expect_type = false;
                    }
                }
            }
            SyntaxKind::QualifiedName | SyntaxKind::TypeRef
                if expect_type && type_parts.is_none() =>
            {
                if let Some((parts, _)) = qualified_name_parts(child) {
                    type_parts = Some(parts.into_iter().map(|(name, _)| name).collect());
                    expect_type = false;
                }
            }
            _ => {}
        }
    }

    Some((instance?, type_parts?))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::db) enum ProgramInstanceTarget {
    Resolved(SymbolId),
    Ambiguous(Vec<SymbolId>),
}

impl ProgramInstanceTarget {
    pub(in crate::db) fn symbol_id(&self) -> Option<SymbolId> {
        match self {
            ProgramInstanceTarget::Resolved(symbol_id) => Some(*symbol_id),
            ProgramInstanceTarget::Ambiguous(_) => None,
        }
    }

    pub(in crate::db) fn is_ambiguous(&self) -> bool {
        matches!(self, ProgramInstanceTarget::Ambiguous(_))
    }

    pub(in crate::db) fn candidates(&self) -> &[SymbolId] {
        match self {
            ProgramInstanceTarget::Resolved(symbol_id) => std::slice::from_ref(symbol_id),
            ProgramInstanceTarget::Ambiguous(candidates) => candidates,
        }
    }
}

pub(in crate::db) type ProgramInstanceMap = FxHashMap<SmolStr, ProgramInstanceTarget>;

pub(in crate::db) fn lookup_program_instance<'a>(
    program_instances: &'a ProgramInstanceMap,
    name: &str,
) -> Option<&'a ProgramInstanceTarget> {
    let normalized = SmolStr::new(name.to_ascii_uppercase());
    program_instances.get(&normalized)
}

pub(in crate::db) fn collect_program_instances(
    symbols: &SymbolTable,
    root: &SyntaxNode,
) -> ProgramInstanceMap {
    let mut program_instances = FxHashMap::default();

    for node in root
        .descendants()
        .filter(|n| n.kind() == SyntaxKind::ProgramConfig)
    {
        let Some((instance, type_parts)) = program_config_instance_and_type(&node) else {
            continue;
        };
        let symbol_id = symbols.resolve_qualified(&type_parts).or_else(|| {
            if type_parts.len() == 1 {
                symbols.lookup(type_parts[0].as_str())
            } else {
                None
            }
        });
        let Some(symbol_id) = symbol_id else {
            continue;
        };
        let Some(symbol) = symbols.get(symbol_id) else {
            continue;
        };
        if !matches!(symbol.kind, SymbolKind::Program) {
            continue;
        }
        let normalized_instance = SmolStr::new(instance.as_str().to_ascii_uppercase());
        program_instances
            .entry(normalized_instance)
            .and_modify(|entry| match entry {
                ProgramInstanceTarget::Resolved(existing) => {
                    *entry = ProgramInstanceTarget::Ambiguous(vec![*existing, symbol_id]);
                }
                ProgramInstanceTarget::Ambiguous(candidates) => candidates.push(symbol_id),
            })
            .or_insert(ProgramInstanceTarget::Resolved(symbol_id));
    }

    program_instances
}

pub(in crate::db) fn resolve_access_path_target(
    symbols: &SymbolTable,
    program_instances: &ProgramInstanceMap,
    node: &SyntaxNode,
) -> Option<AccessTarget> {
    let access = parse_access_path(node)?;
    let current_symbol = match lookup_program_instance(program_instances, access.root.as_str()) {
        Some(ProgramInstanceTarget::Resolved(symbol_id)) => *symbol_id,
        Some(ProgramInstanceTarget::Ambiguous(_)) => return None,
        None => match symbols.resolve_unique_symbol_name(access.root.as_str()) {
            UniqueSymbolResolution::Single(symbol_id) => symbol_id,
            UniqueSymbolResolution::NotFound | UniqueSymbolResolution::Ambiguous => return None,
        },
    };
    resolve_access_path_target_from_symbol(symbols, current_symbol, &access.segments)
}

pub(in crate::db) fn resolve_access_path_target_from_symbol(
    symbols: &SymbolTable,
    root_symbol: SymbolId,
    segments: &[AccessPathSegment],
) -> Option<AccessTarget> {
    let mut current_symbol = root_symbol;
    let mut current_type = symbols.resolve_alias_type(symbols.get(current_symbol)?.type_id);

    for segment in segments {
        match segment {
            AccessPathSegment::Index(count) => {
                let resolved = symbols.resolve_alias_type(current_type);
                let Some(Type::Array {
                    element,
                    dimensions,
                }) = symbols.type_by_id(resolved)
                else {
                    return None;
                };
                if *count != dimensions.len() {
                    return None;
                }
                current_type = *element;
            }
            AccessPathSegment::Field(name) => {
                if let Some(symbol) = symbols.get(current_symbol) {
                    if matches!(symbol.kind, SymbolKind::Namespace | SymbolKind::Program) {
                        let child = symbols
                            .resolve_member_symbol_in_hierarchy(current_symbol, name.as_str())?;
                        current_symbol = child;
                        current_type = symbols.resolve_alias_type(symbols.get(child)?.type_id);
                        continue;
                    }
                }

                let resolved = symbols.resolve_alias_type(current_type);
                match symbols.type_by_id(resolved) {
                    Some(Type::Struct { fields, .. }) => {
                        let field_type = fields
                            .iter()
                            .find(|field| field.name.eq_ignore_ascii_case(name.as_str()))
                            .map(|field| field.type_id)?;
                        current_type = field_type;
                    }
                    Some(Type::Union { variants, .. }) => {
                        let field_type = variants
                            .iter()
                            .find(|variant| variant.name.eq_ignore_ascii_case(name.as_str()))
                            .map(|variant| variant.type_id)?;
                        current_type = field_type;
                    }
                    Some(
                        Type::FunctionBlock { .. } | Type::Class { .. } | Type::Interface { .. },
                    ) => {
                        let member =
                            symbols.resolve_member_symbol_in_type(current_type, name.as_str())?;
                        current_symbol = member;
                        current_type = symbols.resolve_alias_type(symbols.get(member)?.type_id);
                    }
                    _ => return None,
                }
            }
        }
    }

    Some(AccessTarget {
        symbol_id: current_symbol,
        leaf_type: current_type,
    })
}
