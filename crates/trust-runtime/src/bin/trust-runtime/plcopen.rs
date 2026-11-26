//! PLCopen XML command handlers.

use std::path::PathBuf;

use anyhow::Context;

use trust_runtime::bundle::detect_bundle_path;
use trust_runtime::plcopen::{
    export_project_to_xml_with_target, import_xml_to_project, supported_profile,
    PlcopenExportReport, PlcopenExportTarget, PlcopenImportReport,
};

use crate::cli::{PlcopenAction, PlcopenExportTargetArg};
use crate::style;

pub fn run_plcopen(action: PlcopenAction) -> anyhow::Result<()> {
    match action {
        PlcopenAction::Profile { json } => {
            let profile = supported_profile();
            if json {
                outln!("{}", serde_json::to_string_pretty(&profile)?);
                return Ok(());
            }
            outln!("{}", style::accent("PLCopen profile (ST-complete subset)"));
            outln!("Namespace: {}", profile.namespace);
            outln!("Profile: {}", profile.profile);
            outln!("Version: {}", profile.version);
            outln!("Source mapping: {}", profile.source_mapping);
            outln!("Vendor extension hook: {}", profile.vendor_extension_hook);
            outln!("Supported subset:");
            for item in &profile.strict_subset {
                outln!(" - {item}");
            }
            outln!("Unsupported nodes:");
            for item in &profile.unsupported_nodes {
                outln!(" - {item}");
            }
            outln!("Compatibility matrix:");
            for entry in &profile.compatibility_matrix {
                outln!(
                    " - [{}] {}: {}",
                    entry.status,
                    entry.capability,
                    entry.notes
                );
            }
            outln!("Round-trip limits:");
            for item in &profile.round_trip_limits {
                outln!(" - {item}");
            }
            outln!("Known gaps:");
            for item in &profile.known_gaps {
                outln!(" - {item}");
            }
            Ok(())
        }
        PlcopenAction::Export {
            project,
            output,
            target,
            json,
        } => run_export(project, output, target, json),
        PlcopenAction::Import {
            input,
            project,
            json,
        } => run_import(input, project, json),
    }
}

fn run_export(
    project: Option<PathBuf>,
    output: Option<PathBuf>,
    target: PlcopenExportTargetArg,
    json: bool,
) -> anyhow::Result<()> {
    let project_root = resolve_project(project)?;
    let target = to_export_target(target);
    let output_path = output.unwrap_or_else(|| default_export_output_path(&project_root, target));
    let report = export_project_to_xml_with_target(&project_root, &output_path, target)?;
    if json {
        outln!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_export_report(&report);
    }
    Ok(())
}

fn run_import(input: PathBuf, project: Option<PathBuf>, json: bool) -> anyhow::Result<()> {
    let project_root = resolve_project(project)?;
    if !input.is_file() {
        anyhow::bail!("input PLCopen file '{}' does not exist", input.display());
    }
    let report = import_xml_to_project(&input, &project_root)?;
    if json {
        outln!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_import_report(&report);
    }
    Ok(())
}

fn resolve_project(project: Option<PathBuf>) -> anyhow::Result<PathBuf> {
    match project {
        Some(path) => Ok(path),
        None => match detect_bundle_path(None) {
            Ok(path) => Ok(path),
            Err(_) => std::env::current_dir().context("failed to resolve current directory"),
        },
    }
}

fn to_export_target(target: PlcopenExportTargetArg) -> PlcopenExportTarget {
    match target {
        PlcopenExportTargetArg::Generic => PlcopenExportTarget::Generic,
        PlcopenExportTargetArg::Ab => PlcopenExportTarget::AllenBradley,
        PlcopenExportTargetArg::Siemens => PlcopenExportTarget::Siemens,
        PlcopenExportTargetArg::Schneider => PlcopenExportTarget::Schneider,
    }
}

fn default_export_output_path(
    project_root: &std::path::Path,
    target: PlcopenExportTarget,
) -> PathBuf {
    let file_name = if target == PlcopenExportTarget::Generic {
        "plcopen.xml".to_string()
    } else {
        format!("plcopen.{}.xml", target.file_suffix())
    };
    project_root.join("interop").join(file_name)
}

