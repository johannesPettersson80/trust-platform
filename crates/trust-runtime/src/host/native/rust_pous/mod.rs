//! A project's Rust POUs (specification 48 §4, §6.1): the project's crate in `rust/`, or the
//! project's own folder in a Rust-first project (§14), the Rust POUs its sources declare with the
//! paths the program's generated code names them by, and the generated Structured Text the rest of
//! the project reads: the declarations, `src/generated/rust_pous.st` (RU-23, RU-24), and in a
//! Rust-first project `src/generated/trust/` with the configuration of its `trust.toml` (RU-67), and
//! the heartbeats of the services `trust.toml` declares (RU-63).
//!
//! The crate's sources are read, not compiled: each attributed item is read with `trust-plc-decl`,
//! the reader the attributes check items with, so the declarations and the compiled code agree by
//! construction (RU-24).

mod configuration;
mod layout;
mod rules;
mod services;
mod structured_text;
mod tree;
mod types;

pub use layout::{layout, Layout, BINDINGS_FOLDER, CRATE_FOLDER, DECLARATIONS_FILE, SERVICES_FILE};
pub use services::{declared_services, service_writes, ServiceWrites};
pub use structured_text::StructuredTextUse;
pub use types::{RustEnumeration, RustStructure};

pub(super) use rules::check_crate;

use std::fmt;
use std::path::Path;

/// A Rust POU of the project's crate.
#[derive(Debug, Clone, PartialEq)]
pub struct RustPou {
    /// The POU as its attributes declare it.
    pub pou: trust_plc_decl::Pou,
    /// The file its item lies in, relative to the project with `/`: `rust/src/filter.rs`.
    pub file: String,
    /// The line of its item in that file, 1-based.
    pub line: usize,
    /// Where its item lies in that file, from its attribute to its name, which a tool that shows
    /// the POU's source opens (specification 48 RU-31, RU-50).
    pub item: ItemSpan,
    /// Where each of its methods lies, in the order of `pou.methods`: its file, relative to the
    /// project, and its item from its attribute to its name (RU-32, RU-50).
    pub method_items: Vec<(String, ItemSpan)>,
    /// The path the program's generated code names the item by from the crate's root:
    /// `["filter", "FilterEma"]` for `crate::filter::FilterEma`.
    pub path: Vec<String>,
}

/// Where a Rust item lies in its file: the line, from 1, and the column, from 0 and in characters,
/// of its start and of its end, as the compiler counts them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ItemSpan {
    /// Where its attribute starts.
    pub start: (usize, usize),
    /// Where its name ends.
    pub end: (usize, usize),
}

/// The project's crate and its Rust POUs.
#[derive(Debug, Clone, PartialEq)]
pub struct RustCrate {
    /// The crate's name as Rust code names it: `[lib] name`, else the package's name with `_` for
    /// `-`.
    pub name: String,
    /// The crate's package, as its manifest names it.
    pub package: String,
    /// The crate's root file: `[lib] path`, else `src/lib.rs` of the crate's folder.
    pub root: std::path::PathBuf,
    /// The Rust POUs, in the order the crate's modules declare them.
    pub pous: Vec<RustPou>,
    /// The enumerations, in the order the crate's modules declare them (RU-25).
    pub enumerations: Vec<RustEnumeration>,
    /// The structures, in the order the crate's modules declare them (RU-25).
    pub structures: Vec<RustStructure>,
    /// Where the crate and its generated Structured Text lie in the project.
    pub layout: Layout,
    /// The Structured Text the crate calls, by the names it writes under `st::` (RU-38, RU-39).
    pub structured_text: Vec<StructuredTextUse>,
}

/// Something of the project's crate that keeps its declarations from being written, where it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustPouError {
    /// The file, relative to the project with `/`.
    pub file: String,
    /// The line, 1-based; 0 for the file as a whole.
    pub line: usize,
    /// The column, 1-based; 0 for the file as a whole.
    pub column: usize,
    /// What is wrong and what to do.
    pub message: String,
}

