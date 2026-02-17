//! PLCopen XML interchange (ST-focused subset profile).

#![allow(missing_docs)]

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::{Deserialize, Serialize};
use trust_syntax::lexer::{lex, TokenKind};
use trust_syntax::parser;
use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

const PLCOPEN_NAMESPACE: &str = "http://www.plcopen.org/xml/tc6_0200";
const PROFILE_NAME: &str = "trust-st-complete-v1";
const SOURCE_MAP_DATA_NAME: &str = "trust.sourceMap";
const VENDOR_EXT_DATA_NAME: &str = "trust.vendorExtensions";
const EXPORT_ADAPTER_DATA_NAME: &str = "trust.exportAdapter";
const CODESYS_APPLICATION_DATA_NAME: &str = "http://www.3s-software.com/plcopenxml/application";
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
    source: String,
    line: usize,
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
    pou_entries: Vec<CodesysExportObjectEntry>,
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

pub fn supported_profile() -> PlcopenProfile {
    PlcopenProfile {
        namespace: PLCOPEN_NAMESPACE,
        profile: PROFILE_NAME,
        version: "TC6 XML v2.0 (ST-complete subset)",
        strict_subset: vec![
            "project/fileHeader/contentHeader",
            "types/pous/pou[pouType=program|function|functionBlock]",
            "types/dataTypes/dataType[baseType subset: elementary|derived|array|struct|enum|subrange] (import/export)",
            "instances/configurations/resources/tasks/program instances",
            "CODESYS addData/globalVars (import/export)",
            "CODESYS addData/projectstructure folder mapping (import/export)",
            "pou/body/ST plain-text bodies",
            "addData/data[name=trust.sourceMap|trust.vendorExtensions|trust.exportAdapter]",
        ],
        unsupported_nodes: vec![
            "graphical bodies (FBD/LD/SFC)",
            "vendor-specific nodes (preserved via hooks, not interpreted)",
            "dataTypes outside supported baseType subset",
        ],
        compatibility_matrix: vec![
            PlcopenCompatibilityMatrixEntry {
                capability: "POU import/export: PROGRAM/FUNCTION/FUNCTION_BLOCK with ST body",
                status: "supported",
                notes: "Aliases such as PRG/FC/FB are normalized on import.",
            },
            PlcopenCompatibilityMatrixEntry {
                capability: "Source mapping metadata",
                status: "supported",
                notes: "Embedded addData trust.sourceMap + deterministic source-map sidecar JSON.",
            },
            PlcopenCompatibilityMatrixEntry {
                capability: "Vendor extension node preservation",
                status: "partial",
                notes: "Unknown addData/vendor fragments are preserved and re-injectable, but not semantically interpreted.",
            },
            PlcopenCompatibilityMatrixEntry {
                capability: "Vendor ecosystem migration heuristics",
                status: "partial",
                notes: "Detected ecosystems are advisory diagnostics for migration workflows, not semantic guarantees.",
            },
            PlcopenCompatibilityMatrixEntry {
                capability: "PLCopen dataTypes import (elementary/derived/array/struct/enum/subrange subset)",
                status: "supported",
                notes: "Supported dataType baseType nodes are imported into generated ST TYPE declarations under src/.",
            },
            PlcopenCompatibilityMatrixEntry {
                capability: "PLCopen dataTypes export (elementary/derived/array/struct/enum/subrange subset)",
                status: "partial",
                notes: "Export emits supported TYPE declarations into types/dataTypes. Unsupported ST forms are skipped with warnings.",
            },
            PlcopenCompatibilityMatrixEntry {
                capability: "Project model import/export (instances/configurations/resources/tasks/program instances)",
                status: "supported",
                notes: "ST configuration/resource/task/program-instance model is imported/exported with deterministic naming and diagnostics.",
            },
            PlcopenCompatibilityMatrixEntry {
                capability: "CODESYS global variable lists (addData/globalVars)",
                status: "supported",
                notes: "Import prefers interface-as-plaintext for VAR_GLOBAL fidelity and falls back to variable node synthesis; export emits deterministic CODESYS globalVars metadata.",
            },
            PlcopenCompatibilityMatrixEntry {
                capability: "CODESYS project structure folder mapping (addData/projectstructure)",
                status: "partial",
                notes: "Import/export mirrors deterministic source-folder hierarchies for POUs/GVLs; unsupported library/device-tree object semantics remain metadata only.",
            },
            PlcopenCompatibilityMatrixEntry {
                capability: "Vendor library compatibility shims (selected timer/edge aliases)",
                status: "partial",
                notes: "Import can normalize selected Siemens/Rockwell/Schneider/Mitsubishi aliases to IEC FB names and reports each shim application.",
            },
            PlcopenCompatibilityMatrixEntry {
                capability: "Export adapters v1 (Allen-Bradley/Siemens/Schneider)",
                status: "partial",
                notes: "Export emits target-specific adapter diagnostics/manual-step reports, but native vendor project packages remain out of scope.",
            },
            PlcopenCompatibilityMatrixEntry {
                capability: "Graphical bodies (FBD/LD/SFC) and advanced runtime deployment resources",
                status: "unsupported",
                notes: "ST-complete subset remains ST-only and does not import graphical networks or advanced deployment metadata semantics.",
            },
            PlcopenCompatibilityMatrixEntry {
                capability: "Vendor AOIs, advanced library semantics, and platform-specific pragmas",
                status: "unsupported",
                notes: "Shim catalog is intentionally narrow; unsupported content is reported in migration diagnostics and known-gaps docs.",
            },
        ],
        source_mapping: "Export writes deterministic source-map sidecar JSON and embeds trust.sourceMap in addData.",
        vendor_extension_hook:
            "Import preserves unknown addData/vendor nodes to plcopen.vendor-extensions.imported.xml; export re-injects plcopen.vendor-extensions.xml.",
        round_trip_limits: vec![
            "Round-trip guarantees preserve ST POU signatures (name/type/body intent) for ST-complete supported inputs.",
            "Round-trip guarantees preserve supported ST dataType signatures (name + supported baseType graph).",
            "Round-trip guarantees preserve supported configuration/resource/task/program-instance wiring intent.",
            "Round-trip preserves supported CODESYS globalVars declarations (plaintext-first import strategy).",
            "Round-trip preserves deterministic folder placement intent for supported CODESYS projectstructure object trees.",
            "Round-trip does not preserve vendor formatting/layout, graphical networks, or runtime deployment metadata.",
            "Round-trip can rename output source files to sanitized unique names inside src/.",
            "Round-trip may normalize selected vendor library symbols to IEC equivalents when shim rules apply during import.",
            "Round-trip preserves unknown vendor addData as opaque fragments, not executable semantics.",
        ],
        known_gaps: vec![
            "No import/export for SFC/LD/FBD bodies.",
            "Vendor library shim coverage is limited to the published baseline alias catalog.",
            "No semantic translation for vendor-specific AOI/FB internal behavior beyond simple symbol remapping.",
            "No guaranteed equivalence for vendor pragmas, safety metadata, or online deployment tags.",
            "Export adapters do not generate native vendor project archives (.L5X/.apxx/.project).",
        ],
    }
}

pub fn export_project_to_xml(
    project_root: &Path,
    output_path: &Path,
) -> anyhow::Result<PlcopenExportReport> {
    export_project_to_xml_with_target(project_root, output_path, PlcopenExportTarget::Generic)
}

fn resolve_existing_source_root(project_root: &Path) -> anyhow::Result<PathBuf> {
    let src_root = project_root.join("src");
    if src_root.is_dir() {
        return Ok(src_root);
    }

    anyhow::bail!(
        "invalid project folder '{}': missing src/ directory",
        project_root.display()
    );
}

fn resolve_or_create_source_root(project_root: &Path) -> anyhow::Result<PathBuf> {
    let src_root = project_root.join("src");
    if src_root.is_dir() {
        return Ok(src_root);
    }

    std::fs::create_dir_all(&src_root)
        .with_context(|| format!("failed to create '{}'", src_root.display()))?;
    Ok(src_root)
}

pub fn export_project_to_xml_with_target(
    project_root: &Path,
    output_path: &Path,
    target: PlcopenExportTarget,
) -> anyhow::Result<PlcopenExportReport> {
    let sources_root = resolve_existing_source_root(project_root)?;

    let sources = load_sources(project_root, &sources_root)?;
    if sources.is_empty() {
        anyhow::bail!("no ST sources found under {}", sources_root.display());
    }
    let source_analysis = analyze_export_sources(&sources);

    let mut warnings = Vec::new();
    let mut declarations = Vec::new();
    let mut data_type_decls = Vec::new();
    let mut global_var_lists = Vec::new();
    let mut configurations = Vec::new();

    for source in &sources {
        let (mut declared, mut source_warnings) = extract_pou_declarations(source);
        declarations.append(&mut declared);
        warnings.append(&mut source_warnings);

        let (mut declared_types, mut type_warnings) = extract_data_type_declarations(source);
        data_type_decls.append(&mut declared_types);
        warnings.append(&mut type_warnings);

        let (mut source_configs, mut config_warnings) = extract_configuration_declarations(source);
        configurations.append(&mut source_configs);
        warnings.append(&mut config_warnings);

        let (mut source_globals, mut global_warnings) = extract_global_var_declarations(source);
        global_var_lists.append(&mut source_globals);
        warnings.append(&mut global_warnings);
    }

    if declarations.is_empty()
        && data_type_decls.is_empty()
        && global_var_lists.is_empty()
        && configurations.is_empty()
    {
        anyhow::bail!(
            "no PLCopen ST-complete declarations discovered (supported: POUs, TYPE blocks, VAR_GLOBAL blocks, CONFIGURATION/RESOURCE/TASK/PROGRAM)"
        );
    }

    declarations.sort_by(|left, right| {
        left.pou_type
            .as_xml()
            .cmp(right.pou_type.as_xml())
            .then(left.name.cmp(&right.name))
            .then(left.source.cmp(&right.source))
    });

    data_type_decls.sort_by(|left, right| {
        left.name
            .to_ascii_lowercase()
            .cmp(&right.name.to_ascii_lowercase())
            .then(left.source.cmp(&right.source))
            .then(left.line.cmp(&right.line))
    });

    global_var_lists.sort_by(|left, right| {
        left.name
            .to_ascii_lowercase()
            .cmp(&right.name.to_ascii_lowercase())
            .then(left.source.cmp(&right.source))
            .then(left.line.cmp(&right.line))
    });

    let mut deduped_types = Vec::new();
    let mut seen_type_names = BTreeSet::new();
    for decl in data_type_decls {
        let key = decl.name.to_ascii_lowercase();
        if seen_type_names.insert(key) {
            deduped_types.push(decl);
        } else {
            warnings.push(format!(
                "{}:{} duplicate TYPE declaration '{}' skipped for PLCopen export",
                decl.source, decl.line, decl.name
            ));
        }
    }

    for config in &mut configurations {
        config.tasks.sort_by(|left, right| {
            left.name
                .to_ascii_lowercase()
                .cmp(&right.name.to_ascii_lowercase())
        });
        config.programs.sort_by(|left, right| {
            left.instance_name
                .to_ascii_lowercase()
                .cmp(&right.instance_name.to_ascii_lowercase())
        });
        config.resources.sort_by(|left, right| {
            left.name
                .to_ascii_lowercase()
                .cmp(&right.name.to_ascii_lowercase())
        });
        for resource in &mut config.resources {
            resource.tasks.sort_by(|left, right| {
                left.name
                    .to_ascii_lowercase()
                    .cmp(&right.name.to_ascii_lowercase())
            });
            resource.programs.sort_by(|left, right| {
                left.instance_name
                    .to_ascii_lowercase()
                    .cmp(&right.instance_name.to_ascii_lowercase())
            });
        }
    }
    configurations.sort_by(|left, right| {
        left.name
            .to_ascii_lowercase()
            .cmp(&right.name.to_ascii_lowercase())
    });

    let source_map = SourceMapPayload {
        profile: PROFILE_NAME.to_string(),
        namespace: PLCOPEN_NAMESPACE.to_string(),
        entries: declarations
            .iter()
            .map(|decl| SourceMapEntry {
                name: decl.name.clone(),
                pou_type: decl.pou_type.as_xml().to_string(),
                source: decl.source.clone(),
                line: decl.line,
            })
            .collect(),
    };
    let source_map_json = serde_json::to_string_pretty(&source_map)?;

    let project_name = project_root
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("project");
    let generated_at = now_iso8601();

    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str(&format!(
        "<project xmlns=\"{}\" profile=\"{}\">\n",
        PLCOPEN_NAMESPACE, PROFILE_NAME
    ));
    xml.push_str(&format!(
        "  <fileHeader companyName=\"truST\" productName=\"trust-runtime\" productVersion=\"{}\" creationDateTime=\"{}\"/>\n",
        escape_xml_attr(env!("CARGO_PKG_VERSION")),
        escape_xml_attr(&generated_at)
    ));
    xml.push_str(&format!(
        "  <contentHeader name=\"{}\"/>\n",
        escape_xml_attr(project_name)
    ));
    xml.push_str("  <types>\n");

    let mut exported_data_type_count = 0usize;
    if !deduped_types.is_empty() {
        xml.push_str("    <dataTypes>\n");
        for data_type in &deduped_types {
            if let Some(base_type_xml) =
                type_expression_to_plcopen_base_type_xml(&data_type.type_expr)
            {
                xml.push_str(&format!(
                    "      <dataType name=\"{}\">\n",
                    escape_xml_attr(&data_type.name)
                ));
                xml.push_str("        <baseType>\n");
                for line in base_type_xml.lines() {
                    xml.push_str("          ");
                    xml.push_str(line);
                    xml.push('\n');
                }
                xml.push_str("        </baseType>\n");
                xml.push_str("      </dataType>\n");
                exported_data_type_count += 1;
            } else {
                warnings.push(format!(
                    "{}:{} unsupported TYPE expression for '{}' skipped in PLCopen dataTypes export",
                    data_type.source, data_type.line, data_type.name
                ));
            }
        }
        xml.push_str("    </dataTypes>\n");
    }

    xml.push_str("    <pous>\n");

    for decl in &declarations {
        xml.push_str(&format!(
            "      <pou name=\"{}\" pouType=\"{}\">\n",
            escape_xml_attr(&decl.name),
            decl.pou_type.as_xml()
        ));
        xml.push_str("        <body>\n");
        xml.push_str("          <ST><![CDATA[");
        xml.push_str(&escape_cdata(&decl.body));
        xml.push_str("]]></ST>\n");
        xml.push_str("        </body>\n");
        xml.push_str("      </pou>\n");
    }

    xml.push_str("    </pous>\n");
    xml.push_str("  </types>\n");

    let mut exported_resource_count = 0usize;
    let mut exported_task_count = 0usize;
    let mut exported_program_instance_count = 0usize;
    if !configurations.is_empty() {
        xml.push_str("  <instances>\n");
        xml.push_str("    <configurations>\n");
        for configuration in &configurations {
            xml.push_str(&format!(
                "      <configuration name=\"{}\">\n",
                escape_xml_attr(&configuration.name)
            ));

            for task in &configuration.tasks {
                append_task_xml(&mut xml, task, 8);
                exported_task_count += 1;
            }
            for program in &configuration.programs {
                append_program_instance_xml(&mut xml, program, 8);
                exported_program_instance_count += 1;
            }

            for resource in &configuration.resources {
                exported_resource_count += 1;
                xml.push_str(&format!(
                    "        <resource name=\"{}\" target=\"{}\">\n",
                    escape_xml_attr(&resource.name),
                    escape_xml_attr(&resource.target)
                ));
                for task in &resource.tasks {
                    append_task_xml(&mut xml, task, 10);
                    exported_task_count += 1;
                }
                for program in &resource.programs {
                    append_program_instance_xml(&mut xml, program, 10);
                    exported_program_instance_count += 1;
                }
                xml.push_str("        </resource>\n");
            }

            xml.push_str("      </configuration>\n");
        }
        xml.push_str("    </configurations>\n");
        xml.push_str("  </instances>\n");
    }

    let validation_context = ExportTargetValidationContext {
        pou_count: declarations.len(),
        data_type_count: exported_data_type_count,
        configuration_count: configurations.len(),
        resource_count: exported_resource_count,
        task_count: exported_task_count,
        program_instance_count: exported_program_instance_count,
        source_count: sources.len(),
        analysis: source_analysis,
    };
    let adapter_contract = build_export_adapter_contract(target, &validation_context);
    if let Some(contract) = &adapter_contract {
        for diagnostic in &contract.diagnostics {
            if !diagnostic.severity.eq_ignore_ascii_case("info") {
                warnings.push(format!(
                    "{} [{}]: {}",
                    diagnostic.code,
                    target.id(),
                    diagnostic.message
                ));
            }
        }
    }

    let codesys_metadata =
        build_codesys_export_metadata(&declarations, &global_var_lists, &mut warnings);
    xml.push_str("  <addData>\n");
    xml.push_str(&format!(
        "    <data name=\"{}\" handleUnknown=\"implementation\"><text><![CDATA[{}]]></text></data>\n",
        SOURCE_MAP_DATA_NAME,
        escape_cdata(&source_map_json)
    ));
    if let Some(contract) = &adapter_contract {
        let adapter_payload = serde_json::json!({
            "target": target.id(),
            "target_label": target.label(),
            "diagnostics": contract.diagnostics,
            "manual_steps": contract.manual_steps,
            "limitations": contract.limitations,
        });
        let adapter_json = serde_json::to_string_pretty(&adapter_payload)?;
        xml.push_str(&format!(
            "    <data name=\"{}\" handleUnknown=\"implementation\"><text><![CDATA[{}]]></text></data>\n",
            EXPORT_ADAPTER_DATA_NAME,
            escape_cdata(&adapter_json)
        ));
    }
    append_codesys_export_add_data(&mut xml, &codesys_metadata, &mut warnings);

    let vendor_hook_path = project_root.join(VENDOR_EXTENSION_HOOK_FILE);
    if vendor_hook_path.is_file() {
        let vendor_text = std::fs::read_to_string(&vendor_hook_path).with_context(|| {
            format!(
                "failed to read vendor extension hook '{}'",
                vendor_hook_path.display()
            )
        })?;
        xml.push_str(&format!(
            "    <data name=\"{}\" handleUnknown=\"implementation\"><text><![CDATA[{}]]></text></data>\n",
            VENDOR_EXT_DATA_NAME,
            escape_cdata(&vendor_text)
        ));
    }
    xml.push_str("  </addData>\n");
    xml.push_str("</project>\n");

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create PLCopen output directory '{}'",
                parent.display()
            )
        })?;
    }

    std::fs::write(output_path, xml)
        .with_context(|| format!("failed to write '{}'", output_path.display()))?;

    let source_map_path = output_path.with_extension("source-map.json");
    std::fs::write(&source_map_path, format!("{}\n", source_map_json)).with_context(|| {
        format!(
            "failed to write source-map sidecar '{}'",
            source_map_path.display()
        )
    })?;

    let mut siemens_scl_bundle_dir = None;
    let mut siemens_scl_files = Vec::new();
    if target == PlcopenExportTarget::Siemens {
        let (bundle_dir, bundle_files) = export_siemens_scl_bundle(
            output_path,
            &declarations,
            &deduped_types,
            &configurations,
            &mut warnings,
        )?;
        siemens_scl_bundle_dir = Some(bundle_dir);
        siemens_scl_files = bundle_files;
    }

    let mut adapter_report_path = None;
    let mut adapter_diagnostics = Vec::new();
    let mut adapter_manual_steps = Vec::new();
    let mut adapter_limitations = Vec::new();
    if let Some(contract) = adapter_contract {
        let adapter_path = adapter_report_path_for_output(output_path);
        let adapter_report = PlcopenExportAdapterReport {
            target: target.id().to_string(),
            target_label: target.label().to_string(),
            source_xml: output_path.to_path_buf(),
            source_map_path: source_map_path.clone(),
            siemens_scl_bundle_dir: siemens_scl_bundle_dir.clone(),
            siemens_scl_files: siemens_scl_files.clone(),
            diagnostics: contract.diagnostics,
            manual_steps: contract.manual_steps,
            limitations: contract.limitations,
        };
        let adapter_json = serde_json::to_string_pretty(&adapter_report)?;
        std::fs::write(&adapter_path, format!("{adapter_json}\n")).with_context(|| {
            format!(
                "failed to write target adapter report '{}'",
                adapter_path.display()
            )
        })?;

        adapter_report_path = Some(adapter_path);
        adapter_diagnostics = adapter_report.diagnostics;
        adapter_manual_steps = adapter_report.manual_steps;
        adapter_limitations = adapter_report.limitations;
    }

    Ok(PlcopenExportReport {
        target: target.id().to_string(),
        output_path: output_path.to_path_buf(),
        source_map_path,
        adapter_report_path,
        siemens_scl_bundle_dir,
        siemens_scl_files,
        adapter_diagnostics,
        adapter_manual_steps,
        adapter_limitations,
        pou_count: declarations.len(),
        data_type_count: exported_data_type_count,
        configuration_count: configurations.len(),
        resource_count: exported_resource_count,
        task_count: exported_task_count,
        program_instance_count: exported_program_instance_count,
        exported_global_var_lists: codesys_metadata.global_var_lists.len(),
        exported_project_structure_nodes: codesys_metadata.exported_project_structure_nodes,
        exported_folder_paths: codesys_metadata.exported_folder_paths,
        source_count: sources.len(),
        warnings,
    })
}

