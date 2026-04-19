const DEPENDENCY_MANIFEST_FILES: &[&str] = &[
    "trust-lsp.toml",
    ".trust-lsp.toml",
    "trustlsp.toml",
];

/// Build output summary for a bundle.
#[derive(Debug, Clone)]
pub struct BundleBuildReport {
    /// Written bytecode path (program.stbc).
    pub program_path: PathBuf,
    /// Source files included in the build.
    pub sources: Vec<PathBuf>,
    /// Resolved dependency roots included in this build.
    pub dependency_roots: Vec<PathBuf>,
    /// Resolved dependency names in deterministic order.
    pub resolved_dependencies: Vec<String>,
}

/// Project layout summary without building bytecode.
#[derive(Debug, Clone)]
pub struct ProjectInspectionReport {
    /// Resolved project source root.
    pub sources_root: PathBuf,
    /// Project manifest path if one was found.
    pub manifest_path: Option<PathBuf>,
    /// Source files included from the project and resolved local dependencies.
    pub sources: Vec<PathBuf>,
    /// Resolved dependency roots included in this project.
    pub dependency_roots: Vec<PathBuf>,
    /// Resolved dependency names in deterministic order.
    pub resolved_dependencies: Vec<String>,
}

#[derive(Debug, Clone)]
struct DependencySpec {
    name: String,
    path: PathBuf,
    version: Option<String>,
}

#[derive(Debug, Clone)]
struct ResolvedDependency {
    name: String,
    path: PathBuf,
    version: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DependencyVisitState {
    Visiting,
    Done,
}

#[derive(Debug, Default, Deserialize)]
struct DependencyManifestFile {
    #[serde(default)]
    package: PackageSection,
    #[serde(default)]
    dependencies: BTreeMap<String, ManifestDependencyEntry>,
}

#[derive(Debug, Default, Deserialize)]
struct PackageSection {
    version: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ManifestDependencyEntry {
    Path(String),
    Detailed(ManifestDependencySection),
}

impl ManifestDependencyEntry {
    fn path(&self) -> &str {
        match self {
            ManifestDependencyEntry::Path(path) => path,
            ManifestDependencyEntry::Detailed(section) => section.path.as_str(),
        }
    }

    fn version(&self) -> Option<String> {
        match self {
            ManifestDependencyEntry::Path(_) => None,
            ManifestDependencyEntry::Detailed(section) => section.version.clone(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct ManifestDependencySection {
    path: String,
    version: Option<String>,
}
