pub fn init_registry(
    registry_root: &Path,
    visibility: RegistryVisibility,
    auth_token: Option<String>,
) -> anyhow::Result<RegistrySettings> {
    ensure_registry_layout(registry_root)?;
    let section = RegistryTomlSection {
        version: REGISTRY_SCHEMA_VERSION,
        visibility,
        auth_token: normalize_token(auth_token),
    };
    enforce_private_contract(&section)?;
    write_registry_config(registry_root, &section)?;
    if !registry_index_path(registry_root).is_file() {
        write_registry_index(registry_root, &RegistryIndex::default())?;
    }
    Ok(RegistrySettings {
        version: section.version,
        visibility: section.visibility,
    })
}

pub fn load_registry_settings(registry_root: &Path) -> anyhow::Result<RegistrySettings> {
    let section = load_registry_config(registry_root)?;
    Ok(RegistrySettings {
        version: section.version,
        visibility: section.visibility,
    })
}

pub fn publish_package(request: PublishRequest) -> anyhow::Result<PublishReport> {
    let section = load_registry_config(&request.registry_root)?;
    ensure_access(&section, request.token.as_deref())?;

    let bundle_root = canonical_or_self(&request.bundle_root);
    let bundle = RuntimeBundle::load(&bundle_root)
        .map_err(|err| anyhow::anyhow!("invalid bundle '{}': {err}", bundle_root.display()))?;

    let package_name = request
        .package_name
        .unwrap_or_else(|| bundle.runtime.resource_name.to_string());
    let package_name = normalize_required_field("package name", package_name.as_str())?;
    validate_identifier("package name", package_name.as_str())?;

    let version = normalize_required_field("package version", request.version.as_str())?;
    validate_identifier("package version", version.as_str())?;

    let package_root = package_root(&request.registry_root, &package_name, &version);
    if package_root.exists() {
        anyhow::bail!("package already exists: {}/{}", package_name, version);
    }
    let bundle_out = package_root.join("bundle");
    copy_dir_recursive(&bundle_root, &bundle_out)?;

    let files = collect_file_digests(&bundle_out)?;
    if files.is_empty() {
        anyhow::bail!("package payload is empty");
    }
    let total_bytes = files.iter().map(|entry| entry.bytes).sum();
    let package_sha256 = aggregate_package_sha(&files);

    let metadata = PackageMetadata {
        name: package_name,
        version,
        resource_name: bundle.runtime.resource_name.to_string(),
        bundle_version: bundle.runtime.bundle_version,
        published_at_unix: now_secs(),
        total_bytes,
        package_sha256,
        files,
    };
    let metadata_path = package_root.join(PACKAGE_METADATA_FILE);
    write_json_file(&metadata_path, &metadata)?;
    update_registry_index(&request.registry_root, &metadata)?;

    Ok(PublishReport {
        package_root,
        metadata_path,
        metadata,
    })
}

pub fn download_package(request: DownloadRequest) -> anyhow::Result<DownloadReport> {
    let section = load_registry_config(&request.registry_root)?;
    ensure_access(&section, request.token.as_deref())?;

    let package_root = package_root(&request.registry_root, &request.name, &request.version);
    let metadata = load_package_metadata(&package_root)?;
    if request.verify_before_install {
        verify_bundle_tree_against_metadata(&package_root.join("bundle"), &metadata)?;
    }

    ensure_empty_output_dir(&request.output_root)?;
    copy_dir_recursive(&package_root.join("bundle"), &request.output_root)?;
    if request.verify_before_install {
        verify_bundle_tree_against_metadata(&request.output_root, &metadata)?;
    }

    Ok(DownloadReport {
        output_root: request.output_root,
        metadata,
    })
}

pub fn verify_package(request: VerifyRequest) -> anyhow::Result<VerifyReport> {
    let section = load_registry_config(&request.registry_root)?;
    ensure_access(&section, request.token.as_deref())?;
    let package_root = package_root(&request.registry_root, &request.name, &request.version);
    let metadata = load_package_metadata(&package_root)?;
    verify_bundle_tree_against_metadata(&package_root.join("bundle"), &metadata)?;
    Ok(VerifyReport {
        verified_files: metadata.files.len(),
        metadata,
    })
}

pub fn list_packages(request: ListRequest) -> anyhow::Result<Vec<PackageSummary>> {
    let section = load_registry_config(&request.registry_root)?;
    ensure_access(&section, request.token.as_deref())?;
    let index = load_registry_index(&request.registry_root)?;
    Ok(index.packages)
}
