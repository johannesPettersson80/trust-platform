//! The check of a project as the build checks it, without a program image (specification 11
//! §6.8.1c): its Structured Text with the build's front and generator for the target, the variables
//! its services may write against the program's tables, and, for a project whose Rust crate
//! declares Rust POUs, the crate read as the build reads it and checked by the toolchain's `cargo
//! check` on the build's package.

use std::path::Path;

use super::{
    cargo_build, crate_build, front, front_of, generate_source_with_conversions, init, model,
    recursion, rust_pous, service_table, write_layer_one, NativeError, Options, RustDiagnostic,
    Toolchain,
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
/// constructs, the crate's findings and errors where they are ([`NativeError::Rust`]), a service's
/// path that no write may reach, as the build refuses it from its image (specification 48 RU-63),
/// and [`NativeError::Toolchain`] for a project with a crate when no toolchain with cargo and the
/// SDK's sources is found.
pub fn check_project(
    project: &Path,
    sources: &[SourceFile],
    options: &Options,
    conversions: ConversionProfile,
    toolchain: impl FnOnce() -> Result<Toolchain, NativeError>,
    work: &Path,
) -> Result<Vec<RustDiagnostic>, NativeError> {
    let services = rust_pous::service_writes(project)
        .map_err(|message| NativeError::Compile(format!("trust.toml: {message}")))?;
    let Some(crate_build) = crate_build(project, options)? else {
        if services.is_empty() {
            generate_source_with_conversions(sources, options, conversions)?;
        } else {
            let front = front::Front::analyze(sources, &[], conversions)?;
            init::check_declared_conversions(&front, conversions)?;
            let ids = front.statement_ids();
            let registry = front.lowered.builder.registry().clone();
            let mut model =
                model::Model::new(&front.lowered, registry, &[], &front.direct_addresses()?)?;
            model.conversions = conversions;
            let generated =
                recursion::generate(&front, &mut model, &ids, options.target, options.hooks)?;
            service_table::check_against_program(
                &front,
                &mut model,
                &generated,
                options.target,
                &services,
            )?;
        }
        return Ok(Vec::new());
    };
    // The crate is checked in the build's work folder, which a build of the project does not
    // write meanwhile (specification 11 §6.8.1d).
    let _lock = super::lock_project_build(project).map_err(|error| {
        NativeError::Compile(format!("lock the build of {}: {error}", project.display()))
    })?;
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
    let warnings = cargo_build::check(
        &toolchain,
        &generated.source,
        options.target,
        work,
        &crate_build.project,
    )?;
    // The services' paths after the crate, as the build checks them once the module is compiled.
    service_table::check_against_program(
        &front,
        &mut model,
        &generated,
        options.target,
        &services,
    )?;
    Ok(warnings)
}
