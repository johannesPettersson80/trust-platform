//! Compiled programs, the build (specification 44 §3 to §6): a project's Structured Text into a
//! typed Rust crate, the crate into a native module with the pinned Rust compiler, and the module
//! with its tables into a program image that every truST kernel runs where it lies.
//!
//! The steps: trust-hir checks the sources and the harness lowers their declarations (`front`);
//! every variable is laid out in its area (`model`, `types`); every POU's statements become Rust
//! (`lower`) and the crate is written around them (`emit`); the runtime's own initialization gives
//! the starting memory (`init`); the toolchain compiles and links the crate (`toolchain`), the
//! module is checked (`module`) and the image is written (`tables`).

pub mod bundle;
mod classes;
mod dispatch;
mod emit;
mod entries;
mod frame_init;
mod front;
mod init;
mod located;
mod lower;
mod model;
mod module;
mod recursion;
mod retain_map;
mod run;
mod settings;
mod signing;
mod stack;
mod std_blocks;
mod tables;
mod toolchain;
mod tools;
mod types;

use std::path::Path;

use crate::harness::SourceFile;
use crate::stdlib::conversions::ConversionProfile;
pub use run::{load, load_kept, HostCode};
pub use settings::{conversion_profile, debug_hooks, signing_key_setting};
pub use signing::{
    computer_trusted_keys, config_folder, create_key, project_key, read_key, sign, trusted_keys_in,
    user_key,
};
pub use toolchain::{stack_frames, triple, Toolchain, ToolchainSource};
pub use trust_runtime_core::native::image::Target;
use trust_runtime_core::native::image::{Image, ImageWriter, MemoryFigure, SectionId};
pub use trust_runtime_core::native::program::{Device, NativeProgram, NoHooks, ScanHooks};

/// The file of a project's program image for the computer that runs it, where the project kept
/// `program.stbc` (specification 44 §8.8, NX-15).
pub const PROGRAM_IMAGE_FILE: &str = "program.tpi";

/// The device this computer is to a compiled program (specification 44 §8.8, NX-15): its CPU, no
/// maximum for program memory, and the task stack and statement budget of
/// [`DeviceDescriptor::hosted`](trust_runtime_core::deployment::DeviceDescriptor::hosted), trusting
/// every signer, as the tools run the images they build; `None` on a CPU that runs no program
/// image.
#[must_use]
pub fn this_computer() -> Option<Device> {
    trust_runtime_core::deployment::DeviceDescriptor::hosted()
        .native
        .map(|native| native.device(None, &[]))
}

/// [`this_computer`] as trust-runtime admits the program it is given (specification 44 §5.11): with
/// the keys of the computer's `device.toml`.
///
/// # Errors
///
/// [`NativeError::Signing`] when the computer's trusted keys cannot be read.
pub fn runtime_device() -> Result<Option<Device>, NativeError> {
    let trusted_keys = computer_trusted_keys()?;
    Ok(this_computer().map(|device| Device {
        trusted_keys,
        ..device
    }))
}

/// Admits `image` on this computer as trust-runtime admits the program it is given at its start
/// (specification 44 §10.2, §8.8): this computer's device, with the keys of its `device.toml`.
///
/// # Errors
///
/// [`NativeError::Image`] on a CPU that runs no program image and for an image the device refuses,
/// and [`NativeError::Signing`] when the computer's trusted keys cannot be read.
pub fn admit_on_this_computer(image: &[u8]) -> Result<(), NativeError> {
    let device = runtime_device()?
        .ok_or_else(|| NativeError::Image(String::from("this computer's CPU runs none")))?;
    trust_runtime_core::native::program::admit(image, &device)
        .map_err(|refusal| NativeError::Image(format!("refused: {refusal}")))?;
    Ok(())
}

/// Refuses `image` for the resource `configured`, the name a computer's `runtime.toml` gives, when
/// the image's program declares another `RESOURCE` in its `CONFIGURATION`, compared without regard
/// to case: an image runs the resource it was built from (specification 10 §4.7, specification 44
/// §5.2). A program that declares no `CONFIGURATION`, or the legacy placeholder named `RESOURCE`,
/// runs under the name it is given.
///
/// # Errors
///
/// [`crate::error::RuntimeError::InvalidConfig`] naming both resources;
/// [`crate::error::RuntimeError::InvalidBundle`] for an image that does not parse.
pub fn check_resource(image: &[u8], configured: &str) -> Result<(), crate::error::RuntimeError> {
    use crate::error::RuntimeError;
    let parsed = Image::parse(image)
        .map_err(|error| RuntimeError::InvalidBundle(error.to_string().into()))?;
    match parsed.resource_name() {
        Some(declared)
            if !declared.eq_ignore_ascii_case(configured)
                && !declared.eq_ignore_ascii_case("RESOURCE") =>
        {
            Err(RuntimeError::InvalidConfig(
                format!(
                    "resource '{configured}' of runtime.toml is not the RESOURCE the program \
                     declares ('{declared}')"
                )
                .into(),
            ))
        }
        _ => Ok(()),
    }
}

