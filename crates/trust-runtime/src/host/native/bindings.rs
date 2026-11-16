//! The bindings outside a build (specification 48 RU-39, RU-77): `trust-dev rust declarations`
//! writes layer 1 as the build writes it, so that rust-analyzer and the crate's own cargo find `st`
//! before the project's first build.

use std::path::Path;

use crate::harness::SourceFile;
use crate::stdlib::conversions::ConversionProfile;

use super::emit::layer_one_library;
use super::{cargo_build, front_of, generated_as, rust_pous, NativeError, Target};

/// Writes the bindings of the project in `project` from its Structured Text `sources`, with the
/// conversions `conversions`, into its `build/trust_st`, the text every build writes; a project
/// whose crate calls no Structured Text has none.
///
/// # Errors
///
/// [`NativeError`] with the crate's errors, the Structured Text's and the generator's, as the
/// build reports them, and when the files cannot be written; the folder stays as it was.
pub fn write_bindings(
    project: &Path,
    sources: &[SourceFile],
    conversions: ConversionProfile,
) -> Result<(), NativeError> {
    let krate = rust_pous::read(project).map_err(|errors| {
        let lines: Vec<String> = errors.iter().map(ToString::to_string).collect();
        NativeError::Compile(lines.join("\n"))
    })?;
    let Some(krate) = krate.filter(|krate| !krate.structured_text.is_empty()) else {
        return Ok(());
    };
    let front = front_of(sources, &[], conversions, Some(&krate))?;
    let target = Target::this_cpu().ok_or_else(|| {
        NativeError::Toolchain("this computer is no target of specification 44".into())
    })?;
    // One text for every target and image, as the build writes it (RU-39).
    let release = generated_as(&front, &[], conversions, target, false)?;
    let debug = generated_as(&front, &[], conversions, target, true)?;
    if let (Some(release), Some(debug)) = (release.layer_one, debug.layer_one) {
        let library = layer_one_library(&release.module, &debug.module, &release.bindings);
        cargo_build::write_layer_one(&project.join(rust_pous::BINDINGS_FOLDER), &library)?;
    }
    Ok(())
}
