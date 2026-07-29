//! Phase 17 Rust production-fact extraction.

use std::fs;
use std::path::{Component, Path};
use std::process::Command;

use anyhow::{bail, Context, Result};

mod cargo_universe;
mod cfg_policy;
mod model;
mod module_graph;
mod product_census;
mod rust_extract;
mod tooling_universe;
mod universe;

#[cfg(test)]
pub use cargo_universe::discover_cargo_targets;
pub use model::{ModuleGraph, RustFact, RustScan};
#[cfg(test)]
pub use module_graph::scan_source_topology;
pub use module_graph::{
    scan_crate_root_module_graph, scan_crate_root_module_graph_including_tests, scan_module_graph,
    scan_source_topologies,
};
pub use rust_extract::extract_rust_source;
pub use tooling_universe::scan_tooling_rust_universe;
pub use universe::scan_rust_universe;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustExtractArgs {
    pub path: String,
    pub module: String,
    pub topology_for: Vec<String>,
}

pub fn parse_rust_extract_args(arguments: &[String]) -> Result<RustExtractArgs> {
    let mut path = None;
    let mut module = None;
    let mut topology_for = Vec::new();
    let mut index = 0;
    while index < arguments.len() {
        let flag = &arguments[index];
        if !matches!(flag.as_str(), "--path" | "--module" | "--topology-for") {
            bail!("unknown code-spec-rust argument {flag}")
        }
        let value = arguments
            .get(index + 1)
            .ok_or_else(|| anyhow::anyhow!("{flag} requires a value"))?;
        match flag.as_str() {
            "--path" if path.is_none() => path = Some(value.clone()),
            "--module" if module.is_none() => module = Some(value.clone()),
            "--topology-for" => topology_for.push(value.clone()),
            "--path" | "--module" => {
                bail!("duplicate code-spec-rust argument {flag}")
            }
            _ => unreachable!("closed flag set checked above"),
        }
        index += 2;
    }
    let path = path.ok_or_else(|| anyhow::anyhow!("code-spec-rust requires --path"))?;
    validate_repository_relative("--path", &path)?;
    let module = module.ok_or_else(|| anyhow::anyhow!("code-spec-rust requires --module"))?;
    if module.trim().is_empty() {
        bail!("code-spec-rust --module must not be empty")
    }
    topology_for.sort();
    let topology_count = topology_for.len();
    topology_for.dedup();
    if topology_for.len() != topology_count {
        bail!("duplicate code-spec-rust argument --topology-for")
    }
    for selected in &topology_for {
        validate_repository_relative("--topology-for", selected)?;
    }
    Ok(RustExtractArgs {
        path,
        module,
        topology_for,
    })
}

pub fn run_rust_extract(root: &Path, arguments: &[String]) -> Result<()> {
    let arguments = parse_rust_extract_args(arguments)?;
    if !arguments.topology_for.is_empty() {
        let selected_paths = arguments
            .topology_for
            .iter()
            .map(|selected| root.join(selected))
            .collect::<Vec<_>>();
        let topology = scan_source_topologies(
            root,
            &root.join(&arguments.path),
            &arguments.module,
            &selected_paths
                .iter()
                .map(|path| path.as_path())
                .collect::<Vec<_>>(),
        )?;
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "schema_version": 1,
                "selections": topology,
            }))?
        );
        return Ok(());
    }
    let source_path = root.join(&arguments.path);
    let source = fs::read_to_string(&source_path)
        .with_context(|| format!("read Rust source {}", source_path.display()))?;
    let scan = extract_rust_source(&arguments.module, &source)?;
    println!("{}", serde_json::to_string(&scan)?);
    Ok(())
}

fn validate_repository_relative(flag: &str, value: &str) -> Result<()> {
    let parsed = Path::new(value);
    if parsed.is_absolute()
        || parsed
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        bail!("code-spec-rust {flag} must stay repository-relative")
    }
    Ok(())
}

pub fn run_rust_universe(root: &Path) -> Result<()> {
    let output = Command::new("cargo")
        .args(["metadata", "--format-version", "1"])
        .current_dir(root)
        .output()
        .context("run cargo metadata for code-spec Rust denominator")?;
    if !output.status.success() {
        bail!(
            "cargo metadata failed for code-spec Rust denominator:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let metadata: serde_json::Value =
        serde_json::from_slice(&output.stdout).context("parse cargo metadata JSON")?;
    let scan = scan_rust_universe(root, &metadata);
    println!("{}", serde_json::to_string(&scan)?);
    if !scan.diagnostics.is_empty() {
        bail!(
            "code-spec Rust denominator has {} blocking diagnostic(s)",
            scan.diagnostics.len()
        );
    }
    Ok(())
}

pub fn run_rust_tooling(root: &Path) -> Result<()> {
    let scan = scan_tooling_rust_universe(root);
    println!("{}", serde_json::to_string(&scan)?);
    if !scan.diagnostics.is_empty() {
        bail!(
            "code-spec Rust tooling denominator has {} blocking diagnostic(s)",
            scan.diagnostics.len()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod tooling_universe_tests;

#[cfg(test)]
mod risk_policy_tests;
