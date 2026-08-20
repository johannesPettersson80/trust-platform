#[derive(Debug, Clone, Serialize)]
pub struct PlcopenExportReport {
    pub target: String,
    pub output_path: PathBuf,
    pub source_map_path: PathBuf,
    pub adapter_report_path: Option<PathBuf>,
    pub siemens_scl_bundle_dir: Option<PathBuf>,
    pub siemens_scl_files: Vec<PathBuf>,
    pub adapter_diagnostics: Vec<PlcopenExportAdapterDiagnostic>,
    pub adapter_manual_steps: Vec<String>,
    pub adapter_limitations: Vec<String>,
    pub pou_count: usize,
    pub data_type_count: usize,
    pub configuration_count: usize,
    pub resource_count: usize,
    pub task_count: usize,
    pub program_instance_count: usize,
    pub exported_global_var_lists: usize,
    pub exported_project_structure_nodes: usize,
    pub exported_folder_paths: usize,
    pub source_count: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PlcopenExportTarget {
    Generic,
    AllenBradley,
    Siemens,
    Schneider,
}

impl PlcopenExportTarget {
    pub fn id(self) -> &'static str {
        match self {
            Self::Generic => "generic-plcopen",
            Self::AllenBradley => "allen-bradley",
            Self::Siemens => "siemens-tia",
            Self::Schneider => "schneider-ecostruxure",
        }
    }

    pub fn file_suffix(self) -> &'static str {
        match self {
            Self::Generic => "generic",
            Self::AllenBradley => "ab",
            Self::Siemens => "siemens",
            Self::Schneider => "schneider",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Generic => "Generic PLCopen XML",
            Self::AllenBradley => "Allen-Bradley / Studio 5000",
            Self::Siemens => "Siemens TIA Portal",
            Self::Schneider => "Schneider EcoStruxure",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PlcopenExportAdapterDiagnostic {
    pub code: String,
    pub severity: String,
    pub message: String,
    pub action: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlcopenExportAdapterReport {
    pub target: String,
    pub target_label: String,
    pub source_xml: PathBuf,
    pub source_map_path: PathBuf,
    pub siemens_scl_bundle_dir: Option<PathBuf>,
    pub siemens_scl_files: Vec<PathBuf>,
    pub diagnostics: Vec<PlcopenExportAdapterDiagnostic>,
    pub manual_steps: Vec<String>,
    pub limitations: Vec<String>,
}
