//! HMI declaration discovery. This module never lowers executable code or creates a Runtime.
//! HIR owns parsing, type resolution and imported declarations; this is a presentation adapter.
use std::collections::BTreeMap;
use std::sync::Arc;
use trust_hir::project::{Project, SourceKey};
use trust_hir::symbols::{Symbol, SymbolId, SymbolKind, SymbolTable, VarQualifier, Visibility};
use trust_hir::types::{Type, TypeId};
use trust_runtime::hmi::catalogue::ValueKind;
use trust_runtime::hmi::variable_tree::{ArrayShape, VariableAccess, VariableKind, VariableNode};

pub(in crate::handlers::commands) struct Declarations {
    pub symbols: Arc<SymbolTable>,
    pub registry: trust_hir::types::TypeRegistry,
    pub task_intervals: Vec<(String, String)>,
    pub source_paths: std::collections::HashMap<trust_hir::db::FileId, std::path::PathBuf>,
    pub source_texts: std::collections::HashMap<trust_hir::db::FileId, String>,
    names: BTreeMap<String, SymbolId>,
    qualified: std::collections::HashMap<SymbolId, String>,
    pub variables: Vec<VariableNode>,
}

impl Declarations {
    pub fn read(sources: &[super::super::LoadedSource]) -> Arc<Self> {
        use sha2::{Digest, Sha256};
        use std::sync::{Mutex, OnceLock};
        type Cache = std::collections::VecDeque<(String, Arc<Declarations>)>;
        static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
        let mut digest = Sha256::new();
        for source in sources {
            digest.update(source.path.to_string_lossy().as_bytes());
            digest.update([0]);
            digest.update(source.text.as_bytes());
            digest.update([0]);
        }
        let key = hex::encode(digest.finalize()).to_string();
        let mut cache = CACHE
            .get_or_init(Default::default)
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some((_, index)) = cache.iter().find(|(stored, _)| stored == &key) {
            return index.clone();
        }
        let index = Arc::new(Self::collect(sources));
        cache.push_front((key, index.clone()));
        cache.truncate(2);
        index
    }
    fn collect(sources: &[super::super::LoadedSource]) -> Self {
        let _trace = trust_hir::timing::CompilerStage::new("hmi.declarations", sources.len());
        let mut project = Project::new();
        let anchor = project.set_source_text(
            SourceKey::from_virtual("hmi-declaration-index"),
            String::new(),
        );
        let mut source_paths = std::collections::HashMap::new();
        let source_texts: std::collections::HashMap<_, _> = sources
            .iter()
            .map(|source| {
                let id = project
                    .set_source_text(SourceKey::from_path(&source.path), source.text.clone());
                source_paths.insert(id, source.path.clone());
                (id, source.text.clone())
            })
            .collect();
        let allowed = source_texts.keys().copied().collect();
        // Collect declarations once. Unlike analyze/build_runtime this does not check/lower
        // PROGRAM statements, materialize instances, initialize I/O or execute user methods.
        let symbols = project
            .database()
            .file_symbols_with_project_filtered(anchor, &allowed);
        let qualified: std::collections::HashMap<_, _> = symbols
            .declaration_catalog(anchor)
            .entries()
            .iter()
            .map(|entry| (entry.symbol_id(), entry.qualified_name().display()))
            .collect();
        let names = symbols
            .declaration_catalog(anchor)
            .entries()
            .iter()
            .map(|entry| {
                (
                    entry.qualified_name().display().to_ascii_uppercase(),
                    entry.symbol_id(),
                )
            })
            .collect();
        let mut result = Self {
            source_paths,
            source_texts,
            task_intervals: super::tasks::task_intervals(sources),
            registry: symbols.declaration_types(),
            symbols,
            names,
            qualified,
            variables: Vec::new(),
        };
        let texts: Vec<_> = sources.iter().map(|s| s.text.as_str()).collect();
        let configured = super::super::control_links::program_types(&texts);
        let mut roots = Vec::new();
        for symbol in result.symbols.iter() {
            if matches!(symbol.kind, SymbolKind::Program) {
                let ty = result.name(symbol.id);
                let configured_names: Vec<_> = configured
                    .iter()
                    .filter(|(_, kind)| kind.eq_ignore_ascii_case(&ty))
                    .collect();
                if configured_names.is_empty() {
                    roots.push((symbol.name.to_string(), ty, symbol.id));
                } else {
                    roots.extend(
                        configured_names
                            .into_iter()
                            .map(|(name, _)| (name.clone(), ty.clone(), symbol.id)),
                    );
                }
            }
        }
        roots.sort_by(|a, b| a.0.cmp(&b.0));
        for (name, ty, owner) in roots {
            let children = result.children(owner, &name, false, 0);
            result.variables.push(VariableNode {
                path: name.clone(),
                name,
                type_name: ty,
                kind: VariableKind::Program,
                kinds: vec![],
                writable: false,
                write_block: None,
                access: None,
                children,
                array: None,
            });
        }
        let globals: Vec<_> = result
            .symbols
            .iter()
            .filter(|symbol| {
                matches!(
                    symbol.kind,
                    SymbolKind::Variable {
                        qualifier: VarQualifier::Global
                    } | SymbolKind::Constant
                ) && symbol.parent.is_none_or(|parent| {
                    result
                        .symbols
                        .get(parent)
                        .is_some_and(|owner| matches!(owner.kind, SymbolKind::Namespace))
                })
            })
            .map(|symbol| {
                (
                    result.name(symbol.id),
                    symbol.type_id,
                    symbol.is_constant || matches!(symbol.kind, SymbolKind::Constant),
                )
            })
            .collect();
        for (name, ty, constant) in globals {
            result
                .variables
                .push(result.node(&name, &name, ty, constant, 0));
        }
        result
    }