fn print_export_report(report: &PlcopenExportReport) {
    outln!(
        "{}",
        style::success(format!("Wrote {}", report.output_path.display()))
    );
    outln!("Target adapter: {}", report.target);
    outln!(
        "Exported {} POU(s), {} data type(s), {} global list(s), {} configuration(s), {} resource(s), {} task(s), {} program instance(s) from {} source file(s)",
        report.pou_count,
        report.data_type_count,
        report.exported_global_var_lists,
        report.configuration_count,
        report.resource_count,
        report.task_count,
        report.program_instance_count,
        report.source_count
    );
    outln!(
        "CODESYS project structure: {} object node(s), {} folder path(s)",
        report.exported_project_structure_nodes,
        report.exported_folder_paths
    );
    outln!("Source map: {}", report.source_map_path.display());
    if let Some(bundle_dir) = &report.siemens_scl_bundle_dir {
        outln!("Siemens SCL bundle: {}", bundle_dir.display());
        outln!("Siemens SCL files: {}", report.siemens_scl_files.len());
    }
    if let Some(adapter_report_path) = &report.adapter_report_path {
        outln!("Adapter report: {}", adapter_report_path.display());
    }
    if !report.adapter_diagnostics.is_empty() {
        outln!("Target diagnostics:");
        for diagnostic in report.adapter_diagnostics.iter().take(10) {
            outln!(
                " - {} {}: {} ({})",
                diagnostic.severity,
                diagnostic.code,
                diagnostic.message,
                diagnostic.action
            );
        }
        if report.adapter_diagnostics.len() > 10 {
            outln!(" - ... +{}", report.adapter_diagnostics.len() - 10);
        }
    }
    if !report.adapter_manual_steps.is_empty() {
        outln!("Manual steps:");
        for step in &report.adapter_manual_steps {
            outln!(" - {step}");
        }
    }
    if !report.adapter_limitations.is_empty() {
        outln!("Adapter limitations:");
        for limitation in &report.adapter_limitations {
            outln!(" - {limitation}");
        }
    }
    if !report.warnings.is_empty() {
        outln!(
            "{}",
            style::warning(format!("{} warning(s):", report.warnings.len()))
        );
        for warning in &report.warnings {
            outln!(" - {warning}");
        }
    }
}

fn print_import_report(report: &PlcopenImportReport) {
    outln!(
        "{}",
        style::success(format!(
            "Imported {} POU(s) into {}",
            report.imported_pous,
            report.project_root.display()
        ))
    );
    outln!(
        "Discovered {} POU(s), source coverage {:.2}%, semantic loss {:.2}%",
        report.discovered_pous,
        report.source_coverage_percent,
        report.semantic_loss_percent
    );
    outln!(
        "Imported {} data type(s), {} interface(s), {} global list(s), {} configuration(s), {} resource(s), {} task(s), {} program instance(s)",
        report.imported_data_types,
        report.imported_interfaces,
        report.imported_global_var_lists,
        report.imported_configurations,
        report.imported_resources,
        report.imported_tasks,
        report.imported_program_instances
    );
    outln!(
        "Discovered {} global list(s), imported {} project-structure object node(s), used {} folder path(s)",
        report.discovered_global_var_lists,
        report.imported_project_structure_nodes,
        report.imported_folder_paths
    );
    outln!("Detected ecosystem: {}", report.detected_ecosystem);
    outln!(
        "Compatibility coverage: {:.2}% (supported={}, partial={}, unsupported={}, verdict={})",
        report.compatibility_coverage.support_percent,
        report.compatibility_coverage.supported_items,
        report.compatibility_coverage.partial_items,
        report.compatibility_coverage.unsupported_items,
        report.compatibility_coverage.verdict
    );
    outln!(
        "Migration report: {}",
        report.migration_report_path.display()
    );
    if !report.applied_library_shims.is_empty() {
        outln!("Applied library shims:");
        for shim in report.applied_library_shims.iter().take(10) {
            outln!(
                " - {}: {} -> {} ({} occurrence(s)) [{}]",
                shim.vendor,
                shim.source_symbol,
                shim.replacement_symbol,
                shim.occurrences,
                shim.notes
            );
        }
        if report.applied_library_shims.len() > 10 {
            outln!(" - ... +{}", report.applied_library_shims.len() - 10);
        }
    }
    for path in report.written_sources.iter().take(10) {
        outln!(" - {}", path.display());
    }
    if report.written_sources.len() > 10 {
        outln!(" - ... +{}", report.written_sources.len() - 10);
    }
    if !report.unsupported_nodes.is_empty() {
        outln!(
            "{}",
            style::warning(format!(
                "{} unsupported node(s) preserved/skipped",
                report.unsupported_nodes.len()
            ))
        );
    }
    if !report.unsupported_diagnostics.is_empty() {
        outln!(
            "{}",
            style::warning(format!(
                "{} structured compatibility diagnostic(s):",
                report.unsupported_diagnostics.len()
            ))
        );
        for diagnostic in report.unsupported_diagnostics.iter().take(10) {
            if let Some(pou) = &diagnostic.pou {
                outln!(
                    " - {} {} [{}] pou={}: {} ({})",
                    diagnostic.severity,
                    diagnostic.code,
                    diagnostic.node,
                    pou,
                    diagnostic.message,
                    diagnostic.action
                );
            } else {
                outln!(
                    " - {} {} [{}]: {} ({})",
                    diagnostic.severity,
                    diagnostic.code,
                    diagnostic.node,
                    diagnostic.message,
                    diagnostic.action
                );
            }
        }
        if report.unsupported_diagnostics.len() > 10 {
            outln!(" - ... +{}", report.unsupported_diagnostics.len() - 10);
        }
    }
    if !report.warnings.is_empty() {
        outln!(
            "{}",
            style::warning(format!("{} warning(s):", report.warnings.len()))
        );
        for warning in &report.warnings {
            outln!(" - {warning}");
        }
    }
}
