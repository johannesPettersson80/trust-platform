const PLCOPEN_NAMESPACE: &str = "http://www.plcopen.org/xml/tc6_0200";
const PROFILE_NAME: &str = "trust-st-complete-v1";
const SOURCE_MAP_DATA_NAME: &str = "trust.sourceMap";
const VENDOR_EXT_DATA_NAME: &str = "trust.vendorExtensions";
const EXPORT_ADAPTER_DATA_NAME: &str = "trust.exportAdapter";
const CODESYS_APPLICATION_DATA_NAME: &str = "http://www.3s-software.com/plcopenxml/application";
const CODESYS_POU_DATA_NAME: &str = "http://www.3s-software.com/plcopenxml/pou";
const CODESYS_METHOD_DATA_NAME: &str = "http://www.3s-software.com/plcopenxml/method";
const CODESYS_PROJECTSTRUCTURE_DATA_NAME: &str =
    "http://www.3s-software.com/plcopenxml/projectstructure";
const CODESYS_INTERFACE_PLAINTEXT_DATA_NAME: &str =
    "http://www.3s-software.com/plcopenxml/interfaceasplaintext";
const CODESYS_OBJECT_ID_DATA_NAME: &str = "http://www.3s-software.com/plcopenxml/objectid";
const VENDOR_EXTENSION_HOOK_FILE: &str = "plcopen.vendor-extensions.xml";
const IMPORTED_VENDOR_EXTENSION_FILE: &str = "plcopen.vendor-extensions.imported.xml";
const MIGRATION_REPORT_FILE: &str = "interop/plcopen-migration-report.json";
const GENERATED_DATA_TYPES_SOURCE_PREFIX: &str = "plcopen_data_types";

const SIEMENS_LIBRARY_SHIMS: &[VendorLibraryShim] = &[
    VendorLibraryShim {
        source_symbol: "SFB3",
        replacement_symbol: "TP",
        notes: "Siemens pulse timer alias mapped to IEC TP.",
    },
    VendorLibraryShim {
        source_symbol: "SFB4",
        replacement_symbol: "TON",
        notes: "Siemens on-delay timer alias mapped to IEC TON.",
    },
    VendorLibraryShim {
        source_symbol: "SFB5",
        replacement_symbol: "TOF",
        notes: "Siemens off-delay timer alias mapped to IEC TOF.",
    },
];

const ROCKWELL_LIBRARY_SHIMS: &[VendorLibraryShim] = &[VendorLibraryShim {
    source_symbol: "TONR",
    replacement_symbol: "TON",
    notes:
        "Rockwell retentive timer alias mapped to IEC TON (review retentive semantics manually).",
}];

const SCHNEIDER_LIBRARY_SHIMS: &[VendorLibraryShim] = &[
    VendorLibraryShim {
        source_symbol: "R_EDGE",
        replacement_symbol: "R_TRIG",
        notes: "Schneider/CODESYS edge alias mapped to IEC R_TRIG.",
    },
    VendorLibraryShim {
        source_symbol: "F_EDGE",
        replacement_symbol: "F_TRIG",
        notes: "Schneider/CODESYS edge alias mapped to IEC F_TRIG.",
    },
];

const MITSUBISHI_LIBRARY_SHIMS: &[VendorLibraryShim] = &[
    VendorLibraryShim {
        source_symbol: "DIFU",
        replacement_symbol: "R_TRIG",
        notes: "Mitsubishi differential-up alias mapped to IEC R_TRIG.",
    },
    VendorLibraryShim {
        source_symbol: "DIFD",
        replacement_symbol: "F_TRIG",
        notes: "Mitsubishi differential-down alias mapped to IEC F_TRIG.",
    },
];

