//! Compiled programs, the build (specification 44 §3 to §6): a project's Structured Text into a
//! typed Rust crate, the crate into a native module with the pinned Rust compiler, and the module
//! with its tables into a program image that every truST kernel runs where it lies.
//!
//! The steps: trust-hir checks the sources and the harness lowers their declarations (`front`);
//! every variable is laid out in its area (`model`, `types`); every POU's statements become Rust
//! (`lower`) and the crate is written around them (`emit`); the runtime's own initialization gives
//! the starting memory (`init`); the toolchain compiles and links the crate (`toolchain`), the
//! module is checked (`module`) and the image is written (`tables`).

mod bindings;
mod build_lock;
pub mod bundle;
mod call_graph;
mod cargo_build;
mod check;
mod classes;
mod crate_graph;
mod dispatch;
mod emit;
mod entries;
mod fb_exit;
mod fb_init;
mod fb_reinit;
mod frame_init;
mod front;
mod init;
mod layers;
mod located;
mod lower;
mod model;
mod module;
mod module_rules;
mod panics;
mod recursion;
mod retain_map;
mod run;
mod rust_budget;
mod rust_messages;
pub mod rust_pous;
mod self_calls;
pub mod service_bindings;
mod service_table;
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
pub use bindings::write_bindings;
pub use build_lock::{lock_project_build, BUILD_LOCK_FILE};
pub use check::check_project;
pub use run::{load, load_kept, read_wall_clock_from, HostCode};
pub use rust_messages::{RustDiagnostic, RustSeverity};
pub use settings::{conversion_profile, debug_hooks, signing_key_setting};
pub use signing::{
    computer_trusted_keys, config_folder, create_key, project_key, read_key, sign, trusted_keys_in,
    user_key,
};
pub use toolchain::{stack_frames, triple, Toolchain, ToolchainSource};
pub use trust_runtime_core::native::image::Target;
use trust_runtime_core::native::image::{Image, ImageWriter, MemoryFigure, SectionId};
pub use trust_runtime_core::native::program::{Device, NativeProgram, NoHooks, ScanHooks};
pub use trust_runtime_services::cycle_step::assertion_json;
pub use trust_runtime_services::debug::sources::{fault_at, StatementAt};

/// The global the test build declares for the mark of a finished multi-cycle test (specification 22, "Multi-cycle tests"):
/// `TEST_FINISHED()` sets it, and the test runner reads it after each cycle. A project may not declare it itself.
pub const TEST_FINISHED_GLOBAL: &str = "TRUST_TEST_FINISHED";

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

/// Where a construct is in the sources, as a check's issue names it (specification 11 §6.8.1c): the
/// source's label, and the 1-based line and column of the construct's first token, the column
/// counted in UTF-16 code units.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcePlace {
    /// The source's label: its path relative to the project, or `file_<n>` without one.
    pub file: String,
    /// The line, 1-based.
    pub line: u32,
    /// The column, 1-based, in UTF-16 code units.
    pub column: u32,
}

/// A project's Rust crate that does not compile (specification 48 RU-44).
#[derive(Debug, Clone)]
pub struct RustFailure {
    /// The compiler's report, in cargo's order, then cargo's own lines.
    pub report: String,
    /// The errors and warnings of the crate that have a place in its files, at least one error.
    pub diagnostics: Vec<RustDiagnostic>,
}