impl fmt::Display for RustPouError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line == 0 {
            write!(f, "{}: {}", self.file, self.message)
        } else {
            write!(
                f,
                "{}:{}:{}: {}",
                self.file, self.line, self.column, self.message
            )
        }
    }
}

/// The project's crate of the project in `project`; `None` for a project without one, which has
/// no Rust (RU-06).
///
/// # Errors
///
/// Every [`RustPouError`] of the crate's manifest and sources: a file that does not read or parse,
/// an item an attribute refuses, a POU that the generated code cannot reach, a name two POUs have.
pub fn read(project: &Path) -> Result<Option<RustCrate>, Vec<RustPouError>> {
    let layout = layout(project).map_err(at_trust_toml)?;
    let folder = project.join(&layout.crate_folder);
    let manifest = folder.join("Cargo.toml");
    if !manifest.is_file() {
        return Ok(None);
    }
    let manifest_name = layout.crate_file("Cargo.toml");
    let whole = |message: String| {
        vec![RustPouError {
            file: manifest_name.clone(),
            line: 0,
            column: 0,
            message,
        }]
    };
    let text = std::fs::read_to_string(&manifest).map_err(|error| whole(error.to_string()))?;
    let table: toml::Table = toml::from_str(&text).map_err(|error| whole(error.to_string()))?;
    let lib = table.get("lib").and_then(toml::Value::as_table);
    let package = table
        .get("package")
        .and_then(|package| package.get("name"))
        .and_then(toml::Value::as_str)
        .ok_or_else(|| whole("the crate's manifest names no package".into()))?
        .to_string();
    let name = lib
        .and_then(|lib| lib.get("name"))
        .and_then(toml::Value::as_str)
        .unwrap_or(&package)
        .replace('-', "_");
    let root = lib
        .and_then(|lib| lib.get("path"))
        .and_then(toml::Value::as_str)
        .unwrap_or("src/lib.rs");
    let root = folder.join(root);
    let mut found = tree::walk(project, &root);
    // A POU's variable of one of the crate's types bears the type's Structured Text name (RU-25).
    let unresolved = types::resolve(&mut found.pous, &found.enumerations, &mut found.structures);
    found.errors.extend(unresolved);
    // The Structured Text the crate calls, through the bindings it names (RU-39).
    let named = table
        .get("dependencies")
        .and_then(toml::Value::as_table)
        .is_some_and(|dependencies| dependencies.contains_key("trust_st"));
    let structured_text = structured_text::calls(
        std::mem::take(&mut found.structured_text),
        named,
        &layout.crate_folder,
        &manifest_name,
        &mut found.errors,
    );
    if found.errors.is_empty() {
        Ok(Some(RustCrate {
            name,
            package,
            root,
            pous: found.pous,
            enumerations: found.enumerations,
            structures: found.structures,
            layout,
            structured_text,
        }))
    } else {
        Err(found.errors)
    }
}

/// The generated declarations of `krate` (RU-23, RU-25): each enumeration's declaration, then each
/// structure's, then each POU's, in the order the crate declares them, with a blank line between
/// two.
#[must_use]
pub fn declarations_text(krate: &RustCrate) -> String {
    let enumerations = krate
        .enumerations
        .iter()
        .map(|found| found.enumeration.declaration(&found.file));
    let structures = krate
        .structures
        .iter()
        .map(|found| found.structure.declaration(&found.file));
    let pous = krate.pous.iter().map(|pou| pou.pou.declaration(&pou.file));
    enumerations
        .chain(structures)
        .chain(pous)
        .collect::<Vec<_>>()
        .join("\n")
}

