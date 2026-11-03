//! Library dependency graph helpers.

use crate::config::{LibraryDependency, LibrarySpec, ProjectConfig};
use rustc_hash::FxHashMap;
use std::collections::{BTreeSet, HashSet};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct LibraryNode {
    pub name: String,
    pub version: Option<String>,
    pub path: PathBuf,
    pub dependencies: Vec<LibraryDependency>,
}

#[derive(Debug, Clone)]
pub struct LibraryGraph {
    pub nodes: Vec<LibraryNode>,
}

#[derive(Debug, Clone)]
pub struct LibraryIssue {
    pub code: &'static str,
    pub message: String,
    pub subject: String,
    pub dependency: Option<String>,
}

pub fn build_library_graph(config: &ProjectConfig) -> LibraryGraph {
    let nodes = config
        .libraries
        .iter()
        .map(|lib| LibraryNode {
            name: lib.name.clone(),
            version: lib.version.clone(),
            path: lib.path.clone(),
            dependencies: lib.dependencies.clone(),
        })
        .collect();
    LibraryGraph { nodes }
}

pub fn library_dependency_issues(config: &ProjectConfig) -> Vec<LibraryIssue> {
    let mut by_name: FxHashMap<String, Vec<&LibrarySpec>> = FxHashMap::default();
    for lib in &config.libraries {
        by_name
            .entry(library_identity(&lib.name))
            .or_default()
            .push(lib);
    }

    let mut version_issues = Vec::new();
    let mut library_names = by_name.keys().cloned().collect::<Vec<_>>();
    library_names.sort();

    for name in library_names {
        let libs = &by_name[&name];
        let mut versions = BTreeSet::new();
        for lib in libs {
            versions.insert(version_label(&lib.version));
        }
        if versions.len() > 1 {
            let versions: Vec<String> = versions.into_iter().collect();
            version_issues.push(LibraryIssue {
                code: "L003",
                message: format!(
                    "Library '{}' declared with conflicting versions ({})",
                    libs[0].name,
                    versions.join(", ")
                ),
                subject: libs[0].name.clone(),
                dependency: None,
            });
        }
    }

    let mut missing_issues = Vec::new();
    let mut mismatch_issues = Vec::new();
    let mut seen_missing = HashSet::new();
    for lib in &config.libraries {
        for dep in &lib.dependencies {
            let Some(candidates) = by_name.get(&library_identity(&dep.name)) else {
                let missing_key = (library_identity(&lib.name), library_identity(&dep.name));
                if !seen_missing.insert(missing_key) {
                    continue;
                }
                missing_issues.push(LibraryIssue {
                    code: "L001",
                    message: format!(
                        "Library '{}' depends on '{}', but it is not configured",
                        lib.name, dep.name
                    ),
                    subject: lib.name.clone(),
                    dependency: Some(dep.name.clone()),
                });
                continue;
            };
            if let Some(required) = dep.version.as_deref() {
                let matched = candidates
                    .iter()
                    .any(|candidate| candidate.version.as_deref() == Some(required));
                if !matched {
                    let mut available: Vec<String> = candidates
                        .iter()
                        .map(|candidate| version_label(&candidate.version))
                        .collect();
                    available.sort();
                    available.dedup();
                    mismatch_issues.push(LibraryIssue {
                        code: "L002",
                        message: format!(
                            "Library '{}' requires '{}' version {}, but available versions are {}",
                            lib.name,
                            dep.name,
                            required,
                            available.join(", ")
                        ),
                        subject: lib.name.clone(),
                        dependency: Some(dep.name.clone()),
                    });
                }
            }
        }
    }

    let mut cycle_issues = Vec::new();
    for cycle in dependency_cycles(&by_name) {
        let label = cycle
            .iter()
            .map(|name| by_name[name][0].name.as_str())
            .collect::<Vec<_>>()
            .join(" -> ");
        cycle_issues.push(LibraryIssue {
            code: "L004",
            message: format!("Dependency cycle detected: {label}"),
            subject: cycle
                .first()
                .map(|name| by_name[name][0].name.clone())
                .unwrap_or_default(),
            dependency: None,
        });
    }

    let mut issues = version_issues;
    issues.extend(missing_issues);
    issues.extend(mismatch_issues);
    issues.extend(cycle_issues);
    issues
}