    pub fn resolve(&self, path: &str) -> Option<&VariableNode> {
        resolve_nodes(&self.variables, path)
    }
    pub fn symbol(&self, name: &str) -> Option<&Symbol> {
        self.names
            .get(&name.to_ascii_uppercase())
            .copied()
            .or_else(|| self.symbols.lookup(name))
            .and_then(|id| self.symbols.get(id))
    }
    fn equipment_symbol(&self, name: &str) -> Option<&Symbol> {
        let ty = self.registry.lookup(name)?;
        match self.registry.get(self.symbols.resolve_alias_type(ty))? {
            Type::FunctionBlock { name } | Type::Class { name } | Type::Interface { name } => {
                self.symbol(name)
            }
            _ => None,
        }
    }
    pub(in crate::handlers::commands) fn name(&self, id: SymbolId) -> String {
        self.qualified
            .get(&id)
            .cloned()
            .or_else(|| self.symbols.get(id).map(|symbol| symbol.name.to_string()))
            .unwrap_or_default()
    }
    fn children(
        &self,
        owner: SymbolId,
        path: &str,
        blocked: bool,
        depth: usize,
    ) -> Vec<VariableNode> {
        if depth > 12 {
            return vec![];
        }
        let mut members: Vec<_> = self
            .symbols
            .iter()
            .filter(|s| s.parent == Some(owner))
            .collect();
        members.sort_by_key(|s| (s.range.start(), s.id.0));
        let in_program = self
            .symbols
            .get(owner)
            .is_some_and(|symbol| matches!(symbol.kind, SymbolKind::Program));
        let mut found: Vec<_> = members
            .into_iter()
            .filter_map(|member| {
                let access = match member.kind {
                    SymbolKind::Variable {
                        qualifier: VarQualifier::Temp,
                    } => return None,
                    SymbolKind::Variable { qualifier } => Some(match qualifier {
                        VarQualifier::Input => VariableAccess::Input,
                        VarQualifier::Output => VariableAccess::Output,
                        VarQualifier::InOut => VariableAccess::InOut,
                        _ => VariableAccess::Internal,
                    }),
                    SymbolKind::Parameter { direction } => Some(match direction {
                        trust_hir::symbols::ParamDirection::In => VariableAccess::Input,
                        trust_hir::symbols::ParamDirection::Out => VariableAccess::Output,
                        trust_hir::symbols::ParamDirection::InOut => VariableAccess::InOut,
                    }),
                    SymbolKind::Constant => None,
                    SymbolKind::Property { has_get: true, .. }
                        if member.visibility == Visibility::Public =>
                    {
                        None
                    }
                    _ => return None,
                };
                if matches!(
                    member.visibility,
                    Visibility::Private | Visibility::Protected
                ) {
                    return None;
                }
                let readonly = blocked
                    || member.is_constant
                    || (!in_program
                        && matches!(
                            access,
                            Some(VariableAccess::Output | VariableAccess::Internal)
                        ))
                    || matches!(member.kind, SymbolKind::Property { .. });
                let mut node = self.node(
                    &member.name,
                    &format!("{path}.{}", member.name),
                    member.type_id,
                    readonly,
                    depth + 1,
                );
                node.access = access;
                Some(node)
            })
            .collect();
        if let Some(reference) = self.symbols.extends_reference(owner)
            && let trust_hir::semantic::SemanticOutcome::Resolved(base) = reference.outcome()
        {
            for member in self.children(*base, path, blocked, depth + 1) {
                if !found
                    .iter()
                    .any(|node| node.name.eq_ignore_ascii_case(&member.name))
                {
                    found.push(member);
                }
            }
        }
        found
    }
    pub(in crate::handlers::commands) fn node(
        &self,
        name: &str,
        path: &str,
        ty: TypeId,
        blocked: bool,
        depth: usize,
    ) -> VariableNode {
        let resolved = self.symbols.resolve_alias_type(ty);
        let typename = self
            .symbols
            .type_name(ty)
            .map(|s| s.to_string())
            .unwrap_or_else(|| "Unknown".into());
        let mut node = VariableNode {
            name: name.into(),
            path: path.into(),
            type_name: typename,
            kind: VariableKind::Value,
            kinds: vec![],
            writable: !blocked,
            write_block: blocked.then(|| "Read-only declaration".into()),
            access: None,
            children: vec![],
            array: None,
        };
        if depth > 12 {
            node.writable = false;
            return node;
        }
        match self.symbols.type_by_id(resolved) {
            Some(Type::FunctionBlock { name } | Type::Class { name }) => {
                node.kind = VariableKind::FunctionBlock;
                node.writable = false;
                if let Some(owner) = self.symbol(name) {
                    node.children = self.children(owner.id, path, blocked, depth);
                }
            }
            Some(Type::Struct { fields, .. }) => {
                node.kind = VariableKind::Structure;
                node.kinds = vec![ValueKind::Structure];
                node.children = fields
                    .iter()
                    .map(|field| {
                        self.node(
                            &field.name,
                            &format!("{path}.{}", field.name),
                            field.type_id,
                            blocked,
                            depth + 1,
                        )
                    })
                    .collect();
            }
            Some(Type::Array {
                element,
                dimensions,
            }) => {
                node.kind = VariableKind::Array;
                node.kinds = vec![ValueKind::Array];
                let indices = dimensions
                    .iter()
                    .map(|(low, _)| low.to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                node.array = Some(ArrayShape {
                    dimensions: dimensions.iter().map(|(a, b)| [*a, *b]).collect(),
                    element: Box::new(self.node(
                        name,
                        &format!("{path}[{indices}]"),
                        *element,
                        blocked,
                        depth + 1,
                    )),
                });
            }
            Some(Type::Bool) => node.kinds = vec![ValueKind::Boolean, ValueKind::Value],
            Some(Type::Enum { .. }) => node.kinds = vec![ValueKind::Enumeration, ValueKind::Value],
            Some(Type::String { .. } | Type::WString { .. } | Type::Char | Type::WChar) => {
                node.kinds = vec![ValueKind::Text, ValueKind::Value]
            }
            Some(
                Type::Time
                | Type::LTime
                | Type::Date
                | Type::LDate
                | Type::Tod
                | Type::LTod
                | Type::Dt
                | Type::Ldt,
            ) => node.kinds = vec![ValueKind::Time, ValueKind::Value],
            Some(Type::Byte | Type::Word | Type::DWord | Type::LWord) => {
                node.kinds = vec![ValueKind::Number, ValueKind::Value]
            }
            Some(ty) if ty.is_numeric() => node.kinds = vec![ValueKind::Number, ValueKind::Value],
            _ => node.writable = false,
        }
        node
    }
}

/// What the language server asks of a block's declarations for equipment (specification 39,
/// §10, §21.3): whether it implements an interface, and its members with their types.
impl Declarations {
    pub(in crate::handlers::commands) fn implements_capability(
        &self,
        block: &str,
        interface: &str,
    ) -> bool {
        use trust_hir::semantic::SemanticOutcome;
        let Some(owner) = self.equipment_symbol(block) else {
            return false;
        };
        let mut pending = vec![owner.id];
        let mut visited = std::collections::HashSet::new();
        while let Some(id) = pending.pop() {
            if !visited.insert(id) {
                continue;
            }
            if self.name(id).eq_ignore_ascii_case(interface) {
                return true;
            }
            for reference in self
                .symbols
                .implements_references(id)
                .into_iter()
                .chain(self.symbols.extends_reference(id))
            {
                if let SemanticOutcome::Resolved(target) = reference.outcome() {
                    pending.push(*target);
                }
            }
        }
        false
    }
    /// The block a function block extends, with its namespace.
    pub(in crate::handlers::commands) fn base_of(&self, block: &str) -> Option<String> {
        use trust_hir::semantic::SemanticOutcome;
        let owner = self.equipment_symbol(block)?;
        match self.symbols.extends_reference(owner.id)?.outcome() {
            SemanticOutcome::Resolved(target) => Some(self.name(*target)),
            _ => None,
        }
    }
    pub(in crate::handlers::commands) fn block_members(
        &self,
        block: &str,
    ) -> Option<Vec<(String, TypeId)>> {
        use trust_hir::semantic::SemanticOutcome;
        let mut owner = Some(self.equipment_symbol(block)?.id);
        let mut visited = std::collections::HashSet::new();
        let mut names = std::collections::BTreeSet::new();
        let mut result = Vec::new();
        while let Some(id) = owner {
            if !visited.insert(id) {
                return None;
            }
            for member in self.symbols.iter().filter(|s| s.parent == Some(id)) {
                if matches!(
                    member.kind,
                    SymbolKind::Variable { .. }
                        | SymbolKind::Parameter { .. }
                        | SymbolKind::Property { .. }
                ) && names.insert(member.name.to_ascii_uppercase())
                {
                    result.push((member.name.to_string(), member.type_id));
                }
            }
            owner = self.symbols.extends_reference(id).and_then(|reference| {
                match reference.outcome() {
                    SemanticOutcome::Resolved(target) => Some(*target),
                    _ => None,
                }
            });
        }
        Some(result)
    }
}

/// The pair check reads blocks through the same declarations (specification 39, §8.6).
impl trust_runtime::hmi::binding_checks::PairProgram for Declarations {
    fn block_members(&self, name: &str) -> Option<Vec<(String, TypeId)>> {
        Declarations::block_members(self, name)
    }
    fn type_of(&self, id: TypeId) -> Option<&Type> {
        self.registry.get(id)
    }
}

pub(in crate::handlers::commands) fn resolve_nodes<'a>(
    variables: &'a [VariableNode],
    path: &str,
) -> Option<&'a VariableNode> {
    use trust_runtime::hmi::member_paths::{MemberPath, PathSegment};
    MemberPath::parse(path).ok()?;
    let mut current = variables
        .iter()
        .filter(|node| {
            path.get(..node.path.len())
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case(&node.path))
                && (path.len() == node.path.len()
                    || matches!(path.as_bytes().get(node.path.len()), Some(b'.' | b'[')))
        })
        .max_by_key(|node| node.path.len())?;
    let parsed = MemberPath::parse(&format!("Root{}", &path[current.path.len()..])).ok()?;
    for step in parsed.segments {
        current = match step {
            PathSegment::Field(name) => current
                .children
                .iter()
                .find(|node| node.name.eq_ignore_ascii_case(&name))?,
            PathSegment::Index(indices) => {
                let array = current.array.as_ref()?;
                if indices.len() != array.dimensions.len()
                    || indices
                        .iter()
                        .zip(&array.dimensions)
                        .any(|(index, [low, high])| index < low || index > high)
                {
                    return None;
                }
                &array.element
            }
        };
    }
    Some(current)
}

