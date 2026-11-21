//! The service table of a program image (specification 44 §5.14; specification 48 RU-63): each
//! service of the project's `trust.toml` whose `writes` names a variable, with the paths of the
//! variables it may write, and the build's check of those paths against the program, which names
//! the service and the path.

use trust_runtime_core::native::image::{
    Image, ImageWriter, SectionId, ServiceRecord, StringPool, TABLE_PREFIX_BYTES,
};
use trust_runtime_core::native::reflect::{resolve_writable, Located};

use super::emit::Generated;
use super::front::Front;
use super::model::Model;
use super::rust_pous::ServiceWrites;
use super::{init, tables, NativeError, Target};

/// Writes the service table of `services` into `writer`: a record per service, then the path list,
/// each service's paths in the order `writes` gives them. An image without a service that writes
/// has no table.
pub(super) fn write(writer: &mut ImageWriter, pool: &mut StringPool, services: &[ServiceWrites]) {
    if services.is_empty() {
        return;
    }
    let mut records = Vec::with_capacity(services.len());
    let mut paths = Vec::new();
    for service in services {
        records.push(ServiceRecord {
            name: pool.add(&service.name),
            path_first: u32::try_from(paths.len() / 4).unwrap_or(u32::MAX),
            path_count: u32::try_from(service.writes.len()).unwrap_or(u32::MAX),
            reserved: 0,
        });
        for path in &service.writes {
            paths.extend_from_slice(&pool.add(path).to_le_bytes());
        }
    }
    writer.table(&records, &paths);
}

/// The services' paths checked against the program of `front`, `model` and `generated` without a
/// module, as [`check`] checks them against the build's image: the program's areas, variables and
/// types are written as the image's tables, and the paths resolve against them (specification 11
/// §6.8.1c). A project whose services write nothing has nothing to check.
///
/// # Errors
///
/// [`check`]'s refusals, and what the tables or the image refuse as the build does.
pub(super) fn check_against_program(
    front: &Front,
    model: &mut Model<'_>,
    generated: &Generated,
    target: Target,
    services: &[ServiceWrites],
) -> Result<(), NativeError> {
    if services.is_empty() {
        return Ok(());
    }
    let memory = init::starting_memory(&front.lowered, model)?;
    let mut writer = ImageWriter::new(
        target,
        u16::try_from(trust_plc_abi::INTERFACE_VERSION).unwrap_or(u16::MAX),
    );
    // Every section an image must have, an empty table until the program's tables replace theirs:
    // the paths resolve against the areas, the variables and the types alone.
    for id in SectionId::ALL.into_iter().filter(|id| id.required()) {
        writer.section(id, vec![0; TABLE_PREFIX_BYTES]);
    }
    let mut pool = StringPool::new();
    tables::areas(&mut writer, model, &memory);
    tables::symbols(
        &mut writer,
        &mut pool,
        model,
        front,
        &generated.pous,
        &generated.tools,
    )?;
    tables::types(&mut writer, &mut pool, model);
    writer.section(SectionId::Strings, pool.into_bytes());
    let image = writer
        .finish()
        .map_err(|error| NativeError::Image(format!("{error:?}")))?;
    let parsed = Image::parse(&image).map_err(|error| NativeError::Image(error.to_string()))?;
    check(&parsed, services)
}

/// Every path of every service names one elementary value of the program that is no constant, as
/// a write resolves a path, and each variable has one service that writes it, given once.
///
/// # Errors
///
/// [`NativeError::Compile`] with a line for each path the program refuses, naming the service, the
/// path and why.
pub(super) fn check(image: &Image<'_>, services: &[ServiceWrites]) -> Result<(), NativeError> {
    let mut errors = Vec::new();
    let mut written: Vec<(Located, &str, &str)> = Vec::new();
    for service in services {
        for path in &service.writes {
            let name = &service.name;
            match resolve_writable(image, path) {
                Ok(at) => match written.iter().find(|(seen, ..)| *seen == at) {
                    Some((_, other, earlier)) if *other == name.as_str() => errors.push(format!(
                        "trust.toml: [services.{name}] writes \"{path}\", the variable \
                         \"{earlier}\" names, twice (specification 48 RU-63)"
                    )),
                    Some((_, other, earlier)) => errors.push(format!(
                        "trust.toml: [services.{name}] writes \"{path}\", which [services.{other}] \
                         writes as \"{earlier}\": a variable has one service that writes it \
                         (specification 48 RU-63)"
                    )),
                    None => written.push((at, name, path)),
                },
                Err(error) => errors.push(format!(
                    "trust.toml: [services.{name}] writes \"{path}\": {error} (specification 48 \
                     RU-63)"
                )),
            }
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(NativeError::Compile(errors.join("\n")))
    }
}