/// Why a project cannot be compiled.
#[derive(Debug)]
pub enum NativeError {
    /// The sources have errors, or a declaration the build needs is missing.
    Compile(String),
    /// The sources have the errors trust-hir reports, each where it is (specification 11
    /// §6.8.1c).
    Sources(crate::harness::CompileError),
    /// A construct the generator does not compile yet: a build error, never a fallback
    /// (specification 44 NX-09).
    Unsupported {
        /// The construct.
        construct: String,
        /// Where it is in the sources; none for a construct of the declarations that the build
        /// works out without a place, such as an instance's `FB_init` arguments.
        at: Option<SourcePlace>,
    },
    /// More than one construct the generator does not compile yet, each an
    /// [`NativeError::Unsupported`], in the order of their places: the build goes on after each
    /// one to find the others (specification 11 §6.8.1c).
    AllUnsupported(Vec<NativeError>),
    /// The Rust toolchain is missing a part or failed.
    Toolchain(String),
    /// This release's toolchain is not installed and `TRUST_TOOLCHAIN` names none (specification 44
    /// §4.6): one sentence that names the toolchain and what installs it.
    ToolchainNotInstalled(String),
    /// The project's Rust crate does not compile (specification 48 RU-44).
    Rust(RustFailure),
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
            Self::Sources(error) => write!(f, "{error}"),
            Self::Unsupported {
                construct,
                at: Some(place),
            } => write!(
                f,
                "{construct} is not compiled yet ({}:{}; specification 44 NX-09)",
                place.file, place.line
            ),
            Self::Unsupported {
                construct,
                at: None,
            } => write!(
                f,
                "{construct} is not compiled yet (the declarations; specification 44 NX-09)"
            ),
            Self::AllUnsupported(each) => {
                for (index, refusal) in each.iter().enumerate() {
                    if index > 0 {
                        f.write_str("\n")?;
                    }
                    write!(f, "{refusal}")?;
                }
                Ok(())
            }
            Self::Toolchain(message) => write!(f, "the Rust toolchain: {message}"),
            Self::ToolchainNotInstalled(advice) => f.write_str(advice),
            Self::Rust(failure) => write!(f, "the project's Rust crate: {}", failure.report),
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
            at: None,
        }
    }

    /// Whether this error is a construct that the generator does not compile yet, or several.
    pub(crate) fn is_unsupported(&self) -> bool {
        matches!(self, Self::Unsupported { .. } | Self::AllUnsupported(_))
    }

    /// The error of the constructs `refused` that one build met, none of them compiled yet: the
    /// one construct as it is, or every one in the order of their places, a construct without a
    /// place last (specification 11 §6.8.1c). `refused` holds errors for which
    /// [`NativeError::is_unsupported`] is true; `None` when it is empty.
    pub(crate) fn refusals(refused: Vec<NativeError>) -> Option<Self> {
        let mut each: Vec<NativeError> = refused
            .into_iter()
            .flat_map(|error| match error {
                Self::AllUnsupported(inner) => inner,
                other => vec![other],
            })
            .collect();
        each.sort_by(|a, b| place_order(a).cmp(&place_order(b)));
        match each.len() {
            0 => None,
            1 => each.pop(),
            _ => Some(Self::AllUnsupported(each)),
        }
    }
}

