use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SoftwareMap {
    pub schema_version: u32,
    pub workspace_root: String,
    pub generated_by: String,
    pub packages: Vec<PackageSummary>,
    pub workspace_edges: Vec<WorkspaceEdge>,
    pub crate_module_summaries: Vec<ModuleSummary>,
    pub source_files: Vec<SourceFileSummary>,
    pub largest_files: Vec<SourceFileSummary>,
    pub import_edges: Vec<ImportEdge>,
    pub runtime_top_level_modules: Vec<String>,
    pub runtime_cli_commands: Vec<String>,
    pub runtime_cli_actions: Vec<CliActionSummary>,
    pub runtime_bin_modules: Vec<String>,
    pub unsafe_summary: UnsafeSummary,
    pub diagram_facts: Vec<DiagramFact>,
    pub tool_results: Vec<ToolResult>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PackageSummary {
    pub name: String,
    pub manifest_path: String,
    pub targets: Vec<TargetSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TargetSummary {
    pub name: String,
    pub kind: Vec<String>,
    pub src_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkspaceEdge {
    pub from: String,
    pub to: String,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModuleSummary {
    pub crate_name: String,
    pub module_name: String,
    pub path: String,
    pub file_count: usize,
    pub line_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceFileSummary {
    pub path: String,
    pub line_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ImportEdge {
    pub from_file: String,
    pub from_module: String,
    pub to_module: String,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CliActionSummary {
    pub name: String,
    pub variants: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct UnsafeSummary {
    pub unsafe_occurrences: usize,
    pub panic_like_occurrences: usize,
    pub owner: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiagramFact {
    pub path: String,
    pub components: Vec<String>,
    pub edges: Vec<DiagramEdge>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiagramEdge {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ToolResult {
    pub name: String,
    pub status: ToolStatus,
    pub details: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolStatus {
    Pass,
    Finding,
    Partial,
    Failed,
    NotRun,
}

impl ToolStatus {
    pub const ALL: [Self; 5] = [
        Self::Pass,
        Self::Finding,
        Self::Partial,
        Self::Failed,
        Self::NotRun,
    ];
}

impl SoftwareMap {
    pub fn new(workspace_root: impl Into<String>) -> Self {
        Self {
            schema_version: 1,
            workspace_root: workspace_root.into(),
            generated_by: "cargo xtask architecture-doctor --full-map".to_string(),
            packages: Vec::new(),
            workspace_edges: Vec::new(),
            crate_module_summaries: Vec::new(),
            source_files: Vec::new(),
            largest_files: Vec::new(),
            import_edges: Vec::new(),
            runtime_top_level_modules: Vec::new(),
            runtime_cli_commands: Vec::new(),
            runtime_cli_actions: Vec::new(),
            runtime_bin_modules: Vec::new(),
            unsafe_summary: UnsafeSummary::default(),
            diagram_facts: Vec::new(),
            tool_results: Vec::new(),
        }
    }

    pub fn to_stable_json(&self) -> serde_json::Result<String> {
        let mut map = self.clone();
        map.sort_stable();
        serde_json::to_string_pretty(&map)
    }

    fn sort_stable(&mut self) {
        self.packages.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then_with(|| left.manifest_path.cmp(&right.manifest_path))
        });
        for package in &mut self.packages {
            package.targets.sort_by(|left, right| {
                left.name
                    .cmp(&right.name)
                    .then_with(|| left.src_path.cmp(&right.src_path))
            });
        }
        self.workspace_edges.sort_by(|left, right| {
            left.from
                .cmp(&right.from)
                .then_with(|| left.to.cmp(&right.to))
                .then_with(|| left.kind.cmp(&right.kind))
        });
        self.crate_module_summaries.sort_by(|left, right| {
            left.crate_name
                .cmp(&right.crate_name)
                .then_with(|| left.module_name.cmp(&right.module_name))
                .then_with(|| left.path.cmp(&right.path))
        });
        self.source_files
            .sort_by(|left, right| left.path.cmp(&right.path));
        self.largest_files.sort_by(|left, right| {
            right
                .line_count
                .cmp(&left.line_count)
                .then_with(|| left.path.cmp(&right.path))
        });
        self.import_edges.sort_by(|left, right| {
            left.from_file
                .cmp(&right.from_file)
                .then_with(|| left.line.cmp(&right.line))
                .then_with(|| left.to_module.cmp(&right.to_module))
        });
        self.runtime_top_level_modules.sort();
        self.runtime_top_level_modules.dedup();
        self.runtime_cli_commands.sort();
        self.runtime_cli_actions
            .sort_by(|left, right| left.name.cmp(&right.name));
        for action in &mut self.runtime_cli_actions {
            action.variants.sort();
        }
        self.runtime_bin_modules.sort();
        self.runtime_bin_modules.dedup();
        self.diagram_facts
            .sort_by(|left, right| left.path.cmp(&right.path));
        for diagram in &mut self.diagram_facts {
            diagram.components.sort();
            diagram.components.dedup();
            diagram.edges.sort_by(|left, right| {
                left.from
                    .cmp(&right.from)
                    .then_with(|| left.to.cmp(&right.to))
            });
            diagram.edges.dedup();
        }
        self.tool_results
            .sort_by(|left, right| left.name.cmp(&right.name));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_json_does_not_depend_on_collection_order() {
        let forward = sample_map(false).to_stable_json().unwrap();
        let reverse = sample_map(true).to_stable_json().unwrap();

        assert_eq!(forward, reverse);
        assert!(forward.contains("\"schema_version\": 1"));
        assert!(forward.contains("\"status\": \"not_run\""));
    }

    #[test]
    fn stable_json_sorts_nested_targets() {
        let json = sample_map(false).to_stable_json().unwrap();
        let alpha_idx = json.find("\"alpha-bin\"").unwrap();
        let zeta_idx = json.find("\"zeta-lib\"").unwrap();

        assert!(alpha_idx < zeta_idx);
    }

    fn sample_map(reverse: bool) -> SoftwareMap {
        let mut map = SoftwareMap::new("/repo");
        map.packages = vec![
            PackageSummary {
                name: "zeta".to_string(),
                manifest_path: "crates/zeta/Cargo.toml".to_string(),
                targets: vec![
                    TargetSummary {
                        name: "zeta-lib".to_string(),
                        kind: vec!["lib".to_string()],
                        src_path: "crates/zeta/src/lib.rs".to_string(),
                    },
                    TargetSummary {
                        name: "alpha-bin".to_string(),
                        kind: vec!["bin".to_string()],
                        src_path: "crates/zeta/src/main.rs".to_string(),
                    },
                ],
            },
            PackageSummary {
                name: "alpha".to_string(),
                manifest_path: "crates/alpha/Cargo.toml".to_string(),
                targets: Vec::new(),
            },
        ];
        map.workspace_edges = vec![
            WorkspaceEdge {
                from: "zeta".to_string(),
                to: "alpha".to_string(),
                kind: "normal".to_string(),
            },
            WorkspaceEdge {
                from: "alpha".to_string(),
                to: "zeta".to_string(),
                kind: "dev".to_string(),
            },
        ];
        map.crate_module_summaries = vec![
            ModuleSummary {
                crate_name: "zeta".to_string(),
                module_name: "runtime".to_string(),
                path: "crates/zeta/src/runtime".to_string(),
                file_count: 2,
                line_count: 20,
            },
            ModuleSummary {
                crate_name: "alpha".to_string(),
                module_name: "lib".to_string(),
                path: "crates/alpha/src/lib.rs".to_string(),
                file_count: 1,
                line_count: 5,
            },
        ];
        map.source_files = vec![
            SourceFileSummary {
                path: "crates/zeta/src/lib.rs".to_string(),
                line_count: 10,
            },
            SourceFileSummary {
                path: "crates/alpha/src/lib.rs".to_string(),
                line_count: 5,
            },
        ];
        map.largest_files = map.source_files.clone();
        map.import_edges = vec![ImportEdge {
            from_file: "crates/zeta/src/lib.rs".to_string(),
            from_module: "zeta".to_string(),
            to_module: "alpha".to_string(),
            line: 1,
        }];
        map.runtime_top_level_modules = vec!["web".to_string(), "control".to_string()];
        map.runtime_cli_commands = vec!["Run".to_string(), "Agent".to_string()];
        map.runtime_cli_actions = vec![CliActionSummary {
            name: "BenchAction".to_string(),
            variants: vec!["Project".to_string(), "Init".to_string()],
        }];
        map.runtime_bin_modules = vec!["run".to_string(), "agent".to_string()];
        map.unsafe_summary = UnsafeSummary {
            unsafe_occurrences: 1,
            panic_like_occurrences: 2,
            owner: "architecture".to_string(),
            status: "tracked".to_string(),
        };
        map.diagram_facts = vec![DiagramFact {
            path: "docs/diagrams/example.puml".to_string(),
            components: vec!["crate_alpha".to_string(), "crate_zeta".to_string()],
            edges: vec![DiagramEdge {
                from: "crate_alpha".to_string(),
                to: "crate_zeta".to_string(),
            }],
        }];
        map.tool_results = vec![
            ToolResult {
                name: "zeta-tool".to_string(),
                status: ToolStatus::Pass,
                details: vec!["ok".to_string()],
            },
            ToolResult {
                name: "alpha-tool".to_string(),
                status: ToolStatus::NotRun,
                details: vec!["pending".to_string()],
            },
        ];
        if reverse {
            map.packages.reverse();
            for package in &mut map.packages {
                package.targets.reverse();
            }
            map.workspace_edges.reverse();
            map.crate_module_summaries.reverse();
            map.source_files.reverse();
            map.largest_files.reverse();
            map.import_edges.reverse();
            map.runtime_top_level_modules.reverse();
            map.runtime_cli_commands.reverse();
            map.runtime_cli_actions.reverse();
            for action in &mut map.runtime_cli_actions {
                action.variants.reverse();
            }
            map.runtime_bin_modules.reverse();
            map.diagram_facts.reverse();
            map.tool_results.reverse();
        }
        map
    }
}
