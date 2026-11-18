//! The Rust starts of a new project (specification 25 §3.2, specification 48 RU-51): a Structured
//! Text project with a Rust crate in `rust/`, whose function block `src/Main.st` calls, and a
//! Rust-first project, whose crate is the project and whose `trust.toml` is its configuration
//! (specification 48 §14). Each writes its crate, `rust-toolchain.toml` and the Structured Text that
//! `trust-dev rust declarations` writes for them, so that the project builds as it is created, with
//! no Rust of the user's (RU-76).

use std::path::Path;

use toml_edit::DocumentMut;

use super::{
    io_error, write_files, NewProjectError, ProjectStart, CONFIG_ST_SOURCE, IO_TOML_SOURCE,
    LAUNCH_JSON_SOURCE, PROJECT_TOML_SOURCE, RUNTIME_TOML_SOURCE, VSCODE_SETTINGS_SOURCE,
};
use crate::native::{triple, Target};

/// The workspace's `rust-toolchain.toml`, whose channel is the compiler the toolchain bundle
/// carries (specification 48 RU-74).
const WORKSPACE_TOOLCHAIN: &str = include_str!("../../../../rust-toolchain.toml");

/// The names cargo refuses for a package: Rust's keywords, the crates Rust builds in, and the
/// folders of cargo's own build (cargo's `restricted_names`).
const REFUSED_PACKAGE_NAMES: [&str; 61] = [
    "abstract",
    "as",
    "async",
    "await",
    "become",
    "box",
    "break",
    "const",
    "continue",
    "crate",
    "do",
    "dyn",
    "else",
    "enum",
    "extern",
    "false",
    "final",
    "fn",
    "for",
    "gen",
    "if",
    "impl",
    "in",
    "let",
    "loop",
    "macro",
    "match",
    "mod",
    "move",
    "mut",
    "override",
    "priv",
    "pub",
    "ref",
    "return",
    "self",
    "static",
    "struct",
    "super",
    "trait",
    "true",
    "try",
    "type",
    "typeof",
    "unsafe",
    "unsized",
    "use",
    "virtual",
    "where",
    "while",
    "yield",
    "std",
    "core",
    "alloc",
    "test",
    "proc_macro",
    "proc-macro",
    "build",
    "deps",
    "examples",
    "incremental",
];

/// `rust/src/lib.rs` of a Structured Text project with Rust: specification 48 RU-22's function
/// block.
const FILTER_CRATE: &str = r#"//! The project's Rust POUs (specification 48). When you save this file, truST writes their
//! Structured Text declarations into `src/generated/rust_pous.st`, and `src/Main.st` calls them
//! as its own function blocks.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

/// Exponential moving average of one input: at each call, `y` moves `alpha` of the way to `x`.
#[trust_fb(name = "FILTER_EMA")]
#[budget("15us")]
pub struct FilterEma {
    #[input]
    pub x: f64,
    #[input]
    #[init(0.1)]
    pub alpha: f64,
    #[output]
    pub y: f64,
    primed: bool,
}

impl Body for FilterEma {
    fn body(&mut self, _cx: &mut Cx) -> Result<(), Fault> {
        if self.primed {
            self.y += self.alpha * (self.x - self.y);
        } else {
            self.y = self.x;
            self.primed = true;
        }
        Ok(())
    }
}
"#;

/// `src/Main.st` of a Structured Text project with Rust: it smooths `Raw` with the crate's
/// function block.
const FILTER_MAIN_ST: &str = r#"PROGRAM Main
VAR
    Raw : LREAL;
    Smoothed : LREAL;
    Filter : FILTER_EMA;
END_VAR
Filter(x := Raw, alpha := 0.1);
Smoothed := Filter.y;
END_PROGRAM
"#;

/// `src/lib.rs` of a Rust-first project: the program that `trust.toml` runs.
const LAMP_CRATE: &str = r#"//! The project's Rust POUs (specification 48 §14): `trust.toml` runs the program `MAIN` on its
//! task. When you save this file, truST writes the Structured Text that declares them, and the
//! configuration of `trust.toml`, into `src/generated/trust/`.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

/// Switches `lamp` on and off every 50 runs: every half second on the 10 ms task that
/// `trust.toml` gives it.
#[trust_program(name = "MAIN")]
#[budget("1us")]
pub struct Main {
    #[output]
    pub lamp: bool,
    runs: u32,
}

impl Body for Main {
    fn body(&mut self, _cx: &mut Cx) -> Result<(), Fault> {
        self.runs += 1;
        if self.runs == 50 {
            self.runs = 0;
            self.lamp = !self.lamp;
        }
        Ok(())
    }
}
"#;

/// `trust.toml` of a Rust-first project: the resource of `runtime.toml`, one task every 10 ms and
/// the program `MAIN` on it (specification 48 §14.2, Appendix B).
const LAMP_TRUST_TOML: &str = r#"# The project's configuration (specification 48 §14.2): truST writes it into
# src/generated/trust/configuration.st, which the build runs.
[rust]
crate = "."

[resource]
name = "MainRes"

[tasks.MainTask]
interval = "10ms"
priority = 1

[programs.MainInstance]
type = "MAIN"
task = "MainTask"
"#;