pub fn import_xml_to_project(
    xml_path: &Path,
    project_root: &Path,
) -> anyhow::Result<PlcopenImportReport> {
    let xml_text = std::fs::read_to_string(xml_path)
        .with_context(|| format!("failed to read PLCopen XML '{}'", xml_path.display()))?;
    let document = roxmltree::Document::parse(&xml_text)
        .with_context(|| format!("failed to parse PLCopen XML '{}'", xml_path.display()))?;

    let root = document.root_element();
    if root.tag_name().name() != "project" {
        anyhow::bail!(
            "invalid PLCopen XML: expected root <project>, found <{}>",
            root.tag_name().name()
        );
    }

    let mut warnings = Vec::new();
    let mut unsupported_nodes = Vec::new();
    let mut unsupported_diagnostics = Vec::new();
    let mut written_sources = Vec::new();
    let mut seen_files = HashSet::new();
    let mut migration_entries = Vec::new();
    let mut applied_shim_counts: BTreeMap<(String, String, String, String), usize> =
        BTreeMap::new();
    let mut discovered_pous = 0usize;
    let mut loss_warnings = 0usize;
    let mut imported_data_types = 0usize;

    if let Some(namespace) = root.tag_name().namespace() {
        if namespace != PLCOPEN_NAMESPACE {
            warnings.push(format!(
                "unexpected namespace '{}'; expected '{}'",
                namespace, PLCOPEN_NAMESPACE
            ));
        }
    }

    inspect_unsupported_structure(
        root,
        &mut unsupported_nodes,
        &mut warnings,
        &mut unsupported_diagnostics,
    );

    let source_map = parse_embedded_source_map(root);
    let detected_ecosystem = detect_vendor_ecosystem(root, &xml_text);
    let promoted_program_pous = detect_program_pous_used_as_types(root);
    let project_structure_map = parse_codesys_project_structure(root);
    let mut imported_folder_paths = HashSet::new();

    let sources_root = resolve_or_create_source_root(project_root)?;

    if let Some((path, count)) = import_data_types_to_sources(
        root,
        &sources_root,
        &mut seen_files,
        &mut warnings,
        &mut unsupported_nodes,
        &mut unsupported_diagnostics,
        &mut loss_warnings,
    )? {
        imported_data_types = count;
        written_sources.push(path);
    }

    let project_model_stats = import_project_model_to_sources(
        root,
        &sources_root,
        &mut seen_files,
        &mut warnings,
        &mut unsupported_nodes,
        &mut unsupported_diagnostics,
        &mut loss_warnings,
    )?;
    written_sources.extend(project_model_stats.written_sources.iter().cloned());

    let global_var_stats = import_global_var_lists_to_sources(
        root,
        &sources_root,
        &mut seen_files,
        &mut warnings,
        &mut unsupported_nodes,
        &mut unsupported_diagnostics,
        &mut loss_warnings,
        &project_structure_map,
        &mut imported_folder_paths,
    )?;
    written_sources.extend(global_var_stats.written_sources.iter().cloned());

    for pou in collect_import_pou_nodes(root) {
        discovered_pous += 1;
        let pou_name = extract_pou_name(pou);
        let entry_name = pou_name
            .clone()
            .unwrap_or_else(|| format!("unnamed_{discovered_pous}"));
        let pou_type_raw = attribute_ci(pou, "pouType").or_else(|| attribute_ci(pou, "type"));
        let resolved_pou_type = pou_type_raw.as_deref().and_then(PlcopenPouType::from_xml);
        let st_body = extract_st_body(pou);

        let Some(name) = pou_name else {
            warnings.push("skipping <pou> without name attribute".to_string());
            unsupported_diagnostics.push(unsupported_diagnostic(
                "PLCO201",
                "warning",
                "pou",
                "POU skipped because required name attribute is missing",
                None,
                "Skipped from import and counted in semantic-loss scoring",
            ));
            loss_warnings += 1;
            migration_entries.push(PlcopenMigrationEntry {
                name: entry_name,
                pou_type_raw,
                resolved_pou_type: resolved_pou_type.map(|kind| kind.as_xml().to_string()),
                status: "skipped".to_string(),
                reason: Some("missing name".to_string()),
            });
            continue;
        };

        let Some(pou_type_raw) = pou_type_raw else {
            warnings.push(format!("skipping pou '{}': missing pouType", name));
            unsupported_diagnostics.push(unsupported_diagnostic(
                "PLCO202",
                "warning",
                "pou",
                "POU skipped because pouType/type attribute is missing",
                Some(name.clone()),
                "Skipped from import and counted in semantic-loss scoring",
            ));
            loss_warnings += 1;
            migration_entries.push(PlcopenMigrationEntry {
                name,
                pou_type_raw: None,
                resolved_pou_type: None,
                status: "skipped".to_string(),
                reason: Some("missing pouType/type attribute".to_string()),
            });
            continue;
        };

        let Some(mut pou_type) = PlcopenPouType::from_xml(&pou_type_raw) else {
            warnings.push(format!(
                "skipping pou '{}': unsupported pouType '{}'",
                name, pou_type_raw
            ));
            unsupported_nodes.push(format!("pouType:{}", pou_type_raw));
            unsupported_diagnostics.push(unsupported_diagnostic(
                "PLCO203",
                "warning",
                format!("pouType:{pou_type_raw}"),
                format!("POU type '{pou_type_raw}' is outside the ST-complete subset"),
                Some(name.clone()),
                "POU skipped; convert to PROGRAM/FUNCTION/FUNCTION_BLOCK or supported aliases",
            ));
            loss_warnings += 1;
            migration_entries.push(PlcopenMigrationEntry {
                name,
                pou_type_raw: Some(pou_type_raw),
                resolved_pou_type: None,
                status: "skipped".to_string(),
                reason: Some("unsupported pouType".to_string()),
            });
            continue;
        };

        if pou_type == PlcopenPouType::Program
            && promoted_program_pous.contains(&name.to_ascii_lowercase())
        {
            pou_type = PlcopenPouType::FunctionBlock;
            warnings.push(format!(
                "promoted pou '{}' from program to functionBlock because it is referenced as a type",
                name
            ));
            unsupported_diagnostics.push(unsupported_diagnostic(
                "PLCO210",
                "info",
                "pou/pouType",
                "POU pouType promoted from program to functionBlock based on cross-reference usage",
                Some(name.clone()),
                "Review promoted POU kind for semantic parity with the source vendor model",
            ));
        }

        let Some(reconstructed_source) = synthesize_import_pou_source(
            pou,
            pou_type,
            &name,
            st_body.as_deref(),
            &mut warnings,
            &mut unsupported_diagnostics,
        ) else {
            if st_body.is_none() {
                warnings.push(format!("skipping pou '{}': missing body/ST", name));
                unsupported_diagnostics.push(unsupported_diagnostic(
                    "PLCO204",
                    "warning",
                    "pou/body",
                    "POU skipped because body/ST payload is missing",
                    Some(name.clone()),
                    "POU skipped; provide an ST body in PLCopen XML",
                ));
                loss_warnings += 1;
                migration_entries.push(PlcopenMigrationEntry {
                    name,
                    pou_type_raw: Some(pou_type_raw),
                    resolved_pou_type: Some(pou_type.as_xml().to_string()),
                    status: "skipped".to_string(),
                    reason: Some("missing body/ST".to_string()),
                });
                continue;
            }
            warnings.push(format!("skipping pou '{}': empty ST body", name));
            unsupported_diagnostics.push(unsupported_diagnostic(
                "PLCO205",
                "warning",
                "pou/body/ST",
                "POU skipped because ST body is empty",
                Some(name.clone()),
                "POU skipped; provide non-empty ST source text",
            ));
            loss_warnings += 1;
            migration_entries.push(PlcopenMigrationEntry {
                name,
                pou_type_raw: Some(pou_type_raw),
                resolved_pou_type: Some(pou_type.as_xml().to_string()),
                status: "skipped".to_string(),
                reason: Some("empty ST body".to_string()),
            });
            continue;
        };

        let (reconstructed_source, injected_global_externals) =
            inject_required_var_external_declarations(
                &reconstructed_source,
                &global_var_stats.qualified_list_externals,
            );
        for external in injected_global_externals {
            warnings.push(format!(
                "inserted VAR_EXTERNAL '{}' declaration in pou '{}'",
                external, name
            ));
            unsupported_diagnostics.push(unsupported_diagnostic(
                "PLCO603",
                "info",
                "pou/interface/externalVars",
                format!(
                    "Inserted VAR_EXTERNAL declaration for qualified global list '{}'",
                    external
                ),
                Some(name.clone()),
                "Review and keep inserted external declaration if the POU uses qualified global list access",
            ));
        }

        let folder_segments =
            resolve_codesys_folder_segments_for_node(pou, &name, &project_structure_map);
        let candidate = unique_source_path_with_segments(
            &sources_root,
            &folder_segments,
            &name,
            &mut seen_files,
        );

        let (shimmed_body, shim_applications) =
            apply_vendor_library_shims(&reconstructed_source, &detected_ecosystem);
        for application in shim_applications {
            warnings.push(format!(
                "applied vendor library shim in pou '{}': {} -> {} ({} occurrence(s))",
                name,
                application.source_symbol,
                application.replacement_symbol,
                application.occurrences
            ));
            unsupported_diagnostics.push(unsupported_diagnostic(
                "PLCO301",
                "info",
                format!("vendor-shim:{}", application.source_symbol),
                format!(
                    "Vendor library shim mapped '{}' to '{}'",
                    application.source_symbol, application.replacement_symbol
                ),
                Some(name.clone()),
                application.notes.clone(),
            ));
            let key = (
                application.vendor,
                application.source_symbol,
                application.replacement_symbol,
                application.notes,
            );
            *applied_shim_counts.entry(key).or_insert(0) += application.occurrences;
        }

        write_text_file_with_parents(&candidate, &shimmed_body)?;
        track_imported_folder_path(&candidate, &sources_root, &mut imported_folder_paths);
        written_sources.push(candidate);

        migration_entries.push(PlcopenMigrationEntry {
            name: name.clone(),
            pou_type_raw: Some(pou_type_raw),
            resolved_pou_type: Some(pou_type.as_xml().to_string()),
            status: "imported".to_string(),
            reason: None,
        });

        if let Some(entry) = source_map.as_ref().and_then(|map| {
            map.entries
                .iter()
                .find(|entry| entry.name.eq_ignore_ascii_case(&name))
        }) {
            warnings.push(format!(
                "source map: pou '{}' originated from {}:{}",
                name, entry.source, entry.line
            ));
        }
    }

    if discovered_pous == 0 && imported_data_types == 0 {
        warnings.push("no <pou> nodes discovered in input XML".to_string());
        unsupported_diagnostics.push(unsupported_diagnostic(
            "PLCO206",
            "warning",
            "types/pous",
            "Input XML does not contain importable <pou> elements",
            None,
            "Provide PLCopen ST POUs under project/types/pous",
        ));
        loss_warnings += 1;
    }

    let imported_pous = migration_entries
        .iter()
        .filter(|entry| entry.status == "imported")
        .count();
    let importable_pous = imported_pous;
    let skipped_pous = discovered_pous.saturating_sub(imported_pous);
    let source_coverage_percent = calculate_source_coverage(imported_pous, discovered_pous);
    let semantic_loss_percent = calculate_semantic_loss(
        imported_pous,
        discovered_pous,
        unsupported_nodes.len(),
        loss_warnings,
    );
    let applied_library_shims = applied_shim_counts
        .into_iter()
        .map(
            |((vendor, source_symbol, replacement_symbol, notes), occurrences)| {
                PlcopenLibraryShimApplication {
                    vendor,
                    source_symbol,
                    replacement_symbol,
                    occurrences,
                    notes,
                }
            },
        )
        .collect::<Vec<_>>();
    let shimmed_occurrences = applied_library_shims
        .iter()
        .map(|entry| entry.occurrences)
        .sum::<usize>();
    let compatibility_coverage = calculate_compatibility_coverage(
        imported_pous,
        skipped_pous,
        unsupported_nodes.len(),
        shimmed_occurrences,
    );

    let preserved_vendor_extensions =
        preserve_vendor_extensions(root, &xml_text, project_root, &mut warnings)?;
    let migration_report = PlcopenMigrationReport {
        profile: PROFILE_NAME.to_string(),
        namespace: root
            .tag_name()
            .namespace()
            .unwrap_or(PLCOPEN_NAMESPACE)
            .to_string(),
        source_xml: xml_path.to_path_buf(),
        project_root: project_root.to_path_buf(),
        detected_ecosystem: detected_ecosystem.clone(),
        discovered_pous,
        importable_pous,
        imported_pous,
        skipped_pous,
        imported_data_types,
        discovered_configurations: project_model_stats.discovered_configurations,
        imported_configurations: project_model_stats.imported_configurations,
        imported_resources: project_model_stats.imported_resources,
        imported_tasks: project_model_stats.imported_tasks,
        imported_program_instances: project_model_stats.imported_program_instances,
        discovered_global_var_lists: global_var_stats.discovered_global_var_lists,
        imported_global_var_lists: global_var_stats.imported_global_var_lists,
        imported_project_structure_nodes: project_structure_map.object_count,
        imported_folder_paths: imported_folder_paths.len(),
        source_coverage_percent,
        semantic_loss_percent,
        compatibility_coverage: compatibility_coverage.clone(),
        unsupported_nodes: unsupported_nodes.clone(),
        unsupported_diagnostics: unsupported_diagnostics.clone(),
        applied_library_shims: applied_library_shims.clone(),
        warnings: warnings.clone(),
        entries: migration_entries,
    };
    let migration_report_path = write_migration_report(project_root, &migration_report)?;

    if written_sources.is_empty() {
        anyhow::bail!(
            "no importable PLCopen ST content found in {} (migration report: {})",
            xml_path.display(),
            migration_report_path.display()
        );
    }

    Ok(PlcopenImportReport {
        project_root: project_root.to_path_buf(),
        imported_pous,
        discovered_pous,
        imported_data_types,
        discovered_configurations: project_model_stats.discovered_configurations,
        imported_configurations: project_model_stats.imported_configurations,
        imported_resources: project_model_stats.imported_resources,
        imported_tasks: project_model_stats.imported_tasks,
        imported_program_instances: project_model_stats.imported_program_instances,
        discovered_global_var_lists: global_var_stats.discovered_global_var_lists,
        imported_global_var_lists: global_var_stats.imported_global_var_lists,
        imported_project_structure_nodes: project_structure_map.object_count,
        imported_folder_paths: imported_folder_paths.len(),
        written_sources,
        warnings,
        unsupported_nodes,
        preserved_vendor_extensions,
        migration_report_path,
        source_coverage_percent,
        semantic_loss_percent,
        detected_ecosystem,
        compatibility_coverage,
        unsupported_diagnostics,
        applied_library_shims,
    })
}

fn load_sources(project_root: &Path, sources_root: &Path) -> anyhow::Result<Vec<LoadedSource>> {
    let mut paths = BTreeSet::new();
    for pattern in ["**/*.st", "**/*.ST", "**/*.pou", "**/*.POU"] {
        let glob_pattern = format!("{}/{}", sources_root.display(), pattern);
        for entry in glob::glob(&glob_pattern)
            .with_context(|| format!("invalid source glob '{}'", glob_pattern))?
        {
            paths.insert(entry?);
        }
    }

    let mut sources = Vec::with_capacity(paths.len());
    for path in paths {
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read source '{}'", path.display()))?;
        let relative = path
            .strip_prefix(project_root)
            .map_or_else(|_| path.clone(), Path::to_path_buf);
        sources.push(LoadedSource {
            path: relative,
            text,
        });
    }
    Ok(sources)
}

fn extract_pou_declarations(source: &LoadedSource) -> (Vec<PouDecl>, Vec<String>) {
    let mut declarations = Vec::new();
    let mut warnings = Vec::new();

    let parsed = parser::parse(&source.text);
    let syntax = parsed.syntax();

    for node in syntax.children() {
        let Some(pou_type) = node_to_pou_type(&node) else {
            if is_unsupported_top_level(&node) {
                let line = line_for_node(&source.text, &node);
                warnings.push(format!(
                    "{}:{} unsupported top-level node '{:?}' skipped for PLCopen ST-complete subset",
                    source.path.display(),
                    line,
                    node.kind()
                ));
            }
            continue;
        };

        let Some(name) = declaration_name(&node) else {
            continue;
        };

        if is_test_pou(&node) {
            let line = line_for_node(&source.text, &node);
            warnings.push(format!(
                "{}:{} test POU '{}' exported as standard '{}'",
                source.path.display(),
                line,
                name,
                pou_type.as_xml()
            ));
        }

        let line = line_for_node(&source.text, &node);
        declarations.push(PouDecl {
            name,
            pou_type,
            body: normalize_body_text(node.text().to_string()),
            source: source.path.display().to_string(),
            line,
        });
    }

    (declarations, warnings)
}

fn extract_global_var_declarations(source: &LoadedSource) -> (Vec<GlobalVarDecl>, Vec<String>) {
    let mut declarations = Vec::new();
    let mut warnings = Vec::new();
    let lines = source.text.lines().collect::<Vec<_>>();
    let struct_fields_by_type = parse_struct_type_declarations(&source.text);
    let mut index = 0usize;
    let mut block_index = 0usize;
    let base_name = source
        .path
        .file_stem()
        .and_then(|value| value.to_str())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| "GlobalVars".to_string());

    while index < lines.len() {
        let upper = lines[index].trim().to_ascii_uppercase();
        if !upper.starts_with("VAR_GLOBAL") {
            index += 1;
            continue;
        }

        let mut block_start = index;
        while block_start > 0 {
            let prev = lines[block_start - 1].trim();
            if prev.is_empty() {
                break;
            }
            if prev.starts_with('{') && prev.ends_with('}') {
                block_start -= 1;
            } else {
                break;
            }
        }

        let Some(end_index) = (index..lines.len())
            .find(|line_index| lines[*line_index].trim().eq_ignore_ascii_case("END_VAR"))
        else {
            warnings.push(format!(
                "{}:{} unterminated VAR_GLOBAL block skipped during PLCopen export",
                source.path.display(),
                index + 1
            ));
            break;
        };

        let body = normalize_body_text(lines[block_start..=end_index].join("\n"));
        let mut variables = parse_global_var_entries_from_st_block(&body);
        let name = if block_index == 0 {
            base_name.clone()
        } else {
            format!("{base_name}_{}", block_index + 1)
        };
        if variables.len() == 1 {
            let wrapper = &variables[0];
            if wrapper.name.eq_ignore_ascii_case(&name) {
                let type_key = wrapper.type_expr.trim().to_ascii_lowercase();
                if let Some(struct_fields) = struct_fields_by_type.get(&type_key) {
                    variables = struct_fields.clone();
                }
            }
        }
        declarations.push(GlobalVarDecl {
            name,
            body,
            source: source.path.display().to_string(),
            source_path: source.path.clone(),
            line: block_start + 1,
            variables,
        });
        block_index += 1;
        index = end_index + 1;
    }

    (declarations, warnings)
}

fn parse_struct_type_declarations(source: &str) -> BTreeMap<String, Vec<GlobalVarVariableDecl>> {
    let mut map = BTreeMap::new();
    let lines = source.lines().collect::<Vec<_>>();
    let mut index = 0usize;

    while index < lines.len() {
        let trimmed = lines[index].trim();
        let Some((lhs, rhs)) = trimmed.split_once(':') else {
            index += 1;
            continue;
        };
        if !rhs.trim().eq_ignore_ascii_case("STRUCT") {
            index += 1;
            continue;
        }

        let type_name = lhs.trim();
        if type_name.is_empty() {
            index += 1;
            continue;
        }

        index += 1;
        let mut field_lines = Vec::new();
        while index < lines.len() && !lines[index].trim().eq_ignore_ascii_case("END_STRUCT") {
            field_lines.push(lines[index]);
            index += 1;
        }
        if index >= lines.len() {
            break;
        }

        let mut pseudo_block = String::from("VAR_GLOBAL\n");
        for field in field_lines {
            pseudo_block.push_str(field);
            pseudo_block.push('\n');
        }
        pseudo_block.push_str("END_VAR\n");

        let fields = parse_global_var_entries_from_st_block(&pseudo_block);
        if !fields.is_empty() {
            map.insert(type_name.to_ascii_lowercase(), fields);
        }

        index += 1;
    }

    map
}

fn parse_global_var_entries_from_st_block(block: &str) -> Vec<GlobalVarVariableDecl> {
    let mut entries = Vec::new();
    let mut in_block = false;

    for line in block.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with('{') && trimmed.ends_with('}') {
            continue;
        }
        if trimmed.to_ascii_uppercase().starts_with("VAR_GLOBAL") {
            in_block = true;
            continue;
        }
        if trimmed.eq_ignore_ascii_case("END_VAR") {
            break;
        }
        if !in_block || trimmed.starts_with("//") || trimmed.starts_with("(*") {
            continue;
        }

        let declaration = trimmed.trim_end_matches(';').trim();
        let Some((lhs, rhs)) = declaration.split_once(':') else {
            continue;
        };
        let raw_name = lhs.trim();
        if raw_name.is_empty() {
            continue;
        }
        let var_name = raw_name
            .split_whitespace()
            .next()
            .map(ToOwned::to_owned)
            .unwrap_or_default();
        if var_name.is_empty() {
            continue;
        }
        let (type_expr, initial_value) = match rhs.split_once(":=") {
            Some((type_part, initial_part)) => (
                type_part.trim().to_string(),
                Some(initial_part.trim().to_string()),
            ),
            None => (rhs.trim().to_string(), None),
        };
        if type_expr.is_empty() {
            continue;
        }
        entries.push(GlobalVarVariableDecl {
            name: var_name,
            type_expr,
            initial_value,
        });
    }

    entries
}

fn node_to_pou_type(node: &SyntaxNode) -> Option<PlcopenPouType> {
    match node.kind() {
        SyntaxKind::Program => Some(PlcopenPouType::Program),
        SyntaxKind::Function => Some(PlcopenPouType::Function),
        SyntaxKind::FunctionBlock => Some(PlcopenPouType::FunctionBlock),
        _ => None,
    }
}

fn is_unsupported_top_level(node: &SyntaxNode) -> bool {
    matches!(
        node.kind(),
        SyntaxKind::Class
            | SyntaxKind::Interface
            | SyntaxKind::Namespace
            | SyntaxKind::Configuration
            | SyntaxKind::TypeDecl
            | SyntaxKind::Action
    )
}

fn is_test_pou(node: &SyntaxNode) -> bool {
    first_non_trivia_token(node).is_some_and(|kind| {
        matches!(
            kind,
            SyntaxKind::KwTestProgram | SyntaxKind::KwTestFunctionBlock
        )
    })
}

fn first_non_trivia_token(node: &SyntaxNode) -> Option<SyntaxKind> {
    node.children_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| !token.kind().is_trivia())
        .map(|token| token.kind())
}

fn declaration_name(node: &SyntaxNode) -> Option<String> {
    node.children()
        .find(|child| child.kind() == SyntaxKind::Name)
        .map(|name| name.text().to_string().trim().to_string())
        .filter(|text| !text.is_empty())
}

fn line_for_node(source: &str, node: &SyntaxNode) -> usize {
    let offset = node
        .children_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| !token.kind().is_trivia())
        .map(|token| usize::from(token.text_range().start()))
        .unwrap_or(0);
    source[..offset]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1
}

fn now_iso8601() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{timestamp}Z")
}

fn escape_xml_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\'', "&apos;")
}

fn escape_cdata(value: &str) -> String {
    value.replace("]]>", "]]]]><![CDATA[>")
}

fn normalize_body_text(text: impl Into<String>) -> String {
    let mut normalized = text.into().replace("\r\n", "\n").replace('\r', "\n");
    if !normalized.ends_with('\n') {
        normalized.push('\n');
    }
    normalized
}

fn analyze_export_sources(sources: &[LoadedSource]) -> ExportSourceAnalysis {
    let mut analysis = ExportSourceAnalysis::default();
    for source in sources {
        let upper = source.text.to_ascii_uppercase();
        if upper.contains("VAR RETAIN") || upper.contains(" RETAIN ") || upper.contains("\nRETAIN")
        {
            analysis.has_retain_keyword = true;
        }
        if upper.contains("%I") || upper.contains("%Q") || upper.contains("%M") {
            analysis.has_direct_address_markers = true;
        }
        if upper.contains("SFB3") || upper.contains("SFB4") || upper.contains("SFB5") {
            analysis.has_siemens_aliases = true;
        }
        if upper.contains("TONR") {
            analysis.has_rockwell_aliases = true;
        }
        if upper.contains("R_EDGE") || upper.contains("F_EDGE") {
            analysis.has_schneider_aliases = true;
        }
    }
    analysis
}

