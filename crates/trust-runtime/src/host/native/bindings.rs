//! The bindings outside a build (specification 48 RU-39, RU-77): `trust-dev rust declarations`
//! writes layer 1 as the build writes it, so that rust-analyzer and the crate's own cargo find `st`
//! before the project's first build.

use std::path::Path;

use crate::harness::SourceFile;
use crate::stdlib::conversions::ConversionProfile;

use super::{cargo_build, front_of, model, recursion, rust_pous, NativeError, Target};

/// Writes the bindings of the project in `project` from its Structured Text `sources`, with the
/// conversions `conversions`, into its `build/trust_st`, generated for this computer without the
/// debugger's hooks; a project whose crate calls no Structured Text has none.
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
    let ids = front.statement_ids();
    let registry = front.lowered.builder.registry().clone();
    let mut model = model::Model::new(&front.lowered, registry, &[], &front.direct_addresses()?)?;
    model.conversions = conversions;
    let target = Target::this_cpu().ok_or_else(|| {
        NativeError::Toolchain("this computer is no target of specification 44".into())
    })?;
    let generated = recursion::generate(&front, &mut model, &ids, target, false)?;
    if let Some(layer_one) = &generated.layer_one {
        cargo_build::write_layer_one(
            &project.join(rust_pous::BINDINGS_FOLDER),
            &layer_one.library,
        )?;
    }
    Ok(())
}