#[derive(Debug, Clone, Serialize)]
pub struct PlcopenProfile {
    pub namespace: &'static str,
    pub profile: &'static str,
    pub version: &'static str,
    pub strict_subset: Vec<&'static str>,
    pub unsupported_nodes: Vec<&'static str>,
    pub compatibility_matrix: Vec<PlcopenCompatibilityMatrixEntry>,
    pub source_mapping: &'static str,
    pub vendor_extension_hook: &'static str,
    pub round_trip_limits: Vec<&'static str>,
    pub known_gaps: Vec<&'static str>,
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlcopenImportGlobalVarMode {
    #[default]
    NativeVendorParity,
    StrictIecAdapter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PlcopenImportOptions {
    pub global_var_mode: PlcopenImportGlobalVarMode,
}

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
pub struct PlcopenCompatibilityMatrixEntry {
    pub capability: &'static str,
    pub status: &'static str,
    pub notes: &'static str,
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

#[derive(Debug, Clone, Serialize)]
pub struct PlcopenLibraryShimApplication {
    pub vendor: String,
    pub source_symbol: String,
    pub replacement_symbol: String,
    pub occurrences: usize,
    pub notes: String,
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

#[derive(Debug, Clone, Copy)]
struct VendorLibraryShim {
    source_symbol: &'static str,
    replacement_symbol: &'static str,
    notes: &'static str,
}

#[derive(Debug, Clone)]
struct LoadedSource {
    path: PathBuf,
    text: String,
}

#[derive(Debug, Clone)]
struct PouDecl {
    name: String,
    pou_type: PlcopenPouType,
    body: String,
    methods: Vec<CodesysMethodDecl>,
    source: String,
    line: usize,
}

#[derive(Debug, Clone)]
struct CodesysMethodDecl {
    owner_name: String,
    name: String,
    return_type: Option<String>,
    body: String,
    interface_plaintext: String,
    sections: Vec<CodesysMethodVarSection>,
    source: String,
    line: usize,
}

#[derive(Debug, Clone)]
struct CodesysMethodVarSection {
    xml_name: &'static str,
    constant: bool,
    retain: bool,
    nonretain: bool,
    persistent: bool,
    nonpersistent: bool,
    variables: Vec<InterfaceVariableDecl>,
}

#[derive(Debug, Clone)]
struct InterfaceVariableDecl {
    name: String,
    type_expr: String,
    initial_value: Option<String>,
}

#[derive(Debug, Clone)]
struct DataTypeDecl {
    name: String,
    type_expr: String,
    source: String,
    line: usize,
}

#[derive(Debug, Clone)]
struct GlobalVarDecl {
    name: String,
    body: String,
    source: String,
    source_path: PathBuf,
    line: usize,
    variables: Vec<GlobalVarVariableDecl>,
}

#[derive(Debug, Clone)]
struct GlobalVarVariableDecl {
    name: String,
    type_expr: String,
    initial_value: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct TaskDecl {
    name: String,
    interval: Option<String>,
    single: Option<String>,
    priority: Option<String>,
}

#[derive(Debug, Clone)]
struct ProgramBindingDecl {
    instance_name: String,
    task_name: Option<String>,
    type_name: String,
}

#[derive(Debug, Clone)]
struct ResourceDecl {
    name: String,
    target: String,
    tasks: Vec<TaskDecl>,
    programs: Vec<ProgramBindingDecl>,
}

#[derive(Debug, Clone)]
struct ConfigurationDecl {
    name: String,
    tasks: Vec<TaskDecl>,
    programs: Vec<ProgramBindingDecl>,
    resources: Vec<ResourceDecl>,
}

#[derive(Debug, Clone, Default)]
struct ImportProjectModelStats {
    discovered_configurations: usize,
    imported_configurations: usize,
    imported_resources: usize,
    imported_tasks: usize,
    imported_program_instances: usize,
    written_sources: Vec<PathBuf>,
}

#[derive(Debug, Clone, Default)]
struct ImportGlobalVarStats {
    discovered_global_var_lists: usize,
    imported_global_var_lists: usize,
    written_sources: Vec<PathBuf>,
    qualified_list_externals: Vec<QualifiedGlobalListExternalDecl>,
}

#[derive(Debug, Clone)]
struct QualifiedGlobalListExternalDecl {
    list_name: String,
    type_name: String,
}

#[derive(Debug, Clone, Default)]
struct CodesysProjectStructureMap {
    object_paths_by_id: BTreeMap<String, Vec<String>>,
    unique_object_paths_by_name: BTreeMap<String, Vec<String>>,
    object_count: usize,
}

#[derive(Debug, Clone)]
struct CodesysExportObjectEntry {
    name: String,
    object_id: String,
    folder_segments: Vec<String>,
    parent_segments: Vec<String>,
}

#[derive(Debug, Clone)]
struct CodesysProjectObjectNode {
    name: String,
    object_id: String,
    children: Vec<CodesysProjectObjectNode>,
}

#[derive(Debug, Clone)]
struct CodesysExportMetadata {
    global_var_lists: Vec<(GlobalVarDecl, CodesysExportObjectEntry)>,
    pou_entries: Vec<(PouDecl, CodesysExportObjectEntry)>,
    method_entries: Vec<(CodesysMethodDecl, CodesysExportObjectEntry)>,
    project_structure_root: CodesysProjectObjectNode,
    exported_project_structure_nodes: usize,
    exported_folder_paths: usize,
}

#[derive(Debug, Clone, Default)]
struct ExportSourceAnalysis {
    has_retain_keyword: bool,
    has_direct_address_markers: bool,
    has_siemens_aliases: bool,
    has_rockwell_aliases: bool,
    has_schneider_aliases: bool,
}

#[derive(Debug, Clone)]
struct ExportTargetValidationContext {
    pou_count: usize,
    data_type_count: usize,
    configuration_count: usize,
    resource_count: usize,
    task_count: usize,
    program_instance_count: usize,
    source_count: usize,
    analysis: ExportSourceAnalysis,
}

#[derive(Debug, Clone)]
struct PlcopenExportAdapterContract {
    diagnostics: Vec<PlcopenExportAdapterDiagnostic>,
    manual_steps: Vec<String>,
    limitations: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlcopenPouType {
    Program,
    Function,
    FunctionBlock,
}

impl PlcopenPouType {
    fn as_xml(self) -> &'static str {
        match self {
            Self::Program => "program",
            Self::Function => "function",
            Self::FunctionBlock => "functionBlock",
        }
    }

    fn declaration_keyword(self) -> &'static str {
        match self {
            Self::Program => "PROGRAM",
            Self::Function => "FUNCTION",
            Self::FunctionBlock => "FUNCTION_BLOCK",
        }
    }

    fn end_keyword(self) -> &'static str {
        match self {
            Self::Program => "END_PROGRAM",
            Self::Function => "END_FUNCTION",
            Self::FunctionBlock => "END_FUNCTION_BLOCK",
        }
    }

    fn from_xml(text: &str) -> Option<Self> {
        let normalized = text
            .chars()
            .filter(|ch| ch.is_ascii_alphanumeric())
            .map(|ch| ch.to_ascii_lowercase())
            .collect::<String>();
        match normalized.as_str() {
            "program" | "prg" => Some(Self::Program),
            "function" | "fc" | "fun" => Some(Self::Function),
            "functionblock" | "fb" => Some(Self::FunctionBlock),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SourceMapEntry {
    name: String,
    pou_type: String,
    source: String,
    line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SourceMapPayload {
    profile: String,
    namespace: String,
    entries: Vec<SourceMapEntry>,
}