fn adapter_report_path_for_output(output_path: &Path) -> PathBuf {
    let file_name = output_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("plcopen.xml");
    output_path.with_file_name(format!("{file_name}.adapter-report.json"))
}

fn siemens_scl_bundle_dir_for_output(output_path: &Path) -> PathBuf {
    let file_name = output_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("plcopen.siemens.xml");
    output_path.with_file_name(format!("{file_name}.scl"))
}

fn render_siemens_scl_types_file(types: &[DataTypeDecl]) -> String {
    let mut text = String::new();
    text.push_str("// Generated by trust-runtime plcopen export --target siemens\n");
    text.push_str("// Import in TIA Portal: External source files -> Add new external file\n\n");
    for data_type in types {
        text.push_str("TYPE\n");
        text.push_str(&format!(
            "    {} : {};\n",
            data_type.name.trim(),
            data_type.type_expr.trim()
        ));
        text.push_str("END_TYPE\n\n");
    }
    text
}

fn render_siemens_scl_pou(decl: &PouDecl) -> (String, Vec<String>) {
    let mut warnings = Vec::new();
    if !matches!(decl.pou_type, PlcopenPouType::Program) {
        return (decl.body.clone(), warnings);
    }

    let mut lines = decl
        .body
        .lines()
        .map(std::string::ToString::to_string)
        .collect::<Vec<_>>();
    let first_decl_idx = lines
        .iter()
        .position(|line| line.trim().to_ascii_uppercase().starts_with("PROGRAM "));
    let last_end_idx = lines
        .iter()
        .rposition(|line| line.trim().eq_ignore_ascii_case("END_PROGRAM"));

    match (first_decl_idx, last_end_idx) {
        (Some(start), Some(end)) if start < end => {
            lines[start] = format!("ORGANIZATION_BLOCK \"{}\"", decl.name);
            lines[end] = "END_ORGANIZATION_BLOCK".to_string();
            (lines.join("\n"), warnings)
        }
        _ => {
            warnings.push(format!(
                "{}:{} could not rewrite PROGRAM '{}' to Siemens ORGANIZATION_BLOCK form; exported IEC PROGRAM body as-is",
                decl.source, decl.line, decl.name
            ));
            (decl.body.clone(), warnings)
        }
    }
}

fn export_siemens_scl_bundle(
    output_path: &Path,
    declarations: &[PouDecl],
    data_types: &[DataTypeDecl],
    configurations: &[ConfigurationDecl],
    warnings: &mut Vec<String>,
) -> anyhow::Result<(PathBuf, Vec<PathBuf>)> {
    let bundle_dir = siemens_scl_bundle_dir_for_output(output_path);
    std::fs::create_dir_all(&bundle_dir).with_context(|| {
        format!(
            "failed to create Siemens SCL export directory '{}'",
            bundle_dir.display()
        )
    })?;

    let mut written = Vec::new();

    if !data_types.is_empty() {
        let types_path = bundle_dir.join("000_types.scl");
        let types_text = render_siemens_scl_types_file(data_types);
        std::fs::write(&types_path, types_text)
            .with_context(|| format!("failed to write '{}'", types_path.display()))?;
        written.push(types_path);
    }

    for (index, decl) in declarations.iter().enumerate() {
        let kind = match decl.pou_type {
            PlcopenPouType::Program => "ob",
            PlcopenPouType::Function => "fc",
            PlcopenPouType::FunctionBlock => "fb",
        };
        let file_name = format!(
            "{:03}_{}_{}.scl",
            index + 1,
            kind,
            sanitize_filename(&decl.name)
        );
        let path = bundle_dir.join(file_name);
        let (body, mut body_warnings) = render_siemens_scl_pou(decl);
        warnings.append(&mut body_warnings);

        let mut file_text = String::new();
        file_text.push_str("// Generated by trust-runtime plcopen export --target siemens\n");
        file_text.push_str(&format!("// Source: {}:{}\n\n", decl.source, decl.line));
        file_text.push_str(&body);
        if !body.ends_with('\n') {
            file_text.push('\n');
        }

        std::fs::write(&path, file_text)
            .with_context(|| format!("failed to write '{}'", path.display()))?;
        written.push(path);
    }

    if !configurations.is_empty() {
        let notes_path = bundle_dir.join("900_tia_mapping_notes.txt");
        let mut notes = String::new();
        notes.push_str("Generated by trust-runtime plcopen export --target siemens\n");
        notes.push_str("CONFIGURATION/RESOURCE/TASK/PROGRAM wiring requires manual OB/task mapping in TIA Portal.\n");
        notes.push_str("Use interop/*.adapter-report.json for detailed migration diagnostics and manual steps.\n");
        std::fs::write(&notes_path, notes)
            .with_context(|| format!("failed to write '{}'", notes_path.display()))?;
    }

    Ok((bundle_dir, written))
}

fn export_adapter_diagnostic(
    code: &str,
    severity: &str,
    message: impl Into<String>,
    action: impl Into<String>,
) -> PlcopenExportAdapterDiagnostic {
    PlcopenExportAdapterDiagnostic {
        code: code.to_string(),
        severity: severity.to_string(),
        message: message.into(),
        action: action.into(),
    }
}

fn build_export_adapter_contract(
    target: PlcopenExportTarget,
    context: &ExportTargetValidationContext,
) -> Option<PlcopenExportAdapterContract> {
    let mut diagnostics = Vec::new();
    let has_project_model = context.configuration_count > 0
        || context.resource_count > 0
        || context.task_count > 0
        || context.program_instance_count > 0;

    let (manual_steps, limitations) = match target {
        PlcopenExportTarget::Generic => return None,
        PlcopenExportTarget::AllenBradley => {
            diagnostics.push(export_adapter_diagnostic(
                "PLCO7AB0",
                "info",
                format!(
                    "Generated AB adapter artifact from {} source file(s) and {} ST declaration(s).",
                    context.source_count,
                    context.pou_count + context.data_type_count
                ),
                "Use the adapter report as the import checklist for Studio 5000 migration.",
            ));
            if has_project_model {
                diagnostics.push(export_adapter_diagnostic(
                    "PLCO7AB1",
                    "warning",
                    "Configuration/resource/task/program bindings require manual task mapping in Studio 5000.",
                    "Recreate periodic/continuous task wiring and bind imported program routines manually.",
                ));
            }
            if context.analysis.has_direct_address_markers {
                diagnostics.push(export_adapter_diagnostic(
                    "PLCO7AB2",
                    "warning",
                    "Detected direct `%I/%Q/%M` addressing markers.",
                    "Map tags to controller I/O aliases manually and verify address classes before deployment.",
                ));
            }
            if context.analysis.has_retain_keyword {
                diagnostics.push(export_adapter_diagnostic(
                    "PLCO7AB3",
                    "warning",
                    "Detected RETAIN usage that may not map 1:1 to Logix persistence semantics.",
                    "Review controller-scoped retentive tags and startup/reset behavior in commissioning tests.",
                ));
            }
            if context.analysis.has_siemens_aliases || context.analysis.has_schneider_aliases {
                diagnostics.push(export_adapter_diagnostic(
                    "PLCO7AB4",
                    "warning",
                    "Detected non-AB vendor alias symbols in ST sources.",
                    "Normalize vendor-specific aliases to IEC/AB-native symbols before final import.",
                ));
            }

            (
                vec![
                    "Import the generated PLCopen XML via your AB migration flow (converter/toolchain of choice).".to_string(),
                    "Recreate task classes and scan rates in Studio 5000, then bind each program routine.".to_string(),
                    "Rebind `%I/%Q/%M` markers to controller tags and physical I/O aliases.".to_string(),
                    "Run conformance and project acceptance tests after migration.".to_string(),
                ],
                vec![
                    "v1 generates PLCopen XML + adapter diagnostics, not native .L5X output.".to_string(),
                    "AOI internals, safety signatures, and controller module metadata are not generated.".to_string(),
                    "Retentive/runtime startup semantics require manual validation on target hardware.".to_string(),
                ],
            )
        }
        PlcopenExportTarget::Siemens => {
            diagnostics.push(export_adapter_diagnostic(
                "PLCO7SI0",
                "info",
                format!(
                    "Generated Siemens adapter artifact from {} source file(s) and {} ST declaration(s).",
                    context.source_count,
                    context.pou_count + context.data_type_count
                ),
                "Use the adapter report as the import checklist for TIA Portal migration.",
            ));
            if has_project_model {
                diagnostics.push(export_adapter_diagnostic(
                    "PLCO7SI1",
                    "warning",
                    "Configuration/resource/task/program bindings require manual OB/task mapping in TIA Portal.",
                    "Map PLCopen tasks to cyclic/event OBs and bind program instances explicitly.",
                ));
            }
            if context.analysis.has_direct_address_markers {
                diagnostics.push(export_adapter_diagnostic(
                    "PLCO7SI2",
                    "warning",
                    "Detected direct `%I/%Q/%M` addressing markers.",
                    "Reconcile address markers with TIA memory areas and hardware configuration manually.",
                ));
            }
            if context.analysis.has_rockwell_aliases {
                diagnostics.push(export_adapter_diagnostic(
                    "PLCO7SI3",
                    "warning",
                    "Detected Rockwell-specific library aliases in ST sources.",
                    "Replace Rockwell aliases/functions with IEC or Siemens-native equivalents before import.",
                ));
            }

            (
                vec![
                    "Import generated `.scl` files from the Siemens SCL sidecar bundle via TIA Portal: External source files -> Add new external file.".to_string(),
                    "Generate blocks from each imported source file in TIA Portal.".to_string(),
                    "Map tasks/program instances to OB scheduling and call hierarchy in TIA Portal.".to_string(),
                    "Validate memory-marker and retentive data behavior against PLC commissioning tests.".to_string(),
                    "Run conformance plus project-specific smoke tests after migration.".to_string(),
                ],
                vec![
                    "v1 generates PLCopen XML + `.scl` source bundle + adapter diagnostics, not native .apXX project archives.".to_string(),
                    "Hardware topology, technology objects, and safety project metadata are not generated.".to_string(),
                    "Vendor library semantics beyond symbol-level mapping remain manual migration work.".to_string(),
                ],
            )
        }
        PlcopenExportTarget::Schneider => {
            diagnostics.push(export_adapter_diagnostic(
                "PLCO7SC0",
                "info",
                format!(
                    "Generated Schneider adapter artifact from {} source file(s) and {} ST declaration(s).",
                    context.source_count,
                    context.pou_count + context.data_type_count
                ),
                "Use the adapter report as the import checklist for EcoStruxure migration.",
            ));
            if has_project_model {
                diagnostics.push(export_adapter_diagnostic(
                    "PLCO7SC1",
                    "warning",
                    "Configuration/resource/task/program bindings require manual task scheduling setup in EcoStruxure.",
                    "Rebuild task classes and program assignment explicitly after import.",
                ));
            }
            if context.analysis.has_direct_address_markers {
                diagnostics.push(export_adapter_diagnostic(
                    "PLCO7SC2",
                    "warning",
                    "Detected direct `%I/%Q/%M` addressing markers.",
                    "Rebind addressing to controller I/O map and validate with target hardware mapping rules.",
                ));
            }
            if context.analysis.has_siemens_aliases || context.analysis.has_rockwell_aliases {
                diagnostics.push(export_adapter_diagnostic(
                    "PLCO7SC3",
                    "warning",
                    "Detected non-Schneider vendor aliases in ST sources.",
                    "Normalize aliases to IEC/Schneider-supported equivalents before import.",
                ));
            }
            if context.analysis.has_retain_keyword {
                diagnostics.push(export_adapter_diagnostic(
                    "PLCO7SC4",
                    "warning",
                    "Detected RETAIN usage that may need explicit persistence configuration.",
                    "Verify retained variable classes and persistence files in EcoStruxure runtime settings.",
                ));
            }

            (
                vec![
                    "Import the generated PLCopen XML via the Schneider/CODESYS interchange path.".to_string(),
                    "Recreate task scheduling and program assignment in EcoStruxure project settings.".to_string(),
                    "Rebind hardware addresses and persistence settings before deployment.".to_string(),
                    "Run conformance and project integration tests after migration.".to_string(),
                ],
                vec![
                    "v1 generates PLCopen XML + adapter diagnostics, not native EcoStruxure project archives.".to_string(),
                    "Device-tree, bus topology, and safety metadata are not generated.".to_string(),
                    "Vendor-specific library internals remain manual migration work beyond symbol-level adaptation.".to_string(),
                ],
            )
        }
    };

    Some(PlcopenExportAdapterContract {
        diagnostics,
        manual_steps,
        limitations,
    })
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>()
}

#[cfg(test)]
fn is_element_named(node: roxmltree::Node<'_, '_>, name: &str) -> bool {
    node.is_element() && node.tag_name().name() == name
}

fn is_element_named_ci(node: roxmltree::Node<'_, '_>, name: &str) -> bool {
    node.is_element() && node.tag_name().name().eq_ignore_ascii_case(name)
}

fn attribute_ci(node: roxmltree::Node<'_, '_>, name: &str) -> Option<String> {
    node.attributes()
        .find(|attribute| attribute.name().eq_ignore_ascii_case(name))
        .map(|attribute| attribute.value().to_string())
}

fn extract_pou_name(node: roxmltree::Node<'_, '_>) -> Option<String> {
    attribute_ci(node, "name")
        .or_else(|| attribute_ci(node, "pouName"))
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .or_else(|| {
            node.children()
                .find(|child| is_element_named_ci(*child, "name"))
                .and_then(extract_text_content)
        })
}

fn extract_st_body(node: roxmltree::Node<'_, '_>) -> Option<String> {
    let body = node
        .children()
        .find(|child| is_element_named_ci(*child, "body"))?;
    for preferred in ["ST", "st", "text", "Text", "xhtml"] {
        if let Some(candidate) = body
            .descendants()
            .find(|entry| is_element_named_ci(*entry, preferred))
            .and_then(extract_text_content)
        {
            return Some(candidate);
        }
    }
    extract_text_content(body)
}

