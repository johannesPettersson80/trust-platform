/// The file that holds a project's settings, its libraries and its build settings (specification
/// 14), by the names it may have, in the order they are looked for.
pub(crate) const PROJECT_SETTINGS_FILES: &[&str] =
    &["trust-lsp.toml", ".trust-lsp.toml", "trustlsp.toml"];

/// What a build of a project's program image wrote (specification 44 §4.1).
#[derive(Debug, Clone)]
pub struct BundleBuildReport {
    /// The program image.
    pub image: PathBuf,
    /// Its deployment record (specification 44 §10.1).
    pub record: PathBuf,
    /// The container of the image and its record that `trust-runtime ctl install` installs
    /// (specification 34 §5.1, §11.2).
    pub install: PathBuf,
    /// Whether the image is a debug image (specification 44 §13.2).
    pub debug: bool,
    /// The image's size in bytes.
    pub image_bytes: usize,
    /// The memory figure the image carries.
    pub figure: trust_runtime_core::native::image::MemoryFigure,
    /// The public key that signed the image, hexadecimal (specification 44 §5.11).
    pub signer: String,
    /// Hash-bound OpenOT definition generated when the source set declares OpenOT data.
    pub openot_definition_path: Option<PathBuf>,
    /// Source files included in the build.
    pub sources: Vec<PathBuf>,
    /// Resolved dependency roots included in this build.
    pub dependency_roots: Vec<PathBuf>,
    /// Resolved dependency names in deterministic order.
    pub resolved_dependencies: Vec<String>,
    /// The report, also written as `build/build.json`.
    pub json: serde_json::Value,
}

/// What a check of a project's program found, without building it (specification 44 §4.1).
#[derive(Debug, Clone)]
pub struct BundleCheckReport {
    /// Source files included in the check.
    pub sources: Vec<PathBuf>,
    /// Resolved dependency roots included in this check.
    pub dependency_roots: Vec<PathBuf>,
    /// Resolved dependency names in deterministic order.
    pub resolved_dependencies: Vec<String>,
    /// The names of the resources the program's configuration declares: none for a program
    /// without a `CONFIGURATION`, which runs under the name its project gives it (specification
    /// 11 §6.7.1).
    pub resources: Vec<String>,
}

/// Project layout summary without building the program.
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