/// The path the program's generated code builds each structure of `krate` by, by its Structured
/// Text name in capitals (RU-25, RU-28).
pub(super) fn structure_paths(krate: &RustCrate) -> std::collections::HashMap<String, String> {
    krate
        .structures
        .iter()
        .map(|found| {
            (
                found.structure.name.to_ascii_uppercase(),
                format!(
                    "::{}::{}",
                    super::cargo_build::PROJECT_CRATE,
                    found.path.join("::")
                ),
            )
        })
        .collect()
}

/// What [`write_declarations`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Written {
    /// The file held the crate's declarations already.
    UpToDate,
    /// The file was written with this many POUs.
    Wrote(usize),
    /// The project has no crate or its crate no POU, and the file that held declarations is
    /// removed.
    Removed,
    /// The project has no crate or its crate no POU, and there is no file.
    Nothing,
}

/// What [`write_generated`] did to each generated file of a project.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Generated {
    /// The declarations of the crate's POUs (RU-24).
    pub declarations: Written,
    /// A Rust-first project's configuration (RU-67); `None` for a project without one.
    pub configuration: Option<Written>,
    /// The heartbeats of the services `trust.toml` declares (RU-63), [`Written::Wrote`] giving
    /// the number of services.
    pub services: Written,
}

/// Writes the generated declarations of the project in `project` (RU-24, `trust-dev rust
/// declarations`), and its other generated Structured Text with them ([`write_generated`]).
///
/// # Errors
///
/// As [`write_generated`].
pub fn write_declarations(project: &Path) -> Result<Written, Vec<RustPouError>> {
    write_generated(project).map(|generated| generated.declarations)
}

/// Writes the generated Structured Text of the project in `project` (RU-24, RU-63, RU-67): the
/// declarations, which hold its crate's POUs, a project without POUs having no file, the
/// configuration its `trust.toml` declares when it declares one, and the heartbeats of the services
/// its `trust.toml` declares, a project without services having no file, whether the project has a
/// crate or not. Gives what was done to each.
///
/// # Errors
///
/// The crate's and `trust.toml`'s [`RustPouError`]s, and a file that cannot be written as one
/// without a line.
pub fn write_generated(project: &Path) -> Result<Generated, Vec<RustPouError>> {
    let layout = layout(project).map_err(at_trust_toml)?;
    let krate = read(project)?.filter(|krate| !krate.pous.is_empty());
    let declarations = krate
        .as_ref()
        .map(|krate| (declarations_text(krate), krate.pous.len()));
    let written = write_file(project, &layout.declarations, declarations)?;
    let configuration = match (&layout.configuration, &krate) {
        (Some(file), Some(krate)) => {
            let text = configuration::configuration_text(project, krate).map_err(at_trust_toml)?;
            Some(write_file(project, file, Some((text, 1)))?)
        }
        _ => None,
    };
    let services = services::services_text(project).map_err(at_trust_toml)?;
    let services = write_file(project, &layout.services, services)?;
    Ok(Generated {
        declarations: written,
        configuration,
        services,
    })
}

/// Writes `text` into the project's file `label` when it is not there already, `count` being what
/// [`Written::Wrote`] reports; removes the file for no text.
fn write_file(
    project: &Path,
    label: &str,
    text: Option<(String, usize)>,
) -> Result<Written, Vec<RustPouError>> {
    let file = project.join(label);
    let failed = |error: std::io::Error| {
        vec![RustPouError {
            file: label.into(),
            line: 0,
            column: 0,
            message: error.to_string(),
        }]
    };
    let Some((text, count)) = text else {
        if file.is_file() {
            std::fs::remove_file(&file).map_err(failed)?;
            return Ok(Written::Removed);
        }
        return Ok(Written::Nothing);
    };
    if std::fs::read_to_string(&file).is_ok_and(|held| held == text) {
        return Ok(Written::UpToDate);
    }
    if let Some(folder) = file.parent() {
        std::fs::create_dir_all(folder).map_err(failed)?;
    }
    std::fs::write(&file, text).map_err(failed)?;
    Ok(Written::Wrote(count))
}