/// `.vscode/launch.json` of a Rust-first project: the simulator runs its generated configuration.
const RUST_FIRST_LAUNCH_JSON: &str = r#"{
  "version": "0.2.0",
  "configurations": [
    {
      "type": "structured-text",
      "request": "launch",
      "name": "truST Simulator",
      "program": "${workspaceFolder}/src/generated/trust/configuration.st",
      "stopOnEntry": false
    }
  ]
}
"#;

/// Writes the files of the Rust start `start` for the project `name` into `target`, and the
/// Structured Text that `trust-dev rust declarations` writes for them.
///
/// # Errors
///
/// The first file that cannot be written, and generated Structured Text that cannot be written.
pub(super) fn write(target: &Path, name: &str, start: ProjectStart) -> Result<(), NewProjectError> {
    let manifest = manifest(&package_name(name));
    let toolchain = toolchain_file()?;
    match start {
        ProjectStart::StructuredTextWithRust => write_files(
            target,
            &[
                ("src/Main.st", FILTER_MAIN_ST),
                ("src/config.st", CONFIG_ST_SOURCE),
                ("trust-lsp.toml", PROJECT_TOML_SOURCE),
                ("runtime.toml", RUNTIME_TOML_SOURCE),
                ("io.toml", IO_TOML_SOURCE),
                (".vscode/launch.json", LAUNCH_JSON_SOURCE),
                (".vscode/settings.json", VSCODE_SETTINGS_SOURCE),
                ("rust/Cargo.toml", manifest.as_str()),
                ("rust/src/lib.rs", FILTER_CRATE),
                ("rust-toolchain.toml", toolchain.as_str()),
            ],
        )?,
        ProjectStart::RustFirst => write_files(
            target,
            &[
                ("Cargo.toml", manifest.as_str()),
                ("src/lib.rs", LAMP_CRATE),
                ("trust.toml", LAMP_TRUST_TOML),
                ("trust-lsp.toml", PROJECT_TOML_SOURCE),
                ("runtime.toml", RUNTIME_TOML_SOURCE),
                ("io.toml", IO_TOML_SOURCE),
                (".vscode/launch.json", RUST_FIRST_LAUNCH_JSON),
                (".vscode/settings.json", VSCODE_SETTINGS_SOURCE),
                ("rust-toolchain.toml", toolchain.as_str()),
            ],
        )?,
        ProjectStart::StructuredText => return Ok(()),
    }
    crate::native::rust_pous::write_generated(target).map_err(|errors| {
        let lines: Vec<String> = errors.iter().map(ToString::to_string).collect();
        io_error(format!(
            "write the generated Structured Text of {}: {}",
            target.display(),
            lines.join("; ")
        ))
    })?;
    Ok(())
}

/// The crate's package name for the project `name` (specification 25 §3.2): the name in lower
/// case, one `-` for each run of characters other than ASCII letters, digits and `_`, without a
/// `-` or `_` at either end; `plc` for a name that is then empty, `plc-` before one that starts
/// with a digit, and `-plc` after one that cargo refuses as a package's.
fn package_name(name: &str) -> String {
    let mut package = String::new();
    for character in name.chars() {
        if character.is_ascii_alphanumeric() || character == '_' {
            package.push(character.to_ascii_lowercase());
        } else if !package.ends_with('-') {
            package.push('-');
        }
    }
    let mut package = package
        .trim_matches(|character| character == '-' || character == '_')
        .to_string();
    if package.is_empty() {
        package.push_str("plc");
    } else if package.starts_with(|character: char| character.is_ascii_digit()) {
        package.insert_str(0, "plc-");
    }
    if REFUSED_PACKAGE_NAMES.contains(&package.as_str()) {
        package.push_str("-plc");
    }
    package
}

/// The crate's `Cargo.toml`: the package `package`, edition 2024, with `trust-plc` at exactly this
/// release's version (specification 48 RU-08).
fn manifest(package: &str) -> String {
    format!(
        "[package]\nname = \"{package}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n\
         [dependencies]\ntrust-plc = \"={}\"\n",
        env!("CARGO_PKG_VERSION")
    )
}

/// The project's `rust-toolchain.toml` (specification 48 RU-77): the Rust version of the toolchain
/// bundle's compiler, the devices' module targets and `rust-src`, so that the user's own rustup
/// gives rust-analyzer and `cargo test` the compiler of the build.
///
/// # Errors
///
/// A workspace toolchain file without a channel, which a test of this module refuses first.
fn toolchain_file() -> Result<String, NewProjectError> {
    let channel = pinned_channel()
        .ok_or_else(|| io_error("this build of truST names no Rust version".to_string()))?;
    Ok(format!(
        "# The Rust of truST's toolchain bundle, which builds this project's crate (specification\n\
         # 48 RU-77): your own rustup gives rust-analyzer and `cargo test` the same compiler, and\n\
         # the devices' targets for `cargo check --target`.\n\
         [toolchain]\nchannel = \"{channel}\"\ntargets = [\"{}\", \"{}\"]\n\
         components = [\"rust-src\"]\n",
        triple(Target::Rv32imac),
        triple(Target::ThumbV8mMainHf)
    ))
}

/// The channel of the workspace's `rust-toolchain.toml`.
fn pinned_channel() -> Option<String> {
    let toolchain = WORKSPACE_TOOLCHAIN.parse::<DocumentMut>().ok()?;
    toolchain
        .get("toolchain")?
        .get("channel")?
        .as_str()
        .map(str::to_string)
}

#[cfg(test)]
#[path = "new_project_rust_tests.rs"]
mod tests;
