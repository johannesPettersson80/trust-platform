fn load_sources(root: &Path) -> anyhow::Result<Vec<LoadedSource>> {
    let mut paths = BTreeSet::new();
    collect_source_paths(root, &mut paths)?;

    let mut sources = Vec::with_capacity(paths.len());
    for path in paths {
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read source '{}'", path.display()))?;
        sources.push(LoadedSource { path, text });
    }
    Ok(sources)
}

fn collect_source_paths(root: &Path, paths: &mut BTreeSet<PathBuf>) -> anyhow::Result<()> {
    for entry in
        std::fs::read_dir(root).with_context(|| format!("failed to read '{}'", root.display()))?
    {
        let entry =
            entry.with_context(|| format!("failed to read entry in '{}'", root.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to read file type for '{}'", path.display()))?;
        if file_type.is_dir() {
            collect_source_paths(&path, paths)?;
        } else if file_type.is_file() && is_st_source_path(&path) {
            paths.insert(path);
        }
    }
    Ok(())
}

fn is_st_source_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            let extension = extension.to_ascii_lowercase();
            extension == "st" || extension == "pou"
        })
}

/// The test declarations of `sources`, as `trust_runtime::test_discovery` finds them for every tool
/// (specification 22, "Test POU discovery").
fn discover_tests(sources: &[LoadedSource]) -> Vec<DiscoveredTest> {
    trust_runtime::test_discovery::discover_tests(
        sources
            .iter()
            .map(|source| (source.path.as_path(), source.text.as_str())),
    )
}
