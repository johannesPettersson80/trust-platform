pub(super) use super::helpers::*;
use super::*;

mod collect;
mod const_eval;
mod const_utils;
mod precollect;
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

pub(super) struct SymbolCollector<'a> {
    table: SymbolTable,
    diagnostics: DiagnosticBuilder,
    pending_types: Vec<PendingType>,
    /// Parent symbol stack for nested declarations.
    parent_stack: Vec<SymbolId>,
    const_exprs: FxHashMap<(Option<SmolStr>, SmolStr), SyntaxNode>,
    const_values: FxHashMap<(Option<SmolStr>, SmolStr), i64>,
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

    fn build(project_types: Option<&'a dyn ProjectTypeProvider>) -> Self {
        Self {
            table: SymbolTable::new(),
            diagnostics: DiagnosticBuilder::new(),
            pending_types: Vec::new(),
            parent_stack: Vec::new(),
            const_exprs: FxHashMap::default(),
            const_values: FxHashMap::default(),
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

    pub(super) fn constant_expressions(root: &SyntaxNode) -> ConstantExpressions {
        let mut collector = Self::build(None);
        collector.precollect_constants(root, &[], &[]);
        collector.const_exprs
    }

    pub(crate) fn collect_for_project_with_constants(
        mut self,
        root: &SyntaxNode,
        constants: &ConstantExpressions,
    ) -> (
        SymbolTable,
        Vec<Diagnostic>,
        Vec<PendingType>,
        Vec<DeferredInitializer>,
    ) {
        self.const_exprs = constants.clone();
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
    ) -> (SymbolTable, Vec<Diagnostic>) {
        self.const_exprs = constants.clone();
        self.collect(root)
    }

    pub(crate) fn validate_project_after_merge(
        table: SymbolTable,
        root: &SyntaxNode,
        project_roots: &[SyntaxNode],
        deferred: Vec<DeferredInitializer>,
    ) -> (SymbolTable, Vec<Diagnostic>) {
        let mut collector = Self::build(None);
        collector.table = table;
        for initializer in deferred {
            collector.check_aggregate_initializer_fields(initializer.type_id, &initializer.list);
        }
        collector.check_variable_initializer_constant_expressions(root);
        collector.phase_access_and_config(root);
        collector.phase_var_validation_with_config_roots(root, project_roots);
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
        self.phase_var_validation_with_config_roots(root, std::slice::from_ref(root));
    }

    fn phase_var_validation_with_config_roots(
        &mut self,
        root: &SyntaxNode,
        config_roots: &[SyntaxNode],
    ) {
        self.check_var_block_modifiers(root);
        self.check_edge_declarations(root);
        self.check_source_generic_types(root);
        self.check_member_access_declarations(root);
        self.check_overlap_variable_initializers(root);
        self.check_by_value_type_cycles();
        self.check_at_bindings(config_roots);
    }

    fn phase_constants(&mut self) {
        self.evaluate_constants();
        self.table
            .set_const_values(std::mem::take(&mut self.const_values));
    }
}
