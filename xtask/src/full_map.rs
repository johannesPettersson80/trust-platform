use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, bail, Context, Result};

use crate::software_map::{
    PackageSummary, SoftwareMap, SourceFileSummary, TargetSummary, ToolResult, ToolStatus,
    WorkspaceEdge,
};

pub fn architecture_doctor_full_map(root: &Path) -> Result<()> {
    let map = build_software_map(root)?;
    let artifact_dir = full_map_artifact_dir(root)?;
    fs::create_dir_all(&artifact_dir)
        .with_context(|| format!("create {}", artifact_dir.display()))?;
    let json_path = artifact_dir.join("software-map.json");
    fs::write(&json_path, map.to_stable_json()?)
        .with_context(|| format!("write {}", json_path.display()))?;
    println!("wrote {}", json_path.display());

    bail!(
        "architecture-doctor --full-map policy checks are not implemented yet; JSON map artifact was written"
    )
}

fn build_software_map(root: &Path) -> Result<SoftwareMap> {
    let _known_statuses = ToolStatus::ALL;
    let metadata = cargo_metadata(root)?;
    let workspace_members = metadata["workspace_members"]
        .as_array()
        .ok_or_else(|| anyhow!("cargo metadata did not include workspace_members"))?
        .iter()
        .filter_map(serde_json::Value::as_str)
        .map(ToOwned::to_owned)
        .collect::<BTreeSet<_>>();

    let packages_json = metadata["packages"]
        .as_array()
        .ok_or_else(|| anyhow!("cargo metadata did not include packages"))?;
    let workspace_package_names = packages_json
        .iter()
        .filter(|package| {
            package["id"]
                .as_str()
                .is_some_and(|id| workspace_members.contains(id))
        })
        .filter_map(|package| package["name"].as_str())
        .map(ToOwned::to_owned)
        .collect::<BTreeSet<_>>();

    let mut map = SoftwareMap::new(root.display().to_string());
    for package in packages_json {
        let id = package["id"].as_str().unwrap_or_default();
        if !workspace_members.contains(id) {
            continue;
        }
        let name = package["name"].as_str().unwrap_or_default().to_string();
        map.packages.push(PackageSummary {
            name: name.clone(),
            manifest_path: rel_path(
                root,
                Path::new(package["manifest_path"].as_str().unwrap_or_default()),
            ),
            targets: package["targets"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|target| TargetSummary {
                    name: target["name"].as_str().unwrap_or_default().to_string(),
                    kind: target["kind"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(serde_json::Value::as_str)
                        .map(ToOwned::to_owned)
                        .collect(),
                    src_path: rel_path(
                        root,
                        Path::new(target["src_path"].as_str().unwrap_or_default()),
                    ),
                })
                .collect(),
        });
        for dependency in package["dependencies"].as_array().into_iter().flatten() {
            let Some(dep_name) = dependency["name"].as_str() else {
                continue;
            };
            if workspace_package_names.contains(dep_name) {
                map.workspace_edges.push(WorkspaceEdge {
                    from: name.clone(),
                    to: dep_name.to_string(),
                    kind: dependency["kind"].as_str().unwrap_or("normal").to_string(),
                });
            }
        }
    }

    for file in collect_source_files(root)? {
        let source = fs::read_to_string(&file).unwrap_or_default();
        map.source_files.push(SourceFileSummary {
            path: rel_path(root, &file),
            line_count: source.lines().count(),
        });
    }
    map.tool_results.push(ToolResult {
        name: "cargo metadata".to_string(),
        status: ToolStatus::Pass,
        details: vec![format!("workspace packages: {}", map.packages.len())],
    });
    map.tool_results.push(ToolResult {
        name: "source file scan".to_string(),
        status: ToolStatus::Pass,
        details: vec![format!("source files: {}", map.source_files.len())],
    });
    map.tool_results.push(ToolResult {
        name: "full-map policy checks".to_string(),
        status: ToolStatus::NotRun,
        details: vec!["policy checks are implemented in later FULLMAP phases".to_string()],
    });

    Ok(map)
}

fn cargo_metadata(root: &Path) -> Result<serde_json::Value> {
    let output = Command::new("cargo")
        .args(["metadata", "--all-features", "--format-version", "1"])
        .current_dir(root)
        .output()
        .context("run cargo metadata")?;
    if !output.status.success() {
        bail!(
            "cargo metadata failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    serde_json::from_slice(&output.stdout).context("parse cargo metadata JSON")
}

fn full_map_artifact_dir(root: &Path) -> Result<PathBuf> {
    let commit = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(root)
        .output()
        .context("resolve current git commit")?;
    let suffix = if commit.status.success() {
        String::from_utf8_lossy(&commit.stdout).trim().to_string()
    } else {
        "unknown".to_string()
    };
    Ok(root
        .join("target/gate-artifacts")
        .join(format!("full-software-map-{suffix}")))
}

fn collect_source_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for rel in ["crates", "xtask", "scripts"] {
        let dir = root.join(rel);
        if dir.exists() {
            collect_source_files_inner(&dir, &mut files)?;
        }
    }
    Ok(files)
}

fn collect_source_files_inner(path: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(path).with_context(|| format!("read {}", path.display()))? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        if name == "target" || name == "node_modules" || name == "__pycache__" {
            continue;
        }
        if path.is_dir() {
            collect_source_files_inner(&path, files)?;
        } else if matches!(
            path.extension().and_then(|ext| ext.to_str()),
            Some("rs" | "py" | "toml")
        ) {
            files.push(path);
        }
    }
    Ok(())
}

fn rel_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
