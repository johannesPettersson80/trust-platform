pub(super) use super::helpers::*;
use super::*;

mod collect;
mod const_eval;
mod const_utils;
mod precollect;
mod types;
mod validation;
mod variables;

pub(super) struct SymbolCollector<'a> {
    table: SymbolTable,
    diagnostics: DiagnosticBuilder,
    pending_types: Vec<PendingType>,
    /// Parent symbol stack for nested declarations.
    parent_stack: Vec<SymbolId>,
    const_exprs: FxHashMap<(Option<SmolStr>, SmolStr), SyntaxNode>,
    const_values: FxHashMap<(Option<SmolStr>, SmolStr), i64>,
    program_instances: FxHashMap<SmolStr, SymbolId>,
    project_types: Option<&'a dyn ProjectTypeProvider>,
    importing_project_types: FxHashSet<SmolStr>,
    namespace_override: Option<Vec<SmolStr>>,
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
            namespace_override: None,
        }
    }

    pub(super) fn collect(mut self, root: &SyntaxNode) -> (SymbolTable, Vec<Diagnostic>) {
        self.phase_precollect(root);
        self.phase_collect_symbols(root);
        self.phase_access_and_config(root);
        self.phase_resolve_types();
        self.phase_global_links(root);
        self.phase_var_validation(root);
        self.phase_constants();
        (self.table, self.diagnostics.finish())
    }

    pub(crate) fn collect_for_project_with_const_roots(
        mut self,
        root: &SyntaxNode,
        const_roots: &[SyntaxNode],
    ) -> (SymbolTable, Vec<Diagnostic>, Vec<PendingType>) {
        for project_root in const_roots {
            self.precollect_constants(project_root, None);
        }
        self.phase_precollect(root);
        self.phase_collect_symbols(root);
        self.phase_access_and_config(root);
        self.phase_var_validation(root);
        self.phase_constants();
        let pending_types = std::mem::take(&mut self.pending_types);
        (self.table, self.diagnostics.finish(), pending_types)
    }

    fn phase_precollect(&mut self, root: &SyntaxNode) {
        self.precollect_pous(root, &[]);
        self.precollect_types(root, &[]);
        self.precollect_constants(root, None);
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
        self.check_var_block_modifiers(root);
        self.check_at_bindings(root);
    }

    fn phase_constants(&mut self) {
        self.evaluate_constants();
        self.table
            .set_const_values(std::mem::take(&mut self.const_values));
    }
}