fn extract_text_content(node: roxmltree::Node<'_, '_>) -> Option<String> {
    let text = node
        .descendants()
        .filter(|entry| entry.is_text())
        .filter_map(|entry| entry.text())
        .collect::<String>();
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn collect_import_pou_nodes<'a, 'input>(
    root: roxmltree::Node<'a, 'input>,
) -> Vec<roxmltree::Node<'a, 'input>> {
    let mut standard = Vec::new();
    for types in root
        .children()
        .filter(|child| is_element_named_ci(*child, "types"))
    {
        for pous in types
            .children()
            .filter(|child| is_element_named_ci(*child, "pous"))
        {
            for pou in pous
                .children()
                .filter(|child| is_element_named_ci(*child, "pou"))
            {
                standard.push(pou);
            }
        }
    }

    if !standard.is_empty() {
        return standard;
    }

    root.descendants()
        .filter(|node| is_element_named_ci(*node, "pou"))
        .collect()
}

fn sanitize_path_segment(name: &str, fallback: &str) -> String {
    let mut segment = sanitize_filename(name.trim());
    while segment.starts_with('_') {
        segment.remove(0);
    }
    if segment.is_empty() {
        fallback.to_string()
    } else {
        segment
    }
}

fn extract_object_id_from_node(node: roxmltree::Node<'_, '_>) -> Option<String> {
    for data in node
        .descendants()
        .filter(|entry| is_element_named_ci(*entry, "data"))
    {
        let Some(name) = attribute_ci(data, "name") else {
            continue;
        };
        if !name.to_ascii_lowercase().contains("objectid")
            && !name.eq_ignore_ascii_case(CODESYS_OBJECT_ID_DATA_NAME)
        {
            continue;
        }
        if let Some(object_id_node) = data
            .descendants()
            .find(|entry| is_element_named_ci(*entry, "ObjectId"))
            .or_else(|| {
                data.descendants()
                    .find(|entry| is_element_named_ci(*entry, "objectId"))
            })
        {
            if let Some(text) = extract_text_content(object_id_node) {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
        if let Some(text) = extract_text_content(data) {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

fn parse_codesys_project_structure(root: roxmltree::Node<'_, '_>) -> CodesysProjectStructureMap {
    fn walk_object(
        node: roxmltree::Node<'_, '_>,
        parent_path: &[String],
        object_paths_by_id: &mut BTreeMap<String, Vec<String>>,
        object_paths_by_name: &mut BTreeMap<String, Vec<Vec<String>>>,
        object_count: &mut usize,
    ) {
        let name = attribute_ci_any(&node, &["Name", "name"])
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "Object".to_string());
        let mut path = parent_path.to_vec();
        path.push(sanitize_path_segment(&name, "Object"));
        *object_count += 1;

        if let Some(object_id) = attribute_ci_any(&node, &["ObjectId", "objectId", "id"])
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        {
            object_paths_by_id.insert(object_id, path.clone());
        }

        let key = name.trim().to_ascii_lowercase();
        object_paths_by_name
            .entry(key)
            .or_default()
            .push(path.clone());

        for child in node
            .children()
            .filter(|entry| is_element_named_ci(*entry, "Object"))
        {
            walk_object(
                child,
                &path,
                object_paths_by_id,
                object_paths_by_name,
                object_count,
            );
        }
    }

    let mut object_paths_by_id = BTreeMap::new();
    let mut object_paths_by_name: BTreeMap<String, Vec<Vec<String>>> = BTreeMap::new();
    let mut object_count = 0usize;

    for data in root
        .descendants()
        .filter(|node| is_element_named_ci(*node, "data"))
    {
        let Some(name) = attribute_ci(data, "name") else {
            continue;
        };
        if !name.to_ascii_lowercase().contains("projectstructure")
            && !name.eq_ignore_ascii_case(CODESYS_PROJECTSTRUCTURE_DATA_NAME)
        {
            continue;
        }
        for project_structure in data
            .descendants()
            .filter(|node| is_element_named_ci(*node, "ProjectStructure"))
        {
            for object in project_structure
                .children()
                .filter(|child| is_element_named_ci(*child, "Object"))
            {
                walk_object(
                    object,
                    &[],
                    &mut object_paths_by_id,
                    &mut object_paths_by_name,
                    &mut object_count,
                );
            }
        }
    }

    let unique_object_paths_by_name = object_paths_by_name
        .into_iter()
        .filter_map(|(name, paths)| {
            if paths.len() == 1 {
                Some((name, paths[0].clone()))
            } else {
                None
            }
        })
        .collect();

    CodesysProjectStructureMap {
        object_paths_by_id,
        unique_object_paths_by_name,
        object_count,
    }
}

fn resolve_codesys_folder_segments_for_node(
    node: roxmltree::Node<'_, '_>,
    fallback_name: &str,
    project_structure_map: &CodesysProjectStructureMap,
) -> Vec<String> {
    if let Some(object_id) = extract_object_id_from_node(node) {
        if let Some(path) = project_structure_map.object_paths_by_id.get(&object_id) {
            if path.len() >= 2 {
                return path[..path.len() - 1].to_vec();
            }
        }
    }

    let name_key = fallback_name.trim().to_ascii_lowercase();
    if let Some(path) = project_structure_map
        .unique_object_paths_by_name
        .get(&name_key)
    {
        if path.len() >= 2 {
            return path[..path.len() - 1].to_vec();
        }
    }

    Vec::new()
}

#[derive(Debug, Clone)]
struct InterfaceVarSection {
    keyword: &'static str,
    declarations: Vec<String>,
}

#[derive(Debug, Clone, Default)]
struct PouInterfaceMetadata {
    function_return_type: Option<String>,
    header_hint: Option<String>,
    sections: Vec<InterfaceVarSection>,
}

impl PouInterfaceMetadata {
    fn has_details(&self) -> bool {
        self.function_return_type.is_some()
            || self.header_hint.is_some()
            || self
                .sections
                .iter()
                .any(|section| !section.declarations.is_empty())
    }
}

fn detect_program_pous_used_as_types(root: roxmltree::Node<'_, '_>) -> HashSet<String> {
    let mut program_names = HashSet::new();
    for pou in root
        .descendants()
        .filter(|node| is_element_named_ci(*node, "pou"))
    {
        let Some(pou_name) = extract_pou_name(pou) else {
            continue;
        };
        let Some(raw_type) = attribute_ci(pou, "pouType").or_else(|| attribute_ci(pou, "type"))
        else {
            continue;
        };
        if PlcopenPouType::from_xml(&raw_type).is_some_and(|kind| kind == PlcopenPouType::Program) {
            program_names.insert(pou_name.to_ascii_lowercase());
        }
    }

    if program_names.is_empty() {
        return HashSet::new();
    }

    let mut promoted = HashSet::new();
    for derived in root
        .descendants()
        .filter(|node| is_element_named_ci(*node, "derived"))
    {
        let Some(name) = attribute_ci(derived, "name")
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        if program_names.contains(&name) {
            promoted.insert(name);
        }
    }
    promoted
}

fn synthesize_import_pou_source(
    pou_node: roxmltree::Node<'_, '_>,
    pou_type: PlcopenPouType,
    pou_name: &str,
    st_body: Option<&str>,
    warnings: &mut Vec<String>,
    unsupported_diagnostics: &mut Vec<PlcopenUnsupportedDiagnostic>,
) -> Option<String> {
    let normalized_body = st_body.map(normalize_body_text).unwrap_or_default();
    let has_body = !normalized_body.trim().is_empty();
    if has_body && source_has_top_level_pou_declaration(&normalized_body, pou_type) {
        return Some(normalized_body);
    }

    let metadata = extract_pou_interface_metadata(pou_node, pou_type);
    if !has_body && !metadata.has_details() {
        return None;
    }

    let header = render_import_pou_header(
        pou_type,
        pou_name,
        &metadata,
        warnings,
        unsupported_diagnostics,
    )?;
    let mut synthesized = String::new();
    synthesized.push_str(&header);
    synthesized.push('\n');

    for section in &metadata.sections {
        if section.declarations.is_empty() {
            continue;
        }
        synthesized.push_str(section.keyword);
        synthesized.push('\n');
        for declaration in &section.declarations {
            synthesized.push_str(declaration);
            synthesized.push('\n');
        }
        synthesized.push_str("END_VAR\n");
    }

    if has_body {
        synthesized.push_str(normalized_body.trim_end());
        synthesized.push('\n');
        warnings.push(format!(
            "pou '{}' body omitted a top-level declaration wrapper; synthesized '{}'",
            pou_name,
            pou_type.declaration_keyword()
        ));
        unsupported_diagnostics.push(unsupported_diagnostic(
            "PLCO207",
            "info",
            "pou/body/ST",
            "POU ST body lacked declaration wrapper; importer synthesized one",
            Some(pou_name.to_string()),
            "Review synthesized declaration sections for vendor-specific details",
        ));
    } else {
        warnings.push(format!(
            "pou '{}' had missing/empty ST body; synthesized declaration shell from interface metadata",
            pou_name
        ));
        unsupported_diagnostics.push(unsupported_diagnostic(
            "PLCO208",
            "info",
            "pou/interface",
            "POU body missing or empty; importer synthesized a declaration shell",
            Some(pou_name.to_string()),
            "Manual body implementation may still be required after import",
        ));
    }

    if pou_type == PlcopenPouType::Function
        && !function_result_assignment_present(&synthesized, pou_name)
    {
        synthesized.push_str(&format!("{pou_name} := {pou_name};\n"));
        warnings.push(format!(
            "function '{}' lacked an explicit result assignment; inserted default self-assignment",
            pou_name
        ));
        unsupported_diagnostics.push(unsupported_diagnostic(
            "PLCO212",
            "info",
            "pou/body/ST",
            "Function body lacked explicit result assignment; importer inserted a default self-assignment",
            Some(pou_name.to_string()),
            "Review the inserted assignment and replace it with domain-specific return logic",
        ));
    }

    synthesized.push_str(pou_type.end_keyword());
    synthesized.push('\n');
    Some(synthesized)
}

fn source_has_top_level_pou_declaration(source: &str, pou_type: PlcopenPouType) -> bool {
    let parsed = parser::parse(source);
    parsed
        .syntax()
        .children()
        .any(|node| node_to_pou_type(&node).is_some_and(|candidate| candidate == pou_type))
}

fn extract_pou_interface_metadata(
    pou_node: roxmltree::Node<'_, '_>,
    _pou_type: PlcopenPouType,
) -> PouInterfaceMetadata {
    let mut metadata = PouInterfaceMetadata::default();

    if let Some(interface) = first_child_element_ci(pou_node, "interface") {
        metadata.function_return_type = first_child_element_ci(interface, "returnType")
            .and_then(parse_type_expression_container)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        let section_defs = [
            ("inputVars", "VAR_INPUT"),
            ("outputVars", "VAR_OUTPUT"),
            ("inOutVars", "VAR_IN_OUT"),
            ("externalVars", "VAR_EXTERNAL"),
            ("localVars", "VAR"),
            ("tempVars", "VAR_TEMP"),
        ];
        for (xml_name, st_keyword) in section_defs {
            let mut declarations = Vec::new();
            for section in interface
                .children()
                .filter(|child| is_element_named_ci(*child, xml_name))
            {
                declarations.extend(parse_interface_var_declarations(section));
            }
            if !declarations.is_empty() {
                metadata.sections.push(InterfaceVarSection {
                    keyword: st_keyword,
                    declarations,
                });
            }
        }
    }

    metadata.header_hint = extract_interface_plaintext_header(pou_node);
    metadata
}

fn parse_interface_var_declarations(section: roxmltree::Node<'_, '_>) -> Vec<String> {
    let mut declarations = Vec::new();
    for variable in section
        .children()
        .filter(|child| is_element_named_ci(*child, "variable"))
    {
        let Some(name) = attribute_ci(variable, "name")
            .or_else(|| {
                variable
                    .children()
                    .find(|child| is_element_named_ci(*child, "name"))
                    .and_then(extract_text_content)
            })
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let Some(type_expr) = first_child_element_ci(variable, "type")
            .and_then(parse_type_expression_container)
            .or_else(|| {
                first_child_element_ci(variable, "baseType")
                    .and_then(parse_type_expression_container)
            })
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let initializer = first_child_element_ci(variable, "initialValue")
            .and_then(parse_initial_value)
            .map_or_else(String::new, |value| format!(" := {value}"));
        declarations.push(format!("    {name} : {type_expr}{initializer};"));
    }
    declarations
}

fn render_import_pou_header(
    pou_type: PlcopenPouType,
    pou_name: &str,
    metadata: &PouInterfaceMetadata,
    warnings: &mut Vec<String>,
    unsupported_diagnostics: &mut Vec<PlcopenUnsupportedDiagnostic>,
) -> Option<String> {
    match pou_type {
        PlcopenPouType::Program => Some(format!("PROGRAM {pou_name}")),
        PlcopenPouType::FunctionBlock => Some(format!("FUNCTION_BLOCK {pou_name}")),
        PlcopenPouType::Function => {
            if let Some(return_type) = metadata.function_return_type.as_deref() {
                return Some(format!("FUNCTION {pou_name} : {}", return_type.trim()));
            }
            if let Some(return_type) = metadata
                .header_hint
                .as_deref()
                .and_then(parse_function_return_type_from_header)
            {
                return Some(format!("FUNCTION {pou_name} : {}", return_type.trim()));
            }
            warnings.push(format!(
                "function '{}' did not provide an importable return type; defaulting to INT",
                pou_name
            ));
            unsupported_diagnostics.push(unsupported_diagnostic(
                "PLCO211",
                "warning",
                "pou/interface/returnType",
                "Function return type missing in PLCopen interface metadata; defaulted to INT",
                Some(pou_name.to_string()),
                "Review the imported FUNCTION signature and adjust the return type manually",
            ));
            Some(format!("FUNCTION {pou_name} : INT"))
        }
    }
}

fn extract_interface_plaintext_header(pou_node: roxmltree::Node<'_, '_>) -> Option<String> {
    const POU_PREFIXES: [&str; 3] = ["PROGRAM ", "FUNCTION_BLOCK ", "FUNCTION "];
    for data in pou_node
        .descendants()
        .filter(|node| is_element_named_ci(*node, "data"))
    {
        let Some(name) = attribute_ci(data, "name") else {
            continue;
        };
        if !name.to_ascii_lowercase().contains("interfaceasplaintext") {
            continue;
        }
        let Some(text) = extract_text_content(data) else {
            continue;
        };
        for line in text.lines() {
            let trimmed = line.trim();
            let upper = trimmed.to_ascii_uppercase();
            if POU_PREFIXES.iter().any(|prefix| upper.starts_with(prefix)) {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

fn parse_function_return_type_from_header(header: &str) -> Option<String> {
    let (_, suffix) = header.split_once(':')?;
    let return_type = suffix.trim().trim_end_matches(';').trim().to_string();
    if return_type.is_empty() {
        None
    } else {
        Some(return_type)
    }
}

fn function_result_assignment_present(source: &str, function_name: &str) -> bool {
    let target = function_name.trim();
    if target.is_empty() {
        return false;
    }

    let mut in_block_comment = false;
    for line in source.lines() {
        let mut text = line.to_string();

        if in_block_comment {
            if let Some(end) = text.find("*)") {
                text = text[end + 2..].to_string();
                in_block_comment = false;
            } else {
                continue;
            }
        }

        while let Some(start) = text.find("(*") {
            if let Some(end_rel) = text[start + 2..].find("*)") {
                let end = start + 2 + end_rel;
                text.replace_range(start..end + 2, "");
            } else {
                text.truncate(start);
                in_block_comment = true;
                break;
            }
        }

        if let Some(comment) = text.find("//") {
            text.truncate(comment);
        }

        let trimmed = text.trim();
        if trimmed.is_empty() {
            continue;
        }

        let Some((lhs, _rhs)) = trimmed.split_once(":=") else {
            continue;
        };
        if lhs.trim().eq_ignore_ascii_case(target) {
            return true;
        }
    }

    false
}

fn inject_required_var_external_declarations(
    source: &str,
    externals: &[QualifiedGlobalListExternalDecl],
) -> (String, Vec<String>) {
    if externals.is_empty() {
        return (source.to_string(), Vec::new());
    }

    let mut required = Vec::new();
    for external in externals {
        if source_uses_qualified_global_list(source, &external.list_name)
            && !source_has_var_external_symbol(source, &external.list_name)
        {
            required.push(external.clone());
        }
    }

    if required.is_empty() {
        return (source.to_string(), Vec::new());
    }

    let lines = source.lines().collect::<Vec<_>>();
    if lines.is_empty() {
        return (source.to_string(), Vec::new());
    }
    let insert_at = find_var_section_insertion_index(&lines);

    let mut rendered = String::new();
    for (index, line) in lines.iter().enumerate() {
        if index == insert_at {
            rendered.push_str("VAR_EXTERNAL\n");
            for external in &required {
                rendered.push_str(&format!(
                    "    {} : {};\n",
                    external.list_name, external.type_name
                ));
            }
            rendered.push_str("END_VAR\n");
        }
        rendered.push_str(line);
        rendered.push('\n');
    }
    if insert_at >= lines.len() {
        rendered.push_str("VAR_EXTERNAL\n");
        for external in &required {
            rendered.push_str(&format!(
                "    {} : {};\n",
                external.list_name, external.type_name
            ));
        }
        rendered.push_str("END_VAR\n");
    }

    let inserted = required.into_iter().map(|decl| decl.list_name).collect();
    (rendered, inserted)
}

fn source_uses_qualified_global_list(source: &str, list_name: &str) -> bool {
    if list_name.trim().is_empty() {
        return false;
    }
    let lowered_source = source.to_ascii_lowercase();
    let needle = format!("{}.", list_name.trim().to_ascii_lowercase());

    for (index, _) in lowered_source.match_indices(&needle) {
        if index == 0 {
            return true;
        }
        let prev = lowered_source[..index]
            .chars()
            .next_back()
            .unwrap_or_default();
        if !is_identifier_char(prev) {
            return true;
        }
    }

    false
}

fn source_has_var_external_symbol(source: &str, symbol_name: &str) -> bool {
    let target = symbol_name.trim();
    if target.is_empty() {
        return false;
    }

    let mut in_external = false;
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.to_ascii_uppercase().starts_with("VAR_EXTERNAL") {
            in_external = true;
            continue;
        }
        if in_external && trimmed.eq_ignore_ascii_case("END_VAR") {
            in_external = false;
            continue;
        }
        if !in_external {
            continue;
        }

        let Some((lhs, _rhs)) = trimmed.trim_end_matches(';').split_once(':') else {
            continue;
        };
        let name = lhs
            .split(',')
            .next()
            .map(str::trim)
            .unwrap_or_default()
            .split_whitespace()
            .next()
            .unwrap_or_default();
        if name.eq_ignore_ascii_case(target) {
            return true;
        }
    }

    false
}

fn find_var_section_insertion_index(lines: &[&str]) -> usize {
    if lines.is_empty() {
        return 0;
    }

    let mut index = 1usize;
    while index < lines.len() {
        let trimmed = lines[index].trim();
        if trimmed.is_empty() {
            index += 1;
            continue;
        }
        if is_var_section_header(trimmed) {
            index += 1;
            while index < lines.len() && !lines[index].trim().eq_ignore_ascii_case("END_VAR") {
                index += 1;
            }
            if index < lines.len() {
                index += 1;
            }
            continue;
        }
        break;
    }

    index
}

fn is_var_section_header(line: &str) -> bool {
    let upper = line.trim().to_ascii_uppercase();
    upper == "VAR" || upper.starts_with("VAR_")
}

fn is_identifier_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

fn unique_source_path(
    sources_root: &Path,
    base_name: &str,
    seen_files: &mut HashSet<PathBuf>,
) -> PathBuf {
    unique_source_path_with_segments(sources_root, &[], base_name, seen_files)
}

fn unique_source_path_with_segments(
    sources_root: &Path,
    folder_segments: &[String],
    base_name: &str,
    seen_files: &mut HashSet<PathBuf>,
) -> PathBuf {
    let mut file_name = sanitize_filename(base_name);
    if file_name.is_empty() {
        file_name = "unnamed".to_string();
    }
    let mut directory = sources_root.to_path_buf();
    for segment in folder_segments {
        directory = directory.join(sanitize_path_segment(segment, "folder"));
    }

    let mut candidate = directory.join(format!("{file_name}.st"));
    let mut duplicate_index = 2usize;
    while !seen_files.insert(candidate.clone()) {
        candidate = directory.join(format!("{file_name}_{duplicate_index}.st"));
        duplicate_index += 1;
    }
    candidate
}

fn write_text_file_with_parents(path: &Path, text: &str) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create '{}'", parent.display()))?;
    }
    std::fs::write(path, text).with_context(|| format!("failed to write '{}'", path.display()))?;
    Ok(())
}

fn track_imported_folder_path(
    file_path: &Path,
    sources_root: &Path,
    imported_folder_paths: &mut HashSet<String>,
) {
    let Some(parent) = file_path.parent() else {
        return;
    };
    let Ok(relative) = parent.strip_prefix(sources_root) else {
        return;
    };
    if relative.as_os_str().is_empty() {
        return;
    }
    imported_folder_paths.insert(relative.to_string_lossy().replace('\\', "/"));
}

fn append_indent(xml: &mut String, spaces: usize) {
    for _ in 0..spaces {
        xml.push(' ');
    }
}

fn append_task_xml(xml: &mut String, task: &TaskDecl, indent: usize) {
    append_indent(xml, indent);
    xml.push_str(&format!("<task name=\"{}\"", escape_xml_attr(&task.name)));
    if let Some(interval) = &task.interval {
        xml.push_str(&format!(" interval=\"{}\"", escape_xml_attr(interval)));
    }
    if let Some(single) = &task.single {
        xml.push_str(&format!(" single=\"{}\"", escape_xml_attr(single)));
    }
    if let Some(priority) = &task.priority {
        xml.push_str(&format!(" priority=\"{}\"", escape_xml_attr(priority)));
    }
    xml.push_str(" />\n");
}

fn append_program_instance_xml(xml: &mut String, program: &ProgramBindingDecl, indent: usize) {
    append_indent(xml, indent);
    xml.push_str(&format!(
        "<pouInstance name=\"{}\" typeName=\"{}\"",
        escape_xml_attr(&program.instance_name),
        escape_xml_attr(&program.type_name)
    ));
    if let Some(task_name) = &program.task_name {
        xml.push_str(&format!(" task=\"{}\"", escape_xml_attr(task_name)));
    }
    xml.push_str(" />\n");
}

fn source_folder_segments_for_codesys(path: &Path) -> Vec<String> {
    let mut segments = path
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

    if segments
        .first()
        .is_some_and(|segment| segment.eq_ignore_ascii_case("src"))
    {
        segments.remove(0);
    }
    if segments
        .first()
        .is_some_and(|segment| segment.eq_ignore_ascii_case("Application"))
    {
        segments.remove(0);
    }
    if !segments.is_empty() {
        segments.pop();
    }

    segments
        .into_iter()
        .map(|segment| sanitize_path_segment(&segment, "folder"))
        .collect()
}

fn deterministic_codesys_object_id(kind: &str, name: &str, folder_segments: &[String]) -> String {
    let seed = format!(
        "{kind}:{}:{}",
        folder_segments.join("/"),
        name.to_ascii_lowercase()
    );
    let h1 = crc32fast::hash(format!("a:{seed}").as_bytes());
    let h2 = crc32fast::hash(format!("b:{seed}").as_bytes());
    let h3 = crc32fast::hash(format!("c:{seed}").as_bytes());
    let h4 = crc32fast::hash(format!("d:{seed}").as_bytes());
    let tail = ((u64::from(h3 & 0xffff)) << 32) | u64::from(h4);
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        h1,
        (h2 >> 16) as u16,
        (h2 & 0xffff) as u16,
        (h3 >> 16) as u16,
        tail & 0x0000_ffff_ffff_ffff
    )
}

fn build_codesys_project_structure_tree(
    entries: &[CodesysExportObjectEntry],
) -> CodesysProjectObjectNode {
    #[derive(Default)]
    struct NodeBuilder {
        object_id: String,
        children: BTreeMap<String, NodeBuilder>,
    }

    fn insert_entry(node: &mut NodeBuilder, key_segments: &[String], object_id: &str) {
        if key_segments.is_empty() {
            return;
        }
        let key = key_segments[0].clone();
        let child = node.children.entry(key).or_default();
        if key_segments.len() == 1 {
            child.object_id = object_id.to_string();
            return;
        }
        insert_entry(child, &key_segments[1..], object_id);
    }

    fn finalize_node(
        name: &str,
        builder: NodeBuilder,
        parent_path: &[String],
    ) -> CodesysProjectObjectNode {
        let mut path = parent_path.to_vec();
        path.push(name.to_string());
        let children = builder
            .children
            .into_iter()
            .map(|(child_name, child_builder)| finalize_node(&child_name, child_builder, &path))
            .collect::<Vec<_>>();
        let object_id = if builder.object_id.is_empty() {
            deterministic_codesys_object_id("folder", name, &path)
        } else {
            builder.object_id
        };
        CodesysProjectObjectNode {
            name: name.to_string(),
            object_id,
            children,
        }
    }

    let mut root_builder = NodeBuilder {
        object_id: deterministic_codesys_object_id("root", "Application", &[]),
        children: BTreeMap::new(),
    };

    for entry in entries {
        let mut path = entry.folder_segments.clone();
        path.push(entry.name.clone());
        insert_entry(&mut root_builder, &path, &entry.object_id);
    }

    finalize_node("Application", root_builder, &[])
}

fn count_project_structure_nodes(node: &CodesysProjectObjectNode) -> usize {
    1 + node
        .children
        .iter()
        .map(count_project_structure_nodes)
        .sum::<usize>()
}

fn build_codesys_export_metadata(
    declarations: &[PouDecl],
    global_var_lists: &[GlobalVarDecl],
    warnings: &mut Vec<String>,
) -> CodesysExportMetadata {
    let pou_entries = declarations
        .iter()
        .map(|decl| {
            let source_path = PathBuf::from(&decl.source);
            let folder_segments = source_folder_segments_for_codesys(&source_path);
            let object_id = deterministic_codesys_object_id("pou", &decl.name, &folder_segments);
            CodesysExportObjectEntry {
                name: decl.name.clone(),
                object_id,
                folder_segments,
            }
        })
        .collect::<Vec<_>>();

    let mut global_entries = Vec::new();
    for decl in global_var_lists {
        let folder_segments = source_folder_segments_for_codesys(&decl.source_path);
        let object_id = deterministic_codesys_object_id("gvl", &decl.name, &folder_segments);
        if decl.variables.is_empty() {
            warnings.push(format!(
                "{}:{} global list '{}' has no parseable declaration entries; exporting plaintext-only metadata",
                decl.source, decl.line, decl.name
            ));
        }
        global_entries.push((
            decl.clone(),
            CodesysExportObjectEntry {
                name: decl.name.clone(),
                object_id,
                folder_segments,
            },
        ));
    }

    let mut all_entries = pou_entries.clone();
    all_entries.extend(global_entries.iter().map(|(_, entry)| entry.clone()));
    let project_structure_root = build_codesys_project_structure_tree(&all_entries);
    let exported_project_structure_nodes = count_project_structure_nodes(&project_structure_root);

    let mut folder_paths = all_entries
        .iter()
        .filter(|entry| !entry.folder_segments.is_empty())
        .map(|entry| entry.folder_segments.join("/"))
        .collect::<HashSet<_>>();
    if !all_entries.is_empty() {
        folder_paths.insert("Application".to_string());
    }
    let exported_folder_paths = folder_paths.len();

    CodesysExportMetadata {
        global_var_lists: global_entries,
        pou_entries,
        project_structure_root,
        exported_project_structure_nodes,
        exported_folder_paths,
    }
}

fn append_codesys_project_structure_node(
    xml: &mut String,
    node: &CodesysProjectObjectNode,
    indent: usize,
) {
    append_indent(xml, indent);
    xml.push_str(&format!(
        "<Object Name=\"{}\" ObjectId=\"{}\"",
        escape_xml_attr(&node.name),
        escape_xml_attr(&node.object_id)
    ));
    if node.children.is_empty() {
        xml.push_str(" />\n");
        return;
    }

    xml.push_str(">\n");
    for child in &node.children {
        append_codesys_project_structure_node(xml, child, indent + 2);
    }
    append_indent(xml, indent);
    xml.push_str("</Object>\n");
}

fn append_codesys_export_add_data(
    xml: &mut String,
    metadata: &CodesysExportMetadata,
    warnings: &mut Vec<String>,
) {
    if metadata.pou_entries.is_empty() && metadata.global_var_lists.is_empty() {
        return;
    }

    if !metadata.global_var_lists.is_empty() {
        xml.push_str(&format!(
            "    <data name=\"{}\" handleUnknown=\"implementation\">\n",
            CODESYS_APPLICATION_DATA_NAME
        ));
        xml.push_str("      <resource name=\"Application\">\n");
        for (decl, object_entry) in &metadata.global_var_lists {
            xml.push_str(&format!(
                "        <globalVars name=\"{}\">\n",
                escape_xml_attr(&decl.name)
            ));
            for variable in &decl.variables {
                xml.push_str(&format!(
                    "          <variable name=\"{}\">\n",
                    escape_xml_attr(&variable.name)
                ));
                xml.push_str("            <type>\n");
                if let Some(type_xml) =
                    type_expression_to_plcopen_base_type_xml(&variable.type_expr)
                {
                    for line in type_xml.lines() {
                        xml.push_str("              ");
                        xml.push_str(line);
                        xml.push('\n');
                    }
                } else {
                    warnings.push(format!(
                        "{}:{} unsupported global type '{}' in '{}'; exported as derived",
                        decl.source, decl.line, variable.type_expr, variable.name
                    ));
                    xml.push_str(&format!(
                        "              <derived name=\"{}\" />\n",
                        escape_xml_attr(&variable.type_expr)
                    ));
                }
                xml.push_str("            </type>\n");
                if let Some(initial_value) = variable
                    .initial_value
                    .as_ref()
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty())
                {
                    xml.push_str("            <initialValue>\n");
                    xml.push_str(&format!(
                        "              <simpleValue value=\"{}\" />\n",
                        escape_xml_attr(&initial_value)
                    ));
                    xml.push_str("            </initialValue>\n");
                }
                xml.push_str("          </variable>\n");
            }
            xml.push_str("          <addData>\n");
            xml.push_str(&format!(
                "            <data name=\"{}\" handleUnknown=\"implementation\">\n",
                CODESYS_INTERFACE_PLAINTEXT_DATA_NAME
            ));
            xml.push_str("              <InterfaceAsPlainText>\n");
            xml.push_str(&format!(
                "                <xhtml xmlns=\"http://www.w3.org/1999/xhtml\">{}</xhtml>\n",
                escape_xml_attr(&decl.body)
            ));
            xml.push_str("              </InterfaceAsPlainText>\n");
            xml.push_str("            </data>\n");
            xml.push_str(&format!(
                "            <data name=\"{}\" handleUnknown=\"discard\">\n",
                CODESYS_OBJECT_ID_DATA_NAME
            ));
            xml.push_str(&format!(
                "              <ObjectId>{}</ObjectId>\n",
                escape_xml_attr(&object_entry.object_id)
            ));
            xml.push_str("            </data>\n");
            xml.push_str("          </addData>\n");
            xml.push_str("        </globalVars>\n");
        }
        xml.push_str("      </resource>\n");
        xml.push_str("    </data>\n");
    }

    xml.push_str(&format!(
        "    <data name=\"{}\" handleUnknown=\"discard\">\n",
        CODESYS_PROJECTSTRUCTURE_DATA_NAME
    ));
    xml.push_str("      <ProjectStructure>\n");
    append_codesys_project_structure_node(xml, &metadata.project_structure_root, 8);
    xml.push_str("      </ProjectStructure>\n");
    xml.push_str("    </data>\n");
}

fn extract_data_type_declarations(source: &LoadedSource) -> (Vec<DataTypeDecl>, Vec<String>) {
    let mut declarations = Vec::new();
    let mut warnings = Vec::new();
    let lines = source.text.lines().collect::<Vec<_>>();
    let mut line_index = 0usize;

    while line_index < lines.len() {
        if !lines[line_index].trim().eq_ignore_ascii_case("TYPE") {
            line_index += 1;
            continue;
        }

        line_index += 1;
        let mut declaration_text = String::new();
        let mut declaration_start_line = line_index + 1;
        let mut struct_depth = 0usize;

        while line_index < lines.len() {
            let raw_line = lines[line_index];
            let trimmed = raw_line.trim();

            if trimmed.eq_ignore_ascii_case("END_TYPE") {
                if !declaration_text.trim().is_empty() {
                    warnings.push(format!(
                        "{}:{} unfinished TYPE declaration skipped during PLCopen export",
                        source.path.display(),
                        declaration_start_line
                    ));
                }
                break;
            }

            if trimmed.is_empty() {
                line_index += 1;
                continue;
            }

            if declaration_text.trim().is_empty() {
                declaration_start_line = line_index + 1;
            }

            if !declaration_text.is_empty() {
                declaration_text.push('\n');
            }
            declaration_text.push_str(raw_line.trim_end());

            let upper = trimmed.to_ascii_uppercase();
            if upper.contains(": STRUCT") || upper == "STRUCT" {
                struct_depth = struct_depth.saturating_add(1);
            }
            if upper.contains("END_STRUCT") {
                struct_depth = struct_depth.saturating_sub(1);
            }

            if struct_depth == 0 && trimmed.ends_with(';') {
                if let Some((name, type_expr)) = parse_type_declaration_text(&declaration_text) {
                    declarations.push(DataTypeDecl {
                        name,
                        type_expr,
                        source: source.path.display().to_string(),
                        line: declaration_start_line,
                    });
                } else {
                    warnings.push(format!(
                        "{}:{} unsupported TYPE declaration skipped during PLCopen export",
                        source.path.display(),
                        declaration_start_line
                    ));
                }
                declaration_text.clear();
            }

            line_index += 1;
        }

        line_index += 1;
    }

    (declarations, warnings)
}

fn parse_type_declaration_text(text: &str) -> Option<(String, String)> {
    let trimmed = text.trim();
    let colon = trimmed.find(':')?;
    let name = trimmed[..colon].trim().to_string();
    if name.is_empty() {
        return None;
    }
    let mut expr = trimmed[colon + 1..].trim().to_string();
    if expr.ends_with(';') {
        expr.pop();
    }
    let expr = expr.trim().to_string();
    if expr.is_empty() {
        None
    } else {
        Some((name, expr))
    }
}

fn extract_configuration_declarations(
    source: &LoadedSource,
) -> (Vec<ConfigurationDecl>, Vec<String>) {
    let mut declarations = Vec::new();
    let mut warnings = Vec::new();
    let lines = source.text.lines().collect::<Vec<_>>();
    let mut line_index = 0usize;

    while line_index < lines.len() {
        let line = lines[line_index];
        if !line
            .trim_start()
            .to_ascii_uppercase()
            .starts_with("CONFIGURATION ")
        {
            line_index += 1;
            continue;
        }

        let Some(name) = line
            .split_whitespace()
            .nth(1)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
        else {
            warnings.push(format!(
                "{}:{} CONFIGURATION declaration without name skipped",
                source.path.display(),
                line_index + 1
            ));
            line_index += 1;
            continue;
        };

        let mut configuration = ConfigurationDecl {
            name,
            tasks: Vec::new(),
            programs: Vec::new(),
            resources: Vec::new(),
        };
        line_index += 1;

        while line_index < lines.len() {
            let body_line = lines[line_index].trim();
            if body_line.eq_ignore_ascii_case("END_CONFIGURATION") {
                break;
            }

            if body_line.to_ascii_uppercase().starts_with("RESOURCE ") {
                let (resource_name, target) =
                    parse_resource_header(body_line).unwrap_or_else(|| {
                        (
                            format!("Resource{}", configuration.resources.len() + 1),
                            "CPU".to_string(),
                        )
                    });
                let mut resource = ResourceDecl {
                    name: resource_name,
                    target,
                    tasks: Vec::new(),
                    programs: Vec::new(),
                };
                line_index += 1;
                while line_index < lines.len() {
                    let resource_line = lines[line_index].trim();
                    if resource_line.eq_ignore_ascii_case("END_RESOURCE") {
                        break;
                    }
                    if let Some(task) = parse_task_declaration_line(resource_line) {
                        resource.tasks.push(task);
                    } else if let Some(program) = parse_program_binding_line(resource_line) {
                        resource.programs.push(program);
                    }
                    line_index += 1;
                }
                configuration.resources.push(resource);
            } else if let Some(task) = parse_task_declaration_line(body_line) {
                configuration.tasks.push(task);
            } else if let Some(program) = parse_program_binding_line(body_line) {
                configuration.programs.push(program);
            }

            line_index += 1;
        }

        declarations.push(configuration);
        line_index += 1;
    }

    (declarations, warnings)
}

fn parse_resource_header(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim().trim_end_matches(';');
    let mut parts = trimmed.split_whitespace();
    if !parts.next()?.eq_ignore_ascii_case("RESOURCE") {
        return None;
    }
    let name = parts.next()?.to_string();
    let mut target = "CPU".to_string();
    while let Some(token) = parts.next() {
        if token.eq_ignore_ascii_case("ON") {
            if let Some(value) = parts.next() {
                target = value.to_string();
            }
            break;
        }
    }
    Some((name, target))
}

fn parse_task_declaration_line(line: &str) -> Option<TaskDecl> {
    let trimmed = line.trim();
    if !trimmed.to_ascii_uppercase().starts_with("TASK ") {
        return None;
    }
    let no_suffix = trimmed.trim_end_matches(';');
    let rest = no_suffix.get(4..)?.trim();
    let task_name_end = rest
        .find(|ch: char| ch.is_whitespace() || ch == '(')
        .unwrap_or(rest.len());
    let name = rest[..task_name_end].trim();
    if name.is_empty() {
        return None;
    }

    let mut task = TaskDecl {
        name: name.to_string(),
        ..TaskDecl::default()
    };

    if let (Some(open), Some(close)) = (rest.find('('), rest.rfind(')')) {
        if close > open {
            let init = &rest[open + 1..close];
            for item in init.split(',') {
                let Some((key, value)) = item.split_once(":=") else {
                    continue;
                };
                let key = key.trim().to_ascii_uppercase();
                let value = value.trim();
                if value.is_empty() {
                    continue;
                }
                match key.as_str() {
                    "INTERVAL" => task.interval = Some(normalize_task_interval_literal(value)),
                    "SINGLE" => task.single = Some(value.to_string()),
                    "PRIORITY" => task.priority = Some(value.to_string()),
                    _ => {}
                }
            }
        }
    }

    Some(task)
}

fn normalize_task_interval_literal(value: &str) -> String {
    let trimmed = value.trim();
    let upper = trimmed.to_ascii_uppercase();
    if upper.starts_with("T#") || upper.starts_with("TIME#") || upper.starts_with("LTIME#") {
        return trimmed.to_string();
    }
    if upper.starts_with("PT") && upper.ends_with('S') {
        let number = &upper[2..upper.len() - 1];
        if let Ok(seconds) = number.parse::<f64>() {
            if seconds >= 1.0 && (seconds.fract() - 0.0).abs() < f64::EPSILON {
                return format!("T#{}s", seconds as u64);
            }
            return format!("T#{}ms", (seconds * 1000.0).round() as i64);
        }
    }
    if upper.starts_with("PT") && upper.ends_with("MS") {
        let number = &upper[2..upper.len() - 2];
        if let Ok(millis) = number.parse::<i64>() {
            return format!("T#{}ms", millis);
        }
    }
    trimmed.to_string()
}

fn parse_program_binding_line(line: &str) -> Option<ProgramBindingDecl> {
    let trimmed = line.trim();
    if !trimmed.to_ascii_uppercase().starts_with("PROGRAM ") {
        return None;
    }
    let mut rest = trimmed.trim_end_matches(';').get(7..)?.trim();
    if rest.to_ascii_uppercase().starts_with("RETAIN ") {
        rest = rest.get(7..)?.trim();
    } else if rest.to_ascii_uppercase().starts_with("NON_RETAIN ") {
        rest = rest.get(11..)?.trim();
    }
    let (lhs, rhs) = rest.split_once(':')?;
    let mut lhs_parts = lhs.split_whitespace();
    let instance_name = lhs_parts.next()?.trim().to_string();
    if instance_name.is_empty() {
        return None;
    }

    let mut task_name = None;
    while let Some(token) = lhs_parts.next() {
        if token.eq_ignore_ascii_case("WITH") {
            task_name = lhs_parts.next().map(ToOwned::to_owned);
            break;
        }
    }

    let rhs = rhs.trim();
    let type_name = rhs
        .split_once('(')
        .map_or(rhs, |(head, _)| head)
        .trim()
        .trim_end_matches(';')
        .to_string();
    if type_name.is_empty() {
        return None;
    }

    Some(ProgramBindingDecl {
        instance_name,
        task_name,
        type_name,
    })
}

fn type_expression_to_plcopen_base_type_xml(type_expr: &str) -> Option<String> {
    let trimmed = type_expr.trim();
    if trimmed.is_empty() {
        return None;
    }
    let upper = trimmed.to_ascii_uppercase();

    if upper.starts_with("ARRAY[") {
        return type_expr_array_to_xml(trimmed);
    }
    if upper.starts_with("STRUCT") {
        return type_expr_struct_to_xml(trimmed);
    }
    if trimmed.starts_with('(') && trimmed.ends_with(')') {
        return type_expr_enum_to_xml(trimmed);
    }
    if let Some(value) = type_expr_subrange_to_xml(trimmed) {
        return Some(value);
    }
    type_expr_simple_to_xml(trimmed)
}

fn type_expr_array_to_xml(type_expr: &str) -> Option<String> {
    let open = type_expr.find('[')?;
    let close = type_expr.find(']')?;
    if close <= open {
        return None;
    }
    let dims_text = type_expr[open + 1..close].trim();
    let base_text = type_expr[close + 1..].trim();
    let of_pos = base_text.to_ascii_uppercase().find("OF")?;
    let base_expr = base_text[of_pos + 2..].trim();
    let base_xml = type_expression_to_plcopen_base_type_xml(base_expr)?;

    let mut xml = String::from("<array>\n");
    for dimension in dims_text.split(',') {
        let (lower, upper) = dimension.split_once("..")?;
        xml.push_str(&format!(
            "  <dimension lower=\"{}\" upper=\"{}\"/>\n",
            escape_xml_attr(lower.trim()),
            escape_xml_attr(upper.trim())
        ));
    }
    xml.push_str("  <baseType>\n");
    for line in base_xml.lines() {
        xml.push_str("    ");
        xml.push_str(line);
        xml.push('\n');
    }
    xml.push_str("  </baseType>\n");
    xml.push_str("</array>");
    Some(xml)
}

fn type_expr_struct_to_xml(type_expr: &str) -> Option<String> {
    let upper = type_expr.to_ascii_uppercase();
    let end_index = upper.rfind("END_STRUCT")?;
    let body = type_expr.get("STRUCT".len()..end_index)?.trim();
    let mut xml = String::from("<struct>\n");

    for raw_line in body.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let line = line.trim_end_matches(';').trim();
        let (name, rhs) = line.split_once(':')?;
        let field_name = name.trim();
        if field_name.is_empty() {
            continue;
        }
        let (field_type, field_init) = match rhs.split_once(":=") {
            Some((type_part, init_part)) => (type_part.trim(), Some(init_part.trim())),
            None => (rhs.trim(), None),
        };
        let field_xml = type_expression_to_plcopen_base_type_xml(field_type)?;

        xml.push_str(&format!(
            "  <variable name=\"{}\">\n",
            escape_xml_attr(field_name)
        ));
        xml.push_str("    <type>\n");
        for line in field_xml.lines() {
            xml.push_str("      ");
            xml.push_str(line);
            xml.push('\n');
        }
        xml.push_str("    </type>\n");
        if let Some(initial_value) = field_init.filter(|value| !value.is_empty()) {
            xml.push_str("    <initialValue>\n");
            xml.push_str(&format!(
                "      <simpleValue value=\"{}\"/>\n",
                escape_xml_attr(initial_value)
            ));
            xml.push_str("    </initialValue>\n");
        }
        xml.push_str("  </variable>\n");
    }

    xml.push_str("</struct>");
    Some(xml)
}

fn type_expr_enum_to_xml(type_expr: &str) -> Option<String> {
    let inner = type_expr
        .trim()
        .strip_prefix('(')?
        .strip_suffix(')')?
        .trim();
    if inner.is_empty() {
        return None;
    }

    let mut xml = String::from("<enum>\n  <values>\n");
    for item in inner.split(',') {
        let value = item.trim();
        if value.is_empty() {
            continue;
        }
        if let Some((name, raw)) = value.split_once(":=") {
            xml.push_str(&format!(
                "    <value name=\"{}\" value=\"{}\"/>\n",
                escape_xml_attr(name.trim()),
                escape_xml_attr(raw.trim())
            ));
        } else {
            xml.push_str(&format!(
                "    <value name=\"{}\"/>\n",
                escape_xml_attr(value)
            ));
        }
    }
    xml.push_str("  </values>\n</enum>");
    Some(xml)
}

fn type_expr_subrange_to_xml(type_expr: &str) -> Option<String> {
    let open = type_expr.rfind('(')?;
    let close = type_expr.rfind(')')?;
    if close <= open {
        return None;
    }
    let base_expr = type_expr[..open].trim();
    let range = type_expr[open + 1..close].trim();
    let (lower, upper) = range.split_once("..")?;
    let base_xml = type_expression_to_plcopen_base_type_xml(base_expr)?;

    let mut xml = String::from(&format!(
        "<subrange lower=\"{}\" upper=\"{}\">\n",
        escape_xml_attr(lower.trim()),
        escape_xml_attr(upper.trim())
    ));
    xml.push_str("  <baseType>\n");
    for line in base_xml.lines() {
        xml.push_str("    ");
        xml.push_str(line);
        xml.push('\n');
    }
    xml.push_str("  </baseType>\n");
    xml.push_str("</subrange>");
    Some(xml)
}

fn type_expr_simple_to_xml(type_expr: &str) -> Option<String> {
    let trimmed = type_expr.trim();
    let upper = trimmed.to_ascii_uppercase();
    if upper.starts_with("STRING[") && upper.ends_with(']') {
        let length = trimmed[7..trimmed.len() - 1].trim();
        return Some(format!("<string length=\"{}\"/>", escape_xml_attr(length)));
    }
    if upper.starts_with("WSTRING[") && upper.ends_with(']') {
        let length = trimmed[8..trimmed.len() - 1].trim();
        return Some(format!("<wstring length=\"{}\"/>", escape_xml_attr(length)));
    }
    if upper == "STRING" {
        return Some("<string />".to_string());
    }
    if upper == "WSTRING" {
        return Some("<wstring />".to_string());
    }

    if is_elementary_type_tag(&upper.to_ascii_lowercase()) {
        return Some(format!("<{} />", upper.to_ascii_lowercase()));
    }

    if trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '.')
    {
        return Some(format!("<derived name=\"{}\"/>", escape_xml_attr(trimmed)));
    }
    None
}

fn import_data_types_to_sources(
    root: roxmltree::Node<'_, '_>,
    sources_root: &Path,
    seen_files: &mut HashSet<PathBuf>,
    warnings: &mut Vec<String>,
    unsupported_nodes: &mut Vec<String>,
    unsupported_diagnostics: &mut Vec<PlcopenUnsupportedDiagnostic>,
    loss_warnings: &mut usize,
) -> anyhow::Result<Option<(PathBuf, usize)>> {
    let mut declarations = Vec::new();
    let mut imported_count = 0usize;
    let mut seen_names = BTreeSet::new();

    for data_type in root
        .descendants()
        .filter(|node| is_element_named_ci(*node, "dataType"))
        .filter(|node| {
            node.ancestors()
                .any(|ancestor| is_element_named_ci(ancestor, "dataTypes"))
        })
    {
        let Some(name) = attribute_ci(data_type, "name")
            .or_else(|| {
                data_type
                    .children()
                    .find(|child| is_element_named_ci(*child, "name"))
                    .and_then(extract_text_content)
            })
            .map(|raw| raw.trim().to_string())
            .filter(|value| !value.is_empty())
        else {
            unsupported_nodes.push("types/dataTypes/unnamed".to_string());
            unsupported_diagnostics.push(unsupported_diagnostic(
                "PLCO401",
                "warning",
                "types/dataTypes/dataType",
                "dataType entry skipped because required name attribute is missing",
                None,
                "Provide a non-empty dataType name before import",
            ));
            *loss_warnings += 1;
            continue;
        };

        let name_key = name.to_ascii_lowercase();
        if !seen_names.insert(name_key) {
            unsupported_nodes.push(format!("types/dataTypes/{name}"));
            unsupported_diagnostics.push(unsupported_diagnostic(
                "PLCO403",
                "warning",
                format!("types/dataTypes/{name}"),
                format!("dataType '{}' skipped because the name is duplicated", name),
                None,
                "Rename duplicate dataType entries to unique names before import",
            ));
            *loss_warnings += 1;
            continue;
        }

        let Some(type_expr) = parse_data_type_expression(data_type) else {
            unsupported_nodes.push(format!("types/dataTypes/{name}"));
            unsupported_diagnostics.push(unsupported_diagnostic(
                "PLCO402",
                "warning",
                format!("types/dataTypes/{name}"),
                format!(
                    "dataType '{}' uses an unsupported or missing baseType representation",
                    name
                ),
                None,
                "Supported baseType subset: elementary, derived, array, struct, enum, subrange",
            ));
            *loss_warnings += 1;
            continue;
        };

        declarations.push(format_data_type_declaration(&name, &type_expr));
        imported_count += 1;
    }

    if imported_count == 0 {
        return Ok(None);
    }

    let mut source = String::from("TYPE\n");
    for declaration in declarations {
        source.push_str(&declaration);
        source.push('\n');
    }
    source.push_str("END_TYPE\n");

    let path = unique_source_path(sources_root, GENERATED_DATA_TYPES_SOURCE_PREFIX, seen_files);
    write_text_file_with_parents(&path, &source)?;

    warnings.push(format!(
        "imported {} PLCopen dataType declaration(s) into {}",
        imported_count,
        path.display()
    ));
    Ok(Some((path, imported_count)))
}

#[allow(clippy::too_many_arguments)]
fn import_global_var_lists_to_sources(
    root: roxmltree::Node<'_, '_>,
    sources_root: &Path,
    seen_files: &mut HashSet<PathBuf>,
    warnings: &mut Vec<String>,
    unsupported_nodes: &mut Vec<String>,
    unsupported_diagnostics: &mut Vec<PlcopenUnsupportedDiagnostic>,
    loss_warnings: &mut usize,
    project_structure_map: &CodesysProjectStructureMap,
    imported_folder_paths: &mut HashSet<String>,
) -> anyhow::Result<ImportGlobalVarStats> {
    let mut stats = ImportGlobalVarStats::default();

    for global_vars in root
        .descendants()
        .filter(|node| is_element_named_ci(*node, "globalVars"))
    {
        stats.discovered_global_var_lists += 1;

        let default_name = format!("GlobalVars{}", stats.discovered_global_var_lists);
        let name = attribute_ci(global_vars, "name")
            .or_else(|| {
                global_vars
                    .children()
                    .find(|child| is_element_named_ci(*child, "name"))
                    .and_then(extract_text_content)
            })
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or(default_name.clone());

        let global_source = extract_codesys_global_vars_plaintext(global_vars)
            .or_else(|| synthesize_global_vars_source(global_vars, warnings))
            .map(normalize_body_text);
        let Some(global_source) = global_source else {
            warnings.push(format!(
                "skipping globalVars '{}': missing plaintext/variable declarations",
                name
            ));
            unsupported_nodes.push(format!("addData/globalVars/{name}"));
            unsupported_diagnostics.push(unsupported_diagnostic(
                "PLCO601",
                "warning",
                format!("addData/globalVars/{name}"),
                "globalVars entry skipped because no importable declarations were found",
                None,
                "Provide interface-as-plaintext or variable/type entries in the globalVars node",
            ));
            *loss_warnings += 1;
            continue;
        };

        let variables = parse_global_var_entries_from_st_block(&global_source);
        if variables.is_empty() {
            warnings.push(format!(
                "skipping globalVars '{}': no parseable VAR_GLOBAL declarations found",
                name
            ));
            unsupported_nodes.push(format!("addData/globalVars/{name}"));
            unsupported_diagnostics.push(unsupported_diagnostic(
                "PLCO602",
                "warning",
                format!("addData/globalVars/{name}"),
                "globalVars plaintext was present but no declarations could be parsed",
                None,
                "Ensure declarations follow 'name : TYPE [:= value];' syntax inside VAR_GLOBAL",
            ));
            *loss_warnings += 1;
            continue;
        }

        let list_identifier = sanitize_st_identifier(&name, "GlobalVars");
        if !list_identifier.eq_ignore_ascii_case(&name) {
            warnings.push(format!(
                "normalized globalVars identifier '{}' -> '{}'",
                name, list_identifier
            ));
        }

        let var_global_header_suffix = extract_var_global_header_suffix(&global_source);
        let is_qualified_only = codesys_global_vars_is_qualified_only(global_vars, &global_source);

        let (rendered_source, qualified_external) = if is_qualified_only {
            let type_name = format!("{list_identifier}_TYPE");
            let configuration_name = format!("{list_identifier}_Globals");

            let mut rendered = String::new();
            rendered.push_str("TYPE\n");
            rendered.push_str(&format!("{type_name} : STRUCT\n"));
            for variable in &variables {
                rendered.push_str("    ");
                rendered.push_str(&format_global_var_declaration(variable));
                rendered.push('\n');
            }
            rendered.push_str("END_STRUCT\n");
            rendered.push_str("END_TYPE\n\n");
            rendered.push_str(&format!("CONFIGURATION {configuration_name}\n"));
            rendered.push_str(&render_var_global_header(&var_global_header_suffix));
            rendered.push('\n');
            rendered.push_str(&format!("    {list_identifier} : {type_name};\n"));
            rendered.push_str("END_VAR\n");
            rendered.push_str("END_CONFIGURATION\n");

            warnings.push(format!(
                "mapped qualified_only globalVars '{}' to configuration/type wrapper for cross-file compatibility",
                name
            ));
            (
                rendered,
                Some(QualifiedGlobalListExternalDecl {
                    list_name: list_identifier.clone(),
                    type_name,
                }),
            )
        } else {
            let configuration_name = format!("{list_identifier}_Globals");
            let mut rendered = String::new();
            rendered.push_str(&format!("CONFIGURATION {configuration_name}\n"));
            rendered.push_str(&render_var_global_header(&var_global_header_suffix));
            rendered.push('\n');
            for variable in &variables {
                rendered.push_str("    ");
                rendered.push_str(&format_global_var_declaration(variable));
                rendered.push('\n');
            }
            rendered.push_str("END_VAR\n");
            rendered.push_str("END_CONFIGURATION\n");
            (rendered, None)
        };

        let folder_segments =
            resolve_codesys_folder_segments_for_node(global_vars, &name, project_structure_map);
        let candidate =
            unique_source_path_with_segments(sources_root, &folder_segments, &name, seen_files);
        write_text_file_with_parents(&candidate, &rendered_source)?;
        track_imported_folder_path(&candidate, sources_root, imported_folder_paths);

        if let Some(external_decl) = qualified_external {
            stats.qualified_list_externals.push(external_decl);
        }
        stats.imported_global_var_lists += 1;
        warnings.push(format!(
            "imported globalVars '{}' into {}",
            name,
            candidate.display()
        ));
        stats.written_sources.push(candidate);
    }

    Ok(stats)
}

fn extract_var_global_header_suffix(source: &str) -> String {
    for line in source.lines() {
        let trimmed = line.trim();
        let upper = trimmed.to_ascii_uppercase();
        if upper.starts_with("VAR_GLOBAL") {
            return trimmed["VAR_GLOBAL".len()..].trim().to_string();
        }
    }
    String::new()
}

fn render_var_global_header(suffix: &str) -> String {
    if suffix.trim().is_empty() {
        "VAR_GLOBAL".to_string()
    } else {
        format!("VAR_GLOBAL {}", suffix.trim())
    }
}

fn format_global_var_declaration(variable: &GlobalVarVariableDecl) -> String {
    let mut declaration = format!("{} : {}", variable.name.trim(), variable.type_expr.trim());
    if let Some(initial_value) = variable.initial_value.as_deref() {
        let init = initial_value.trim();
        if !init.is_empty() {
            declaration.push_str(" := ");
            declaration.push_str(init);
        }
    }
    declaration.push(';');
    declaration
}

fn codesys_global_vars_is_qualified_only(node: roxmltree::Node<'_, '_>, source: &str) -> bool {
    let lowered_source = source.to_ascii_lowercase();
    if lowered_source.contains("attribute 'qualified_only'")
        || lowered_source.contains("attribute \"qualified_only\"")
    {
        return true;
    }

    for attribute in node
        .descendants()
        .filter(|entry| is_element_named_ci(*entry, "Attribute"))
    {
        let Some(name) = attribute_ci_any(&attribute, &["Name", "name"])
            .map(|value| value.trim().to_ascii_lowercase())
        else {
            continue;
        };
        if name != "qualified_only" {
            continue;
        }
        let value = attribute_ci_any(&attribute, &["Value", "value"]).unwrap_or_default();
        let normalized = value.trim().to_ascii_lowercase();
        if normalized.is_empty() || normalized == "1" || normalized == "true" {
            return true;
        }
    }

    false
}

fn extract_codesys_global_vars_plaintext(node: roxmltree::Node<'_, '_>) -> Option<String> {
    for data in node
        .descendants()
        .filter(|entry| is_element_named_ci(*entry, "data"))
    {
        let Some(name) = attribute_ci(data, "name") else {
            continue;
        };
        if !name.to_ascii_lowercase().contains("interfaceasplaintext")
            && !name.eq_ignore_ascii_case(CODESYS_INTERFACE_PLAINTEXT_DATA_NAME)
        {
            continue;
        }
        if let Some(text) = extract_text_content(data) {
            if !text.trim().is_empty() {
                return Some(text);
            }
        }
    }
    None
}

fn synthesize_global_vars_source(
    node: roxmltree::Node<'_, '_>,
    warnings: &mut Vec<String>,
) -> Option<String> {
    let mut out = String::new();
    for attribute in node
        .descendants()
        .filter(|entry| is_element_named_ci(*entry, "Attribute"))
    {
        let Some(name) = attribute_ci_any(&attribute, &["Name", "name"])
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        if let Some(value) = attribute_ci_any(&attribute, &["Value", "value"]) {
            if value.trim().is_empty() {
                out.push_str(&format!("{{attribute '{}'}}\n", name));
            } else {
                out.push_str(&format!("{{attribute '{}': '{}'}}\n", name, value.trim()));
            }
        } else {
            out.push_str(&format!("{{attribute '{}'}}\n", name));
        }
    }

    let declarations = parse_interface_var_declarations(node);
    if declarations.is_empty() {
        return None;
    }
    warnings.push(
        "synthesized globalVars text from variable entries (interface-as-plaintext missing)"
            .to_string(),
    );
    out.push_str("VAR_GLOBAL\n");
    for declaration in declarations {
        out.push_str(&declaration);
        out.push('\n');
    }
    out.push_str("END_VAR\n");
    Some(out)
}

fn import_project_model_to_sources(
    root: roxmltree::Node<'_, '_>,
    sources_root: &Path,
    seen_files: &mut HashSet<PathBuf>,
    warnings: &mut Vec<String>,
    unsupported_nodes: &mut Vec<String>,
    unsupported_diagnostics: &mut Vec<PlcopenUnsupportedDiagnostic>,
    loss_warnings: &mut usize,
) -> anyhow::Result<ImportProjectModelStats> {
    let mut stats = ImportProjectModelStats::default();
    let mut configurations = Vec::new();

    for instances in root
        .children()
        .filter(|child| is_element_named_ci(*child, "instances"))
    {
        let mut discovered = false;
        for holder in instances
            .children()
            .filter(|child| is_element_named_ci(*child, "configurations"))
        {
            for configuration in holder
                .children()
                .filter(|child| is_element_named_ci(*child, "configuration"))
            {
                configurations.push(parse_configuration_model(configuration));
                discovered = true;
            }
        }
        if !discovered {
            for configuration in instances
                .children()
                .filter(|child| is_element_named_ci(*child, "configuration"))
            {
                configurations.push(parse_configuration_model(configuration));
                discovered = true;
            }
        }
        if !discovered {
            let direct_resources = instances
                .children()
                .filter(|child| is_element_named_ci(*child, "resource"))
                .collect::<Vec<_>>();
            if !direct_resources.is_empty() {
                let mut synthetic = ConfigurationDecl {
                    name: "ImportedConfiguration".to_string(),
                    tasks: Vec::new(),
                    programs: Vec::new(),
                    resources: Vec::new(),
                };
                for resource in direct_resources {
                    synthetic.resources.push(parse_resource_model(resource));
                }
                configurations.push(synthetic);
                discovered = true;
            }
        }
        if !discovered {
            unsupported_nodes.push("instances".to_string());
            unsupported_diagnostics.push(unsupported_diagnostic(
                "PLCO501",
                "warning",
                "instances",
                "PLCopen instances section is present but does not contain importable configuration/resource nodes",
                None,
                "Provide <configuration> entries under <instances>/<configurations> or direct <instances>",
            ));
            *loss_warnings += 1;
        }
    }

    stats.discovered_configurations = configurations.len();
    if configurations.is_empty() {
        return Ok(stats);
    }

    let mut used_configuration_names = HashSet::new();
    for (index, mut configuration) in configurations.into_iter().enumerate() {
        let default_name = format!("ImportedConfiguration{}", index + 1);
        let mut configuration_name = sanitize_st_identifier(&configuration.name, &default_name);
        if configuration_name != configuration.name {
            warnings.push(format!(
                "normalized configuration name '{}' -> '{}'",
                configuration.name, configuration_name
            ));
        }
        configuration_name = unique_identifier(configuration_name, &mut used_configuration_names);
        configuration.name = configuration_name;

        normalize_configuration_model(
            &mut configuration,
            warnings,
            unsupported_diagnostics,
            loss_warnings,
        );

        let source_text = render_configuration_source(&configuration);
        let path = unique_source_path(
            sources_root,
            &format!("plcopen_configuration_{}", configuration.name),
            seen_files,
        );
        write_text_file_with_parents(&path, &source_text).with_context(|| {
            format!(
                "failed to write imported configuration '{}'",
                path.display()
            )
        })?;
        stats.written_sources.push(path);
        stats.imported_configurations += 1;
        stats.imported_resources += configuration.resources.len();
        stats.imported_tasks += configuration.tasks.len();
        stats.imported_program_instances += configuration.programs.len();
        for resource in &configuration.resources {
            stats.imported_tasks += resource.tasks.len();
            stats.imported_program_instances += resource.programs.len();
        }
    }

    if stats.imported_configurations > 0 {
        warnings.push(format!(
            "imported {} PLCopen configuration(s), {} resource(s), {} task(s), {} program instance(s)",
            stats.imported_configurations,
            stats.imported_resources,
            stats.imported_tasks,
            stats.imported_program_instances
        ));
    }

    Ok(stats)
}

fn parse_configuration_model(node: roxmltree::Node<'_, '_>) -> ConfigurationDecl {
    let mut tasks = Vec::new();
    let mut programs = Vec::new();
    let mut resources = Vec::new();

    for child in node.children().filter(|child| child.is_element()) {
        if is_element_named_ci(child, "task") {
            if let Some(task) = parse_task_model(child) {
                tasks.push(task);
            }
        } else if let Some(program) = parse_program_instance_model(child, None) {
            programs.push(program);
        } else if is_element_named_ci(child, "resource") {
            resources.push(parse_resource_model(child));
        } else if is_element_named_ci(child, "resources") {
            for resource in child
                .children()
                .filter(|entry| is_element_named_ci(*entry, "resource"))
            {
                resources.push(parse_resource_model(resource));
            }
        }
    }

    ConfigurationDecl {
        name: attribute_ci_any(&node, &["name", "configurationName"])
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "ImportedConfiguration".to_string()),
        tasks,
        programs,
        resources,
    }
}

fn parse_resource_model(node: roxmltree::Node<'_, '_>) -> ResourceDecl {
    let mut tasks = Vec::new();
    let mut programs = Vec::new();

    for child in node.children().filter(|child| child.is_element()) {
        if is_element_named_ci(child, "task") {
            if let Some(task) = parse_task_model(child) {
                let task_name = task.name.clone();
                tasks.push(task);
                for nested in child.children().filter(|entry| entry.is_element()) {
                    if let Some(program) = parse_program_instance_model(nested, Some(&task_name)) {
                        programs.push(program);
                    }
                }
            }
        } else if let Some(program) = parse_program_instance_model(child, None) {
            programs.push(program);
        }
    }

    ResourceDecl {
        name: attribute_ci_any(&node, &["name", "resourceName"])
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "ImportedResource".to_string()),
        target: attribute_ci_any(&node, &["target", "type", "on"])
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "CPU".to_string()),
        tasks,
        programs,
    }
}

