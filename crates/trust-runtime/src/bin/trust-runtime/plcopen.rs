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
                println!("{}", serde_json::to_string_pretty(&profile)?);
                return Ok(());
            }
            println!("{}", style::accent("PLCopen profile (ST-complete subset)"));
            println!("Namespace: {}", profile.namespace);
            println!("Profile: {}", profile.profile);
            println!("Version: {}", profile.version);
            println!("Source mapping: {}", profile.source_mapping);
            println!("Vendor extension hook: {}", profile.vendor_extension_hook);
            println!("Supported subset:");
            for item in &profile.strict_subset {
                println!(" - {item}");
            }
            println!("Unsupported nodes:");
            for item in &profile.unsupported_nodes {
                println!(" - {item}");
            }
            println!("Compatibility matrix:");
            for entry in &profile.compatibility_matrix {
                println!(
                    " - [{}] {}: {}",
                    entry.status, entry.capability, entry.notes
                );
            }
            println!("Round-trip limits:");
            for item in &profile.round_trip_limits {
                println!(" - {item}");
            }
            println!("Known gaps:");
            for item in &profile.known_gaps {
                println!(" - {item}");
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
        println!("{}", serde_json::to_string_pretty(&report)?);
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
        println!("{}", serde_json::to_string_pretty(&report)?);
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
    println!(
        "{}",
        style::success(format!("Wrote {}", report.output_path.display()))
    );
    println!("Target adapter: {}", report.target);
    println!(
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
    println!(
        "CODESYS project structure: {} object node(s), {} folder path(s)",
        report.exported_project_structure_nodes, report.exported_folder_paths
    );
    println!("Source map: {}", report.source_map_path.display());
    if let Some(bundle_dir) = &report.siemens_scl_bundle_dir {
        println!("Siemens SCL bundle: {}", bundle_dir.display());
        println!("Siemens SCL files: {}", report.siemens_scl_files.len());
    }
    if let Some(adapter_report_path) = &report.adapter_report_path {
        println!("Adapter report: {}", adapter_report_path.display());
    }
    if !report.adapter_diagnostics.is_empty() {
        println!("Target diagnostics:");
        for diagnostic in report.adapter_diagnostics.iter().take(10) {
            println!(
                " - {} {}: {} ({})",
                diagnostic.severity, diagnostic.code, diagnostic.message, diagnostic.action
            );
        }
        if report.adapter_diagnostics.len() > 10 {
            println!(" - ... +{}", report.adapter_diagnostics.len() - 10);
        }
    }
    if !report.adapter_manual_steps.is_empty() {
        println!("Manual steps:");
        for step in &report.adapter_manual_steps {
            println!(" - {step}");
        }
    }
    if !report.adapter_limitations.is_empty() {
        println!("Adapter limitations:");
        for limitation in &report.adapter_limitations {
            println!(" - {limitation}");
        }
    }
    if !report.warnings.is_empty() {
        println!(
            "{}",
            style::warning(format!("{} warning(s):", report.warnings.len()))
        );
        for warning in &report.warnings {
            println!(" - {warning}");
        }
    }
}

fn print_import_report(report: &PlcopenImportReport) {
    println!(
        "{}",
        style::success(format!(
            "Imported {} POU(s) into {}",
            report.imported_pous,
            report.project_root.display()
        ))
    );
    println!(
        "Discovered {} POU(s), source coverage {:.2}%, semantic loss {:.2}%",
        report.discovered_pous, report.source_coverage_percent, report.semantic_loss_percent
    );
    println!(
        "Imported {} data type(s), {} global list(s), {} configuration(s), {} resource(s), {} task(s), {} program instance(s)",
        report.imported_data_types,
        report.imported_global_var_lists,
        report.imported_configurations,
        report.imported_resources,
        report.imported_tasks,
        report.imported_program_instances
    );
    println!(
        "Discovered {} global list(s), imported {} project-structure object node(s), used {} folder path(s)",
        report.discovered_global_var_lists,
        report.imported_project_structure_nodes,
        report.imported_folder_paths
    );
    println!("Detected ecosystem: {}", report.detected_ecosystem);
    println!(
        "Compatibility coverage: {:.2}% (supported={}, partial={}, unsupported={}, verdict={})",
        report.compatibility_coverage.support_percent,
        report.compatibility_coverage.supported_items,
        report.compatibility_coverage.partial_items,
        report.compatibility_coverage.unsupported_items,
        report.compatibility_coverage.verdict
    );
    println!(
        "Migration report: {}",
        report.migration_report_path.display()
    );
    if !report.applied_library_shims.is_empty() {
        println!("Applied library shims:");
        for shim in report.applied_library_shims.iter().take(10) {
            println!(
                " - {}: {} -> {} ({} occurrence(s)) [{}]",
                shim.vendor,
                shim.source_symbol,
                shim.replacement_symbol,
                shim.occurrences,
                shim.notes
            );
        }
        if report.applied_library_shims.len() > 10 {
            println!(" - ... +{}", report.applied_library_shims.len() - 10);
        }
    }
    for path in report.written_sources.iter().take(10) {
        println!(" - {}", path.display());
    }
    if report.written_sources.len() > 10 {
        println!(" - ... +{}", report.written_sources.len() - 10);
    }
    if !report.unsupported_nodes.is_empty() {
        println!(
            "{}",
            style::warning(format!(
                "{} unsupported node(s) preserved/skipped",
                report.unsupported_nodes.len()
            ))
        );
    }
    if !report.unsupported_diagnostics.is_empty() {
        println!(
            "{}",
            style::warning(format!(
                "{} structured compatibility diagnostic(s):",
                report.unsupported_diagnostics.len()
            ))
        );
        for diagnostic in report.unsupported_diagnostics.iter().take(10) {
            if let Some(pou) = &diagnostic.pou {
                println!(
                    " - {} {} [{}] pou={}: {} ({})",
                    diagnostic.severity,
                    diagnostic.code,
                    diagnostic.node,
                    pou,
                    diagnostic.message,
                    diagnostic.action
                );
            } else {
                println!(
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
            println!(" - ... +{}", report.unsupported_diagnostics.len() - 10);
        }
    }
    if !report.warnings.is_empty() {
        println!(
            "{}",
            style::warning(format!("{} warning(s):", report.warnings.len()))
        );
        for warning in &report.warnings {
            println!(" - {warning}");
        }
    }
}
