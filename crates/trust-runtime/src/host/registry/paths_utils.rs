fn package_root(registry_root: &Path, name: &str, version: &str) -> PathBuf {
    registry_packages_path(registry_root)
        .join(name)
        .join(version)
}

fn registry_config_path(registry_root: &Path) -> PathBuf {
    registry_root.join(REGISTRY_CONFIG_FILE)
}

fn registry_index_path(registry_root: &Path) -> PathBuf {
    registry_root.join(REGISTRY_INDEX_FILE)
}

fn registry_packages_path(registry_root: &Path) -> PathBuf {
    registry_root.join(REGISTRY_PACKAGES_DIR)
}

fn normalize_token(token: Option<String>) -> Option<String> {
    token.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn normalize_required_field(label: &str, value: &str) -> anyhow::Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        anyhow::bail!("{label} is required");
    }
    Ok(trimmed.to_string())
}

fn validate_identifier(label: &str, value: &str) -> anyhow::Result<()> {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    {
        return Ok(());
    }
    anyhow::bail!("{label} contains unsupported characters (allowed: A-Z a-z 0-9 - _ .)");
}

fn canonical_or_self(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn hex_string(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push_str(&format!("{byte:02x}"));
    }
    output
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
