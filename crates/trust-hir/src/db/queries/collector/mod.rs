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

pub(super) struct SymbolCollector<'a> {
    table: SymbolTable,
    temporal_storage: TemporalStorage,
    diagnostics: DiagnosticBuilder,
    pending_types: Vec<PendingType>,
    /// Parent symbol stack for nested declarations.
    parent_stack: Vec<SymbolId>,
    const_exprs: FxHashMap<(Option<SmolStr>, SmolStr), (FileId, SyntaxNode)>,
    const_values: FxHashMap<(Option<SmolStr>, SmolStr), i64>,
    /// When set (project analysis), constant-eval errors are only reported
    /// for entries collected from this file — a stub's failures must not
    /// surface under another file's path.
    const_origin_filter: Option<FileId>,
    program_instances: ProgramInstanceMap,
    project_types: Option<&'a dyn ProjectTypeProvider>,
    importing_project_types: FxHashSet<SmolStr>,
    diagnosed_project_type_import_failures: FxHashSet<SmolStr>,
    namespace_override: Option<Vec<SmolStr>>,
}

impl<'a> SymbolCollector<'a> {
    pub(super) fn with_project_types(project_types: &'a dyn ProjectTypeProvider) -> Self {
        Self::build(Some(project_types))
    }

    pub(super) fn with_temporal_storage(mut self, storage: TemporalStorage) -> Self {
        self.temporal_storage = storage;
        self
    }

    fn build(project_types: Option<&'a dyn ProjectTypeProvider>) -> Self {
        Self {
            table: SymbolTable::new(),
            temporal_storage: TemporalStorage::default(),
            diagnostics: DiagnosticBuilder::new(),
            pending_types: Vec::new(),
            parent_stack: Vec::new(),
            const_exprs: FxHashMap::default(),
            const_values: FxHashMap::default(),
            const_origin_filter: None,
            program_instances: FxHashMap::default(),
            project_types,
            importing_project_types: FxHashSet::default(),
            diagnosed_project_type_import_failures: FxHashSet::default(),
            namespace_override: None,
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

    pub(crate) fn collect_for_project_with_const_roots(
        mut self,
        root: &SyntaxNode,
        origin_file_id: FileId,
        const_roots: &[(FileId, SyntaxNode)],
    ) -> (SymbolTable, Vec<Diagnostic>, Vec<PendingType>) {
        self.const_origin_filter = Some(origin_file_id);
        for (origin, project_root) in const_roots {
            self.precollect_constants(project_root, &[], &[], *origin);
        }
        self.phase_precollect(root);
        self.phase_collect_symbols(root);
        self.phase_constants();
        let pending_types = std::mem::take(&mut self.pending_types);
        (self.table, self.diagnostics.finish(), pending_types)
    }

    pub(crate) fn collect_with_project_const_roots(
        mut self,
        root: &SyntaxNode,
        origin_file_id: FileId,
        const_roots: &[(FileId, SyntaxNode)],
    ) -> (SymbolTable, Vec<Diagnostic>) {
        self.const_origin_filter = Some(origin_file_id);
        for (origin, project_root) in const_roots {
            self.precollect_constants(project_root, &[], &[], *origin);
        }
        self.collect(root)
    }

    pub(crate) fn validate_project_after_merge(
        table: SymbolTable,
        root: &SyntaxNode,
        project_roots: &[SyntaxNode],
        temporal_storage: TemporalStorage,
    ) -> (SymbolTable, Vec<Diagnostic>) {
        let mut collector = Self::build(None).with_temporal_storage(temporal_storage);
        collector.table = table;
        collector.check_variable_initializer_constant_expressions(root);
        collector.phase_access_and_config(root);
        collector.phase_var_validation_with_config_roots(root, project_roots);
        (collector.table, collector.diagnostics.finish())
    }

    fn phase_precollect(&mut self, root: &SyntaxNode) {
        self.precollect_pous(root, &[]);
        self.precollect_types(root, &[]);
        self.precollect_constants(
            root,
            &[],
            &[],
            self.const_origin_filter.unwrap_or(FileId(0)),
        );
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