/// The target `name` names: a device, a CPU, a Rust target triple or `host`, this computer's CPU
/// (specification 44 §4.4); `None` for a name that names none.
#[must_use]
pub fn target_named(name: &str) -> Option<Target> {
    match name.to_ascii_lowercase().as_str() {
        "host" => Target::this_cpu(),
        "esp32c6" | "esp32-c6" | "riscv32imac" | "riscv32imac-unknown-none-elf" => {
            Some(Target::Rv32imac)
        }
        "stm32u5a9" | "thumbv8m" | "thumbv8m.main-none-eabihf" => Some(Target::ThumbV8mMainHf),
        "x86_64" | "x86-64" | "x86_64-unknown-linux-gnu" | "x86_64-pc-windows-msvc" => {
            Some(Target::X86_64)
        }
        "aarch64" | "arm64" | "aarch64-unknown-linux-gnu" | "aarch64-apple-darwin" => {
            Some(Target::Aarch64)
        }
        _ => None,
    }
}

/// Why a project cannot be compiled.
#[derive(Debug)]
pub enum NativeError {
    /// The sources have errors, or a declaration the build needs is missing.
    Compile(String),
    /// A construct the generator does not compile yet: a build error, never a fallback
    /// (specification 44 NX-09).
    Unsupported {
        /// The construct.
        construct: String,
        /// Where it is.
        at: String,
    },
    /// The Rust toolchain is missing or failed.
    Toolchain(String),
    /// The linked module has something a module must not have.
    Module(String),
    /// The image could not be written or does not read back.
    Image(String),
    /// The project's settings file cannot be read, or its build setting is not one the build takes
    /// (specification 44 §13.2).
    Settings(String),
    /// A signing key cannot be read or made, or the computer's trusted keys cannot be read
    /// (specification 44 §5.11).
    Signing(String),
}

impl std::fmt::Display for NativeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Compile(message) => f.write_str(message),
            Self::Unsupported { construct, at } => write!(
                f,
                "{construct} is not compiled yet ({at}; specification 44 NX-09)"
            ),
            Self::Toolchain(message) => write!(f, "the Rust toolchain: {message}"),
            Self::Module(message) => write!(f, "the compiled module: {message}"),
            Self::Image(message) => write!(f, "the program image: {message}"),
            Self::Settings(message) => write!(f, "the project's settings: {message}"),
            Self::Signing(message) => write!(f, "the signature: {message}"),
        }
    }
}

impl std::error::Error for NativeError {}

impl NativeError {
    /// A construct of the declarations the generator does not compile yet.
    pub(crate) fn unsupported(construct: String) -> Self {
        Self::Unsupported {
            construct,
            at: "the declarations".into(),
        }
    }
}

/// How to build.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// The CPU the module is for.
    pub target: Target,
    /// Whether every statement calls the statement hook, a debug image (specification 44 §13.2);
    /// without the hooks the build is a release image, which says so in its header and cannot be
    /// debugged.
    pub hooks: bool,
}

/// A built program.
pub struct Build {
    /// The program image, `TRSTI001`.
    pub image: Vec<u8>,
    /// The program instances by their number in the image, which
    /// [`NativeProgram::run_instance`] takes.
    pub instances: Vec<String>,
    /// The generated crate.
    pub source: String,
    /// The memory figure the image carries.
    pub figure: MemoryFigure,
    /// The name of the `RESOURCE` the program declares; `None` for a program without a
    /// `CONFIGURATION`, which runs under the name its project gives it (specification 11 §6.7.1).
    pub resource: Option<smol_str::SmolStr>,
}

/// The generated crate of `sources`, without compiling it.
///
/// # Errors
///
/// [`NativeError`] for sources with errors or a construct the generator does not compile.
pub fn generate_source(sources: &[SourceFile], options: &Options) -> Result<String, NativeError> {
    generate_source_with_conversions(sources, options, ConversionProfile::Iec)
}

/// The generated crate of `sources` as [`generate_source`] writes it, with the conversion rules
/// `conversions`, the project's `vendor_profile` (specification 07 §2.10).
///
/// # Errors
///
/// As [`generate_source`].
pub fn generate_source_with_conversions(
    sources: &[SourceFile],
    options: &Options,
    conversions: ConversionProfile,
) -> Result<String, NativeError> {
    let front = front::Front::analyze(sources, &[])?;
    init::check_declared_conversions(&front, conversions)?;
    let ids = front.statement_ids();
    let registry = front.lowered.builder.registry().clone();
    let mut model = model::Model::new(&front.lowered, registry, &[], &front.direct_addresses()?)?;
    model.conversions = conversions;
    let generated = recursion::generate(&front, &mut model, &ids, options.target, options.hooks)?;
    Ok(generated.source)
}

/// Builds the program image of `sources` with `toolchain`, working in folder `work`.
///
/// # Errors
///
/// [`NativeError`] naming the step that failed.
pub fn build(
    sources: &[SourceFile],
    options: &Options,
    toolchain: &Toolchain,
    work: &Path,
) -> Result<Build, NativeError> {
    build_with_tests(sources, &[], options, toolchain, work)
}