/// `message`, an error of `trust.toml`, as the one error of the project's file.
fn at_trust_toml(message: String) -> Vec<RustPouError> {
    vec![RustPouError {
        file: "trust.toml".into(),
        line: 0,
        column: 0,
        message,
    }]
}

/// The timing of a project's Rust POUs (specification 48 RU-46, RU-47, Appendix B): each POU's
/// budget for the build's target, and whether the deployment's profile is `production`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Timing {
    /// The budget of each Rust POU in nanoseconds, by its name in capitals.
    pub(super) budgets: std::collections::HashMap<String, u64>,
    /// `[timing] profile = "production"`: RU-47's findings are errors.
    pub(super) production: bool,
}

/// The timing of `krate`'s POUs for the target `triple`, from their budgets and `trust.toml`'s
/// `[timing]` in the project in `project`: its `profile`, and `[timing.budgets]`, which may lower a
/// POU's budget for a target and never raise it (RU-46).
///
/// # Errors
///
/// A `trust.toml` that does not read, a profile other than `development` and `production`, a budget
/// of a POU the crate has not, one that does not read, and one above the POU's own.
pub(super) fn timing(project: &Path, triple: &str, krate: &RustCrate) -> Result<Timing, String> {
    let mut timing = Timing {
        // A method's budget by its block's and its own name, as a call of it names it (RU-32).
        budgets: krate
            .pous
            .iter()
            .flat_map(|pou| {
                let block = pou.pou.name.to_ascii_uppercase();
                let methods = pou.pou.methods.iter().map(move |method| {
                    (
                        format!("{block}.{}", method.name.to_ascii_uppercase()),
                        method.budget_ns,
                    )
                });
                core::iter::once((pou.pou.name.to_ascii_uppercase(), pou.pou.budget_ns))
                    .chain(methods)
            })
            .collect(),
        production: false,
    };
    let file = project.join("trust.toml");
    let Ok(text) = std::fs::read_to_string(&file) else {
        return Ok(timing);
    };
    let table: toml::Table =
        toml::from_str(&text).map_err(|error| format!("trust.toml: {error}"))?;
    let Some(section) = table.get("timing").and_then(toml::Value::as_table) else {
        return Ok(timing);
    };
    match section.get("profile").map(|profile| profile.as_str()) {
        None | Some(Some("development")) => {}
        Some(Some("production")) => timing.production = true,
        Some(_) => {
            return Err(
                "trust.toml: [timing] profile is \"development\" or \"production\" (specification \
                 48 RU-47)"
                    .into(),
            )
        }
    }
    let budgets = section.get("budgets").and_then(toml::Value::as_table);
    for (pou, targets) in budgets.into_iter().flatten() {
        let key = pou.to_ascii_uppercase();
        let Some(declared) = timing.budgets.get(&key).copied() else {
            return Err(format!(
                "trust.toml: [timing.budgets] names {pou}, which is no Rust POU of the crate"
            ));
        };
        let Some(text) = targets.get(triple).and_then(toml::Value::as_str) else {
            continue;
        };
        let lowered = trust_plc_decl::duration_nanoseconds(text).ok_or_else(|| {
            format!("trust.toml: the budget \"{text}\" of {pou} is no duration: `40us`, `1.5ms`")
        })?;
        if lowered > declared {
            return Err(format!(
                "trust.toml: the budget of {pou} for {triple} is {text}, above its own; trust.toml \
                 may lower a budget, never raise it (specification 48 RU-46)"
            ));
        }
        timing.budgets.insert(key, lowered);
    }
    Ok(timing)
}

/// How the generated code calls a Rust POU (specification 48 RU-28): the Rust path of its item
/// in the build's crate graph and its variables in declaration order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RustLink {
    /// Its number in a debug image's Rust POU table (specification 44 §5.13).
    pub(super) number: u32,
    /// The path of the struct or the function: `::trust_project::filter::FilterEma`.
    pub(super) call: String,
    /// The variables' names in declaration order, the order the call takes them.
    pub(super) variables: Vec<String>,
    /// A function block's methods (RU-32).
    pub(super) methods: Vec<MethodLink>,
}

