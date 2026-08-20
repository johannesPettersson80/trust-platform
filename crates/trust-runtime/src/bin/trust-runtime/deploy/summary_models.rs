#[derive(Debug)]
struct SourceDiff {
    added: Vec<String>,
    removed: Vec<String>,
    modified: Vec<String>,
}

impl SourceDiff {
    fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.modified.is_empty()
    }
}

struct BundleChangeSummary {
    current_path: PathBuf,
    previous_path: Option<PathBuf>,
    runtime_changed: bool,
    runtime_changes: Vec<String>,
    io_changed: bool,
    io_changes: Vec<String>,
    bytecode_changed: bool,
    source_diff: SourceDiff,
}

impl BundleChangeSummary {
    fn new(previous: Option<&RuntimeBundle>, next: &RuntimeBundle) -> anyhow::Result<Self> {
        let previous_root = previous.map(|bundle| bundle.root.as_path());
        let current_path = fs::canonicalize(&next.root)
            .with_context(|| format!("resolve deployed bundle path {}", next.root.display()))?;
        let runtime_changed =
            file_content_changed(previous_root, next.root.as_path(), "runtime.toml")?;
        let runtime_changes = diff_runtime(previous.map(|b| &b.runtime), &next.runtime);
        let io_changed =
            optional_file_content_changed(previous_root, next.root.as_path(), "io.toml")?;
        let io_changes = diff_io(previous.map(|b| &b.io), &next.io);
        let bytecode_changed = previous
            .map(|b| b.bytecode != next.bytecode)
            .unwrap_or(true);
        let source_diff = diff_sources(previous_root, next.root.as_path())?;
        Ok(Self {
            current_path,
            previous_path: previous.map(|b| b.root.clone()),
            runtime_changed,
            runtime_changes,
            io_changed,
            io_changes,
            bytecode_changed,
            source_diff,
        })
    }

    fn print(&self) {
        println!("{}", self.render());
    }

    fn render(&self) -> String {
        let mut lines = Vec::new();
        lines.push("Deployment summary:".to_string());
        lines.push(format!(
            "current project version: {}",
            self.current_path.display()
        ));
        if let Some(path) = &self.previous_path {
            lines.push(format!("previous project version: {}", path.display()));
        } else {
            lines.push("previous project version: none".to_string());
        }
        if !self.runtime_changed {
            lines.push("runtime.toml: unchanged".to_string());
        } else {
            lines.push("runtime.toml changes:".to_string());
            if self.runtime_changes.is_empty() {
                lines.push("  - configuration content updated".to_string());
            } else {
                for change in &self.runtime_changes {
                    lines.push(format!("  - {change}"));
                }
            }
        }
        if !self.io_changed {
            lines.push("io.toml: unchanged".to_string());
        } else {
            lines.push("io.toml changes:".to_string());
            if self.io_changes.is_empty() {
                lines.push("  - configuration content updated".to_string());
            } else {
                for change in &self.io_changes {
                    lines.push(format!("  - {change}"));
                }
            }
        }
        lines.push(format!(
            "program.stbc: {}",
            if self.bytecode_changed {
                "updated"
            } else {
                "unchanged"
            }
        ));
        if self.source_diff.is_empty() {
            lines.push("sources: unchanged".to_string());
        } else {
            if !self.source_diff.added.is_empty() {
                lines.push(format!(
                    "sources added: {}",
                    self.source_diff.added.join(", ")
                ));
            }
            if !self.source_diff.removed.is_empty() {
                lines.push(format!(
                    "sources removed: {}",
                    self.source_diff.removed.join(", ")
                ));
            }
            if !self.source_diff.modified.is_empty() {
                lines.push(format!(
                    "sources modified: {}",
                    self.source_diff.modified.join(", ")
                ));
            }
        }
        lines.join("\n")
    }
}