fn library_identity(name: &str) -> String {
    name.to_ascii_uppercase()
}

fn version_label(version: &Option<String>) -> String {
    version
        .as_deref()
        .map(|value| value.to_string())
        .unwrap_or_else(|| "unspecified".to_string())
}

fn dependency_cycles(by_name: &FxHashMap<String, Vec<&LibrarySpec>>) -> Vec<Vec<String>> {
    let mut adjacency: FxHashMap<String, Vec<String>> = FxHashMap::default();
    for (name, libs) in by_name {
        for lib in libs {
            let edges = adjacency.entry(name.clone()).or_default();
            for dependency in &lib.dependencies {
                let dependency_name = library_identity(&dependency.name);
                if by_name.contains_key(&dependency_name) {
                    edges.push(dependency_name);
                }
            }
        }
    }
    for edges in adjacency.values_mut() {
        edges.sort();
        edges.dedup();
    }

    let mut state: FxHashMap<String, VisitState> = FxHashMap::default();
    let mut stack = Vec::new();
    let mut seen_keys = BTreeSet::new();
    let mut cycles = Vec::new();
    let mut names: Vec<&str> = adjacency.keys().map(String::as_str).collect();
    names.sort();
    names.dedup();
    for name in names {
        detect_cycle(
            name,
            &adjacency,
            &mut state,
            &mut stack,
            &mut seen_keys,
            &mut cycles,
        );
    }
    cycles
}

fn detect_cycle(
    current: &str,
    adjacency: &FxHashMap<String, Vec<String>>,
    state: &mut FxHashMap<String, VisitState>,
    stack: &mut Vec<String>,
    seen_keys: &mut BTreeSet<String>,
    cycles: &mut Vec<Vec<String>>,
) {
    match state.get(current).copied() {
        Some(VisitState::Visiting) | Some(VisitState::Done) => return,
        None => {}
    }
    state.insert(current.to_string(), VisitState::Visiting);
    stack.push(current.to_string());

    if let Some(edges) = adjacency.get(current) {
        for next in edges {
            match state.get(next.as_str()).copied() {
                Some(VisitState::Visiting) => {
                    if let Some(idx) = stack.iter().position(|candidate| candidate == next) {
                        let nodes = stack[idx..].iter().map(String::as_str).collect::<Vec<_>>();
                        let cycle = canonical_cycle(&nodes);
                        let key = cycle.join("->");
                        if seen_keys.insert(key) {
                            cycles.push(cycle);
                        }
                    }
                }
                Some(VisitState::Done) => {}
                None => detect_cycle(next, adjacency, state, stack, seen_keys, cycles),
            }
        }
    }

    let _ = stack.pop();
    state.insert(current.to_string(), VisitState::Done);
}

fn canonical_cycle(nodes: &[&str]) -> Vec<String> {
    if nodes.is_empty() {
        return Vec::new();
    }
    let mut best: Vec<&str> = nodes.to_vec();
    for offset in 1..nodes.len() {
        let mut rotated = nodes[offset..].to_vec();
        rotated.extend_from_slice(&nodes[..offset]);
        if rotated < best {
            best = rotated;
        }
    }
    let mut cycle: Vec<String> = best.iter().map(|value| (*value).to_string()).collect();
    cycle.push(best[0].to_string());
    cycle
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VisitState {
    Visiting,
    Done,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(prefix: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let dir = std::env::temp_dir().join(format!("{prefix}-{stamp}"));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn library_dependency_issues_report_cycles() {
        let root = temp_dir("trustlsp-library-cycle");
        let config = ProjectConfig::from_contents(
            &root,
            Some(root.join("trust-lsp.toml")),
            r#"
[[libraries]]
name = "A"
path = "libs/a"
dependencies = [{ name = "B" }]

[[libraries]]
name = "B"
path = "libs/b"
dependencies = [{ name = "A" }]
"#,
        );
        let issues = library_dependency_issues(&config);
        assert!(issues.iter().any(|issue| issue.code == "L004"));
        assert!(issues
            .iter()
            .any(|issue| issue.message.contains("A -> B -> A")));
        fs::remove_dir_all(root).ok();
    }
}

#[cfg(test)]
#[path = "library_graph/contract_tests.rs"]
mod contract_tests;