#[cfg(test)]
mod tests {
    use super::super::super::LoadedSource;
    use super::*;
    fn read(text: &str) -> Arc<Declarations> {
        Declarations::read(&[LoadedSource {
            path: "/declarations/main.st".into(),
            text: text.into(),
        }])
    }
    #[test]
    fn declaration_discovery_does_not_require_executable_program_statements() {
        let index=read("FUNCTION_BLOCK Pump\nVAR_OUTPUT Running:BOOL; END_VAR\nEND_FUNCTION_BLOCK\nPROGRAM Main\nVAR P101:Pump; Level:REAL; END_VAR\nMissingFunction();\nEND_PROGRAM");
        assert_eq!(index.resolve("Main.P101").unwrap().type_name, "Pump");
        assert_eq!(
            index.resolve("Main.P101.Running").unwrap().kinds,
            vec![ValueKind::Boolean, ValueKind::Value]
        );
        assert!(!index.resolve("Main.P101.Running").unwrap().writable);
        assert!(index.resolve("Main.Level").unwrap().writable);
    }
    #[test]
    fn configured_instances_arrays_and_inherited_capabilities_use_hir_declarations() {
        let index=read("INTERFACE IRun\nMETHOD Start END_METHOD\nEND_INTERFACE\nFUNCTION_BLOCK Base IMPLEMENTS IRun\nVAR_OUTPUT Running:BOOL; END_VAR\nMETHOD PUBLIC Start END_METHOD\nEND_FUNCTION_BLOCK\nFUNCTION_BLOCK Pump EXTENDS Base\nEND_FUNCTION_BLOCK\nPROGRAM Control\nVAR Pumps:ARRAY[2..4] OF Pump; END_VAR\nEND_PROGRAM\nCONFIGURATION C\nRESOURCE R ON PLC\nTASK Fast(INTERVAL:=T#20ms,PRIORITY:=1);\nPROGRAM Main WITH Fast:Control;\nEND_RESOURCE\nEND_CONFIGURATION");
        assert!(index.implements_capability("Pump", "IRun"));
        assert!(index.resolve("Main.Pumps[3].Running").is_some());
        assert!(index.resolve("Main.Pumps[1].Running").is_none());
        assert_eq!(index.task_intervals, vec![("Main".into(), "T#20ms".into())]);
    }
    #[test]
    fn generation_metadata_includes_inherited_input_output_and_inout_parameters() {
        let index = read("FUNCTION_BLOCK Base VAR_INPUT Id:ULINT; CycleTime:TIME; END_VAR VAR_OUTPUT Ready:BOOL; END_VAR VAR_IN_OUT Feedback:BOOL; END_VAR END_FUNCTION_BLOCK FUNCTION_BLOCK Device EXTENDS Base VAR InternalCount:UINT; END_VAR END_FUNCTION_BLOCK");
        let members = index.block_members("Device").unwrap();
        for (name, ty) in [
            ("Id", TypeId::ULINT),
            ("CycleTime", TypeId::TIME),
            ("Ready", TypeId::BOOL),
            ("Feedback", TypeId::BOOL),
            ("InternalCount", TypeId::UINT),
        ] {
            assert!(
                members
                    .iter()
                    .any(|(member, actual)| member == name && *actual == ty),
                "missing {name}: {members:?}"
            );
        }
    }
    #[test]
    fn source_edit_invalidates_declared_type_snapshot_without_losing_other_files() {
        let a = read("PROGRAM Main VAR Value:BOOL; END_VAR END_PROGRAM");
        let b = read("PROGRAM Main VAR Value:REAL; END_VAR END_PROGRAM");
        assert_eq!(a.resolve("Main.Value").unwrap().type_name, "BOOL");
        assert_eq!(b.resolve("Main.Value").unwrap().type_name, "REAL");
    }
}
