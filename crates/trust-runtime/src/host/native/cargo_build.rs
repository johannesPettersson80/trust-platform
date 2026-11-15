//! The build of a program whose project has a Rust crate (specification 48 §9.3): the generated
//! crate becomes a cargo package that depends on the project's crate, the SDK's crates are taken
//! from the toolchain's sources, and the toolchain's cargo and compiler build the whole crate graph
//! into one module with the flags of specification 44 §4.5, which the build without a crate passes
//! to the compiler itself (`Toolchain::compile`).
//!
//! Every crate of the graph is compiled from source with the module's code generation flags, so
//! the project's crate and the generated code share one interface crate, and one link with
//! link-time optimization makes the module. The build runs offline: the crates the project's lock
//! names come from the vendored sources or the cargo cache (RU-10).

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::toolchain::{cpu, triple, LINKER_SCRIPT};
use super::{NativeError, Toolchain};
use trust_runtime_core::native::image::Target;

/// The name the generated code gives the project's crate.
pub(super) const PROJECT_CRATE: &str = "trust_project";

/// The SDK's crates the project's crate may name, which the build takes from the toolchain's
/// sources whatever version the project's manifest names (RU-12).
const SDK_CRATES: [&str; 5] = [
    "trust-plc",
    "trust-plc-macros",
    "trust-plc-decl",
    "trust-plc-abi",
    "trust-plc-lib",
];

/// The project's crate a build links.
#[derive(Debug, Clone)]
pub(super) struct ProjectCrate {
    /// Its folder, `rust/` of the project.
    pub(super) folder: PathBuf,
    /// Its package's name.
    pub(super) package: String,
}

/// The manifest of the generated crate's package in `work`, for `target`, depending on `project`
/// and on the SDK's crates in `sdk`.
fn manifest(target: Target, sdk: &Path, project: &ProjectCrate) -> String {
    let path = |crate_name: &str| {
        sdk.join(crate_name)
            .display()
            .to_string()
            .replace('\\', "/")
    };
    let mut text = String::from(
        "[package]\nname = \"trust_module\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n\
         [[bin]]\nname = \"trust_module\"\npath = \"module.rs\"\ntest = false\nbench = false\n\n\
         [dependencies]\n",
    );
    for crate_name in ["trust-plc-abi", "trust-plc-lib"] {
        let _ = writeln!(text, "{crate_name} = {{ path = \"{}\" }}", path(crate_name));
    }
    // A computer's module takes the memory functions from trust-plc-mem; a chip's target builtins
    // have them (specification 44 §4.5).
    if target.is_computer() {
        let _ = writeln!(
            text,
            "trust-plc-mem = {{ path = \"{}\" }}",
            path("trust-plc-mem")
        );
    }
    let _ = writeln!(
        text,
        "{PROJECT_CRATE} = {{ path = \"{}\", package = \"{}\" }}",
        project.folder.display().to_string().replace('\\', "/"),
        project.package
    );
    text.push_str("\n[patch.crates-io]\n");
    for crate_name in SDK_CRATES {
        let _ = writeln!(text, "{crate_name} = {{ path = \"{}\" }}", path(crate_name));
    }
    // The module's code generation of specification 44 §4.5: one unit, link-time optimization,
    // abort on panic, integer overflow without checks.
    text.push_str(
        "\n[profile.release]\nopt-level = 2\ncodegen-units = 1\nlto = \"fat\"\npanic = \"abort\"\n\
         overflow-checks = false\ndebug = 0\n\n[workspace]\n",
    );
    text
}

