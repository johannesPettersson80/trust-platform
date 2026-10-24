//! Compiled programs, the build (specification 44 §3 to §6): a project's Structured Text into a
//! typed Rust crate, the crate into a native module with the pinned Rust compiler, and the module
//! with its tables into a program image that every truST kernel runs where it lies.
//!
//! The steps: trust-hir checks the sources and the harness lowers their declarations (`front`);
//! every variable is laid out in its area (`model`, `types`); every POU's statements become Rust
//! (`lower`) and the crate is written around them (`emit`); the runtime's own initialization gives
//! the starting memory (`init`); the toolchain compiles and links the crate (`toolchain`), the
//! module is checked (`module`) and the image is written (`tables`).

mod classes;
mod dispatch;
mod emit;
mod entries;
mod frame_init;
mod front;
mod init;
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
mod types;

use std::path::Path;

use crate::harness::SourceFile;
pub use run::{load, load_kept, HostCode};
pub use settings::{debug_hooks, signing_key_setting};
pub use signing::{
    computer_trusted_keys, config_folder, create_key, project_key, read_key, sign, trusted_keys_in,
    user_key,
};
pub use toolchain::{stack_frames, triple, Toolchain};
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
        .map(|native| native.device(None))
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
}

/// The generated crate of `sources`, without compiling it.
///
/// # Errors
///
/// [`NativeError`] for sources with errors or a construct the generator does not compile.
pub fn generate_source(sources: &[SourceFile], options: &Options) -> Result<String, NativeError> {
    let front = front::Front::analyze(sources, &[])?;
    let ids = front.statement_ids();
    let registry = front.lowered.builder.registry().clone();
    let mut model = model::Model::new(&front.lowered, registry, &[])?;
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
    let front = front::Front::analyze(sources, tests)?;
    let ids = front.statement_ids();
    let registry = front.lowered.builder.registry().clone();
    let mut model = model::Model::new(&front.lowered, registry, tests)?;
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
    })
}