fn parse_task_model(node: roxmltree::Node<'_, '_>) -> Option<TaskDecl> {
    let name = attribute_ci_any(&node, &["name", "taskName"])
        .or_else(|| {
            node.children()
                .find(|child| is_element_named_ci(*child, "name"))
                .and_then(extract_text_content)
        })?
        .trim()
        .to_string();
    if name.is_empty() {
        return None;
    }

    let interval = attribute_ci_any(&node, &["interval", "cycle", "cycleTime", "period"])
        .or_else(|| {
            node.children()
                .find(|child| is_element_named_ci(*child, "interval"))
                .and_then(|entry| {
                    attribute_ci_any(&entry, &["value"]).or_else(|| extract_text_content(entry))
                })
        })
        .map(|value| normalize_task_interval_literal(&value));

    let single = attribute_ci_any(&node, &["single", "event", "trigger"]);
    let priority = attribute_ci_any(&node, &["priority"]);

    Some(TaskDecl {
        name,
        interval,
        single,
        priority,
    })
}

fn parse_program_instance_model(
    node: roxmltree::Node<'_, '_>,
    inherited_task_name: Option<&str>,
) -> Option<ProgramBindingDecl> {
    let node_name = node.tag_name().name();
    if !node_name.eq_ignore_ascii_case("program")
        && !node_name.eq_ignore_ascii_case("pouInstance")
        && !node_name.eq_ignore_ascii_case("programInstance")
        && !node_name.eq_ignore_ascii_case("instance")
    {
        return None;
    }

    let instance_name = attribute_ci_any(&node, &["name", "instanceName", "programName"])
        .or_else(|| {
            node.children()
                .find(|child| is_element_named_ci(*child, "name"))
                .and_then(extract_text_content)
        })?
        .trim()
        .to_string();
    if instance_name.is_empty() {
        return None;
    }

    let type_name = attribute_ci_any(&node, &["typeName", "type", "pouName", "programType"])
        .or_else(|| {
            node.children()
                .find(|child| is_element_named_ci(*child, "type"))
                .and_then(|entry| {
                    attribute_ci_any(&entry, &["name"]).or_else(|| extract_text_content(entry))
                })
        })?
        .trim()
        .to_string();
    if type_name.is_empty() {
        return None;
    }

    let task_name = attribute_ci_any(&node, &["task", "taskName", "withTask"])
        .or_else(|| inherited_task_name.map(ToOwned::to_owned))
        .filter(|value| !value.trim().is_empty());

    Some(ProgramBindingDecl {
        instance_name,
        task_name,
        type_name,
    })
}

