fn resolve_suite_root(suite_root: Option<PathBuf>) -> anyhow::Result<PathBuf> {
    let root = match suite_root {
        Some(path) => path,
        None => {
            let cwd = std::env::current_dir().context("resolve current directory")?;
            cwd.join("conformance")
        }
    };
    if !root.is_dir() {
        bail!(
            "conformance suite root '{}' does not exist or is not a directory",
            root.display()
        );
    }
    Ok(root)
}

fn discover_cases(suite_root: &Path) -> anyhow::Result<Vec<CaseDefinition>> {
    let cases_root = suite_root.join("cases");
    if cases_root.is_dir() {
        let mut categories = fs::read_dir(&cases_root)
            .with_context(|| format!("read conformance cases root '{}'", cases_root.display()))?
            .collect::<Result<Vec<_>, _>>()
            .with_context(|| format!("list conformance cases root '{}'", cases_root.display()))?;
        categories.sort_by_key(|entry| entry.file_name());
        for entry in categories {
            if !entry
                .file_type()
                .with_context(|| {
                    format!("inspect conformance category '{}'", entry.path().display())
                })?
                .is_dir()
            {
                continue;
            }
            let category = entry.file_name().to_string_lossy().to_string();
            if !CATEGORIES.contains(&category.as_str()) {
                bail!(
                    "unknown conformance category directory '{}'",
                    entry.path().display()
                );
            }
        }
    }

    let mut cases = Vec::new();
    for category in CATEGORIES {
        let category_root = suite_root.join("cases").join(category);
        if !category_root.is_dir() {
            continue;
        }
        let mut entries = fs::read_dir(&category_root)
            .with_context(|| format!("read case category '{}'", category_root.display()))?
            .collect::<Result<Vec<_>, _>>()
            .with_context(|| format!("list case category '{}'", category_root.display()))?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let case_id = entry.file_name().to_string_lossy().to_string();
            let manifest_path = path.join("manifest.toml");
            if !manifest_path.is_file() {
                bail!(
                    "conformance case directory '{}' is missing required manifest.toml",
                    path.display()
                );
            }
            let manifest = parse_manifest(&manifest_path)?;
            if manifest.id != case_id {
                bail!(
                    "manifest id '{}' does not match case directory '{}'",
                    manifest.id,
                    case_id
                );
            }
            if manifest.category != category {
                bail!(
                    "manifest category '{}' does not match directory category '{}'",
                    manifest.category,
                    category
                );
            }
            if !is_valid_case_id(&manifest.id, category) {
                bail!(
                    "case id '{}' violates conformance naming rules for category '{}'",
                    manifest.id,
                    category
                );
            }
            cases.push(CaseDefinition {
                id: case_id,
                category: category.to_string(),
                dir: path,
                manifest,
            });
        }
    }
    Ok(cases)
}

fn parse_manifest(path: &Path) -> anyhow::Result<CaseManifest> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("read case manifest '{}'", path.display()))?;
    let mut manifest: CaseManifest =
        toml::from_str(&text).with_context(|| format!("parse manifest '{}'", path.display()))?;
    if manifest.sources.is_empty() {
        manifest.sources = vec!["program.st".to_string()];
    }
    if manifest.id.trim().is_empty() {
        bail!("manifest '{}' is missing non-empty `id`", path.display());
    }
    if manifest.category.trim().is_empty() {
        bail!(
            "manifest '{}' is missing non-empty `category`",
            path.display()
        );
    }
    Ok(manifest)
}
