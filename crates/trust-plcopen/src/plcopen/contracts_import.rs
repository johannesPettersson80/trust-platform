#[derive(Debug, Clone, Serialize)]
pub struct PlcopenImportReport {
    pub project_root: PathBuf,
    pub written_sources: Vec<PathBuf>,
    pub imported_pous: usize,
    pub discovered_pous: usize,
    pub imported_data_types: usize,
    pub discovered_configurations: usize,
    pub imported_configurations: usize,
    pub imported_resources: usize,
    pub imported_tasks: usize,
    pub imported_program_instances: usize,
    pub discovered_global_var_lists: usize,
    pub imported_global_var_lists: usize,
    pub imported_project_structure_nodes: usize,
    pub imported_folder_paths: usize,
    pub warnings: Vec<String>,
    pub unsupported_nodes: Vec<String>,
    pub preserved_vendor_extensions: Option<PathBuf>,
    pub migration_report_path: PathBuf,
    pub source_coverage_percent: f64,
    pub semantic_loss_percent: f64,
    pub detected_ecosystem: String,
    pub compatibility_coverage: PlcopenCompatibilityCoverage,
    pub unsupported_diagnostics: Vec<PlcopenUnsupportedDiagnostic>,
    pub applied_library_shims: Vec<PlcopenLibraryShimApplication>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PlcopenImportGlobalVarMode {
    #[default]
    NativeVendorParity,
    StrictIecAdapter,
}


#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PlcopenImportOptions {
    pub global_var_mode: PlcopenImportGlobalVarMode,
}