fn normalize_configuration_model(
    configuration: &mut ConfigurationDecl,
    warnings: &mut Vec<String>,
    unsupported_diagnostics: &mut Vec<PlcopenUnsupportedDiagnostic>,
    loss_warnings: &mut usize,
) {
    let mut used_resource_names = HashSet::new();
    let mut used_task_names = HashSet::new();
    let mut used_program_names = HashSet::new();

    configuration.name = sanitize_st_identifier(&configuration.name, "ImportedConfiguration");
    for task in &mut configuration.tasks {
        let original = task.name.clone();
        let mut normalized = sanitize_st_identifier(&task.name, "Task");
        normalized = unique_identifier(normalized, &mut used_task_names);
        if normalized != original {
            warnings.push(format!(
                "normalized task name '{}' -> '{}' in configuration '{}'",
                original, normalized, configuration.name
            ));
        }
        task.name = normalized;
    }
    for program in &mut configuration.programs {
        let original = program.instance_name.clone();
        let mut normalized = sanitize_st_identifier(&program.instance_name, "Program");
        normalized = unique_identifier(normalized, &mut used_program_names);
        if normalized != original {
            warnings.push(format!(
                "normalized program instance name '{}' -> '{}' in configuration '{}'",
                original, normalized, configuration.name
            ));
        }
        program.instance_name = normalized;
        program.type_name = sanitize_st_identifier(&program.type_name, "MainProgram");
        if let Some(task_name) = &program.task_name {
            let normalized_task = sanitize_st_identifier(task_name, "Task");
            if used_task_names.contains(&normalized_task.to_ascii_lowercase()) {
                program.task_name = Some(normalized_task);
            } else if let Some(first) = configuration.tasks.first() {
                program.task_name = Some(first.name.clone());
            }
        }
    }

    for resource in &mut configuration.resources {
        let original = resource.name.clone();
        let mut normalized = sanitize_st_identifier(&resource.name, "Resource");
        normalized = unique_identifier(normalized, &mut used_resource_names);
        if normalized != original {
            warnings.push(format!(
                "normalized resource name '{}' -> '{}' in configuration '{}'",
                original, normalized, configuration.name
            ));
        }
        resource.name = normalized;
        resource.target = sanitize_st_identifier(&resource.target, "CPU");

        let mut local_task_names = HashSet::new();
        let mut local_program_names = HashSet::new();
        for task in &mut resource.tasks {
            let original = task.name.clone();
            let mut task_name = sanitize_st_identifier(&task.name, "Task");
            task_name = unique_identifier(task_name, &mut local_task_names);
            if task_name != original {
                warnings.push(format!(
                    "normalized task name '{}' -> '{}' in resource '{}'",
                    original, task_name, resource.name
                ));
            }
            task.name = task_name;
        }
        for program in &mut resource.programs {
            let original = program.instance_name.clone();
            let mut program_name = sanitize_st_identifier(&program.instance_name, "Program");
            program_name = unique_identifier(program_name, &mut local_program_names);
            if program_name != original {
                warnings.push(format!(
                    "normalized program instance name '{}' -> '{}' in resource '{}'",
                    original, program_name, resource.name
                ));
            }
            program.instance_name = program_name;
            program.type_name = sanitize_st_identifier(&program.type_name, "MainProgram");
            if let Some(task_name) = &program.task_name {
                let task_name = sanitize_st_identifier(task_name, "Task");
                program.task_name = Some(task_name);
            }
        }

        if !resource.programs.is_empty() && resource.tasks.is_empty() {
            let auto_task_name = unique_identifier("AutoTask".to_string(), &mut local_task_names);
            resource.tasks.push(TaskDecl {
                name: auto_task_name.clone(),
                interval: Some("T#100ms".to_string()),
                single: None,
                priority: Some("1".to_string()),
            });
            for program in &mut resource.programs {
                if program.task_name.is_none() {
                    program.task_name = Some(auto_task_name.clone());
                }
            }
            warnings.push(format!(
                "resource '{}' had PROGRAM instances without TASK declarations; generated TASK '{}'",
                resource.name, auto_task_name
            ));
            unsupported_diagnostics.push(unsupported_diagnostic(
                "PLCO506",
                "info",
                format!("instances/resource/{}", resource.name),
                "Generated deterministic fallback TASK for resource PROGRAM bindings",
                None,
                "Review generated configuration task timing and priority",
            ));
        }
    }

    if !configuration.programs.is_empty()
        && configuration.tasks.is_empty()
        && configuration.resources.is_empty()
    {
        let auto_task_name = unique_identifier("AutoTask".to_string(), &mut used_task_names);
        configuration.tasks.push(TaskDecl {
            name: auto_task_name.clone(),
            interval: Some("T#100ms".to_string()),
            single: None,
            priority: Some("1".to_string()),
        });
        for program in &mut configuration.programs {
            if program.task_name.is_none() {
                program.task_name = Some(auto_task_name.clone());
            }
        }
        warnings.push(format!(
            "configuration '{}' had PROGRAM instances without TASK declarations; generated TASK '{}'",
            configuration.name, auto_task_name
        ));
        unsupported_diagnostics.push(unsupported_diagnostic(
            "PLCO507",
            "info",
            format!("instances/configuration/{}", configuration.name),
            "Generated deterministic fallback TASK for configuration-level PROGRAM bindings",
            None,
            "Review generated configuration task timing and priority",
        ));
    }

    if configuration.tasks.is_empty()
        && configuration.programs.is_empty()
        && configuration.resources.is_empty()
    {
        *loss_warnings += 1;
        unsupported_diagnostics.push(unsupported_diagnostic(
            "PLCO508",
            "warning",
            format!("instances/configuration/{}", configuration.name),
            "Configuration is empty after import normalization",
            None,
            "Add TASK/PROGRAM/RESOURCE entries to preserve runtime scheduling intent",
        ));
    }
}

fn render_configuration_source(configuration: &ConfigurationDecl) -> String {
    let mut out = String::new();
    out.push_str(&format!("CONFIGURATION {}\n", configuration.name));
    for task in &configuration.tasks {
        out.push_str(&format!("{}\n", format_task_declaration(task)));
    }
    for program in &configuration.programs {
        out.push_str(&format!("{}\n", format_program_binding(program)));
    }
    for resource in &configuration.resources {
        out.push_str(&format!(
            "RESOURCE {} ON {}\n",
            resource.name, resource.target
        ));
        for task in &resource.tasks {
            out.push_str("    ");
            out.push_str(&format_task_declaration(task));
            out.push('\n');
        }
        for program in &resource.programs {
            out.push_str("    ");
            out.push_str(&format_program_binding(program));
            out.push('\n');
        }
        out.push_str("END_RESOURCE\n");
    }
    out.push_str("END_CONFIGURATION\n");
    out
}

fn format_task_declaration(task: &TaskDecl) -> String {
    let mut elements = Vec::new();
    if let Some(single) = task
        .single
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        elements.push(format!("SINGLE := {}", single.trim()));
    }
    if let Some(interval) = task
        .interval
        .as_ref()
        .map(|value| normalize_task_interval_literal(value))
    {
        elements.push(format!("INTERVAL := {}", interval.trim()));
    } else if task.single.is_none() {
        elements.push("INTERVAL := T#100ms".to_string());
    }
    if let Some(priority) = task
        .priority
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        elements.push(format!("PRIORITY := {}", priority.trim()));
    } else {
        elements.push("PRIORITY := 1".to_string());
    }
    format!("TASK {} ({});", task.name, elements.join(", "))
}

fn format_program_binding(program: &ProgramBindingDecl) -> String {
    if let Some(task_name) = &program.task_name {
        format!(
            "PROGRAM {} WITH {} : {};",
            program.instance_name, task_name, program.type_name
        )
    } else {
        format!("PROGRAM {} : {};", program.instance_name, program.type_name)
    }
}

