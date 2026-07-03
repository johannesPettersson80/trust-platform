fn safe_state_changed(
    prev: &trust_runtime::io::IoSafeState,
    next: &trust_runtime::io::IoSafeState,
) -> bool {
    if prev.outputs.len() != next.outputs.len() {
        return true;
    }
    let mut prev_set = BTreeSet::new();
    for (address, value) in &prev.outputs {
        prev_set.insert((format_address(address), format!("{value:?}")));
    }
    let mut next_set = BTreeSet::new();
    for (address, value) in &next.outputs {
        next_set.insert((format_address(address), format!("{value:?}")));
    }
    prev_set != next_set
}

fn format_address(address: &IoAddress) -> String {
    let area = match address.area {
        trust_runtime::memory::IoArea::Input => "I",
        trust_runtime::memory::IoArea::Output => "Q",
        trust_runtime::memory::IoArea::Memory => "M",
    };
    let size = match address.size {
        trust_runtime::io::IoSize::Bit => "X",
        trust_runtime::io::IoSize::Byte => "B",
        trust_runtime::io::IoSize::Word => "W",
        trust_runtime::io::IoSize::DWord => "D",
        trust_runtime::io::IoSize::LWord => "L",
        trust_runtime::io::IoSize::Bytes(_) => "B",
    };
    if address.wildcard {
        return format!("%{area}{size}*");
    }
    if address.size == trust_runtime::io::IoSize::Bit {
        format!("%{area}{size}{}.{}", address.byte, address.bit)
    } else {
        format!("%{area}{size}{}", address.byte)
    }
}

fn collect_sources(root: &Path) -> anyhow::Result<BTreeMap<String, Vec<u8>>> {
    let sources_root = root.join("src");
    if !sources_root.is_dir() {
        return Ok(BTreeMap::new());
    }
    let mut map = BTreeMap::new();
    let patterns = ["**/*.st", "**/*.ST", "**/*.pou", "**/*.POU"];
    for pattern in patterns {
        for entry in glob::glob(&format!("{}/{}", sources_root.display(), pattern))? {
            let path = entry?;
            if !path.is_file() {
                continue;
            }
            let relative = path
                .strip_prefix(&sources_root)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();
            if map.contains_key(&relative) {
                continue;
            }
            let bytes = fs::read(&path)?;
            map.insert(relative, bytes);
        }
    }
    Ok(map)
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
