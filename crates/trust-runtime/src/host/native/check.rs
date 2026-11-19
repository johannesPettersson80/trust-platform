//! The check of a project as the build checks it, without a program image (specification 11
//! §6.8.1c): its Structured Text with the build's front and generator for the target, and, for a
//! project whose Rust crate declares Rust POUs, the crate read as the build reads it and checked
//! by the toolchain's `cargo check` on the build's package.

use std::path::Path;

use super::{
    cargo_build, crate_build, front_of, generate_source_with_conversions, model, recursion,
    write_layer_one, NativeError, Options, RustDiagnostic, Toolchain,
};
use crate::harness::SourceFile;
use crate::stdlib::conversions::ConversionProfile;

/// Checks the project in folder `project` from `sources`, its Structured Text, for the target of
/// `options` under the conversion rules `conversions`, as [`super::build_project`] builds it,
/// without an image: a project without a Rust crate needs no toolchain; one whose crate declares
/// Rust POUs also writes its layer 1 and is checked by the cargo of the toolchain that `toolchain`
/// finds, in folder `work`, where cargo keeps what it checked. The crate's warnings come back.
///
/// # Errors
///
/// What the build refuses before it compiles: the sources' diagnostics and unsupported
/// constructs, the crate's findings and errors where they are ([`NativeError::Rust`]), and
/// [`NativeError::Toolchain`] for a project with a crate when no toolchain with cargo and the
/// SDK's sources is found.
pub fn check_project(
    project: &Path,
    sources: &[SourceFile],
    options: &Options,
    conversions: ConversionProfile,
    toolchain: impl FnOnce() -> Result<Toolchain, NativeError>,
    work: &Path,
) -> Result<Vec<RustDiagnostic>, NativeError> {
    let Some(crate_build) = crate_build(project, options)? else {
        generate_source_with_conversions(sources, options, conversions)?;
        return Ok(Vec::new());
    };
    let front = front_of(sources, &[], conversions, Some(&crate_build.krate))?;
    let ids = front.statement_ids();
    let registry = front.lowered.builder.registry().clone();
    let mut model = model::Model::new(&front.lowered, registry, &[], &front.direct_addresses()?)?;
    model.conversions = conversions;
    let generated = recursion::generate(&front, &mut model, &ids, options.target, options.hooks)?;
    write_layer_one(
        &front,
        &[],
        conversions,
        options,
        &generated,
        Some(&crate_build.project),
    )?;
    let toolchain = toolchain().map_err(|error| {
        NativeError::Toolchain(format!(
            "the project's Rust crate is checked with the toolchain's cargo (specification 11 \
             §6.8.1c): {error}"
        ))
    })?;
    cargo_build::check(
        &toolchain,
        &generated.source,
        options.target,
        work,
        &crate_build.project,
    )
}
