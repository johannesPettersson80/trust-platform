// The files a build replaces (specification 11 §6.8.1d).

/// Writes `bytes` as the file at `path` through a temporary file beside it, which then replaces it,
/// so that a program started meanwhile reads the old file or the new one, never a part.
pub(crate) fn write_replacing(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let mut name = path
        .file_name()
        .map(std::ffi::OsStr::to_os_string)
        .unwrap_or_default();
    name.push(".tmp");
    let temporary = path.with_file_name(name);
    fs::write(&temporary, bytes).with_context(|| format!("write {}", temporary.display()))?;
    fs::rename(&temporary, path).with_context(|| format!("replace {}", path.display()))
}
