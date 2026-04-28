use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SoftwareMap {
    pub schema_version: u32,
    pub workspace_root: String,
    pub generated_by: String,
    pub packages: Vec<PackageSummary>,
    pub workspace_edges: Vec<WorkspaceEdge>,
    pub source_files: Vec<SourceFileSummary>,
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
pub struct SourceFileSummary {
    pub path: String,
    pub line_count: usize,
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
            source_files: Vec::new(),
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
        self.source_files
            .sort_by(|left, right| left.path.cmp(&right.path));
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
            map.source_files.reverse();
            map.tool_results.reverse();
        }
        map
    }
}
