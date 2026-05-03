fn update_registry_index(registry_root: &Path, metadata: &PackageMetadata) -> anyhow::Result<()> {
    let mut index = load_registry_index(registry_root).unwrap_or_default();
    index
        .packages
        .retain(|entry| !(entry.name == metadata.name && entry.version == metadata.version));
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
    Ok(index)
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

fn write_json_file(path: &Path, value: &impl Serialize) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let text = serde_json::to_string_pretty(value)?;
    fs::write(path, text).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}