/// Where `error` sorts among the constructs one build did not compile: by its place, and after
/// every place when it has none.
fn place_order(error: &NativeError) -> (bool, &str, u32, u32) {
    match error {
        NativeError::Unsupported {
            at: Some(place), ..
        } => (false, place.file.as_str(), place.line, place.column),
        _ => (true, "", 0, 0),
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
    /// The program type of each instance, in the order of [`Build::instances`]: a bench finds a
    /// Rust program's instances by it (specification 48 RU-48).
    pub instance_types: Vec<String>,
    /// The generated crate.
    pub source: String,
    /// The memory figure the image carries.
    pub figure: MemoryFigure,
    /// The name of the `RESOURCE` the program declares; `None` for a program without a
    /// `CONFIGURATION`, which runs under the name its project gives it (specification 11 §6.7.1).
    pub resource: Option<smol_str::SmolStr>,
    /// What the build warns of: a task whose Rust POUs may take longer than its interval, or that
    /// calls one in a loop without a bound (specification 48 RU-47).
    pub warnings: Vec<String>,
    /// The compiler's warnings of the project's Rust crate, each where it is (specification 48
    /// RU-44); none for a project without a crate.
    pub crate_warnings: Vec<RustDiagnostic>,
    /// The crates of the image's crate table (specification 44 §5.12, specification 48 RU-79);
    /// none for a project without a crate.
    pub crates: Vec<trust_runtime_core::deployment::RustCrate>,
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
    let front = front::Front::analyze(sources, &[], conversions)?;
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
    build_sources(
        sources,
        tests,
        options,
        conversions,
        toolchain,
        work,
        ProjectParts::default(),
    )
}

/// The module targets whose scan crate graphs the audit reads (specification 44 §4.4).
const AUDITED_TARGETS: [Target; 4] = [
    Target::Rv32imac,
    Target::ThumbV8mMainHf,
    Target::X86_64,
    Target::Aarch64,
];

/// The crates of the scan crate graph of the project in folder `project` that an advisory of the
/// RustSec database can name (specification 48 RU-79), each with its name and version, once, in
/// order: for every module target, from cargo's metadata of the build's package as the build reads
/// it, every crate but the project's crate and the packages the build generates. The build's
/// package for each target is written into `work`, in a folder named after the target's triple.
/// `None` when the project's build compiles no crate: it has none, or the crate declares no Rust
/// POU (§9.1).
///
/// # Errors
///
/// [`NativeError::Compile`] when the crate's sources or `trust.toml` cannot be read, or the crate
/// calls Structured Text and its bindings are not written (RU-39); [`NativeError::Toolchain`] as
/// the build's cargo fails.
pub fn scan_crate_graph(
    project: &Path,
    toolchain: &Toolchain,
    work: &Path,
) -> Result<Option<Vec<(String, String)>>, NativeError> {
    let krate = rust_pous::read(project).map_err(|errors| {
        let lines: Vec<String> = errors.iter().map(ToString::to_string).collect();
        NativeError::Compile(lines.join("\n"))
    })?;
    let Some(krate) = krate.filter(|krate| !krate.pous.is_empty()) else {
        return Ok(None);
    };
    let bindings =
        (!krate.structured_text.is_empty()).then(|| project.join(rust_pous::BINDINGS_FOLDER));
    if bindings
        .as_ref()
        .is_some_and(|folder| !folder.join("Cargo.toml").is_file())
    {
        return Err(NativeError::Compile(format!(
            "the crate calls Structured Text and {} is not written: run `trust-dev rust \
             declarations` or build the project first (specification 48 RU-39)",
            rust_pous::BINDINGS_FOLDER
        )));
    }
    let crate_folder = cargo_build::ProjectCrate {
        folder: project.join(&krate.layout.crate_folder),
        label: krate.layout.crate_folder.replace('\\', "/"),
        package: krate.package.clone(),
        bindings,
        debug: false,
    };
    let mut crates = std::collections::BTreeSet::new();
    for target in AUDITED_TARGETS {
        crates.extend(cargo_build::graph_crates(
            toolchain,
            target,
            &work.join(triple(target)),
            &crate_folder,
        )?);
    }
    Ok(Some(crates.into_iter().collect()))
}

/// What a build takes from the project beyond its Structured Text: its Rust crate, when it declares
/// Rust POUs, and the variables its services may write (specification 48 §9, RU-63).
#[derive(Default)]
struct ProjectParts {
    crate_build: Option<CrateBuild>,
    services: Vec<rust_pous::ServiceWrites>,
}

/// The Rust crate a build compiles with the program, and the crate cargo links (specification 48
/// §9).
struct CrateBuild {
    /// The project's folder, whose `trust.toml` holds the measured times (specification 48 RU-48).
    root: std::path::PathBuf,
    project: cargo_build::ProjectCrate,
    krate: rust_pous::RustCrate,
    /// The Rust POUs' budgets for the build's target (RU-46) and whether the deployment's profile
    /// is `production`, which makes RU-47's findings errors.
    timing: rust_pous::Timing,
}

/// Builds the program image of the project in folder `project` from `sources`, its Structured
/// Text, as [`build_with_conversions`] does, with the project's Rust crate when it declares Rust
/// POUs (specification 48 §9.1): the committed declarations must be what the crate's sources
/// declare (RU-24), and the toolchain's cargo builds the program's module with the crate (§9.3).
/// Every project's committed heartbeats of its services must be what its `trust.toml` declares
/// (RU-63), whether it has a crate or not.
///
/// # Errors
///
/// [`NativeError`] naming the step that failed: the crate's errors where they are, generated files
/// that are out of date, and those of [`build_with_conversions`].
pub fn build_project(
    project: &Path,
    sources: &[SourceFile],
    tests: &[smol_str::SmolStr],
    options: &Options,
    conversions: ConversionProfile,
    toolchain: &Toolchain,
    work: &Path,
) -> Result<Build, NativeError> {
    let crate_build = crate_build(project, options)?;
    let services = rust_pous::service_writes(project)
        .map_err(|message| NativeError::Compile(format!("trust.toml: {message}")))?;
    build_sources(
        sources,
        tests,
        options,
        conversions,
        toolchain,
        work,
        ProjectParts {
            crate_build,
            services,
        },
    )
}

/// The project's Rust crate in folder `project` as a build for `options` takes it, when it declares
/// Rust POUs (specification 48 §9.1): its services' heartbeats are what `trust.toml` declares
/// (RU-63), its committed declarations what its sources declare (RU-24), its own rules hold
/// (RU-08), and its POUs' budgets for the target are read (RU-46). None for a project without a
/// crate or whose crate declares no Rust POU.
///
/// # Errors
///
/// [`NativeError::Compile`] with each finding.
fn crate_build(project: &Path, options: &Options) -> Result<Option<CrateBuild>, NativeError> {
    let layout = rust_pous::layout(project)
        .map_err(|message| NativeError::Compile(format!("trust.toml: {message}")))?;
    rust_pous::check_services(project, &layout).map_err(NativeError::Compile)?;
    let krate = rust_pous::read(project).map_err(|errors| {
        let lines: Vec<String> = errors.iter().map(ToString::to_string).collect();
        NativeError::Compile(lines.join("\n"))
    })?;
    let Some(krate) = krate else {
        return Ok(None);
    };
    rust_pous::check_declarations(project, &krate).map_err(NativeError::Compile)?;
    if krate.pous.is_empty() {
        return Ok(None);
    }
    // The crate's own rules hold before anything compiles (specification 48 RU-08).
    rust_pous::check_crate(project, &krate.layout, &krate.root).map_err(|errors| {
        let lines: Vec<String> = errors.iter().map(ToString::to_string).collect();
        NativeError::Compile(lines.join("\n"))
    })?;
    let timing =
        rust_pous::timing(project, triple(options.target), &krate).map_err(NativeError::Compile)?;
    Ok(Some(CrateBuild {
        root: project.to_path_buf(),
        project: cargo_build::ProjectCrate {
            folder: project.join(&krate.layout.crate_folder),
            label: krate.layout.crate_folder.replace('\\', "/"),
            package: krate.package.clone(),
            bindings: (!krate.structured_text.is_empty())
                .then(|| project.join(rust_pous::BINDINGS_FOLDER)),
            debug: options.hooks,
        },
        krate,
        timing,
    }))
}

/// Writes layer 1, the Structured Text that the project's crate `project` calls, where the crate's
/// manifest names it, when `generated` has one: one text for every target and image (specification
/// 48 RU-39), so the other image's module too.
///
/// # Errors
///
/// [`NativeError`] of the other image's generation or of the write.
fn write_layer_one(
    front: &front::Front,
    tests: &[smol_str::SmolStr],
    conversions: ConversionProfile,
    options: &Options,
    generated: &emit::Generated,
    project: Option<&cargo_build::ProjectCrate>,
) -> Result<(), NativeError> {
    let (
        Some(layer_one),
        Some(cargo_build::ProjectCrate {
            bindings: Some(folder),
            ..
        }),
    ) = (&generated.layer_one, project)
    else {
        return Ok(());
    };
    let other = generated_as(front, tests, conversions, options.target, !options.hooks)?;
    let other = other
        .layer_one
        .map(|other| other.module)
        .unwrap_or_default();
    let (release, debug) = if options.hooks {
        (&other, &layer_one.module)
    } else {
        (&layer_one.module, &other)
    };
    let library = emit::layer_one_library(release, debug, &layer_one.bindings);
    cargo_build::write_layer_one(folder, &library)
}

/// The checked and lowered `sources` with the project's crate `krate`, when there is one: how the
/// generated code calls its Rust POUs, builds its structures and which Structured Text it calls
/// (specification 48 §9).
fn front_of(
    sources: &[SourceFile],
    tests: &[smol_str::SmolStr],
    conversions: ConversionProfile,
    krate: Option<&rust_pous::RustCrate>,
) -> Result<front::Front, NativeError> {
    // The OpenOT authoring of annotated variables is compiled with the program, and a build whose
    // authoring does not validate fails (specification 27 §10.3, §10.4); the header's hash is of
    // the sources as written.
    let instrumented = crate::openot_authoring::try_instrument_source_files(sources)
        .map_err(|error| NativeError::Compile(format!("OpenOT authoring: {error}")))?;
    let links = krate.map(rust_pous::links).unwrap_or_default();
    let mut front = front::Front::analyze_with_rust(&instrumented, tests, conversions, links)?;
    if let Some(krate) = krate {
        front.rust_structures = rust_pous::structure_paths(krate);
        front.structured_text.clone_from(&krate.structured_text);
    }
    init::check_declared_conversions(&front, conversions)?;
    Ok(front)
}

/// The generated crate of the checked `front` for `target`, with the debugger's hooks when `hooks`,
/// generated on a model of its own, so that what one generation records stays the build's.
fn generated_as(
    front: &front::Front,
    tests: &[smol_str::SmolStr],
    conversions: ConversionProfile,
    target: Target,
    hooks: bool,
) -> Result<emit::Generated, NativeError> {
    let ids = front.statement_ids();
    let registry = front.lowered.builder.registry().clone();
    let mut model = model::Model::new(&front.lowered, registry, tests, &front.direct_addresses()?)?;
    model.conversions = conversions;
    recursion::generate(front, &mut model, &ids, target, hooks)
}

/// Builds the program image of `sources`, with the Rust crate `crate_build` when there is one.
fn build_sources(
    sources: &[SourceFile],
    tests: &[smol_str::SmolStr],
    options: &Options,
    conversions: ConversionProfile,
    toolchain: &Toolchain,
    work: &Path,
    parts: ProjectParts,
) -> Result<Build, NativeError> {
    let ProjectParts {
        crate_build,
        services,
    } = parts;
    let (project, krate, timing) = match crate_build {
        Some(CrateBuild {
            root,
            project,
            krate,
            timing,
        }) => (Some(project), Some(krate), Some((timing, root))),
        None => (None, None, None),
    };
    let front = front_of(sources, tests, conversions, krate.as_ref())?;
    let ids = front.statement_ids();
    let registry = front.lowered.builder.registry().clone();
    let mut model = model::Model::new(&front.lowered, registry, tests, &front.direct_addresses()?)?;
    model.conversions = conversions;
    let generated = recursion::generate(&front, &mut model, &ids, options.target, options.hooks)?;
    write_layer_one(
        &front,
        tests,
        conversions,
        options,
        &generated,
        project.as_ref(),
    )?;
    // The Rust work of each task against its interval (specification 48 RU-47): warnings, or errors
    // in a `production` profile, whose image, one without test programs, which `trust-dev test` and
    // `trust-dev rust bench` build, needs a current measurement of each Rust POU its tasks run and
    // sums the larger of its budget and its measured time (RU-48).
    let warnings = match &timing {
        Some((timing, root)) => {
            let (budgets, unmeasured) = if timing.production && tests.is_empty() {
                let reached = rust_budget::reached(&front, &model, &timing.budgets);
                rust_pous::measured::production_budgets(
                    root,
                    triple(options.target),
                    Target::this_cpu() == Some(options.target),
                    &reached,
                    &timing.budgets,
                )
                .map_err(NativeError::Compile)?
            } else {
                (timing.budgets.clone(), None)
            };
            let findings = rust_budget::findings(&front, &model, &budgets);
            // What refuses a production image is named at once: the measurements it lacks and the
            // tasks its Rust work overruns.
            let refused: Vec<String> = unmeasured.into_iter().chain(findings.clone()).collect();
            if timing.production && !refused.is_empty() {
                return Err(NativeError::Compile(refused.join("\n")));
            }
            findings
        }
        _ => Vec::new(),
    };
    // A debug image's Rust POUs with their budgets for the build's target, in the order of the
    // Rust POU table, by which its generated code names their records (specification 44 §5.13).
    let rust_pous = match (&krate, &timing) {
        (Some(krate), Some((timing, _))) if options.hooks => rust_pous::table(krate, timing)?,
        _ => Vec::new(),
    };
    let memory = init::starting_memory(&front.lowered, &mut model)?;
    let symbols: Vec<String> = generated
        .entries
        .iter()
        .map(|entry| entry.symbol.clone())
        .collect();
    let cargo_build::Compiled {
        elf,
        remarks,
        warnings: crate_warnings,
        crates,
    } = match &project {
        Some(project) => cargo_build::compile(
            toolchain,
            &generated.source,
            options.target,
            &symbols,
            work,
            project,
        )?,
        None => {
            let (elf, remarks) =
                toolchain.compile(&generated.source, options.target, &symbols, work)?;
            cargo_build::Compiled {
                elf,
                remarks,
                warnings: Vec::new(),
                crates: Vec::new(),
            }
        }
    };
    // A Rust POU's panic is named with the POU and the form to use instead, a changing `static`,
    // a recursion through Rust and the target's inexact math with the item (specification 48
    // RU-45).
    if let Some(krate) = &krate {
        let graph = call_graph::read(&elf, options.target)?;
        let calls = panics::panic_calls(&graph);
        if !calls.is_empty() {
            return Err(NativeError::Module(rust_pous::panic_refusal(krate, &calls)));
        }
        let broken = module_rules::refusals(&elf, &graph)?;
        if !broken.is_empty() {
            return Err(NativeError::Module(format!(
                "the module breaks the rules of Rust in the scan (specification 48 §9.4):\n  - {}",
                broken.join("\n  - ")
            )));
        }
    }
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
            crates: &crates,
            rust_pous: &rust_pous,
            services: &services,
        },
    )?;
    let image = writer
        .finish()
        .map_err(|error| NativeError::Image(format!("{error:?}")))?;
    let parsed = Image::parse(&image).map_err(|error| NativeError::Image(error.to_string()))?;
    // Each service's paths against the program, naming the service and the path, before the
    // image's own check refuses them with the table's record (specification 48 RU-63).
    service_table::check(&parsed, &services)?;
    parsed
        .check_tables()
        .map_err(|error| NativeError::Image(error.to_string()))?;
    let instances = model
        .instances
        .iter()
        .map(|instance| instance.name.to_string())
        .collect();
    let instance_types = model
        .instances
        .iter()
        .map(|instance| model.program_types[instance.program].def.name.to_string())
        .collect();
    Ok(Build {
        image,
        instances,
        instance_types,
        source: generated.source,
        figure,
        resource: front
            .lowered
            .configuration
            .as_ref()
            .and_then(|configuration| configuration.resource_name.clone()),
        warnings,
        crate_warnings,
        crates,
    })
}

#[cfg(test)]
#[path = "native_error_tests.rs"]
mod native_error_tests;
