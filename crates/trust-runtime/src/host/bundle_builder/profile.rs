/// The project manifest (`trust-lsp.toml`, `.trust-lsp.toml` or `trustlsp.toml`) in
/// `project_root`, found as the language server and the dependency resolver find it.
#[must_use]
pub fn find_project_manifest(project_root: &Path) -> Option<PathBuf> {
    find_dependency_manifest(project_root)
}