/// Compiles the crate `source` for `target` in folder `work` with the project's crate `project`,
/// and returns the linked module, an ELF file, and the compiler's report of each function's stack
/// frame, as [`Toolchain::compile`] does for a project without a crate.
///
/// # Errors
///
/// [`NativeError::Toolchain`] when the toolchain has no cargo or SDK sources, and with cargo's and
/// the compiler's diagnostics when the build fails.
pub(super) fn compile(
    toolchain: &Toolchain,
    source: &str,
    target: Target,
    entries: &[String],
    work: &Path,
    project: &ProjectCrate,
) -> Result<(Vec<u8>, String), NativeError> {
    let (Some(cargo), Some(sdk)) = (&toolchain.cargo, &toolchain.sdk) else {
        return Err(NativeError::Toolchain(
            "this toolchain has no cargo and SDK sources, which build a project with Rust POUs \
             (specification 48 §9.3): build its folder again with \
             scripts/native/build_toolchain_libraries.sh"
                .into(),
        ));
    };
    // Cargo runs in the build's folder, so every path it is given is absolute: a project named
    // `.` would otherwise be looked for below that folder.
    let absolute = |path: &Path| {
        std::path::absolute(path)
            .map_err(|error| NativeError::Toolchain(format!("{}: {error}", path.display())))
    };
    let work = &absolute(work)?;
    let project = &ProjectCrate {
        folder: absolute(&project.folder)?,
        package: project.package.clone(),
    };
    let failed =
        |error: std::io::Error| NativeError::Toolchain(format!("{}: {error}", work.display()));
    std::fs::create_dir_all(work).map_err(failed)?;
    let script = work.join("module.x");
    std::fs::write(work.join("module.rs"), source)
        .and_then(|()| std::fs::write(&script, LINKER_SCRIPT))
        .and_then(|()| std::fs::write(work.join("Cargo.toml"), manifest(target, sdk, project)))
        .map_err(failed)?;
    // The versions the project's lock pins are kept: the build starts from it (RU-10).
    let lock = project.folder.join("Cargo.lock");
    if lock.is_file() {
        std::fs::copy(&lock, work.join("Cargo.lock")).map_err(failed)?;
    }
    let (cpu, features) = cpu(target);
    // The flags of every crate of the graph: position-independent code without RELRO, the module's
    // CPU, and the build's folders kept out of the module, so that the same project builds the same
    // bytes wherever it lies (specification 44 NX-07).
    let mut flags = vec![
        "-C".to_string(),
        "relocation-model=pic".into(),
        "-C".into(),
        "relro-level=off".into(),
        "-C".into(),
        format!("target-cpu={cpu}"),
        "-C".into(),
        format!("target-feature={features}"),
        format!("--remap-path-prefix={}=.", work.display()),
        format!("--remap-path-prefix={}=project", project.folder.display()),
        format!("--remap-path-prefix={}=sdk", sdk.display()),
    ];
    if let Some(registry) = cargo_home().map(|home| home.join("registry")) {
        flags.push(format!(
            "--remap-path-prefix={}=registry",
            registry.display()
        ));
    }
    check_graph(cargo, toolchain, sdk, target, work, project)?;
    let mut command = Command::new(cargo);
    command
        .current_dir(work)
        .args(["rustc", "--offline", "--release", "--bin", "trust_module"])
        .args(["--target", triple(target)])
        .arg("--")
        .args(["--cfg", "trust_module", "-C", "remark=prolog-epilog"])
        .arg("-C")
        .arg(format!("linker={}", toolchain.linker.display()))
        .args(["-C", "linker-flavor=ld.lld"])
        .arg("-C")
        .arg(format!("link-arg=-T{}", script.display()))
        .args(["-C", "link-arg=--pie", "-C", "link-arg=--no-dynamic-linker"])
        .args(["-C", "link-arg=-nostdlib", "-C", "link-arg=--emit-relocs"]);
    for entry in entries {
        command
            .arg("-C")
            .arg(format!("link-arg=--undefined={entry}"));
    }
    command
        .env("RUSTC", &toolchain.rustc)
        .env("CARGO_TARGET_DIR", work.join("target"))
        .env("CARGO_ENCODED_RUSTFLAGS", flags.join("\u{1f}"))
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_BUILD_RUSTFLAGS")
        .env_remove("RUSTC_WRAPPER")
        .env_remove("RUSTC_WORKSPACE_WRAPPER");
    let result = command
        .output()
        .map_err(|error| NativeError::Toolchain(format!("{}: {error}", cargo.display())))?;
    let diagnostics = String::from_utf8_lossy(&result.stderr).to_string();
    if !result.status.success() {
        let errors: String = diagnostics
            .lines()
            .filter(|line| !line.starts_with("note: <unknown file>:0:0 prolog-epilog"))
            .collect::<Vec<_>>()
            .join("\n");
        return Err(NativeError::Toolchain(errors));
    }
    let output = work
        .join("target")
        .join(triple(target))
        .join("release")
        .join("trust_module");
    let module = std::fs::read(&output)
        .map_err(|error| NativeError::Toolchain(format!("{}: {error}", output.display())))?;
    Ok((module, diagnostics))
}

/// Refuses the scan crate graph of the package in `work` where it breaks specification 48's rules
/// (RU-09, RU-20), from cargo's metadata of it for `target`.
fn check_graph(
    cargo: &Path,
    toolchain: &Toolchain,
    sdk: &Path,
    target: Target,
    work: &Path,
    project: &ProjectCrate,
) -> Result<(), NativeError> {
    let output = Command::new(cargo)
        .current_dir(work)
        .args(["metadata", "--offline", "--format-version", "1"])
        .args(["--filter-platform", triple(target)])
        .env("RUSTC", &toolchain.rustc)
        .env("CARGO_TARGET_DIR", work.join("target"))
        .env_remove("RUSTC_WRAPPER")
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .output()
        .map_err(|error| NativeError::Toolchain(format!("{}: {error}", cargo.display())))?;
    if !output.status.success() {
        return Err(NativeError::Toolchain(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| NativeError::Toolchain(format!("cargo metadata: {error}")))?;
    // The metadata names every manifest by the file system's own path.
    let crate_folder =
        std::fs::canonicalize(&project.folder).unwrap_or_else(|_| project.folder.clone());
    let errors = super::crate_graph::check(&metadata, sdk, &crate_folder);
    if errors.is_empty() {
        Ok(())
    } else {
        Err(NativeError::Compile(errors.join("\n")))
    }
}

/// Cargo's home folder, whose registry holds the downloaded crates' sources.
fn cargo_home() -> Option<PathBuf> {
    std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
                .map(|home| PathBuf::from(home).join(".cargo"))
        })
}
