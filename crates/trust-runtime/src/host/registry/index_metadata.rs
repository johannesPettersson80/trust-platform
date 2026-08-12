fn update_registry_index(
    registry_root: &Path,
    metadata: &PackageMetadata,
    mut index: RegistryIndex,
) -> anyhow::Result<()> {
    if index
        .packages
        .iter()
        .any(|entry| entry.name == metadata.name && entry.version == metadata.version)
    {
        anyhow::bail!(
            "package already exists in registry index: {}/{}",
            metadata.name,
            metadata.version
        );
    }
    index.packages.push(PackageSummary {
        name: metadata.name.clone(),
        version: metadata.version.clone(),
        resource_name: metadata.resource_name.clone(),
        published_at_unix: metadata.published_at_unix,
        total_bytes: metadata.total_bytes,
        package_sha256: metadata.package_sha256.clone(),
    });
    index.packages.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then(left.version.cmp(&right.version))
    });
    index.generated_at_unix = now_secs();
    write_registry_index(registry_root, &index)
}

fn load_registry_index(registry_root: &Path) -> anyhow::Result<RegistryIndex> {
    let path = registry_index_path(registry_root);
    if !path.is_file() {
        return Ok(RegistryIndex::default());
    }
    let text =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let index: RegistryIndex = serde_json::from_str(&text)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    if index.schema_version != REGISTRY_SCHEMA_VERSION {
        anyhow::bail!(
            "unsupported registry index schema version {} (expected {})",
            index.schema_version,
            REGISTRY_SCHEMA_VERSION
        );
    }
    validate_registry_index_order(&index)?;
    Ok(index)
}

fn validate_registry_index_order(index: &RegistryIndex) -> anyhow::Result<()> {
    let mut previous: Option<(&str, &str)> = None;
    for package in &index.packages {
        validate_identifier("package name", package.name.as_str())?;
        validate_identifier("package version", package.version.as_str())?;
        let identity = (package.name.as_str(), package.version.as_str());
        if previous.is_some_and(|previous| previous >= identity) {
            anyhow::bail!(
                "registry index package identities must be unique and sorted by name then version"
            );
        }
        previous = Some(identity);
    }
    Ok(())
}

fn validate_summary_matches_metadata(
    summary: &PackageSummary,
    metadata: &PackageMetadata,
) -> anyhow::Result<()> {
    if summary.name != metadata.name
        || summary.version != metadata.version
        || summary.resource_name != metadata.resource_name
        || summary.published_at_unix != metadata.published_at_unix
        || summary.total_bytes != metadata.total_bytes
        || summary.package_sha256 != metadata.package_sha256
    {
        anyhow::bail!(
            "registry index summary disagrees with package metadata for {}/{}",
            summary.name,
            summary.version
        );
    }
    Ok(())
}

fn write_registry_index(registry_root: &Path, index: &RegistryIndex) -> anyhow::Result<()> {
    write_json_file(&registry_index_path(registry_root), index)
}

fn load_package_metadata(package_root: &Path) -> anyhow::Result<PackageMetadata> {
    let path = package_root.join(PACKAGE_METADATA_FILE);
    if !path.is_file() {
        anyhow::bail!("package metadata missing at {}", path.display());
    }
    let text =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let metadata: PackageMetadata = serde_json::from_str(&text)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(metadata)
}

fn validate_package_metadata(
    metadata: &PackageMetadata,
    expected_name: &str,
    expected_version: &str,
) -> anyhow::Result<()> {
    if metadata.name != expected_name || metadata.version != expected_version {
        anyhow::bail!(
            "package metadata identity mismatch: expected {expected_name}/{expected_version}, found {}/{}",
            metadata.name,
            metadata.version
        );
    }
    validate_identifier("package name", metadata.name.as_str())?;
    validate_identifier("package version", metadata.version.as_str())?;
    let total_bytes = metadata.files.iter().try_fold(0_u64, |total, file| {
        total
            .checked_add(file.bytes)
            .ok_or_else(|| anyhow::anyhow!("package metadata total_bytes overflow"))
    })?;
    if metadata.total_bytes != total_bytes {
        anyhow::bail!(
            "package metadata total_bytes mismatch: declared {}, files total {total_bytes}",
            metadata.total_bytes
        );
    }
    Ok(())
}

fn write_json_file(path: &Path, value: &impl Serialize) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let text = serde_json::to_string_pretty(value)?;
    fs::write(path, text).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}
