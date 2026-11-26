//! Package registry command handlers.

use std::path::PathBuf;

use trust_runtime::bundle::detect_bundle_path;
use trust_runtime::registry::{
    download_package, init_registry, list_packages, publish_package, registry_api_profile,
    verify_package, DownloadRequest, ListRequest, PublishRequest, RegistryVisibility,
    VerifyRequest,
};

use crate::cli::{RegistryAction, RegistryVisibilityArg};
use crate::style;

pub fn run_registry(action: RegistryAction) -> anyhow::Result<()> {
    match action {
        RegistryAction::Profile { json } => run_profile(json),
        RegistryAction::Init {
            root,
            visibility,
            token,
        } => run_init(root, visibility, token),
        RegistryAction::Publish {
            registry,
            project,
            name,
            version,
            token,
        } => run_publish(registry, project, name, version, token),
        RegistryAction::Download {
            registry,
            name,
            version,
            output,
            token,
            verify,
        } => run_download(registry, name, version, output, token, verify),
        RegistryAction::Verify {
            registry,
            name,
            version,
            token,
        } => run_verify(registry, name, version, token),
        RegistryAction::List {
            registry,
            token,
            json,
        } => run_list(registry, token, json),
    }
}

fn run_profile(json: bool) -> anyhow::Result<()> {
    let profile = registry_api_profile();
    if json {
        outln!("{}", serde_json::to_string_pretty(&profile)?);
        return Ok(());
    }
    outln!("{}", style::accent("Package registry profile"));
    outln!("API version: {}", profile.api_version);
    outln!("Schema version: {}", profile.schema_version);
    outln!("Endpoints:");
    for endpoint in &profile.endpoints {
        outln!(
            " - {} {} ({})",
            endpoint.method,
            endpoint.path,
            endpoint.auth
        );
    }
    outln!("Metadata package fields:");
    for field in &profile.metadata_model.package_fields {
        outln!(" - {field}");
    }
    outln!("Metadata file digest fields:");
    for field in &profile.metadata_model.file_digest_fields {
        outln!(" - {field}");
    }
    outln!("Private registry contract:");
    for rule in &profile.private_registry_contract {
        outln!(" - {rule}");
    }
    Ok(())
}

fn run_init(
    root: PathBuf,
    visibility: RegistryVisibilityArg,
    token: Option<String>,
) -> anyhow::Result<()> {
    let settings = init_registry(&root, into_visibility(visibility), token)?;
    outln!(
        "{}",
        style::success(format!("Initialized registry at {}", root.display()))
    );
    outln!(
        "Visibility: {}",
        match settings.visibility {
            RegistryVisibility::Public => "public",
            RegistryVisibility::Private => "private",
        }
    );
    outln!("Schema version: {}", settings.version);
    Ok(())
}

fn run_publish(
    registry: PathBuf,
    project: Option<PathBuf>,
    name: Option<String>,
    version: String,
    token: Option<String>,
) -> anyhow::Result<()> {
    let bundle_root = resolve_project(project)?;
    let report = publish_package(PublishRequest {
        registry_root: registry.clone(),
        bundle_root,
        package_name: name,
        version,
        token,
    })?;
    outln!(
        "{}",
        style::success(format!(
            "Published {}/{}",
            report.metadata.name, report.metadata.version
        ))
    );
    outln!("Registry: {}", registry.display());
    outln!("Package root: {}", report.package_root.display());
    outln!("Metadata: {}", report.metadata_path.display());
    outln!("Files: {}", report.metadata.files.len());
    outln!("Bytes: {}", report.metadata.total_bytes);
    outln!("SHA-256: {}", report.metadata.package_sha256);
    Ok(())
}

fn run_download(
    registry: PathBuf,
    name: String,
    version: String,
    output: PathBuf,
    token: Option<String>,
    verify: bool,
) -> anyhow::Result<()> {
    let report = download_package(DownloadRequest {
        registry_root: registry.clone(),
        name: name.clone(),
        version: version.clone(),
        output_root: output.clone(),
        token,
        verify_before_install: verify,
    })?;
    outln!(
        "{}",
        style::success(format!(
            "Downloaded {}/{} to {}",
            name,
            version,
            report.output_root.display()
        ))
    );
    outln!("Registry: {}", registry.display());
    outln!("Files: {}", report.metadata.files.len());
    outln!("Bytes: {}", report.metadata.total_bytes);
    outln!("SHA-256: {}", report.metadata.package_sha256);
    Ok(())
}

fn run_verify(
    registry: PathBuf,
    name: String,
    version: String,
    token: Option<String>,
) -> anyhow::Result<()> {
    let report = verify_package(VerifyRequest {
        registry_root: registry.clone(),
        name: name.clone(),
        version: version.clone(),
        token,
    })?;
    outln!(
        "{}",
        style::success(format!(
            "Verified {}/{}",
            report.metadata.name, report.metadata.version
        ))
    );
    outln!("Registry: {}", registry.display());
    outln!("Files: {}", report.verified_files);
    outln!("SHA-256: {}", report.metadata.package_sha256);
    Ok(())
}

fn run_list(registry: PathBuf, token: Option<String>, json: bool) -> anyhow::Result<()> {
    let packages = list_packages(ListRequest {
        registry_root: registry.clone(),
        token,
    })?;
    if json {
        outln!("{}", serde_json::to_string_pretty(&packages)?);
        return Ok(());
    }
    outln!("{}", style::accent("Published packages"));
    outln!("Registry: {}", registry.display());
    if packages.is_empty() {
        outln!("{}", style::warning("No packages found."));
        return Ok(());
    }
    for package in &packages {
        outln!(
            " - {}/{} (resource: {}, bytes: {}, sha256: {})",
            package.name,
            package.version,
            package.resource_name,
            package.total_bytes,
            package.package_sha256
        );
    }
    Ok(())
}

fn into_visibility(value: RegistryVisibilityArg) -> RegistryVisibility {
    match value {
        RegistryVisibilityArg::Public => RegistryVisibility::Public,
        RegistryVisibilityArg::Private => RegistryVisibility::Private,
    }
}

fn resolve_project(project: Option<PathBuf>) -> anyhow::Result<PathBuf> {
    match project {
        Some(path) => Ok(path),
        None => detect_bundle_path(None).map_err(anyhow::Error::from),
    }
}
