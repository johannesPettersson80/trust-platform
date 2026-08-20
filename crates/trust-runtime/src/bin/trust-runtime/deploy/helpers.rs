fn safe_state_changed(
    prev: &trust_runtime::io::IoSafeState,
    next: &trust_runtime::io::IoSafeState,
) -> bool {
    if prev.outputs.len() != next.outputs.len() {
        return true;
    }
    let mut matched = vec![false; next.outputs.len()];
    for (previous_address, previous_value) in &prev.outputs {
        let Some(index) =
            next.outputs
                .iter()
                .enumerate()
                .position(|(index, (next_address, next_value))| {
                    !matched[index]
                        && previous_address == next_address
                        && previous_value == next_value
                })
        else {
            return true;
        };
        matched[index] = true;
    }
    false
}

fn collect_sources(root: &Path) -> anyhow::Result<BTreeMap<String, Vec<u8>>> {
    let sources_root = root.join("src");
    match fs::metadata(&sources_root) {
        Ok(metadata) if metadata.is_dir() => {}
        Ok(_) => anyhow::bail!("source path is not a directory: {}", sources_root.display()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(BTreeMap::new());
        }
        Err(error) => {
            return Err(error)
                .with_context(|| format!("inspect source directory {}", sources_root.display()));
        }
    }
    let mut map = BTreeMap::new();
    collect_sources_from_directory(&sources_root, &sources_root, &mut map)?;
    Ok(map)
}

fn collect_sources_from_directory(
    sources_root: &Path,
    directory: &Path,
    map: &mut BTreeMap<String, Vec<u8>>,
) -> anyhow::Result<()> {
    let mut entries = fs::read_dir(directory)
        .with_context(|| format!("read source directory {}", directory.display()))?
        .collect::<Result<Vec<_>, _>>()
        .with_context(|| format!("enumerate source directory {}", directory.display()))?;
    entries.sort_by_key(std::fs::DirEntry::file_name);

    for entry in entries {
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("inspect source path {}", path.display()))?;
        if file_type.is_dir() {
            collect_sources_from_directory(sources_root, &path, map)?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        let is_source = path
            .extension()
            .and_then(std::ffi::OsStr::to_str)
            .is_some_and(|extension| {
                extension.eq_ignore_ascii_case("st") || extension.eq_ignore_ascii_case("pou")
            });
        if !is_source {
            continue;
        }
        let relative = path
            .strip_prefix(sources_root)
            .expect("recursive source path remains beneath source root")
            .to_str()
            .with_context(|| format!("source path is not valid UTF-8: {}", path.display()))?
            .replace(std::path::MAIN_SEPARATOR, "/");
        let bytes =
            fs::read(&path).with_context(|| format!("read source file {}", path.display()))?;
        map.insert(relative, bytes);
    }
    Ok(())
}

fn file_content_changed(
    previous_root: Option<&Path>,
    next_root: &Path,
    file_name: &str,
) -> anyhow::Result<bool> {
    let next_path = next_root.join(file_name);
    let next = fs::read(&next_path)
        .with_context(|| format!("read deployed configuration {}", next_path.display()))?;
    let Some(previous_root) = previous_root else {
        return Ok(true);
    };
    let previous_path = previous_root.join(file_name);
    let previous = fs::read(&previous_path)
        .with_context(|| format!("read previous configuration {}", previous_path.display()))?;
    Ok(previous != next)
}

fn optional_file_content_changed(
    previous_root: Option<&Path>,
    next_root: &Path,
    file_name: &str,
) -> anyhow::Result<bool> {
    let read_optional = |root: &Path| -> anyhow::Result<Option<Vec<u8>>> {
        let path = root.join(file_name);
        match fs::read(&path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error)
                .with_context(|| format!("read deployed configuration {}", path.display())),
        }
    };
    let next = read_optional(next_root)?;
    let Some(previous_root) = previous_root else {
        return Ok(true);
    };
    Ok(read_optional(previous_root)? != next)
}

fn diff_field<T: std::fmt::Display + PartialEq>(
    changes: &mut Vec<String>,
    name: &str,
    prev: &T,
    next: &T,
) {
    if prev != next {
        changes.push(format!("{name}: {prev} -> {next}"));
    }
}

fn token_state<T>(token: Option<&T>) -> &'static str {
    if token.is_some() {
        "set"
    } else {
        "unset"
    }
}

fn path_state(path: Option<&PathBuf>) -> String {
    path.map(|path| path.display().to_string())
        .unwrap_or_else(|| "none".to_string())
}
