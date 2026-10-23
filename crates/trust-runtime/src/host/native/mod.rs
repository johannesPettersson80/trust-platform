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
mod frame_init;
mod front;
mod init;
mod lower;
mod model;
mod module;
mod run;
mod settings;
mod stack;
mod std_blocks;
mod tables;
mod toolchain;
mod types;

use std::path::Path;

use crate::harness::SourceFile;
pub use run::{load, HostCode};
pub use settings::debug_hooks;
pub use toolchain::{stack_frames, triple, Toolchain};
pub use trust_runtime_core::native::image::Target;
use trust_runtime_core::native::image::{Image, ImageWriter, MemoryFigure};
pub use trust_runtime_core::native::program::{Device, NativeProgram, NoHooks, ScanHooks};

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
    let generated = emit::generate(&front, &mut model, &ids, options.target, options.hooks)?;
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
    let generated = emit::generate(&front, &mut model, &ids, options.target, options.hooks)?;
    let memory = init::starting_memory(sources, tests, &mut model)?;
    let symbols: Vec<String> = generated
        .entries
        .iter()
        .map(|entry| entry.symbol.clone())
        .collect();
    let (elf, remarks) = toolchain.compile(&generated.source, options.target, &symbols, work)?;
    let module = module::extract(&elf, options.target, &symbols)?;
    let stack = stack::bound(
        &elf,
        options.target,
        &stack_frames(&remarks),
        &symbols,
        model.class_instances(),
    )?;
    let mut writer = ImageWriter::new(
        options.target,
        u16::try_from(trust_plc_abi::INTERFACE_VERSION).unwrap_or(u16::MAX),
    );
    writer.rustc.clone_from(&toolchain.version);
    // An image without statement hooks is a release image, and its header says so (§13.2).
    writer.release = !options.hooks;
    writer.build_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    writer.module(module.code.clone());
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