/// The header's hash of `sources` (specification 44 §5.2): the SHA-256 of each source's label, a
/// zero byte, its text and a zero byte, in the order of the labels, so that the same sources give
/// the same hash on every computer.
#[must_use]
pub fn sources_hash(sources: &[SourceFile]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut labelled: Vec<(&str, &str)> = sources
        .iter()
        .map(|source| (source.path.as_deref().unwrap_or(""), source.text.as_str()))
        .collect();
    labelled.sort_unstable();
    let mut hasher = Sha256::new();
    for (label, text) in labelled {
        hasher.update(label.as_bytes());
        hasher.update([0]);
        hasher.update(text.as_bytes());
        hasher.update([0]);
    }
    hasher.finalize().into()
}

/// Builds the program image of `sources` as [`build`] does, with each of the test programs `tests`
/// as an instance of its name that no task runs, for `trust-dev test` (specification 13).
///
/// # Errors
///
/// [`NativeError`] naming the step that failed.
pub fn build_with_tests(
    sources: &[SourceFile],
    tests: &[smol_str::SmolStr],
    options: &Options,
    toolchain: &Toolchain,
    work: &Path,
) -> Result<Build, NativeError> {
    build_with_conversions(
        sources,
        tests,
        options,
        ConversionProfile::Iec,
        toolchain,
        work,
    )
}

/// Builds the program image of `sources` as [`build_with_tests`] does, with the conversion rules
/// `conversions`, the project's `vendor_profile` (specification 07 §2.10), which the image's header
/// records (specification 44 §5.2).
///
/// # Errors
///
/// [`NativeError`] naming the step that failed.
pub fn build_with_conversions(
    sources: &[SourceFile],
    tests: &[smol_str::SmolStr],
    options: &Options,
    conversions: ConversionProfile,
    toolchain: &Toolchain,
    work: &Path,
) -> Result<Build, NativeError> {
    // The OpenOT authoring of annotated variables is compiled with the program, and a build whose
    // authoring does not validate fails (specification 27 §10.3, §10.4); the header's hash is of
    // the sources as written.
    let instrumented = crate::openot_authoring::try_instrument_source_files(sources)
        .map_err(|error| NativeError::Compile(format!("OpenOT authoring: {error}")))?;
    let front = front::Front::analyze(&instrumented, tests)?;
    init::check_declared_conversions(&front, conversions)?;
    let ids = front.statement_ids();
    let registry = front.lowered.builder.registry().clone();
    let mut model = model::Model::new(&front.lowered, registry, tests, &front.direct_addresses()?)?;
    model.conversions = conversions;
    let generated = recursion::generate(&front, &mut model, &ids, options.target, options.hooks)?;
    let memory = init::starting_memory(&front.lowered, &mut model)?;
    let symbols: Vec<String> = generated
        .entries
        .iter()
        .map(|entry| entry.symbol.clone())
        .collect();
    let (elf, remarks) = toolchain.compile(&generated.source, options.target, &symbols, work)?;
    let module = module::extract(&elf, options.target, &symbols)?;
    let stack = stack::bound(&elf, options.target, &stack_frames(&remarks), &symbols)?;
    let mut writer = ImageWriter::new(
        options.target,
        u16::try_from(trust_plc_abi::INTERFACE_VERSION).unwrap_or(u16::MAX),
    );
    writer.rustc.clone_from(&toolchain.version);
    // The program library the module links and the sources it was built from (§5.2).
    writer.library = trust_runtime_core::native::library::library_hash(
        &toolchain.program_library(options.target)?,
        options.target,
    );
    writer.source_hash = sources_hash(sources);
    // An image without statement hooks is a release image, and its header says so (§13.2).
    writer.release = !options.hooks;
    // A program that reads the wall clock needs a device that has one (§5.2).
    writer.wall_clock = model.wall_clock;
    // The conversion rules the program was built with, which a running program keeps (§5.2).
    writer.conversion_profile = model.conversions;
    writer.build_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    writer.module(module.code.clone());
    // `SIGN` reserved unsigned, so that a tool that deploys the image signs it in place (§5.11).
    writer.section(
        SectionId::Sign,
        vec![0; trust_runtime_core::native::signature::SIGN_BYTES],
    );
    let figure = tables::write(
        &mut writer,
        &mut model,
        &tables::Inputs {
            front: &front,
            ids: &ids,
            generated: &generated,
            module: &module,
            memory: &memory,
            stack,
        },
    )?;
    let image = writer
        .finish()
        .map_err(|error| NativeError::Image(format!("{error:?}")))?;
    let parsed = Image::parse(&image).map_err(|error| NativeError::Image(error.to_string()))?;
    parsed
        .check_tables()
        .map_err(|error| NativeError::Image(error.to_string()))?;
    let instances = model
        .instances
        .iter()
        .map(|instance| instance.name.to_string())
        .collect();
    Ok(Build {
        image,
        instances,
        source: generated.source,
        figure,
        resource: front
            .lowered
            .configuration
            .as_ref()
            .and_then(|configuration| configuration.resource_name.clone()),
    })
}