/// How the generated code calls a method of a Rust function block (specification 48 RU-32).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MethodLink {
    /// Its number in a debug image's Rust POU table, its block's name and its own joined by a dot
    /// (specification 44 §5.13).
    pub(super) number: u32,
    /// Its Structured Text name.
    pub(super) name: String,
    /// The Rust function's name.
    pub(super) item: String,
    /// Its parameters' names after the context, the order the call takes them.
    pub(super) variables: Vec<String>,
}

/// The names of `krate`'s Rust POUs as Structured Text declares them, a method's as its block's and
/// its own joined by a dot, in the order of a debug image's Rust POU table: by their names in
/// capitals (specification 44 §5.13). A POU's place in it is the number by which the generated code
/// names its record, the same for every target (specification 48 RU-39, RU-49).
pub(super) fn table_names(krate: &RustCrate) -> Vec<String> {
    let mut names: Vec<String> = krate
        .pous
        .iter()
        .flat_map(|pou| {
            core::iter::once(pou.pou.name.clone()).chain(
                pou.pou
                    .methods
                    .iter()
                    .map(move |method| format!("{}.{}", pou.pou.name, method.name)),
            )
        })
        .collect();
    names.sort_by_cached_key(|name| name.to_ascii_uppercase());
    names
}

/// The Rust POU table of a debug image of `krate` (specification 44 §5.13): each POU's name, in the
/// order of [`table_names`], with its budget for the build's target in nanoseconds from `timing`
/// (RU-46).
///
/// # Errors
///
/// [`NativeError::Compile`] for a POU without a budget, which the crate's declarations refuse
/// before.
pub(super) fn table(
    krate: &RustCrate,
    timing: &Timing,
) -> Result<Vec<(String, u64)>, super::NativeError> {
    table_names(krate)
        .into_iter()
        .map(|name| {
            let budget = timing
                .budgets
                .get(&name.to_ascii_uppercase())
                .copied()
                .ok_or_else(|| {
                    super::NativeError::Compile(format!("the Rust POU {name} has no budget"))
                })?;
            Ok((name, budget))
        })
        .collect()
}

/// The calls of `krate`'s POUs, by their Structured Text names in capitals, each with its number
/// in [`table_names`].
pub(super) fn links(krate: &RustCrate) -> std::collections::HashMap<String, RustLink> {
    // Every name is in the table; a miss, which cannot happen, would name no record and end the
    // scan with `ModuleInternal` (specification 44 §8.2) rather than time another POU.
    let numbers: std::collections::HashMap<String, u32> = table_names(krate)
        .iter()
        .zip(0u32..)
        .map(|(name, number)| (name.to_ascii_uppercase(), number))
        .collect();
    let number = |key: &str| numbers.get(key).copied().unwrap_or(u32::MAX);
    krate
        .pous
        .iter()
        .map(|pou| {
            let call = format!(
                "::{}::{}",
                super::cargo_build::PROJECT_CRATE,
                pou.path.join("::")
            );
            let variables = pou
                .pou
                .variables
                .iter()
                .map(|variable| variable.name.clone())
                .collect();
            let methods = pou
                .pou
                .methods
                .iter()
                .map(|method| MethodLink {
                    number: number(
                        &format!("{}.{}", pou.pou.name, method.name).to_ascii_uppercase(),
                    ),
                    name: method.name.clone(),
                    item: method.item.clone(),
                    variables: method
                        .variables
                        .iter()
                        .map(|variable| variable.name.clone())
                        .collect(),
                })
                .collect();
            let key = pou.pou.name.to_ascii_uppercase();
            let link = RustLink {
                number: number(&key),
                call,
                variables,
                methods,
            };
            (key, link)
        })
        .collect()
}

