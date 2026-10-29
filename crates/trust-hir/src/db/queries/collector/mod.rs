pub(super) use super::helpers::*;
use super::*;

mod collect;
mod const_eval;
mod const_utils;
mod precollect;
mod storage;
mod types;
mod validation;
mod variable_initializers;
mod variables;

pub(super) type ConstantExpressions = FxHashMap<(Option<SmolStr>, SmolStr), SyntaxNode>;

/// An instance initialiser of a function block that another file of the project declares, or
/// that extends one: a file's own table holds only a carrier type for such a block, so its members
/// are checked once the project's symbols are merged. A derived block's initialiser may set the
/// variables it inherits (IEC 61131-3 Ed.3 §6.5.1 for initial values; §6.6.5.5.2 rule 2 with
/// §6.6.7.2.7 for inherited variables).
pub(super) struct DeferredInitializer {
    type_id: TypeId,
    list: SyntaxNode,
}

/// The `ConfigInit` nodes (VAR_CONFIG entries) of `roots`, in root and document order.
pub(crate) fn config_inits_of(roots: &[SyntaxNode]) -> Vec<SyntaxNode> {
    roots
        .iter()
        .flat_map(|root| {
            root.descendants()
                .filter(|n| n.kind() == SyntaxKind::ConfigInit)
        })
        .collect()
}

pub(super) struct SymbolCollector<'a> {
    table: SymbolTable,
    diagnostics: DiagnosticBuilder,
    pending_types: Vec<PendingType>,
    /// Parent symbol stack for nested declarations.
    parent_stack: Vec<SymbolId>,
    const_exprs: FxHashMap<(Option<SmolStr>, SmolStr), SyntaxNode>,
    const_values: FxHashMap<(Option<SmolStr>, SmolStr), i64>,
    /// In a project's analysis, the constants this file declares, each with the range of its
    /// first initializer here: a project constant reports its failure only in the file that
    /// declares it, never under another file's path. `None` reports every failure.
    own_constants: Option<FxHashMap<(Option<SmolStr>, SmolStr), TextRange>>,
    /// Under the CODESYS profile, the initializers of plain (non-constant) variables, keyed like
    /// `const_exprs`: only a declaration size (a string length, an array bound) reads them
    /// (docs/IEC_DEVIATIONS.md DEV-033); no other constant context sees them.
    declared_exprs: FxHashMap<(Option<SmolStr>, SmolStr), SyntaxNode>,
    declared_values: FxHashMap<(Option<SmolStr>, SmolStr), i64>,
    program_instances: ProgramInstanceMap,
    project_types: Option<&'a dyn ProjectTypeProvider>,
    importing_project_types: FxHashSet<SmolStr>,
    diagnosed_project_type_import_failures: FxHashSet<SmolStr>,
    namespace_override: Option<Vec<SmolStr>>,
    /// Initialisers left to the check after the merge; `None` checks them at once, as a collector
    /// over the merged project symbols does.
    deferred_initializers: Option<Vec<DeferredInitializer>>,
}

impl<'a> SymbolCollector<'a> {
    pub(super) fn with_project_types(project_types: &'a dyn ProjectTypeProvider) -> Self {
        Self::build(Some(project_types))
    }

    /// The collector for a project analyzed under `dialect`; its table carries it to the checks.
    pub(super) fn with_dialect(mut self, dialect: Dialect) -> Self {
        self.table.set_dialect(dialect);
        self
    }

    fn build(project_types: Option<&'a dyn ProjectTypeProvider>) -> Self {
        Self {
            table: SymbolTable::new(),
            diagnostics: DiagnosticBuilder::new(),
            pending_types: Vec::new(),
            parent_stack: Vec::new(),
            const_exprs: FxHashMap::default(),
            const_values: FxHashMap::default(),
            own_constants: None,
            declared_exprs: FxHashMap::default(),
            declared_values: FxHashMap::default(),
            program_instances: FxHashMap::default(),
            project_types,
            importing_project_types: FxHashSet::default(),
            diagnosed_project_type_import_failures: FxHashSet::default(),
            namespace_override: None,
            deferred_initializers: project_types.map(|_| Vec::new()),
        }
    }

    pub(super) fn collect(mut self, root: &SyntaxNode) -> (SymbolTable, Vec<Diagnostic>) {
        self.phase_precollect(root);
        self.phase_collect_symbols(root);
        self.phase_access_and_config(root);
        self.phase_resolve_types();
        self.check_variable_initializer_constant_expressions(root);
        self.phase_global_links(root);
        self.phase_var_validation(root);
        self.phase_constants();
        (self.table, self.diagnostics.finish())
    }

