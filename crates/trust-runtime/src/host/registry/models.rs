#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RegistryVisibility {
    #[default]
    Public,
    Private,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrySettings {
    pub version: u32,
    pub visibility: RegistryVisibility,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryEndpoint {
    pub method: String,
    pub path: String,
    pub description: String,
    pub auth: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryMetadataModel {
    pub package_fields: Vec<String>,
    pub file_digest_fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryApiProfile {
    pub api_version: String,
    pub schema_version: u32,
    pub endpoints: Vec<RegistryEndpoint>,
    pub metadata_model: RegistryMetadataModel,
    pub private_registry_contract: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageFileDigest {
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageMetadata {
    pub name: String,
    pub version: String,
    pub resource_name: String,
    pub bundle_version: u32,
    pub published_at_unix: u64,
    pub total_bytes: u64,
    pub package_sha256: String,
    pub files: Vec<PackageFileDigest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageSummary {
    pub name: String,
    pub version: String,
    pub resource_name: String,
    pub published_at_unix: u64,
    pub total_bytes: u64,
    pub package_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryIndex {
    pub schema_version: u32,
    pub generated_at_unix: u64,
    pub packages: Vec<PackageSummary>,
}

impl Default for RegistryIndex {
    fn default() -> Self {
        Self {
            schema_version: REGISTRY_SCHEMA_VERSION,
            generated_at_unix: now_secs(),
            packages: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct RegistryTomlFile {
    registry: RegistryTomlSection,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct RegistryTomlSection {
    version: u32,
    visibility: RegistryVisibility,
    auth_token: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PublishRequest {
    pub registry_root: PathBuf,
    pub bundle_root: PathBuf,
    pub package_name: Option<String>,
    pub version: String,
    pub token: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PublishReport {
    pub package_root: PathBuf,
    pub metadata_path: PathBuf,
    pub metadata: PackageMetadata,
}

#[derive(Debug, Clone)]
pub struct DownloadRequest {
    pub registry_root: PathBuf,
    pub name: String,
    pub version: String,
    pub output_root: PathBuf,
    pub token: Option<String>,
    pub verify_before_install: bool,
}

#[derive(Debug, Clone)]
pub struct DownloadReport {
    pub output_root: PathBuf,
    pub metadata: PackageMetadata,
}

#[derive(Debug, Clone)]
pub struct VerifyRequest {
    pub registry_root: PathBuf,
    pub name: String,
    pub version: String,
    pub token: Option<String>,
}

#[derive(Debug, Clone)]
pub struct VerifyReport {
    pub metadata: PackageMetadata,
    pub verified_files: usize,
}

#[derive(Debug, Clone)]
pub struct ListRequest {
    pub registry_root: PathBuf,
    pub token: Option<String>,
}
