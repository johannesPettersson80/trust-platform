use super::*;
use crate::db::diagnostics::is_expression_kind;
use crate::semantic::LEGACY_UNKNOWN_TYPE_ID;

impl SymbolCollector<'_> {
    pub(super) fn check_access_and_config(&mut self, root: &SyntaxNode) {
        self.collect_program_instances(root);
        for node in root.descendants() {
            match node.kind() {
                SyntaxKind::AccessDecl => self.check_access_decl(&node),
                SyntaxKind::ConfigInit => self.check_config_init(&node),
                _ => {}
            }
        }
    }

    pub(super) fn collect_program_instances(&mut self, root: &SyntaxNode) {
        self.program_instances = collect_program_instances(&self.table, root);
    }

    pub(super) fn check_global_external_links(&mut self, root: &SyntaxNode) {
        #[derive(Clone, Copy)]
        struct GlobalInfo {
            type_id: TypeId,
            is_constant: bool,
        }

        #[derive(Clone, Copy)]
        struct ExternalInfo {
            type_id: TypeId,
            is_constant: bool,
            has_initializer: bool,
            range: TextRange,
        }

        let mut globals: FxHashMap<SmolStr, GlobalInfo> = FxHashMap::default();
        let mut externals: Vec<(SmolStr, ExternalInfo)> = Vec::new();

        for block in root
            .descendants()
            .filter(|n| n.kind() == SyntaxKind::VarBlock)
        {
            let qualifier = var_qualifier_from_block(&block);
            let is_constant = var_block_is_constant(&block);

            if qualifier != VarQualifier::Global && qualifier != VarQualifier::External {
                continue;
            }

            for var_decl in block.children().filter(|n| n.kind() == SyntaxKind::VarDecl) {
                let (names, type_id, _) = self.extract_var_decl_info(&var_decl);
                let has_initializer = var_decl.children().any(|n| is_expression_kind(n.kind()));

                for (name, range) in names {
                    match qualifier {
                        VarQualifier::Global => {
                            globals.insert(
                                name.clone(),
                                GlobalInfo {
                                    type_id,
                                    is_constant,
                                },
                            );
                        }
                        VarQualifier::External => {
                            externals.push((
                                name.clone(),
                                ExternalInfo {
                                    type_id,
                                    is_constant,
                                    has_initializer,
                                    range,
                                },
                            ));
                        }
                        _ => {}
                    }
                }
            }
        }

        for (name, ext) in externals {
            let Some(global) = globals.get(&name) else {
                self.diagnostics.error(
                    DiagnosticCode::UndefinedVariable,
                    ext.range,
                    format!("VAR_EXTERNAL '{}' has no matching VAR_GLOBAL", name),
                );
                continue;
            };

            let target_type = self.table.resolve_alias_type(global.type_id);
            let source_type = self.table.resolve_alias_type(ext.type_id);
            if target_type != TypeId::UNKNOWN
                && source_type != TypeId::UNKNOWN
                && target_type != source_type
            {
                self.diagnostics.error(
                    DiagnosticCode::TypeMismatch,
                    ext.range,
                    format!(
                        "VAR_EXTERNAL '{}' type '{}' does not match VAR_GLOBAL type '{}'",
                        name,
                        self.table
                            .type_name(source_type)
                            .unwrap_or_else(|| "?".into()),
                        self.table
                            .type_name(target_type)
                            .unwrap_or_else(|| "?".into())
                    ),
                );
            }

            if global.is_constant && !ext.is_constant {
                self.diagnostics.error(
                    DiagnosticCode::InvalidOperation,
                    ext.range,
                    format!(
                        "VAR_EXTERNAL '{}' must be CONSTANT to match VAR_GLOBAL CONSTANT",
                        name
                    ),
                );
            }

            if ext.has_initializer {
                self.diagnostics.error(
                    DiagnosticCode::InvalidOperation,
                    ext.range,
                    format!("VAR_EXTERNAL '{}' cannot declare an initial value", name),
                );
            }
        }
    }

    pub(super) fn check_var_block_modifiers(&mut self, root: &SyntaxNode) {
        for block in root
            .descendants()
            .filter(|n| n.kind() == SyntaxKind::VarBlock)
        {
            let modifiers = var_block_modifiers(&block);
            let qualifier = var_qualifier_from_block(&block);

            let retention_count = [
                modifiers.retain.is_some(),
                modifiers.non_retain.is_some(),
                modifiers.persistent.is_some(),
            ]
            .into_iter()
            .filter(|v| *v)
            .count();

            if retention_count > 1 {
                self.diagnostics.error(
                    DiagnosticCode::InvalidOperation,
                    block.text_range(),
                    "VAR section cannot combine RETAIN, NON_RETAIN, and PERSISTENT",
                );
            }

            if modifiers.constant && retention_count > 0 {
                let range =
                    retention_modifier_range(&modifiers).unwrap_or_else(|| block.text_range());
                self.diagnostics.error(
                    DiagnosticCode::InvalidOperation,
                    range,
                    "CONSTANT cannot be combined with RETAIN, NON_RETAIN, or PERSISTENT",
                );
            }

            if retention_count > 0
                && !matches!(
                    qualifier,
                    VarQualifier::Local
                        | VarQualifier::Input
                        | VarQualifier::Output
                        | VarQualifier::Global
                        | VarQualifier::Static
                )
            {
                let range =
                    retention_modifier_range(&modifiers).unwrap_or_else(|| block.text_range());
                self.diagnostics.error(
                    DiagnosticCode::InvalidOperation,
                    range,
                    "RETAIN/NON_RETAIN/PERSISTENT not allowed in this VAR section",
                );
            }
        }
    }

    pub(super) fn check_at_bindings(&mut self, roots: &[SyntaxNode]) {
        let mut wildcard_vars = FxHashSet::default();

        for symbol in self.table.iter() {
            if symbol.origin.is_some() {
                continue;
            }
            let Some(address) = symbol.direct_address.as_deref() else {
                continue;
            };
            if !direct_address_has_wildcard(address) {
                continue;
            }

            if matches!(
                symbol.kind,
                SymbolKind::Parameter {
                    direction: ParamDirection::In | ParamDirection::InOut
                }
            ) {
                self.diagnostics.error(
                    DiagnosticCode::InvalidOperation,
                    symbol.range,
                    "incomplete direct address not allowed in VAR_INPUT or VAR_IN_OUT",
                );
            }

            if symbol.parent.is_some() {
                wildcard_vars.insert(symbol.id);
            }
        }

        let mut configured = FxHashSet::default();
        for config_init in roots.iter().flat_map(|root| {
            root.descendants()
                .filter(|n| n.kind() == SyntaxKind::ConfigInit)
        }) {
            let Some(access_path) = config_init
                .children()
                .find(|n| n.kind() == SyntaxKind::AccessPath)
            else {
                continue;
            };
            if let Some(parsed) = parse_access_path(&access_path) {
                if let Some(program_instance) =
                    lookup_program_instance(&self.program_instances, parsed.root.as_str())
                {
                    if program_instance.is_ambiguous() {
                        if let Some(address) = config_init_direct_address(&config_init) {
                            if direct_address_has_wildcard(&address) {
                                self.diagnostics.error(
                                    DiagnosticCode::InvalidOperation,
                                    access_path.text_range(),
                                    "VAR_CONFIG must provide a fully specified direct address",
                                );
                            } else {
                                for candidate in program_instance.candidates() {
                                    if let Some(target) = resolve_access_path_target_from_symbol(
                                        &self.table,
                                        *candidate,
                                        &parsed.segments,
                                    ) {
                                        if wildcard_vars.contains(&target.symbol_id) {
                                            configured.insert(target.symbol_id);
                                        }
                                    }
                                }
                            }
                        }
                        continue;
                    }
                }
            }
            let Some(target) = self.resolve_access_path_target(&access_path) else {
                continue;
            };

            if let Some(address) = config_init_direct_address(&config_init) {
                if direct_address_has_wildcard(&address) {
                    self.diagnostics.error(
                        DiagnosticCode::InvalidOperation,
                        access_path.text_range(),
                        "VAR_CONFIG must provide a fully specified direct address",
                    );
                } else if wildcard_vars.contains(&target.symbol_id) {
                    configured.insert(target.symbol_id);
                }
            }
        }

        for symbol_id in wildcard_vars {
            if configured.contains(&symbol_id) {
                continue;
            }
            let Some(symbol) = self.table.get(symbol_id) else {
                continue;
            };
            let address = symbol.direct_address.as_deref().unwrap_or("%*");
            self.diagnostics.error(
                DiagnosticCode::InvalidOperation,
                symbol.range,
                format!("direct address '{}' requires VAR_CONFIG mapping", address),
            );
        }
    }

    pub(super) fn check_access_decl(&mut self, node: &SyntaxNode) {
        let Some((name, _)) = node
            .children()
            .find(|n| n.kind() == SyntaxKind::Name)
            .and_then(|n| name_from_node(&n))
        else {
            return;
        };
        let Some(access_path) = node.children().find(|n| n.kind() == SyntaxKind::AccessPath) else {
            return;
        };

        let decl_id = match self.table.resolve_unique_symbol_name(name.as_str()) {
            UniqueSymbolResolution::Single(id) => id,
            UniqueSymbolResolution::Ambiguous => {
                self.diagnostics.error(
                    DiagnosticCode::CannotResolve,
                    node.text_range(),
                    format!("VAR_ACCESS declaration '{}' is ambiguous", name),
                );
                return;
            }
            UniqueSymbolResolution::NotFound => return,
        };
        let Some(decl_sym) = self.table.get(decl_id) else {
            return;
        };
        let declared_type = self.table.resolve_alias_type(decl_sym.type_id);

        let Some(target) = self.resolve_access_path_target(&access_path) else {
            if let Some(parsed) = parse_access_path(&access_path) {
                let program_instance =
                    lookup_program_instance(&self.program_instances, parsed.root.as_str());
                if program_instance.is_some_and(ProgramInstanceTarget::is_ambiguous) {
                    self.diagnostics.error(
                        DiagnosticCode::CannotResolve,
                        access_path.text_range(),
                        format!("program instance '{}' is ambiguous", parsed.root),
                    );
                    return;
                }
                let root_declared = program_instance.is_some()
                    || !matches!(
                        self.table.resolve_unique_symbol_name(parsed.root.as_str()),
                        UniqueSymbolResolution::NotFound
                    );
                if !root_declared {
                    self.diagnostics.error(
                        DiagnosticCode::UndefinedVariable,
                        access_path.text_range(),
                        format!("VAR_ACCESS target '{}' is undefined", parsed.root),
                    );
                }
            }
            return;
        };
        let target_type = self.table.resolve_alias_type(target.leaf_type);

        if declared_type != TypeId::UNKNOWN
            && target_type != TypeId::UNKNOWN
            && declared_type != target_type
        {
            self.diagnostics.error(
                DiagnosticCode::TypeMismatch,
                access_path.text_range(),
                format!(
                    "VAR_ACCESS type '{}' does not match access path type '{}'",
                    self.table
                        .type_name(declared_type)
                        .unwrap_or_else(|| "?".into()),
                    self.table
                        .type_name(target_type)
                        .unwrap_or_else(|| "?".into())
                ),
            );
        }
    }

    pub(super) fn check_config_init(&mut self, node: &SyntaxNode) {
        let Some(access_path) = node.children().find(|n| n.kind() == SyntaxKind::AccessPath) else {
            return;
        };
        let Some(target) = self.resolve_access_path_target(&access_path) else {
            if let Some(parsed) = parse_access_path(&access_path) {
                let program_instance =
                    lookup_program_instance(&self.program_instances, parsed.root.as_str());
                if program_instance.is_some_and(ProgramInstanceTarget::is_ambiguous) {
                    self.diagnostics.error(
                        DiagnosticCode::CannotResolve,
                        access_path.text_range(),
                        format!("program instance '{}' is ambiguous", parsed.root),
                    );
                    return;
                }
                let root_declared = program_instance.is_some()
                    || !matches!(
                        self.table.resolve_unique_symbol_name(parsed.root.as_str()),
                        UniqueSymbolResolution::NotFound
                    );
                if root_declared {
                    self.diagnostics.error(
                        DiagnosticCode::InvalidOperation,
                        access_path.text_range(),
                        format!(
                            "VAR_CONFIG target '{}' cannot be resolved",
                            access_path.text()
                        ),
                    );
                } else {
                    self.diagnostics.error(
                        DiagnosticCode::UndefinedVariable,
                        access_path.text_range(),
                        format!("VAR_CONFIG target '{}' is undefined", parsed.root),
                    );
                }
            } else {
                self.diagnostics.error(
                    DiagnosticCode::InvalidOperation,
                    access_path.text_range(),
                    "VAR_CONFIG target must be a variable access path",
                );
            }
            return;
        };
        let Some(target_sym) = self.table.get(target.symbol_id) else {
            return;
        };
        let target_kind = target_sym.kind.clone();
        let target_type = self.table.resolve_alias_type(target.leaf_type);

        let declared_type = node
            .children()
            .find(|n| n.kind() == SyntaxKind::TypeRef)
            .map(|n| self.resolve_type_from_ref(&n))
            .unwrap_or(LEGACY_UNKNOWN_TYPE_ID);
        let declared_type = self.table.resolve_alias_type(declared_type);

        if declared_type != TypeId::UNKNOWN
            && target_type != TypeId::UNKNOWN
            && declared_type != target_type
        {
            self.diagnostics.error(
                DiagnosticCode::TypeMismatch,
                access_path.text_range(),
                format!(
                    "VAR_CONFIG type '{}' does not match target type '{}'",
                    self.table
                        .type_name(declared_type)
                        .unwrap_or_else(|| "?".into()),
                    self.table
                        .type_name(target_type)
                        .unwrap_or_else(|| "?".into())
                ),
            );
        }

        if config_init_has_initializer(node) {
            match target_kind {
                SymbolKind::Constant => {
                    self.diagnostics.error(
                        DiagnosticCode::InvalidOperation,
                        access_path.text_range(),
                        "VAR_CONFIG cannot initialize CONSTANT targets",
                    );
                }
                SymbolKind::Variable { qualifier } => {
                    if matches!(
                        qualifier,
                        VarQualifier::Temp | VarQualifier::External | VarQualifier::InOut
                    ) {
                        self.diagnostics.error(
                            DiagnosticCode::InvalidOperation,
                            access_path.text_range(),
                            "VAR_CONFIG cannot initialize this variable section",
                        );
                    }
                }
                _ => {}
            }
        }
    }

    pub(super) fn resolve_access_path_target(&self, node: &SyntaxNode) -> Option<AccessTarget> {
        resolve_access_path_target(&self.table, &self.program_instances, node)
    }
}
