fn write_registry_config(
    registry_root: &Path,
    section: &RegistryTomlSection,
) -> anyhow::Result<()> {
    let text = toml::to_string_pretty(&RegistryTomlFile {
        registry: section.clone(),
    })?;
    fs::write(registry_config_path(registry_root), text)
        .with_context(|| format!("failed to write {}", REGISTRY_CONFIG_FILE))?;
    Ok(())
}

fn load_registry_config(registry_root: &Path) -> anyhow::Result<RegistryTomlSection> {
    let path = registry_config_path(registry_root);
    if !path.is_file() {
        anyhow::bail!(
            "registry config missing at {} (run `trust-runtime registry init`)",
            path.display()
        );
    }
    let text = fs::read_to_string(&path)
        .with_context(|| format!("failed to read registry config {}", path.display()))?;
    let parsed: RegistryTomlFile = toml::from_str(&text)
        .with_context(|| format!("failed to parse registry config {}", path.display()))?;
    enforce_private_contract(&parsed.registry)?;
    Ok(parsed.registry)
}

fn enforce_private_contract(section: &RegistryTomlSection) -> anyhow::Result<()> {
    if section.version != REGISTRY_SCHEMA_VERSION {
        anyhow::bail!(
            "unsupported registry schema version {} (expected {})",
            section.version,
            REGISTRY_SCHEMA_VERSION
        );
    }
    if matches!(section.visibility, RegistryVisibility::Private)
        && section
            .auth_token
            .as_deref()
            .unwrap_or_default()
            .trim()
            .is_empty()
    {
        anyhow::bail!("private registry requires non-empty auth_token");
    }
    Ok(())
}

fn ensure_access(section: &RegistryTomlSection, token: Option<&str>) -> anyhow::Result<()> {
    if !matches!(section.visibility, RegistryVisibility::Private) {
        return Ok(());
    }
    let Some(expected) = section.auth_token.as_deref() else {
        anyhow::bail!("private registry requires auth_token");
    };
    let provided = token.unwrap_or_default().trim();
    if provided != expected {
        anyhow::bail!("unauthorized: invalid registry token");
    }
    Ok(())
}
