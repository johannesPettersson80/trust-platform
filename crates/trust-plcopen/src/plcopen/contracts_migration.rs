#[derive(Debug, Clone, Serialize)]
pub struct PlcopenMigrationReport {
    pub profile: String,
    pub namespace: String,
    pub source_xml: PathBuf,
    pub project_root: PathBuf,
    pub detected_ecosystem: String,
    pub discovered_pous: usize,
    pub importable_pous: usize,
    pub imported_pous: usize,
    pub skipped_pous: usize,
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
    pub source_coverage_percent: f64,
    pub semantic_loss_percent: f64,
    pub compatibility_coverage: PlcopenCompatibilityCoverage,
    pub unsupported_nodes: Vec<String>,
    pub unsupported_diagnostics: Vec<PlcopenUnsupportedDiagnostic>,
    pub applied_library_shims: Vec<PlcopenLibraryShimApplication>,
    pub warnings: Vec<String>,
    pub entries: Vec<PlcopenMigrationEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlcopenMigrationEntry {
    pub name: String,
    pub pou_type_raw: Option<String>,
    pub resolved_pou_type: Option<String>,
    pub status: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlcopenCompatibilityCoverage {
    pub supported_items: usize,
    pub partial_items: usize,
    pub unsupported_items: usize,
    pub support_percent: f64,
    pub verdict: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlcopenUnsupportedDiagnostic {
    pub code: String,
    pub severity: String,
    pub node: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pou: Option<String>,
    pub action: String,
}
