//! PLCopen XML command handlers.

use std::path::PathBuf;

use anyhow::Context;

use trust_runtime::bundle::detect_bundle_path;
use trust_runtime::plcopen::{
    export_project_to_xml, import_xml_to_project, supported_profile, PlcopenExportReport,
    PlcopenImportReport,
};

use crate::cli::PlcopenAction;
use crate::style;

pub fn run_plcopen(action: PlcopenAction) -> anyhow::Result<()> {
    match action {
        PlcopenAction::Profile { json } => {
            let profile = supported_profile();
            if json {
                println!("{}", serde_json::to_string_pretty(&profile)?);
                return Ok(());
            }
            println!("{}", style::accent("PLCopen profile (strict subset)"));
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
            Ok(())
        }
        PlcopenAction::Export { project, output } => run_export(project, output),
        PlcopenAction::Import { input, project } => run_import(input, project),
    }
}

fn run_export(project: Option<PathBuf>, output: Option<PathBuf>) -> anyhow::Result<()> {
    let project_root = resolve_project(project)?;
    let output_path = output.unwrap_or_else(|| project_root.join("interop").join("plcopen.xml"));
    let report = export_project_to_xml(&project_root, &output_path)?;
    print_export_report(&report);
    Ok(())
}

fn run_import(input: PathBuf, project: Option<PathBuf>) -> anyhow::Result<()> {
    let project_root = resolve_project(project)?;
    if !input.is_file() {
        anyhow::bail!("input PLCopen file '{}' does not exist", input.display());
    }
    let report = import_xml_to_project(&input, &project_root)?;
    print_import_report(&report);
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

fn print_export_report(report: &PlcopenExportReport) {
    println!(
        "{}",
        style::success(format!("Wrote {}", report.output_path.display()))
    );
    println!(
        "Exported {} POU(s) from {} source file(s)",
        report.pou_count, report.source_count
    );
    println!("Source map: {}", report.source_map_path.display());
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