fn sanitize_st_identifier(raw: &str, fallback: &str) -> String {
    let mut out = String::new();
    for (index, ch) in raw.chars().enumerate() {
        let valid = if index == 0 {
            ch.is_ascii_alphabetic() || ch == '_'
        } else {
            ch.is_ascii_alphanumeric() || ch == '_'
        };
        if valid {
            out.push(ch);
        } else if ch.is_ascii_alphanumeric() {
            if index == 0 {
                out.push('_');
                out.push(ch);
            } else {
                out.push(ch);
            }
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        return fallback.to_string();
    }
    if out.chars().next().is_some_and(|ch| ch.is_ascii_digit()) {
        out.insert(0, '_');
    }
    out
}

fn unique_identifier(candidate: String, used_lowercase: &mut HashSet<String>) -> String {
    let base = candidate;
    let mut output = base.clone();
    let mut index = 2usize;
    while !used_lowercase.insert(output.to_ascii_lowercase()) {
        output = format!("{base}_{index}");
        index += 1;
    }
    output
}

fn attribute_ci_any(node: &roxmltree::Node<'_, '_>, names: &[&str]) -> Option<String> {
    names.iter().find_map(|name| attribute_ci(*node, name))
}

fn format_data_type_declaration(name: &str, type_expr: &str) -> String {
    if !type_expr.contains('\n') {
        return format!("  {name} : {type_expr};");
    }

    let mut lines = type_expr.lines();
    let first = lines.next().unwrap_or_default();
    let mut declaration = format!("  {name} : {first}\n");
    for line in lines {
        declaration.push_str("  ");
        declaration.push_str(line);
        declaration.push('\n');
    }
    if declaration.ends_with('\n') {
        declaration.pop();
    }
    declaration.push(';');
    declaration
}

fn parse_data_type_expression(data_type: roxmltree::Node<'_, '_>) -> Option<String> {
    if let Some(base_type) = first_child_element_ci(data_type, "baseType") {
        if let Some(expr) = parse_type_expression_container(base_type) {
            return Some(expr);
        }
    }
    if let Some(type_node) = first_child_element_ci(data_type, "type") {
        if let Some(expr) = parse_type_expression_container(type_node) {
            return Some(expr);
        }
    }
    parse_type_expression_container(data_type)
}

fn parse_type_expression_container(container: roxmltree::Node<'_, '_>) -> Option<String> {
    container
        .children()
        .find(|child| child.is_element())
        .and_then(parse_type_expression_node)
}

fn parse_type_expression_node(node: roxmltree::Node<'_, '_>) -> Option<String> {
    let kind = node.tag_name().name().to_ascii_lowercase();
    if is_elementary_type_tag(&kind) {
        return Some(kind.to_ascii_uppercase());
    }

    match kind.as_str() {
        "derived" => attribute_ci(node, "name")
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty()),
        "string" | "wstring" => {
            let mut base = kind.to_ascii_uppercase();
            if let Some(length) = attribute_ci(node, "length")
                .or_else(|| attribute_ci(node, "maxLength"))
                .map(|raw| raw.trim().to_string())
                .filter(|raw| !raw.is_empty())
            {
                base.push('[');
                base.push_str(&length);
                base.push(']');
            }
            Some(base)
        }
        "array" => parse_array_type_expression(node),
        "struct" => parse_struct_type_expression(node),
        "enum" => parse_enum_type_expression(node),
        "subrange" => parse_subrange_type_expression(node),
        _ => None,
    }
}

fn parse_array_type_expression(array_node: roxmltree::Node<'_, '_>) -> Option<String> {
    let dimensions = array_node
        .children()
        .filter(|child| is_element_named_ci(*child, "dimension"))
        .filter_map(|dimension| {
            let lower = attribute_ci(dimension, "lower")
                .or_else(|| attribute_ci(dimension, "lowerLimit"))?;
            let upper = attribute_ci(dimension, "upper")
                .or_else(|| attribute_ci(dimension, "upperLimit"))?;
            Some(format!("{}..{}", lower.trim(), upper.trim()))
        })
        .collect::<Vec<_>>();
    if dimensions.is_empty() {
        return None;
    }

    let base_expr = first_child_element_ci(array_node, "baseType")
        .and_then(parse_type_expression_container)
        .or_else(|| {
            first_child_element_ci(array_node, "type").and_then(parse_type_expression_container)
        })?;
    Some(format!("ARRAY[{}] OF {}", dimensions.join(", "), base_expr))
}

fn parse_struct_type_expression(struct_node: roxmltree::Node<'_, '_>) -> Option<String> {
    let mut fields = Vec::new();
    for variable in struct_node.children().filter(|child| {
        is_element_named_ci(*child, "variable") || is_element_named_ci(*child, "member")
    }) {
        let Some(name) = attribute_ci(variable, "name")
            .or_else(|| {
                variable
                    .children()
                    .find(|child| is_element_named_ci(*child, "name"))
                    .and_then(extract_text_content)
            })
            .map(|raw| raw.trim().to_string())
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let Some(var_type) = first_child_element_ci(variable, "type")
            .and_then(parse_type_expression_container)
            .or_else(|| {
                first_child_element_ci(variable, "baseType")
                    .and_then(parse_type_expression_container)
            })
        else {
            continue;
        };
        let initializer = first_child_element_ci(variable, "initialValue")
            .and_then(parse_initial_value)
            .map_or_else(String::new, |value| format!(" := {value}"));
        fields.push(format!("    {name} : {var_type}{initializer};"));
    }

    let mut out = String::from("STRUCT\n");
    for field in fields {
        out.push_str(&field);
        out.push('\n');
    }
    out.push_str("END_STRUCT");
    Some(out)
}

fn parse_enum_type_expression(enum_node: roxmltree::Node<'_, '_>) -> Option<String> {
    let values_parent = first_child_element_ci(enum_node, "values").unwrap_or(enum_node);
    let mut values = Vec::new();
    for value in values_parent
        .children()
        .filter(|child| is_element_named_ci(*child, "value"))
    {
        let Some(name) = attribute_ci(value, "name")
            .or_else(|| extract_text_content(value))
            .map(|raw| raw.trim().to_string())
            .filter(|raw| !raw.is_empty())
        else {
            continue;
        };
        if let Some(raw_value) = attribute_ci(value, "value")
            .map(|raw| raw.trim().to_string())
            .filter(|raw| !raw.is_empty())
        {
            values.push(format!("{name} := {raw_value}"));
        } else {
            values.push(name);
        }
    }

    if values.is_empty() {
        None
    } else {
        Some(format!("({})", values.join(", ")))
    }
}

fn parse_subrange_type_expression(subrange_node: roxmltree::Node<'_, '_>) -> Option<String> {
    let lower = attribute_ci(subrange_node, "lower")
        .or_else(|| {
            first_child_element_ci(subrange_node, "range")
                .and_then(|range| attribute_ci(range, "lower"))
        })
        .map(|raw| raw.trim().to_string())
        .filter(|raw| !raw.is_empty())?;
    let upper = attribute_ci(subrange_node, "upper")
        .or_else(|| {
            first_child_element_ci(subrange_node, "range")
                .and_then(|range| attribute_ci(range, "upper"))
        })
        .map(|raw| raw.trim().to_string())
        .filter(|raw| !raw.is_empty())?;
    let base_expr = first_child_element_ci(subrange_node, "baseType")
        .and_then(parse_type_expression_container)
        .or_else(|| {
            first_child_element_ci(subrange_node, "type").and_then(parse_type_expression_container)
        })?;
    Some(format!("{base_expr}({lower}..{upper})"))
}

fn parse_initial_value(initial_value: roxmltree::Node<'_, '_>) -> Option<String> {
    first_child_element_ci(initial_value, "simpleValue")
        .and_then(|simple| attribute_ci(simple, "value").or_else(|| extract_text_content(simple)))
        .or_else(|| extract_text_content(initial_value))
        .map(|raw| raw.trim().to_string())
        .filter(|raw| !raw.is_empty())
}

fn first_child_element_ci<'a, 'input>(
    node: roxmltree::Node<'a, 'input>,
    name: &str,
) -> Option<roxmltree::Node<'a, 'input>> {
    node.children()
        .find(|child| is_element_named_ci(*child, name))
}

fn is_elementary_type_tag(name: &str) -> bool {
    matches!(
        name,
        "bool"
            | "byte"
            | "word"
            | "dword"
            | "lword"
            | "sint"
            | "int"
            | "dint"
            | "lint"
            | "usint"
            | "uint"
            | "udint"
            | "ulint"
            | "real"
            | "lreal"
            | "time"
            | "ltime"
            | "date"
            | "ldate"
            | "tod"
            | "ltod"
            | "dt"
            | "ldt"
            | "char"
            | "wchar"
    )
}

fn vendor_library_shims_for_ecosystem(ecosystem: &str) -> &'static [VendorLibraryShim] {
    match ecosystem {
        "siemens-tia" => SIEMENS_LIBRARY_SHIMS,
        "rockwell-studio5000" => ROCKWELL_LIBRARY_SHIMS,
        "schneider-ecostruxure" | "codesys" | "openplc" => SCHNEIDER_LIBRARY_SHIMS,
        "mitsubishi-gxworks3" => MITSUBISHI_LIBRARY_SHIMS,
        _ => &[],
    }
}

fn apply_vendor_library_shims(
    body: &str,
    ecosystem: &str,
) -> (String, Vec<PlcopenLibraryShimApplication>) {
    let shims = vendor_library_shims_for_ecosystem(ecosystem);
    if shims.is_empty() {
        return (body.to_string(), Vec::new());
    }

    let tokens = lex(body);
    if tokens.is_empty() {
        return (body.to_string(), Vec::new());
    }

    let mut output = String::with_capacity(body.len());
    let mut cursor = 0usize;
    let mut counts: BTreeMap<(String, String, String, String), usize> = BTreeMap::new();

    for (index, token) in tokens.iter().enumerate() {
        let start = usize::from(token.range.start());
        let end = usize::from(token.range.end());
        if start > cursor {
            output.push_str(&body[cursor..start]);
        }

        let token_text = &body[start..end];
        if let Some(shim) = match_library_shim(shims, token_text, &tokens, index) {
            output.push_str(shim.replacement_symbol);
            let key = (
                ecosystem.to_string(),
                shim.source_symbol.to_string(),
                shim.replacement_symbol.to_string(),
                shim.notes.to_string(),
            );
            *counts.entry(key).or_insert(0) += 1;
        } else {
            output.push_str(token_text);
        }
        cursor = end;
    }

    if cursor < body.len() {
        output.push_str(&body[cursor..]);
    }

    let applications = counts
        .into_iter()
        .map(
            |((vendor, source_symbol, replacement_symbol, notes), occurrences)| {
                PlcopenLibraryShimApplication {
                    vendor,
                    source_symbol,
                    replacement_symbol,
                    occurrences,
                    notes,
                }
            },
        )
        .collect();
    (output, applications)
}

fn match_library_shim<'a>(
    shims: &'a [VendorLibraryShim],
    token_text: &str,
    tokens: &[trust_syntax::lexer::Token],
    index: usize,
) -> Option<&'a VendorLibraryShim> {
    let upper = token_text.to_ascii_uppercase();
    let shim = shims
        .iter()
        .find(|candidate| candidate.source_symbol == upper)?;

    let previous = previous_non_trivia_token_kind(tokens, index);
    let next = next_non_trivia_token_kind(tokens, index);
    if previous == Some(TokenKind::Dot) {
        return None;
    }

    let type_position = matches!(
        previous,
        Some(TokenKind::Colon)
            | Some(TokenKind::KwOf)
            | Some(TokenKind::KwExtends)
            | Some(TokenKind::KwRefTo)
    );
    let call_position = next == Some(TokenKind::LParen);
    if type_position || call_position {
        Some(shim)
    } else {
        None
    }
}

fn previous_non_trivia_token_kind(
    tokens: &[trust_syntax::lexer::Token],
    index: usize,
) -> Option<TokenKind> {
    let mut current = index;
    while current > 0 {
        current -= 1;
        let kind = tokens[current].kind;
        if !kind.is_trivia() {
            return Some(kind);
        }
    }
    None
}

fn next_non_trivia_token_kind(
    tokens: &[trust_syntax::lexer::Token],
    index: usize,
) -> Option<TokenKind> {
    let mut current = index + 1;
    while current < tokens.len() {
        let kind = tokens[current].kind;
        if !kind.is_trivia() {
            return Some(kind);
        }
        current += 1;
    }
    None
}

fn calculate_source_coverage(imported: usize, discovered: usize) -> f64 {
    if discovered == 0 {
        return 0.0;
    }
    round_percent((imported as f64 / discovered as f64) * 100.0)
}

fn calculate_semantic_loss(
    imported: usize,
    discovered: usize,
    unsupported_nodes: usize,
    loss_warnings: usize,
) -> f64 {
    if discovered == 0 {
        return 100.0;
    }

    let skipped = discovered.saturating_sub(imported);
    let skipped_ratio = skipped as f64 / discovered as f64;
    let unsupported_ratio =
        unsupported_nodes as f64 / (unsupported_nodes as f64 + discovered as f64);
    let warning_ratio = (loss_warnings as f64 / (discovered as f64 * 2.0)).min(1.0);

    round_percent((skipped_ratio * 70.0) + (unsupported_ratio * 20.0) + (warning_ratio * 10.0))
}

fn calculate_compatibility_coverage(
    imported_pous: usize,
    skipped_pous: usize,
    unsupported_nodes: usize,
    shimmed_occurrences: usize,
) -> PlcopenCompatibilityCoverage {
    let supported_items = imported_pous;
    let partial_items = unsupported_nodes + shimmed_occurrences;
    let unsupported_items = skipped_pous;
    let total = supported_items + partial_items + unsupported_items;
    let support_percent = if total == 0 {
        0.0
    } else {
        round_percent((supported_items as f64 / total as f64) * 100.0)
    };
    let verdict = if total == 0 {
        "none"
    } else if unsupported_items == 0 && partial_items == 0 {
        "full"
    } else if supported_items > 0 {
        "partial"
    } else {
        "low"
    };
    PlcopenCompatibilityCoverage {
        supported_items,
        partial_items,
        unsupported_items,
        support_percent,
        verdict: verdict.to_string(),
    }
}

fn unsupported_diagnostic(
    code: &str,
    severity: &str,
    node: impl Into<String>,
    message: impl Into<String>,
    pou: Option<String>,
    action: impl Into<String>,
) -> PlcopenUnsupportedDiagnostic {
    PlcopenUnsupportedDiagnostic {
        code: code.to_string(),
        severity: severity.to_string(),
        node: node.into(),
        message: message.into(),
        pou,
        action: action.into(),
    }
}

fn round_percent(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn detect_vendor_ecosystem(root: roxmltree::Node<'_, '_>, xml_text: &str) -> String {
    let mut hints = String::new();
    for node in root.descendants().filter(|node| node.is_element()) {
        for attribute in node.attributes() {
            hints.push_str(attribute.value());
            hints.push(' ');
        }
        if is_element_named_ci(node, "data") {
            if let Some(name) = attribute_ci(node, "name") {
                hints.push_str(&name);
                hints.push(' ');
            }
        }
    }
    hints.push_str(xml_text);
    let normalized = hints.to_ascii_lowercase();

    if normalized.contains("twincat") || normalized.contains("beckhoff") {
        "beckhoff-twincat".to_string()
    } else if normalized.contains("openplc")
        || normalized.contains("open plc")
        || normalized.contains("openplc editor")
    {
        "openplc".to_string()
    } else if normalized.contains("schneider")
        || normalized.contains("ecostruxure")
        || normalized.contains("unity pro")
        || normalized.contains("control expert")
    {
        "schneider-ecostruxure".to_string()
    } else if normalized.contains("codesys")
        || normalized.contains("3s-smart")
        || normalized.contains("machine expert")
    {
        "codesys".to_string()
    } else if normalized.contains("siemens")
        || normalized.contains("tia portal")
        || normalized.contains("step7")
    {
        "siemens-tia".to_string()
    } else if normalized.contains("rockwell")
        || normalized.contains("studio 5000")
        || normalized.contains("allen-bradley")
    {
        "rockwell-studio5000".to_string()
    } else if normalized.contains("mitsubishi")
        || normalized.contains("gx works")
        || normalized.contains("gxworks")
        || normalized.contains("melsoft")
    {
        "mitsubishi-gxworks3".to_string()
    } else {
        "generic-plcopen".to_string()
    }
}

fn write_migration_report(
    project_root: &Path,
    report: &PlcopenMigrationReport,
) -> anyhow::Result<PathBuf> {
    let report_path = project_root.join(MIGRATION_REPORT_FILE);
    if let Some(parent) = report_path.parent() {
        std::fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create migration report directory '{}'",
                parent.display()
            )
        })?;
    }
    let json = serde_json::to_string_pretty(report)?;
    std::fs::write(&report_path, format!("{json}\n")).with_context(|| {
        format!(
            "failed to write PLCopen migration report '{}'",
            report_path.display()
        )
    })?;
    Ok(report_path)
}

fn inspect_unsupported_structure(
    root: roxmltree::Node<'_, '_>,
    unsupported_nodes: &mut Vec<String>,
    warnings: &mut Vec<String>,
    unsupported_diagnostics: &mut Vec<PlcopenUnsupportedDiagnostic>,
) {
    for child in root.children().filter(|child| child.is_element()) {
        let name = child.tag_name().name();
        if !matches!(
            name.to_ascii_lowercase().as_str(),
            "fileheader" | "contentheader" | "types" | "instances" | "adddata"
        ) {
            unsupported_nodes.push(name.to_string());
            warnings.push(format!(
                "unsupported PLCopen node '<{}>' preserved as metadata only",
                name
            ));
            unsupported_diagnostics.push(unsupported_diagnostic(
                "PLCO101",
                "warning",
                name,
                format!("Unsupported top-level PLCopen node '<{}>'", name),
                None,
                "Preserved as metadata only; not imported into runtime semantics",
            ));
        }
        if name.eq_ignore_ascii_case("types") {
            for type_child in child.children().filter(|entry| entry.is_element()) {
                let type_name = type_child.tag_name().name();
                if !type_name.eq_ignore_ascii_case("pous")
                    && !type_name.eq_ignore_ascii_case("dataTypes")
                {
                    unsupported_nodes.push(format!("types/{}", type_name));
                    warnings.push(format!(
                        "unsupported PLCopen node '<types>/<{}>' skipped (ST-complete subset)",
                        type_name
                    ));
                    unsupported_diagnostics.push(unsupported_diagnostic(
                        "PLCO102",
                        "warning",
                        format!("types/{type_name}"),
                        format!("Unsupported PLCopen <types>/<{}> section", type_name),
                        None,
                        "Skipped in ST-complete subset; migrate supported ST declarations manually",
                    ));
                }
            }
        } else if name.eq_ignore_ascii_case("instances") {
            for instances_child in child.children().filter(|entry| entry.is_element()) {
                let instances_name = instances_child.tag_name().name();
                if !instances_name.eq_ignore_ascii_case("configurations")
                    && !instances_name.eq_ignore_ascii_case("configuration")
                    && !instances_name.eq_ignore_ascii_case("resource")
                {
                    unsupported_nodes.push(format!("instances/{instances_name}"));
                    warnings.push(format!(
                        "unsupported PLCopen node '<instances>/<{}>' skipped",
                        instances_name
                    ));
                    unsupported_diagnostics.push(unsupported_diagnostic(
                        "PLCO103",
                        "warning",
                        format!("instances/{instances_name}"),
                        format!("Unsupported PLCopen <instances>/<{}> section", instances_name),
                        None,
                        "Skipped in ST-complete subset; provide configurations/resources/tasks/program instances",
                    ));
                }
            }
        }
    }
}

fn parse_embedded_source_map(root: roxmltree::Node<'_, '_>) -> Option<SourceMapPayload> {
    let payload = root
        .descendants()
        .find(|node| {
            is_element_named_ci(*node, "data")
                && attribute_ci(*node, "name").is_some_and(|name| name == SOURCE_MAP_DATA_NAME)
        })
        .and_then(|node| {
            node.children()
                .find(|child| is_element_named_ci(*child, "text"))
                .and_then(extract_text_content)
        })?;
    serde_json::from_str::<SourceMapPayload>(&payload).ok()
}

