use std::fs;
use std::path::PathBuf;

use clap::Parser;
use serde::Serialize;
use trust_twin_compiler::{
    compile_topology_to_view, generate_robot_function_block_from_manifest_toml, ComponentLibrary,
    DoctorRuleResult, TopologyCompileOptions, TopologyCompileStats, TopologyDiagnostic,
};

#[derive(Debug, Parser)]
#[command(version, about = "Validate and compile trust-twin topology sources")]
struct Args {
    #[arg(long)]
    dry_run: bool,
    #[arg(long)]
    json: bool,
    #[arg(long)]
    robot_fb: bool,
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    output: Option<PathBuf>,
}

#[derive(Debug, Serialize)]
struct CompileDryRunReport {
    ok: bool,
    mode: &'static str,
    input: String,
    topology_hash: Option<String>,
    view_hash: Option<String>,
    diagnostics: Vec<TopologyDiagnostic>,
    doctor_results: Vec<DoctorRuleResult>,
    stats: Option<TopologyCompileStats>,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct RobotFbReport {
    ok: bool,
    mode: &'static str,
    input: String,
    function_block_name: Option<String>,
    source_urdf: Option<String>,
    manifest_hash: Option<String>,
    source_hash: Option<String>,
    error: Option<String>,
}

fn main() {
    let args = Args::parse();
    if !args.dry_run && args.output.is_none() {
        eprintln!("--output is required unless --dry-run is set");
        std::process::exit(64);
    }

    let source = match fs::read_to_string(&args.input) {
        Ok(source) => source,
        Err(err) => {
            eprintln!("failed to read {}: {err}", args.input.display());
            std::process::exit(1);
        }
    };
    if args.robot_fb {
        compile_robot_fb(&args, &source);
        return;
    }
    let library = match ComponentLibrary::load_builtin() {
        Ok(library) => library,
        Err(err) => {
            emit_report(
                args.json,
                &CompileDryRunReport {
                    ok: false,
                    mode: "dry-run",
                    input: args.input.display().to_string(),
                    topology_hash: None,
                    view_hash: None,
                    diagnostics: err.diagnostics().to_vec(),
                    doctor_results: Vec::new(),
                    stats: None,
                    error: Some(err.to_string()),
                },
            );
            std::process::exit(1);
        }
    };

    match compile_topology_to_view(&source, &library, &TopologyCompileOptions::default()) {
        Ok(compiled) => {
            if !args.dry_run {
                let output_path = args.output.as_ref().expect("validated output path");
                if let Some(parent) = output_path.parent() {
                    if let Err(err) = fs::create_dir_all(parent) {
                        eprintln!("failed to create {}: {err}", parent.display());
                        std::process::exit(1);
                    }
                }
                if let Err(err) = fs::write(output_path, &compiled.view_toml) {
                    eprintln!("failed to write {}: {err}", output_path.display());
                    std::process::exit(1);
                }
            }
            emit_report(
                args.json,
                &CompileDryRunReport {
                    ok: true,
                    mode: if args.dry_run { "dry-run" } else { "write" },
                    input: args.input.display().to_string(),
                    topology_hash: Some(compiled.topology_hash),
                    view_hash: Some(compiled.view_hash),
                    diagnostics: compiled.diagnostics,
                    doctor_results: compiled.doctor_results,
                    stats: Some(compiled.stats),
                    error: None,
                },
            );
        }
        Err(err) => {
            emit_report(
                args.json,
                &CompileDryRunReport {
                    ok: false,
                    mode: "dry-run",
                    input: args.input.display().to_string(),
                    topology_hash: None,
                    view_hash: None,
                    diagnostics: err.diagnostics().to_vec(),
                    doctor_results: Vec::new(),
                    stats: None,
                    error: Some(err.to_string()),
                },
            );
            std::process::exit(2);
        }
    }
}

fn compile_robot_fb(args: &Args, source: &str) {
    match generate_robot_function_block_from_manifest_toml(source) {
        Ok(generated) => {
            if !args.dry_run {
                let output_path = args.output.as_ref().expect("validated output path");
                if let Some(parent) = output_path.parent() {
                    if let Err(err) = fs::create_dir_all(parent) {
                        eprintln!("failed to create {}: {err}", parent.display());
                        std::process::exit(1);
                    }
                }
                if let Err(err) = fs::write(output_path, &generated.source) {
                    eprintln!("failed to write {}: {err}", output_path.display());
                    std::process::exit(1);
                }
            }
            emit_robot_fb_report(
                args.json,
                &RobotFbReport {
                    ok: true,
                    mode: if args.dry_run {
                        "robot-fb-dry-run"
                    } else {
                        "robot-fb-write"
                    },
                    input: args.input.display().to_string(),
                    function_block_name: Some(generated.function_block_name),
                    source_urdf: Some(generated.source_urdf),
                    manifest_hash: Some(generated.manifest_hash),
                    source_hash: Some(generated.source_hash),
                    error: None,
                },
            );
        }
        Err(err) => {
            emit_robot_fb_report(
                args.json,
                &RobotFbReport {
                    ok: false,
                    mode: "robot-fb-dry-run",
                    input: args.input.display().to_string(),
                    function_block_name: None,
                    source_urdf: None,
                    manifest_hash: None,
                    source_hash: None,
                    error: Some(err.to_string()),
                },
            );
            std::process::exit(2);
        }
    }
}

fn emit_report(json: bool, report: &CompileDryRunReport) {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(report).expect("serialize dry-run report")
        );
        return;
    }
    if report.ok {
        if report.mode == "write" {
            println!(
                "wrote: topology_hash={} view_hash={}",
                report.topology_hash.as_deref().unwrap_or(""),
                report.view_hash.as_deref().unwrap_or("")
            );
        } else {
            println!(
                "ok: topology_hash={} view_hash={}",
                report.topology_hash.as_deref().unwrap_or(""),
                report.view_hash.as_deref().unwrap_or("")
            );
        }
    } else {
        println!("error: {}", report.error.as_deref().unwrap_or("unknown"));
        for diagnostic in &report.diagnostics {
            println!(
                "{} {:?}: {}",
                diagnostic.code, diagnostic.severity, diagnostic.message
            );
        }
    }
}

fn emit_robot_fb_report(json: bool, report: &RobotFbReport) {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(report).expect("serialize robot FB report")
        );
        return;
    }
    if report.ok {
        println!(
            "{}: function_block={} source_hash={}",
            report.mode,
            report.function_block_name.as_deref().unwrap_or(""),
            report.source_hash.as_deref().unwrap_or("")
        );
    } else {
        println!("error: {}", report.error.as_deref().unwrap_or("unknown"));
    }
}