    /// The constants of `root`, and the initializers of its plain variables that a declaration
    /// size may read under the CODESYS profile (docs/IEC_DEVIATIONS.md DEV-033), each by its scope
    /// and name.
    pub(super) fn constant_expressions(
        root: &SyntaxNode,
    ) -> (ConstantExpressions, ConstantExpressions) {
        let mut collector = Self::build(None).with_dialect(Dialect::Codesys);
        collector.precollect_constants(root, &[], &[]);
        (collector.const_exprs, collector.declared_exprs)
    }

    /// Under the CODESYS profile a declaration size reads the initial value of a plain variable of
    /// any file of the project, `declared` (docs/IEC_DEVIATIONS.md DEV-033); the IEC rules read
    /// none.
    fn use_project_declared(&mut self, declared: &ConstantExpressions) {
        if self.table.dialect().is_codesys() {
            self.declared_exprs = declared.clone();
        }
    }

    pub(crate) fn collect_for_project_with_constants(
        mut self,
        root: &SyntaxNode,
        constants: &ConstantExpressions,
        declared: &ConstantExpressions,
    ) -> (
        SymbolTable,
        Vec<Diagnostic>,
        Vec<PendingType>,
        Vec<DeferredInitializer>,
    ) {
        self.const_exprs = constants.clone();
        self.use_project_declared(declared);
        self.own_constants = Some(FxHashMap::default());
        self.phase_precollect(root);
        self.phase_collect_symbols(root);
        self.phase_constants();
        let pending_types = std::mem::take(&mut self.pending_types);
        let deferred = self.deferred_initializers.take().unwrap_or_default();
        (
            self.table,
            self.diagnostics.finish(),
            pending_types,
            deferred,
        )
    }

    pub(crate) fn collect_with_project_constants(
        mut self,
        root: &SyntaxNode,
        constants: &ConstantExpressions,
        declared: &ConstantExpressions,
    ) -> (SymbolTable, Vec<Diagnostic>) {
        self.const_exprs = constants.clone();
        self.use_project_declared(declared);
        self.collect(root)
    }

    /// `config_inits`: the project's `ConfigInit` nodes in project order (`config_inits_of`
    /// over the project's configuration roots); `deferred`: the initialisers that
    /// `collect_for_project_with_constants` left to the check after the merge.
    pub(crate) fn validate_project_after_merge(
        table: SymbolTable,
        root: &SyntaxNode,
        config_inits: &[SyntaxNode],
        deferred: Vec<DeferredInitializer>,
    ) -> (SymbolTable, Vec<Diagnostic>) {
        let mut collector = Self::build(None);
        collector.table = table;
        for initializer in deferred {
            collector.check_aggregate_initializer_fields(initializer.type_id, &initializer.list);
        }
        collector.check_variable_initializer_constant_expressions(root);
        collector.phase_access_and_config(root);
        collector.phase_var_validation_with_config_inits(root, config_inits);
        (collector.table, collector.diagnostics.finish())
    }

    fn phase_precollect(&mut self, root: &SyntaxNode) {
        self.precollect_pous(root, &[]);
        self.precollect_types(root, &[]);
        self.precollect_constants(root, &[], &[]);
    }

    fn phase_collect_symbols(&mut self, root: &SyntaxNode) {
        self.visit_node(root);
    }

    fn phase_access_and_config(&mut self, root: &SyntaxNode) {
        self.check_access_and_config(root);
    }

    fn phase_resolve_types(&mut self) {
        self.resolve_pending_types();
    }

    fn phase_global_links(&mut self, root: &SyntaxNode) {
        self.check_global_external_links(root);
    }

    fn phase_var_validation(&mut self, root: &SyntaxNode) {
        self.phase_var_validation_with_config_inits(
            root,
            &config_inits_of(std::slice::from_ref(root)),
        );
    }

    fn phase_var_validation_with_config_inits(
        &mut self,
        root: &SyntaxNode,
        config_inits: &[SyntaxNode],
    ) {
        self.check_var_block_modifiers(root);
        self.check_edge_declarations(root);
        self.check_source_generic_types(root);
        self.check_member_access_declarations(root);
        self.check_overlap_variable_initializers(root);
        self.check_by_value_type_cycles();
        self.check_at_bindings(config_inits);
    }

    fn phase_constants(&mut self) {
        self.evaluate_constants();
        self.table
            .set_const_values(std::mem::take(&mut self.const_values));
    }
}