fn preserve_vendor_extensions(
    root: roxmltree::Node<'_, '_>,
    xml_text: &str,
    project_root: &Path,
    warnings: &mut Vec<String>,
) -> anyhow::Result<Option<PathBuf>> {
    let mut preserved = Vec::new();

    for node in root.descendants().filter(|node| {
        is_element_named_ci(*node, "data")
            && attribute_ci(*node, "name").is_none_or(|name| name != SOURCE_MAP_DATA_NAME)
    }) {
        let range = node.range();
        if let Some(slice) = xml_text.get(range) {
            preserved.push(slice.trim().to_string());
        }
    }

    if preserved.is_empty() {
        return Ok(None);
    }

    let output = project_root.join(IMPORTED_VENDOR_EXTENSION_FILE);
    let mut content = String::from("<vendorExtensions>\n");
    for fragment in preserved {
        content.push_str("  ");
        content.push_str(&fragment);
        content.push('\n');
    }
    content.push_str("</vendorExtensions>\n");
    std::fs::write(&output, content)
        .with_context(|| format!("failed to write '{}'", output.display()))?;
    warnings.push(format!(
        "preserved vendor extension nodes in {}",
        output.display()
    ));
    Ok(Some(output))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(prefix: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("trust-runtime-{prefix}-{stamp}"));
        std::fs::create_dir_all(&dir).expect("create temp directory");
        dir
    }

    fn write(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create parent");
        }
        std::fs::write(path, content).expect("write file");
    }

    fn pou_signatures(xml: &str) -> Vec<(String, String, String)> {
        let doc = roxmltree::Document::parse(xml).expect("parse XML");
        let mut items = doc
            .descendants()
            .filter(|node| is_element_named(*node, "pou"))
            .filter_map(|pou| {
                let name = pou.attribute("name")?.to_string();
                let pou_type = pou.attribute("pouType")?.to_string();
                let body = pou
                    .children()
                    .find(|child| is_element_named(*child, "body"))
                    .and_then(|body| {
                        body.children()
                            .find(|child| is_element_named(*child, "ST"))
                            .and_then(|st| st.text())
                    })
                    .map(str::trim)
                    .unwrap_or_default()
                    .to_string();
                Some((name, pou_type, body))
            })
            .collect::<Vec<_>>();
        items.sort();
        items
    }

    #[test]
    fn round_trip_export_import_export_preserves_pou_subset() {
        let source_project = temp_dir("plcopen-roundtrip-src");
        write(
            &source_project.join("src/main.st"),
            r#"
PROGRAM Main
VAR
    speed : REAL := 42.5;
END_VAR
END_PROGRAM
"#,
        );
        write(
            &source_project.join("src/calc.st"),
            r#"
FUNCTION Calc : INT
VAR_INPUT
    A : INT;
END_VAR
Calc := A + 1;
END_FUNCTION
"#,
        );

        let xml_a = source_project.join("build/plcopen.xml");
        let export_a = export_project_to_xml(&source_project, &xml_a).expect("export A");
        assert_eq!(export_a.pou_count, 2);
        assert!(export_a.source_map_path.is_file());

        let import_project = temp_dir("plcopen-roundtrip-import");
        let import = import_xml_to_project(&xml_a, &import_project).expect("import");
        assert_eq!(import.imported_pous, 2);
        assert_eq!(import.discovered_pous, 2);
        assert!(import.migration_report_path.is_file());
        assert_eq!(import.source_coverage_percent, 100.0);
        assert_eq!(import.semantic_loss_percent, 0.0);

        let xml_b = import_project.join("build/plcopen.xml");
        let export_b = export_project_to_xml(&import_project, &xml_b).expect("export B");
        assert_eq!(export_b.pou_count, 2);

        let a_text = std::fs::read_to_string(&xml_a).expect("read xml A");
        let b_text = std::fs::read_to_string(&xml_b).expect("read xml B");
        assert_eq!(pou_signatures(&a_text), pou_signatures(&b_text));

        let _ = std::fs::remove_dir_all(source_project);
        let _ = std::fs::remove_dir_all(import_project);
    }

    #[test]
    fn import_reports_unsupported_nodes_and_preserves_vendor_extensions() {
        let project = temp_dir("plcopen-import-unsupported");
        let xml_path = project.join("input.xml");
        write(
            &xml_path,
            r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://www.plcopen.org/xml/tc6_0200">
  <types>
    <dataTypes>
      <dataType name="POINT"/>
    </dataTypes>
    <pous>
      <pou name="Main" pouType="program">
        <body>
          <ST><![CDATA[
PROGRAM Main
VAR
  speed : REAL := 10.0;
END_VAR
END_PROGRAM
]]></ST>
        </body>
      </pou>
    </pous>
  </types>
  <addData>
    <data name="vendor.raw"><text><![CDATA[<vendorNode id="1"/>]]></text></data>
  </addData>
</project>
"#,
        );

        let report = import_xml_to_project(&xml_path, &project).expect("import XML");
        assert_eq!(report.imported_pous, 1);
        assert_eq!(report.discovered_pous, 1);
        assert!(!report.unsupported_nodes.is_empty());
        assert!(report
            .unsupported_nodes
            .iter()
            .any(|entry| entry.contains("types/dataTypes/POINT")));
        assert!(report.migration_report_path.is_file());
        assert!(report.source_coverage_percent > 0.0);
        assert!(report.semantic_loss_percent > 0.0);
        assert_eq!(report.compatibility_coverage.verdict, "partial");
        assert!(report
            .unsupported_diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "PLCO402"));
        let source = std::fs::read_to_string(&report.written_sources[0]).expect("read source");
        assert!(source.contains("PROGRAM Main"));
        let vendor = report
            .preserved_vendor_extensions
            .expect("vendor extension path");
        let vendor_text = std::fs::read_to_string(vendor).expect("read vendor ext");
        assert!(vendor_text.contains("vendor.raw"));

        let _ = std::fs::remove_dir_all(project);
    }

    #[test]
    fn import_supports_data_type_subset_and_generates_type_source() {
        let project = temp_dir("plcopen-import-datatypes");
        let xml_path = project.join("input.xml");
        write(
            &xml_path,
            r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://www.plcopen.org/xml/tc6_0200">
  <types>
    <dataTypes>
      <dataType name="Speed">
        <baseType>
          <int />
        </baseType>
      </dataType>
      <dataType name="Mode">
        <baseType>
          <enum>
            <values>
              <value name="Off"/>
              <value name="Auto"/>
            </values>
          </enum>
        </baseType>
      </dataType>
      <dataType name="Window">
        <baseType>
          <subrange lower="0" upper="100">
            <baseType><int /></baseType>
          </subrange>
        </baseType>
      </dataType>
      <dataType name="Point">
        <baseType>
          <struct>
            <variable name="X"><type><int /></type></variable>
            <variable name="Y"><type><int /></type></variable>
          </struct>
        </baseType>
      </dataType>
      <dataType name="Samples">
        <baseType>
          <array>
            <dimension lower="0" upper="15"/>
            <baseType><int /></baseType>
          </array>
        </baseType>
      </dataType>
    </dataTypes>
  </types>
</project>
"#,
        );

        let report = import_xml_to_project(&xml_path, &project).expect("import XML");
        assert_eq!(report.imported_pous, 0);
        assert_eq!(report.discovered_pous, 0);
        assert_eq!(report.written_sources.len(), 1);
        assert!(report
            .written_sources
            .iter()
            .any(|path| path.ends_with("plcopen_data_types.st")));
        assert!(!report
            .unsupported_diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "PLCO402"));

        let types_source =
            std::fs::read_to_string(&report.written_sources[0]).expect("read generated types");
        assert!(types_source.contains("TYPE"));
        assert!(types_source.contains("Speed : INT;"));
        assert!(types_source.contains("Mode : (Off, Auto);"));
        assert!(types_source.contains("Window : INT(0..100);"));
        assert!(types_source.contains("Point : STRUCT"));
        assert!(types_source.contains("X : INT;"));
        assert!(types_source.contains("Y : INT;"));
        assert!(types_source.contains("Samples : ARRAY[0..15] OF INT;"));
        assert!(types_source.contains("END_TYPE"));

        let _ = std::fs::remove_dir_all(project);
    }

    #[test]
    fn import_applies_siemens_library_shims_and_reports_them() {
        let project = temp_dir("plcopen-import-siemens-shims");
        let xml_path = project.join("siemens.xml");
        write(
            &xml_path,
            r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://www.plcopen.org/xml/tc6_0200">
  <fileHeader companyName="Siemens AG" productName="TIA Portal V18" />
  <types>
    <pous>
      <pou name="MainOb1" pouType="PRG">
        <body>
          <ST><![CDATA[
PROGRAM MainOb1
VAR
    PulseTimer : SFB3;
    DelayTimer : SFB4;
END_VAR
PulseTimer(IN := TRUE, PT := T#200ms);
DelayTimer(IN := PulseTimer.Q, PT := T#2s);
END_PROGRAM
]]></ST>
        </body>
      </pou>
    </pous>
  </types>
</project>
"#,
        );

        let report = import_xml_to_project(&xml_path, &project).expect("import XML");
        assert_eq!(report.detected_ecosystem, "siemens-tia");
        assert!(!report.applied_library_shims.is_empty());
        assert!(report
            .applied_library_shims
            .iter()
            .any(|entry| entry.source_symbol == "SFB3" && entry.replacement_symbol == "TP"));
        assert!(report
            .applied_library_shims
            .iter()
            .any(|entry| entry.source_symbol == "SFB4" && entry.replacement_symbol == "TON"));
        assert!(report
            .unsupported_diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "PLCO301"));

        let source = std::fs::read_to_string(&report.written_sources[0]).expect("read source");
        assert!(source.contains("PulseTimer : TP;"));
        assert!(source.contains("DelayTimer : TON;"));
        assert!(!source.contains("SFB3"));
        assert!(!source.contains("SFB4"));

        let _ = std::fs::remove_dir_all(project);
    }

    #[test]
    fn library_shim_rewrites_type_and_call_sites_only() {
        let body = r#"
PROGRAM Main
VAR
    SFB4 : BOOL := FALSE;
    DelayTimer : SFB4;
END_VAR
SFB4 := TRUE;
DelayTimer(IN := SFB4, PT := T#1s);
END_PROGRAM
"#;

        let (shimmed, applied) = apply_vendor_library_shims(body, "siemens-tia");
        assert_eq!(applied.len(), 1);
        assert!(shimmed.contains("SFB4 : BOOL := FALSE;"));
        assert!(shimmed.contains("DelayTimer : TON;"));
        assert!(shimmed.contains("SFB4 := TRUE;"));
    }

    #[test]
    fn import_synthesizes_codesys_body_only_and_empty_plaintext_pous() {
        let project = temp_dir("plcopen-import-codesys-shell");
        let xml_path = project.join("input.xml");
        write(
            &xml_path,
            r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://www.plcopen.org/xml/tc6_0200">
  <types>
    <pous>
      <pou name="PLC_PRG" pouType="program">
        <interface>
          <localVars>
            <variable name="waterPump">
              <type>
                <derived name="Pump" />
              </type>
            </variable>
          </localVars>
        </interface>
        <body>
          <ST>
            <xhtml xmlns="http://www.w3.org/1999/xhtml">waterpump();</xhtml>
          </ST>
        </body>
      </pou>
      <pou name="Pump" pouType="program">
        <interface />
        <body>
          <ST>
            <xhtml xmlns="http://www.w3.org/1999/xhtml" />
          </ST>
        </body>
        <addData>
          <data name="http://www.3s-software.com/plcopenxml/interfaceasplaintext" handleUnknown="implementation">
            <InterfaceAsPlainText>
              <xhtml xmlns="http://www.w3.org/1999/xhtml">PROGRAM Pump
VAR
END_VAR
</xhtml>
            </InterfaceAsPlainText>
          </data>
        </addData>
      </pou>
    </pous>
  </types>
</project>
"#,
        );

        let report = import_xml_to_project(&xml_path, &project).expect("import XML");
        assert_eq!(report.imported_pous, 2);
        assert_eq!(report.discovered_pous, 2);
        assert_eq!(report.source_coverage_percent, 100.0);
        assert_eq!(report.compatibility_coverage.verdict, "full");
        assert!(report
            .unsupported_diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "PLCO207"
                && diagnostic.pou.as_deref() == Some("PLC_PRG")));
        assert!(report
            .unsupported_diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "PLCO210"
                && diagnostic.pou.as_deref() == Some("Pump")));
        assert!(report
            .unsupported_diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "PLCO208"
                && diagnostic.pou.as_deref() == Some("Pump")));

        let main = std::fs::read_to_string(project.join("src/PLC_PRG.st")).expect("read PLC_PRG");
        assert!(main.contains("PROGRAM PLC_PRG"));
        assert!(main.contains("waterPump : Pump;"));
        assert!(main.contains("waterpump();"));
        assert!(main.contains("END_PROGRAM"));

        let pump = std::fs::read_to_string(project.join("src/Pump.st")).expect("read Pump");
        assert!(pump.contains("FUNCTION_BLOCK Pump"));
        assert!(pump.contains("END_FUNCTION_BLOCK"));
        assert!(!pump.contains("PROGRAM Pump"));

        let _ = std::fs::remove_dir_all(project);
    }

    #[test]
    fn import_codesys_global_vars_and_project_structure_into_application_folder() {
        let project = temp_dir("plcopen-import-codesys-gvl-folders");
        let xml_path = project.join("input.xml");
        write(
            &xml_path,
            r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://www.plcopen.org/xml/tc6_0200">
  <types>
    <pous />
  </types>
  <instances>
    <configurations />
  </instances>
  <addData>
    <data name="http://www.3s-software.com/plcopenxml/application" handleUnknown="implementation">
      <resource name="Application">
        <globalVars name="GVL">
          <variable name="start">
            <type>
              <BOOL />
            </type>
          </variable>
          <variable name="number">
            <type>
              <INT />
            </type>
            <initialValue>
              <simpleValue value="100" />
            </initialValue>
          </variable>
          <addData>
            <data name="http://www.3s-software.com/plcopenxml/interfaceasplaintext" handleUnknown="implementation">
              <InterfaceAsPlainText>
                <xhtml xmlns="http://www.w3.org/1999/xhtml">{attribute 'qualified_only'}
VAR_GLOBAL
    start: BOOL;
    number: INT := 100;
END_VAR</xhtml>
              </InterfaceAsPlainText>
            </data>
            <data name="http://www.3s-software.com/plcopenxml/objectid" handleUnknown="discard">
              <ObjectId>gvl-id</ObjectId>
            </data>
          </addData>
        </globalVars>
        <addData>
          <data name="http://www.3s-software.com/plcopenxml/pou" handleUnknown="implementation">
            <pou name="PLC_PRG" pouType="program">
              <body>
                <ST>
                  <xhtml xmlns="http://www.w3.org/1999/xhtml">GVL.start := TRUE;</xhtml>
                </ST>
              </body>
              <addData>
                <data name="http://www.3s-software.com/plcopenxml/objectid" handleUnknown="discard">
                  <ObjectId>pou-id</ObjectId>
                </data>
              </addData>
            </pou>
          </data>
        </addData>
      </resource>
    </data>
    <data name="http://www.3s-software.com/plcopenxml/projectstructure" handleUnknown="discard">
      <ProjectStructure>
        <Object Name="Application" ObjectId="app-id">
          <Object Name="PLC_PRG" ObjectId="pou-id" />
          <Object Name="GVL" ObjectId="gvl-id" />
        </Object>
      </ProjectStructure>
    </data>
  </addData>
</project>
"#,
        );

        let report = import_xml_to_project(&xml_path, &project).expect("import XML");
        assert_eq!(report.detected_ecosystem, "generic-plcopen");
        assert_eq!(report.imported_pous, 1);
        assert_eq!(report.imported_global_var_lists, 1);
        assert!(report.imported_project_structure_nodes >= 3);
        assert_eq!(report.imported_folder_paths, 1);

        let prg = project.join("src/Application/PLC_PRG.st");
        let gvl = project.join("src/Application/GVL.st");
        assert!(prg.is_file(), "expected PLC_PRG in Application folder");
        assert!(gvl.is_file(), "expected GVL in Application folder");

        let prg_text = std::fs::read_to_string(prg).expect("read prg");
        assert!(prg_text.contains("VAR_EXTERNAL"));
        assert!(prg_text.contains("GVL : GVL_TYPE;"));
        assert!(prg_text.contains("GVL.start := TRUE;"));

        let gvl_text = std::fs::read_to_string(gvl).expect("read gvl");
        assert!(gvl_text.contains("TYPE"));
        assert!(gvl_text.contains("GVL_TYPE : STRUCT"));
        assert!(gvl_text.contains("CONFIGURATION GVL_Globals"));
        assert!(gvl_text.contains("VAR_GLOBAL"));
        assert!(gvl_text.contains("number : INT := 100;"));
        assert!(gvl_text.contains("GVL : GVL_TYPE;"));

        let _ = std::fs::remove_dir_all(project);
    }

    #[test]
    fn import_injects_var_external_for_qualified_globals_and_function_result_assignment() {
        let project = temp_dir("plcopen-import-codesys-qualified-global-externals");
        let xml_path = project.join("input.xml");
        write(
            &xml_path,
            r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://www.plcopen.org/xml/tc6_0200">
  <types>
    <pous>
      <pou name="PLC_PRG" pouType="program">
        <body>
          <ST>
            <xhtml xmlns="http://www.w3.org/1999/xhtml">GVL.start := TRUE;
dosomthingfunction();</xhtml>
          </ST>
        </body>
      </pou>
      <pou name="dosomthingfunction" pouType="function">
        <interface>
          <returnType>
            <INT />
          </returnType>
        </interface>
        <body>
          <ST>
            <xhtml xmlns="http://www.w3.org/1999/xhtml">IF (GVL.start) THEN
  GVL.number := 200;
END_IF</xhtml>
          </ST>
        </body>
      </pou>
    </pous>
  </types>
  <addData>
    <data name="http://www.3s-software.com/plcopenxml/application" handleUnknown="implementation">
      <resource name="Application">
        <globalVars name="GVL">
          <addData>
            <data name="http://www.3s-software.com/plcopenxml/interfaceasplaintext" handleUnknown="implementation">
              <InterfaceAsPlainText>
                <xhtml xmlns="http://www.w3.org/1999/xhtml">{attribute 'qualified_only'}
VAR_GLOBAL
    start: BOOL;
    number: INT := 100;
END_VAR</xhtml>
              </InterfaceAsPlainText>
            </data>
          </addData>
        </globalVars>
      </resource>
    </data>
  </addData>
</project>
"#,
        );

        let report = import_xml_to_project(&xml_path, &project).expect("import XML");
        assert_eq!(report.imported_pous, 2);
        assert_eq!(report.imported_global_var_lists, 1);

        let prg_text =
            std::fs::read_to_string(project.join("src/PLC_PRG.st")).expect("read PLC_PRG");
        assert!(prg_text.contains("VAR_EXTERNAL"));
        assert!(prg_text.contains("GVL : GVL_TYPE;"));
        assert!(prg_text.contains("GVL.start := TRUE;"));

        let function_text = std::fs::read_to_string(project.join("src/dosomthingfunction.st"))
            .expect("read function");
        assert!(function_text.contains("VAR_EXTERNAL"));
        assert!(function_text.contains("GVL : GVL_TYPE;"));
        assert!(function_text.contains("dosomthingfunction := dosomthingfunction;"));

        let _ = std::fs::remove_dir_all(project);
    }

    #[test]
    fn export_emits_codesys_global_vars_and_project_structure_metadata() {
        let project = temp_dir("plcopen-export-codesys-gvl-folders");
        write(
            &project.join("src/Application/PLC_PRG.st"),
            r#"
PROGRAM PLC_PRG
GVL.start := TRUE;
END_PROGRAM
"#,
        );
        write(
            &project.join("src/Application/GVL.st"),
            r#"
{attribute 'qualified_only'}
VAR_GLOBAL
    start: BOOL;
    number: INT := 100;
END_VAR
"#,
        );

        let output = project.join("out/plcopen.xml");
        let report = export_project_to_xml(&project, &output).expect("export XML");
        assert_eq!(report.exported_global_var_lists, 1);
        assert!(report.exported_project_structure_nodes >= 3);
        assert!(report.exported_folder_paths >= 1);

        let xml = std::fs::read_to_string(&output).expect("read xml");
        assert!(xml.contains(CODESYS_APPLICATION_DATA_NAME));
        assert!(xml.contains(CODESYS_PROJECTSTRUCTURE_DATA_NAME));
        assert!(xml.contains("<globalVars name=\"GVL\">"));
        assert!(xml.contains("Object Name=\"GVL\""));

        let imported = temp_dir("plcopen-export-codesys-gvl-folders-import");
        let import_report = import_xml_to_project(&output, &imported).expect("import exported xml");
        assert_eq!(import_report.imported_global_var_lists, 1);
        assert!(imported.join("src/Application/GVL.st").is_file());
        assert!(imported.join("src/Application/PLC_PRG.st").is_file());

        let _ = std::fs::remove_dir_all(project);
        let _ = std::fs::remove_dir_all(imported);
    }

    #[test]
    fn import_rejects_malformed_xml() {
        let project = temp_dir("plcopen-malformed");
        let xml_path = project.join("broken.xml");
        write(&xml_path, "<project><types><pous><pou>");

        let result = import_xml_to_project(&xml_path, &project);
        assert!(result.is_err(), "malformed XML must return error");

        let _ = std::fs::remove_dir_all(project);
    }

    #[test]
    fn export_reinjects_vendor_extension_hook_file() {
        let project = temp_dir("plcopen-export-vendor-hook");
        write(
            &project.join("src/main.st"),
            r#"
PROGRAM Main
END_PROGRAM
"#,
        );
        write(
            &project.join(VENDOR_EXTENSION_HOOK_FILE),
            r#"<vendorData source="external"/>"#,
        );

        let output = project.join("out/plcopen.xml");
        export_project_to_xml(&project, &output).expect("export XML");
        let text = std::fs::read_to_string(output).expect("read output XML");
        assert!(text.contains(VENDOR_EXT_DATA_NAME));
        assert!(text.contains("vendorData"));

        let _ = std::fs::remove_dir_all(project);
    }

    #[test]
    fn export_with_vendor_target_emits_adapter_report_and_metadata() {
        let project = temp_dir("plcopen-export-target-ab");
        write(
            &project.join("src/main.st"),
            r#"
PROGRAM Main
VAR RETAIN
    Counter : INT := 0;
END_VAR
(* address marker for adapter checks: %MW10 *)
END_PROGRAM

CONFIGURATION Plant
TASK MainTask(INTERVAL := T#50ms, PRIORITY := 5);
PROGRAM MainInstance WITH MainTask : Main;
END_CONFIGURATION
"#,
        );

        let output = project.join("out/plcopen.ab.xml");
        let report =
            export_project_to_xml_with_target(&project, &output, PlcopenExportTarget::AllenBradley)
                .expect("export XML with target adapter");

        assert_eq!(report.target, "allen-bradley");
        let adapter_path = report
            .adapter_report_path
            .as_ref()
            .expect("adapter report path");
        assert!(adapter_path.is_file());
        assert!(report
            .adapter_diagnostics
            .iter()
            .any(|entry| entry.code == "PLCO7AB1"));
        assert!(!report.adapter_manual_steps.is_empty());
        assert!(!report.adapter_limitations.is_empty());

        let xml_text = std::fs::read_to_string(&output).expect("read output XML");
        assert!(xml_text.contains(EXPORT_ADAPTER_DATA_NAME));
        assert!(xml_text.contains("allen-bradley"));

        let adapter_text = std::fs::read_to_string(adapter_path).expect("read adapter report");
        assert!(adapter_text.contains("\"target\": \"allen-bradley\""));
        assert!(adapter_text.contains("PLCO7AB1"));

        let _ = std::fs::remove_dir_all(project);
    }

    #[test]
    fn export_siemens_target_emits_scl_bundle_and_program_ob_mapping() {
        let project = temp_dir("plcopen-export-target-siemens-scl");
        write(
            &project.join("src/main.st"),
            r#"
TYPE
    EMode : (Idle, Run);
END_TYPE

FUNCTION_BLOCK FB_Counter
VAR_INPUT
    Enable : BOOL;
END_VAR
VAR_OUTPUT
    Count : INT;
END_VAR
IF Enable THEN
    Count := Count + 1;
END_IF
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    Counter : FB_Counter;
END_VAR
Counter(Enable := TRUE);
END_PROGRAM

CONFIGURATION Plant
TASK MainTask(INTERVAL := T#100ms, PRIORITY := 1);
PROGRAM MainInstance WITH MainTask : Main;
END_CONFIGURATION
"#,
        );

        let output = project.join("out/plcopen.siemens.xml");
        let report =
            export_project_to_xml_with_target(&project, &output, PlcopenExportTarget::Siemens)
                .expect("export XML with Siemens target");

        let bundle_dir = report
            .siemens_scl_bundle_dir
            .as_ref()
            .expect("siemens scl bundle dir");
        assert!(bundle_dir.is_dir(), "expected Siemens SCL bundle directory");
        assert!(
            report.siemens_scl_files.iter().all(|path| path.is_file()),
            "expected all Siemens SCL files to be written"
        );
        assert!(
            report
                .siemens_scl_files
                .iter()
                .any(|path| path.extension().and_then(|value| value.to_str()) == Some("scl")),
            "expected at least one .scl file"
        );

        let main_scl = report
            .siemens_scl_files
            .iter()
            .find(|path| {
                path.file_name()
                    .and_then(|value| value.to_str())
                    .is_some_and(|name| name.contains("_ob_Main.scl"))
            })
            .expect("main OB file");
        let main_text = std::fs::read_to_string(main_scl).expect("read main scl file");
        assert!(main_text.contains("ORGANIZATION_BLOCK \"Main\""));
        assert!(main_text.contains("END_ORGANIZATION_BLOCK"));

        let adapter_path = report
            .adapter_report_path
            .as_ref()
            .expect("adapter report path");
        let adapter_text = std::fs::read_to_string(adapter_path).expect("read adapter report");
        assert!(adapter_text.contains("\"target\": \"siemens-tia\""));
        assert!(adapter_text.contains("siemens_scl_bundle_dir"));

        let _ = std::fs::remove_dir_all(project);
    }

    #[test]
    fn profile_declares_strict_subset_contract() {
        let profile = supported_profile();
        assert_eq!(profile.namespace, PLCOPEN_NAMESPACE);
        assert_eq!(profile.profile, PROFILE_NAME);
        assert!(profile
            .strict_subset
            .iter()
            .any(|item| item.contains("types/pous/pou")));
        assert!(profile
            .compatibility_matrix
            .iter()
            .any(|entry| entry.status == "supported"));
        assert!(profile.compatibility_matrix.iter().any(|entry| {
            entry.status == "partial" && entry.capability.contains("compatibility shims")
        }));
        assert!(!profile.round_trip_limits.is_empty());
        assert!(!profile.known_gaps.is_empty());
    }
}