/// The refusal of a module whose Rust code can panic (specification 44 §8.6; specification 48 RU-14,
/// RU-45): each Rust POU that calls the panic machinery, with its file and line, or else the
/// function that does, with what the call does and the form that returns a fault instead. The code
/// of a Rust POU lies in the generated function that calls it, which the build never inlines, or in
/// a function of the crate that names the POU's item.
pub(super) fn panic_refusal(krate: &RustCrate, calls: &[super::panics::PanicCall]) -> String {
    let mut lines: Vec<String> = Vec::new();
    for call in calls {
        let last = call.caller.rsplit("::").next().unwrap_or(&call.caller);
        let pou = krate.pous.iter().find(|pou| {
            let prefix = match pou.pou.kind {
                trust_plc_decl::Kind::FunctionBlock => "fb_",
                trust_plc_decl::Kind::Function => "f_",
                trust_plc_decl::Kind::Program => "prog_",
            };
            let glue = format!("{prefix}{}", super::types::mangle(&pou.pou.name));
            let item = format!("{}::{}", krate.name, pou.path.join("::"));
            // A program's instances declared `RETAIN` have a body of their own (specification 11
            // §6.7).
            last == glue || last == format!("{glue}_RETAIN") || call.caller.contains(&item)
        });
        let who = match pou {
            Some(pou) => format!("the Rust POU {} ({}:{})", pou.pou.name, pou.file, pou.line),
            None => format!("the function {}", call.caller),
        };
        let (operation, advice) = super::panics::advice(&call.panic);
        let line = format!("{who} can panic: {operation} ({}); {advice}", call.panic);
        if !lines.contains(&line) {
            lines.push(line);
        }
    }
    format!(
        "the module can panic (specification 44 §8.6), which a Rust POU never does (specification \
         48 RU-14):\n  - {}",
        lines.join("\n  - ")
    )
}

/// Whether the project in `project` holds the declarations of `krate` (RU-24): the build refuses
/// a project whose committed declarations are not what its crate declares, naming the command
/// that writes them.
///
/// # Errors
///
/// The refusal's message.
pub(super) fn check_declarations(project: &Path, krate: &RustCrate) -> Result<(), String> {
    let layout = &krate.layout;
    let held = |label: &str| std::fs::read_to_string(project.join(label)).ok();
    let wanted = (!krate.pous.is_empty()).then(|| declarations_text(krate));
    if held(&layout.declarations) != wanted {
        return Err(format!(
            "{} does not hold the declarations of the project's Rust crate: write them with \
             `trust-dev rust declarations` (specification 48 RU-24)",
            layout.declarations
        ));
    }
    if let Some(file) = &layout.configuration {
        let wanted = configuration::configuration_text(project, krate)
            .map_err(|message| format!("trust.toml: {message}"))?;
        if held(file).as_ref() != Some(&wanted) {
            return Err(format!(
                "{file} does not hold the configuration of the project's trust.toml: write it \
                 with `trust-dev rust declarations` (specification 48 RU-67)"
            ));
        }
    }
    Ok(())
}

/// Whether the project in `project` holds the heartbeats of the services its `trust.toml`
/// declares (RU-63), whether it has a crate or not: the build refuses a project whose committed
/// file is not what `trust.toml` declares, or that keeps the file of services `trust.toml` no longer
/// declares, naming the command that writes it.
///
/// # Errors
///
/// The refusal's message.
pub(super) fn check_services(project: &Path, layout: &Layout) -> Result<(), String> {
    let wanted = services::services_text(project)
        .map_err(|message| format!("trust.toml: {message}"))?
        .map(|(text, _)| text);
    let held = std::fs::read_to_string(project.join(&layout.services)).ok();
    if held != wanted {
        return Err(format!(
            "{} does not hold the heartbeats of the services trust.toml declares: write it with \
             `trust-dev rust declarations` (specification 48 RU-63)",
            layout.services
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod sequence_tests;
