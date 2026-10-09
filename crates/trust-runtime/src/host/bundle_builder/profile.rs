/// The conversion rules a project selects with `vendor_profile` in the `[project]` table of
/// its manifest (`trust-lsp.toml`), as the language server reads it
/// (docs/specs/07-standard-functions.md 2.10): the CODESYS rules for `codesys`, `beckhoff`
/// and `twincat`, the IEC rules for `siemens`, `mitsubishi`, `gxworks3`, `iec` and without a
/// manifest or `vendor_profile`. A value that is not a vendor profile is an error, so a
/// misspelled profile cannot silently run the project under other conversion rules.
pub fn project_conversion_profile(
    project_root: &Path,
) -> anyhow::Result<crate::stdlib::conversions::ConversionProfile> {
    use crate::stdlib::conversions::{ConversionProfile, VENDOR_PROFILES};

    let manifest = load_dependency_manifest(project_root).with_context(|| {
        format!(
            "failed to load project manifest at {}",
            project_root.display()
        )
    })?;
    let Some(name) = manifest.project.vendor_profile.as_deref() else {
        return Ok(ConversionProfile::Iec);
    };
    if name.trim().is_empty() {
        return Ok(ConversionProfile::Iec);
    }
    ConversionProfile::from_vendor_profile(name).ok_or_else(|| {
        let manifest_path = find_dependency_manifest(project_root)
            .unwrap_or_else(|| project_root.join(DEPENDENCY_MANIFEST_FILES[0]));
        anyhow::anyhow!(
            "unknown vendor_profile '{}' in {} (supported: {})",
            name.trim(),
            manifest_path.display(),
            VENDOR_PROFILES.join(", ")
        )
    })
}

/// The project manifest (`trust-lsp.toml`, `.trust-lsp.toml` or `trustlsp.toml`) in
/// `project_root`, found as the language server and the dependency resolver find it.
#[must_use]
pub fn find_project_manifest(project_root: &Path) -> Option<PathBuf> {
    find_dependency_manifest(project_root)
}
